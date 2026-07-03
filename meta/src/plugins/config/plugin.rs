//! Config plugin for managing .meta configuration

use anyhow::{anyhow, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use colored::Colorize;
use metarepo_core::{BasePlugin, ConfigFormat, MetaConfig, MetaPlugin, RuntimeConfig};
use std::path::PathBuf;

use super::tui_editor::ConfigEditor;

pub struct ConfigPlugin;

impl Default for ConfigPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigPlugin {
    pub fn new() -> Self {
        Self
    }

    fn handle_migrate(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let target_format = ConfigFormat::parse(
            matches
                .get_one::<String>("format")
                .map(|s| s.as_str())
                .unwrap_or("json"),
        )?;
        let replace = matches.get_flag("replace");
        let force = matches.get_flag("force");

        // Source: the currently-active config file. Required — we can't
        // migrate something that hasn't been initialized.
        let source = config.meta_file_path.clone().ok_or_else(|| {
            anyhow!(
                "No metarepo config found to migrate. Run 'meta init' first or pass --config <path>."
            )
        })?;
        let source_format = ConfigFormat::from_path(&source).unwrap_or(ConfigFormat::Json);

        // Destination: explicit --to, else canonical filename for the target
        // format alongside the source.
        let destination: PathBuf = match matches.get_one::<String>("to") {
            Some(s) => PathBuf::from(s),
            None => {
                let parent = source.parent().unwrap_or_else(|| std::path::Path::new("."));
                parent.join(target_format.canonical_filename())
            }
        };

        if source_format == target_format && source == destination {
            println!(
                "  {} Source is already in {} at {} — nothing to do.",
                "·".bright_black(),
                target_format.label(),
                source.display()
            );
            return Ok(());
        }

        if destination.exists() && !force {
            return Err(anyhow!(
                "Destination {} already exists. Pass --force to overwrite.",
                destination.display()
            ));
        }

        // We already have the parsed config in RuntimeConfig.meta_config, so
        // write it out in the new format.
        config
            .meta_config
            .save_to_file_with_format(&destination, target_format)?;

        println!(
            "  {} Wrote {} ({})",
            "✓".green(),
            destination.display(),
            target_format.label()
        );

        if replace && source != destination {
            std::fs::remove_file(&source)?;
            println!(
                "  {} Removed original {} (--replace)",
                "✓".yellow(),
                source.display()
            );
        } else if source != destination {
            println!(
                "  {} Kept original {} (pass --replace to remove it)",
                "·".bright_black(),
                source.display()
            );
        }

        Ok(())
    }

    fn handle_edit(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let meta_file = if let Some(file) = matches.get_one::<String>("file") {
            PathBuf::from(file)
        } else {
            config
                .meta_file_path
                .clone()
                .ok_or_else(|| anyhow!("Could not find .meta file. Use --file to specify path."))?
        };

        // Launch TUI editor with the declared settings catalog so every
        // setting (core + plugins + modules) is editable, not just projects.
        let mut editor = ConfigEditor::new(meta_file, config.settings_catalog.clone())?;
        editor.run()?;

        Ok(())
    }

    fn handle_show(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let format = matches
            .get_one::<String>("format")
            .map(|s| s.as_str())
            .unwrap_or("json");

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&config.meta_config)?;
                println!("{}", json);
            }
            "yaml" => {
                let yaml = serde_yaml::to_string(&config.meta_config)?;
                println!("{}", yaml);
            }
            "toml" => {
                let toml = toml::to_string_pretty(&config.meta_config)?;
                println!("{}", toml);
            }
            _ => {
                return Err(anyhow!(
                    "Unknown format: {}. Use json, yaml, or toml",
                    format
                ));
            }
        }

        Ok(())
    }

    /// Load the inherited config chain (outermost → nearest) for cascade-aware
    /// reads. Falls back to the active runtime config when no files are found.
    fn config_chain(config: &RuntimeConfig) -> Vec<(PathBuf, MetaConfig)> {
        let mut chain = Vec::new();
        if let Ok(found) = MetaConfig::discover_chain_from(&config.working_dir) {
            for d in found {
                if let Ok(c) = MetaConfig::load_from_file(&d.path) {
                    chain.push((d.path, c));
                }
            }
        }
        if chain.is_empty() {
            let path = config.meta_file_path.clone().unwrap_or_default();
            chain.push((path, config.meta_config.clone()));
        }
        chain
    }

    /// Effective value for a dotted key: the nearest config in the chain that
    /// sets it wins. Returns the value and the file it came from.
    fn effective_dotted<'a>(
        chain: &'a [(PathBuf, MetaConfig)],
        key: &str,
    ) -> Option<(serde_json::Value, &'a PathBuf)> {
        for (path, cfg) in chain.iter().rev() {
            if let Some(v) = cfg.get_dotted(key) {
                return Some((v, path));
            }
        }
        None
    }

    /// The `--root` write target: the outermost config in the chain (shared
    /// defaults). Returns its path and config, or `None` when the chain is empty
    /// or the outermost entry has no real file path (the fallback placeholder).
    fn root_write_target(chain: &[(PathBuf, MetaConfig)]) -> Option<(PathBuf, MetaConfig)> {
        match chain.first() {
            Some((path, cfg)) if !path.as_os_str().is_empty() => Some((path.clone(), cfg.clone())),
            _ => None,
        }
    }

    /// List every configurable setting declared by registered plugins, with its
    /// type, description, effective value, and (when nested) its source file.
    fn handle_list(&self, config: &RuntimeConfig) -> Result<()> {
        if config.settings_catalog.is_empty() {
            println!("No configurable settings are declared by the active plugins.");
            return Ok(());
        }

        let chain = Self::config_chain(config);
        let nearest = chain.last().map(|(p, _)| p.clone());
        let nested = chain.len() > 1;

        println!("{}", "Configurable settings:".bold());
        for setting in &config.settings_catalog {
            let eff = Self::effective_dotted(&chain, &setting.key);
            let value_display = match (&eff, &setting.default) {
                (Some((v, _)), _) => match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                (None, Some(d)) => format!("{} (default)", d),
                (None, None) => "(unset)".to_string(),
            };

            // In a nested workspace, annotate where an inherited value came from.
            let source = match (&eff, nested) {
                (Some((_, p)), true) if Some(*p) != nearest.as_ref() => {
                    format!(
                        "  {}",
                        format!("(inherited from {})", p.display()).bright_black()
                    )
                }
                _ => String::new(),
            };

            println!(
                "  {} [{}]",
                setting.key.cyan(),
                setting.value_type.label().bright_black()
            );
            println!("      {}", setting.description);
            println!("      current: {}{}", value_display.green(), source);
        }

        Ok(())
    }

    fn handle_get(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let key = matches.get_one::<String>("key").unwrap();
        let chain = Self::config_chain(config);
        let nearest = chain.last().map(|(p, _)| p.clone());

        if let Some((value, source)) = Self::effective_dotted(&chain, key) {
            println!("{}", serde_json::to_string_pretty(&value)?);
            if chain.len() > 1 && Some(source) != nearest.as_ref() {
                println!(
                    "{}",
                    format!("(inherited from {})", source.display()).bright_black()
                );
            }
            return Ok(());
        }

        // Unset: if it's a declared setting, surface its default instead of erroring.
        if let Some(setting) = config.settings_catalog.iter().find(|s| &s.key == key) {
            match &setting.default {
                Some(d) => {
                    println!("{}  (default, not set)", d);
                    return Ok(());
                }
                None => return Err(anyhow!("'{}' is not set and has no default", key)),
            }
        }

        Err(anyhow!("Key '{}' not found in config", key))
    }

    fn handle_set(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let key = matches.get_one::<String>("key").unwrap();
        let value_str = matches.get_one::<String>("value").unwrap();
        let to_root = matches.get_flag("root");

        // If the key is a declared setting, validate the value against its type.
        // Otherwise fall back to free-form JSON-or-string parsing (still
        // nested-key safe) so arbitrary config paths keep working.
        let value = match config.settings_catalog.iter().find(|s| &s.key == key) {
            Some(setting) => setting
                .value_type
                .parse(value_str)
                .map_err(|e| anyhow!("Invalid value for '{}': {}", key, e))?,
            None => serde_json::from_str(value_str)
                .unwrap_or_else(|_| serde_json::Value::String(value_str.to_string())),
        };

        // Pick the write target. By default a set lands in the nearest .meta
        // (the active config). With --root it lands in the outermost .meta of
        // the chain — the shared defaults every nested workspace inherits.
        let (meta_file, base_config) = if to_root {
            Self::root_write_target(&Self::config_chain(config)).ok_or_else(|| {
                anyhow!("--root requires a discoverable .meta chain; none was found")
            })?
        } else {
            let path = config
                .meta_file_path
                .clone()
                .ok_or_else(|| anyhow!("Could not find .meta file path"))?;
            (path, config.meta_config.clone())
        };

        // Apply with intermediate objects created as needed (so `skill.dest`
        // works even when the `[skill]` block does not exist yet).
        let updated_config = base_config.with_dotted_set(key, value)?;
        updated_config.save_to_file(&meta_file)?;

        if to_root {
            println!(
                "✓ Config updated: {} = {} {}",
                key,
                value_str,
                format!("(in {})", meta_file.display()).bright_black()
            );
        } else {
            println!("✓ Config updated: {} = {}", key, value_str);
        }

        Ok(())
    }

    fn handle_validate(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let meta_file = if let Some(file) = matches.get_one::<String>("file") {
            PathBuf::from(file)
        } else {
            config
                .meta_file_path
                .clone()
                .ok_or_else(|| anyhow!("Could not find .meta file. Use --file to specify path."))?
        };

        // Try to load the config
        match MetaConfig::load_from_file(&meta_file) {
            Ok(_) => {
                println!("✓ Config file is valid: {}", meta_file.display());
                Ok(())
            }
            Err(e) => {
                println!("✗ Config file validation failed: {}", e);
                Err(e)
            }
        }
    }
}

impl MetaPlugin for ConfigPlugin {
    fn name(&self) -> &str {
        "config"
    }

    fn register_commands(&self, app: Command) -> Command {
        let config_cmd = Command::new("config")
                .about("Manage the workspace configuration file")
                .version(env!("CARGO_PKG_VERSION"))
                .visible_alias("c")
                .after_long_help(metarepo_core::format_help_description(
                    "Inspect and edit the workspace configuration file (.meta / .metarepo).\n\
                     \n\
                     The config holds your registered projects and the typed settings declared\n\
                     by core, plugins, and modules. Subcommands let you open an interactive tree\n\
                     editor, dump the file in json/yaml/toml, read or write individual keys, list\n\
                     declared settings, validate the file, and migrate between formats.\n\
                     \n\
                     Reads are cascade-aware: in a nested workspace, get and list resolve each\n\
                     key to the nearest config that sets it and note where an inherited value\n\
                     came from. Running config with no subcommand opens the editor.\n\
                     \n\
                     Examples:\n  \
                       meta config                       Open the interactive editor\n  \
                       meta config list                  List declared settings and values\n  \
                       meta config get skill.dest        Read one effective value\n",
                ))
                .subcommand_required(false)
                .subcommand(
                    Command::new("edit")
                        .about("Edit the configuration in an interactive TUI")
                        .visible_alias("e")
                        .after_long_help(metarepo_core::format_help_description(
                            "Edit the configuration in a full-screen interactive tree editor.\n\
                             \n\
                             Opens the active config (or the file given with --file) in a TUI with\n\
                             a Config Tree pane and a detail panel. Navigate with the arrow keys or\n\
                             h/j/k/l, expand and collapse nodes, then edit a leaf value with 'e',\n\
                             add with 'a', delete with 'd', and search with '/'. Save with 's' or\n\
                             Ctrl-w; 'q'/Esc quits and guards unsaved edits.\n\
                             \n\
                             The tree covers every declared setting (core, plugins, and modules)\n\
                             alongside your projects, so nothing has to be hand-edited in the file.\n\
                             This is the default action when config is run without a subcommand.\n\
                             \n\
                             Examples:\n  \
                               meta config edit\n  \
                               meta config edit --file ./.meta\n",
                        ))
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .value_name("FILE")
                                .help("Path to .meta file"),
                        ),
                )
                .subcommand(
                    Command::new("show")
                        .about("Print the current configuration")
                        .after_long_help(metarepo_core::format_help_description(
                            "Print the active configuration serialized to a chosen format.\n\
                             \n\
                             Dumps the whole loaded config (projects and all settings) to stdout.\n\
                             Use --format to pick json (default), yaml, or toml. This shows the\n\
                             active config as parsed; it does not resolve inherited values across\n\
                             a nested chain (use get/list for cascade-aware reads).\n\
                             \n\
                             Examples:\n  \
                               meta config show\n  \
                               meta config show --format yaml\n",
                        ))
                        .arg(
                            Arg::new("format")
                                .short('f')
                                .long("format")
                                .value_name("FORMAT")
                                .help("Output format (json, yaml, toml)")
                                .default_value("json")
                                .value_parser(["json", "yaml", "toml"]),
                        ),
                )
                .subcommand(
                    Command::new("get")
                        .about("Read the effective value of a config key")
                        .after_long_help(metarepo_core::format_help_description(
                            "Read the effective value of a single configuration key.\n\
                             \n\
                             Takes a dotted key path and prints its value as JSON. Reads are\n\
                             cascade-aware: in a nested workspace the nearest config that sets the\n\
                             key wins, and an inherited value notes the file it came from. When the\n\
                             key is an unset but declared setting, its default is shown instead.\n\
                             \n\
                             Examples:\n  \
                               meta config get default_bare\n  \
                               meta config get projects.myproject.url\n  \
                               meta config get skill.search-limit\n",
                        ))
                        .arg(Arg::new("key").required(true).value_name("KEY").help(
                            "Config key path (e.g., 'default_bare' or 'projects.myproject.url')",
                        )),
                )
                .subcommand(
                    Command::new("set")
                        .about("Write a value to a config key")
                        .after_long_help(metarepo_core::format_help_description(
                            "Write a value to a configuration key and save the file.\n\
                             \n\
                             Takes a dotted key path and a value, then persists it to the active\n\
                             config. When the key is a declared setting, the value is validated\n\
                             against its type (string, bool, integer, or comma/JSON list) and\n\
                             rejected on mismatch. Otherwise the value is parsed as JSON, falling\n\
                             back to a plain string. Missing intermediate blocks are created, so\n\
                             setting a nested key works even when its parent does not exist yet.\n\
                             Values may begin with a hyphen.\n\
                             \n\
                             By default a write lands in the nearest .meta (the active config). In a\n\
                             nested workspace, pass --root to write to the outermost .meta instead —\n\
                             the shared defaults every nested workspace inherits.\n\
                             \n\
                             Examples:\n  \
                               meta config set skill.search-limit 50\n  \
                               meta config set skill.dest ~/.config/skills\n  \
                               meta config set default_bare true\n  \
                               meta config set skill.dest ~/skills --root   Write to the outermost .meta\n",
                        ))
                        .arg(
                            Arg::new("key")
                                .required(true)
                                .value_name("KEY")
                                .help("Config key path"),
                        )
                        .arg(
                            Arg::new("value")
                                .required(true)
                                .value_name("VALUE")
                                .allow_hyphen_values(true)
                                .help("Value to set"),
                        )
                        .arg(
                            Arg::new("root")
                                .long("root")
                                .action(ArgAction::SetTrue)
                                .help("Write to the outermost .meta in the chain (shared defaults) instead of the nearest"),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List declared settings with type, default, and current value")
                        .visible_alias("ls")
                        .after_long_help(metarepo_core::format_help_description(
                            "List every configurable setting declared by the active plugins.\n\
                             \n\
                             For each declared setting it prints the dotted key, its value type,\n\
                             the description, and the effective current value (or the default when\n\
                             unset). Reads are cascade-aware: in a nested workspace an inherited\n\
                             value is annotated with the file it came from. Use this to discover\n\
                             what can be set before reaching for get or set.\n\
                             \n\
                             Examples:\n  \
                               meta config list\n  \
                               meta config ls\n",
                        )),
                )
                .subcommand(
                    Command::new("validate")
                        .about("Check that the config file parses correctly")
                        .after_long_help(metarepo_core::format_help_description(
                            "Check that the configuration file parses into a valid structure.\n\
                             \n\
                             Loads the active config (or the file given with --file) and reports\n\
                             whether it parses. On success it prints the validated path; on failure\n\
                             it prints the parse error and exits non-zero. Useful in CI or after a\n\
                             hand edit to confirm the file is well-formed.\n\
                             \n\
                             Examples:\n  \
                               meta config validate\n  \
                               meta config validate --file ./.meta\n",
                        ))
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .value_name("FILE")
                                .help("Path to .meta file to validate"),
                        ),
                )
                .subcommand(
                    Command::new("migrate")
                        .about("Convert the workspace config between supported formats")
                        .after_long_help(metarepo_core::format_help_description(
                            "Convert the workspace config to a different format (json, yaml, toml).\n\
                             \n\
                             Reads the active config (auto-discovered or supplied via --config /\n\
                             METAREPO_CONFIG) and writes it back in the chosen format. The\n\
                             destination defaults to the canonical filename for the target format\n\
                             alongside the source, or an explicit path via --to.\n\
                             \n\
                             By default the original file is kept; pass --replace to delete it\n\
                             after the new file is written. Refuses to overwrite an existing\n\
                             destination unless --force is given.\n\
                             \n\
                             Examples:\n  \
                               meta config migrate yaml                  Write .metarepo.yaml next to current\n  \
                               meta config migrate toml --replace        Migrate and remove the old file\n  \
                               meta config migrate json --to .metarepo   Migrate to an explicit path\n",
                        ))
                        .arg(
                            Arg::new("format")
                                .required(true)
                                .value_name("FORMAT")
                                .value_parser(["json", "yaml", "yml", "toml"])
                                .help("Target format"),
                        )
                        .arg(
                            Arg::new("to")
                                .long("to")
                                .value_name("PATH")
                                .help("Explicit destination path (defaults to the canonical filename for the target format alongside the source)"),
                        )
                        .arg(
                            Arg::new("replace")
                                .long("replace")
                                .action(ArgAction::SetTrue)
                                .help("Delete the original config file after the new one is written"),
                        )
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .action(ArgAction::SetTrue)
                                .help("Overwrite the destination if it already exists"),
                        ),
                );
        // The global `--version` arg propagates (global=true) into every
        // subcommand and uses ArgAction::Version, which clap asserts requires a
        // version on each command. Stamp the package version across the whole
        // config subtree so no subcommand trips that assert.
        app.subcommand(config_cmd.mut_subcommands(|c| c.version(env!("CARGO_PKG_VERSION"))))
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("edit", sub_matches)) => self.handle_edit(sub_matches, config),
            Some(("show", sub_matches)) => self.handle_show(sub_matches, config),
            Some(("get", sub_matches)) => self.handle_get(sub_matches, config),
            Some(("set", sub_matches)) => self.handle_set(sub_matches, config),
            Some(("list", _)) => self.handle_list(config),
            Some(("validate", sub_matches)) => self.handle_validate(sub_matches, config),
            Some(("migrate", sub_matches)) => self.handle_migrate(sub_matches, config),
            _ => {
                // Default to edit if no subcommand provided
                self.handle_edit(matches, config)
            }
        }
    }
}

impl BasePlugin for ConfigPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }

    fn description(&self) -> Option<&str> {
        Some("Manage .meta configuration files")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg(json: &str) -> MetaConfig {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn effective_dotted_prefers_nearest() {
        // Chain is outermost → nearest.
        let chain = vec![
            (
                PathBuf::from("/ws/.meta"),
                cfg(r#"{"projects":{},"skill":{"dest":"~/outer","search-limit":99}}"#),
            ),
            (
                PathBuf::from("/ws/inner/.meta"),
                cfg(r#"{"projects":{},"skill":{"dest":"~/inner"}}"#),
            ),
        ];

        // dest is overridden by the inner (nearest) config.
        let (v, src) = ConfigPlugin::effective_dotted(&chain, "skill.dest").unwrap();
        assert_eq!(v, serde_json::json!("~/inner"));
        assert_eq!(src, &PathBuf::from("/ws/inner/.meta"));

        // search-limit is only set in the outer config → inherited.
        let (v, src) = ConfigPlugin::effective_dotted(&chain, "skill.search-limit").unwrap();
        assert_eq!(v, serde_json::json!(99));
        assert_eq!(src, &PathBuf::from("/ws/.meta"));

        // Unset everywhere.
        assert!(ConfigPlugin::effective_dotted(&chain, "skill.api-key").is_none());
    }

    #[test]
    fn root_write_target_picks_outermost() {
        let chain = vec![
            (
                PathBuf::from("/ws/.meta"),
                cfg(r#"{"projects":{},"skill":{"dest":"~/outer"}}"#),
            ),
            (
                PathBuf::from("/ws/inner/.meta"),
                cfg(r#"{"projects":{},"skill":{"dest":"~/inner"}}"#),
            ),
        ];

        let (path, _) = ConfigPlugin::root_write_target(&chain).unwrap();
        assert_eq!(path, PathBuf::from("/ws/.meta"));
    }

    #[test]
    fn root_write_target_none_for_placeholder_path() {
        // The config_chain fallback pushes an empty path when nothing is found.
        let chain = vec![(PathBuf::new(), cfg(r#"{"projects":{}}"#))];
        assert!(ConfigPlugin::root_write_target(&chain).is_none());
    }
}
