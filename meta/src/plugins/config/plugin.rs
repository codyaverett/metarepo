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

    /// List every configurable setting declared by registered plugins, with its
    /// type, description, default, and current value.
    fn handle_list(&self, config: &RuntimeConfig) -> Result<()> {
        if config.settings_catalog.is_empty() {
            println!("No configurable settings are declared by the active plugins.");
            return Ok(());
        }

        println!("{}", "Configurable settings:".bold());
        for setting in &config.settings_catalog {
            let current = config
                .meta_config
                .get_dotted(&setting.key)
                .map(|v| match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                });

            let value_display = match (&current, &setting.default) {
                (Some(v), _) => v.clone(),
                (None, Some(d)) => format!("{} (default)", d),
                (None, None) => "(unset)".to_string(),
            };

            println!(
                "  {} [{}]",
                setting.key.cyan(),
                setting.value_type.label().bright_black()
            );
            println!("      {}", setting.description);
            println!("      current: {}", value_display.green());
        }

        Ok(())
    }

    fn handle_get(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let key = matches.get_one::<String>("key").unwrap();

        if let Some(value) = config.meta_config.get_dotted(key) {
            println!("{}", serde_json::to_string_pretty(&value)?);
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

        // Apply with intermediate objects created as needed (so `skill.dest`
        // works even when the `[skill]` block does not exist yet).
        let updated_config = config.meta_config.with_dotted_set(key, value)?;

        let meta_file = config
            .meta_file_path
            .clone()
            .ok_or_else(|| anyhow!("Could not find .meta file path"))?;

        updated_config.save_to_file(&meta_file)?;

        println!("✓ Config updated: {} = {}", key, value_str);

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
                .about("Manage .meta configuration files")
                .version(env!("CARGO_PKG_VERSION"))
                .visible_alias("c")
                .subcommand_required(false)
                .subcommand(
                    Command::new("edit")
                        .about("Edit config with interactive TUI")
                        .visible_alias("e")
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
                        .about("Display current configuration")
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
                        .about("Get a specific config value")
                        .arg(Arg::new("key").required(true).value_name("KEY").help(
                            "Config key path (e.g., 'default_bare' or 'projects.myproject.url')",
                        )),
                )
                .subcommand(
                    Command::new("set")
                        .about("Set a specific config value")
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
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List configurable settings declared by plugins (key, type, default, current)")
                        .visible_alias("ls"),
                )
                .subcommand(
                    Command::new("validate")
                        .about("Validate .meta file structure")
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
                        .about("Convert the workspace config between supported formats (json|yaml|toml)")
                        .long_about(
                            "Convert the workspace config to a different format.\n\n\
                             Reads the active config (auto-discovered or supplied via --config /\n\
                             METAREPO_CONFIG) and writes it back in the chosen format.\n\n\
                             By default the original file is kept; pass --replace to delete it\n\
                             after the new file is written. Refuses to overwrite an existing\n\
                             destination unless --force is given.\n\n\
                             Examples:\n  \
                               meta config migrate yaml                  Write .metarepo.yaml next to current\n  \
                               meta config migrate toml --replace        Migrate and remove the old file\n  \
                               meta config migrate json --to .metarepo   Migrate to an explicit path",
                        )
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
