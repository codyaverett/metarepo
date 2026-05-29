use anyhow::{anyhow, Context, Result};
use clap::ArgMatches;
use colored::Colorize;
use metarepo_core::{
    arg, command, plugin, BasePlugin, MetaConfig, MetaPlugin, PluginManifest, RuntimeConfig,
};
use std::collections::HashMap;
use std::path::PathBuf;

use super::install::{install_from_spec, resolved_binary_path};
use super::spec::PluginSpec;
use crate::plugins::plugin_loader::ExternalPlugin;

/// Manages external metarepo plugins: install / list / remove / update.
pub struct PluginManagerPlugin;

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn create_plugin() -> impl MetaPlugin {
        plugin("plugin")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Manage metarepo plugins")
            .author("Metarepo Contributors")
            .command(
                command("install")
                    .about("Install a plugin and register it in .metarepo")
                    .long_about(
                        "Install an external plugin and record it under plugins.<name> in the\n\
                         active .metarepo so it loads on the next run.\n\n\
                         Sources (via --from):\n  \
                           crates:<crate>    install from crates.io (default: metarepo-plugin-<name>)\n  \
                           file:<path>       copy a local executable\n  \
                           git+<url>         clone and cargo build --release\n\n\
                         Examples:\n  \
                           meta plugin install hello\n  \
                           meta plugin install hello --version 0.2.0\n  \
                           meta plugin install hello --from file:./target/release/metarepo-plugin-hello\n  \
                           meta plugin install hello --from git+https://github.com/me/metarepo-plugin-hello.git",
                    )
                    .arg(
                        arg("name")
                            .help("Plugin command name (e.g. hello for 'meta hello')")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("from")
                            .long("from")
                            .help("Source spec: crates:<crate>, file:<path>, or git+<url>")
                            .takes_value(true),
                    )
                    .arg(
                        arg("version")
                            .long("version")
                            .help("Version to install (crates.io sources only)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("list")
                    .about("List registered plugins and their install status")
                    .alias("ls"),
            )
            .command(
                command("remove")
                    .about("Remove a plugin from .metarepo (and optionally its binary)")
                    .alias("rm")
                    .arg(
                        arg("name")
                            .help("Plugin command name to remove")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("purge")
                            .long("purge")
                            .help("Also delete the installed binary"),
                    ),
            )
            .command(
                command("update")
                    .about("Reinstall a plugin (or all) from its recorded spec")
                    .arg(
                        arg("name")
                            .help("Plugin to update; omit to update all")
                            .required(false)
                            .takes_value(true),
                    ),
            )
            .handler("install", handle_install)
            .handler("list", handle_list)
            .handler("remove", handle_remove)
            .handler("update", handle_update)
            .build()
    }
}

/// Resolve the active config file path, preferring the one the host already
/// loaded (which honors --config), falling back to discovery.
fn active_meta_file(config: &RuntimeConfig) -> Option<PathBuf> {
    config
        .meta_file_path
        .clone()
        .or_else(MetaConfig::find_meta_file)
}

fn require_meta_file(config: &RuntimeConfig) -> Result<PathBuf> {
    active_meta_file(config).ok_or_else(|| {
        anyhow!("No metarepo config found. Run 'meta init' first, or pass --config <path>.")
    })
}

fn load_plugins(meta_file: &PathBuf) -> Result<(MetaConfig, HashMap<String, String>)> {
    let cfg = MetaConfig::load_from_file(meta_file)?;
    let plugins = cfg.plugins.clone().unwrap_or_default();
    Ok((cfg, plugins))
}

fn handle_install(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Plugin name is required"))?;
    let from = matches.get_one::<String>("from").map(String::as_str);
    let version = matches.get_one::<String>("version").map(String::as_str);

    // Require a config up front so we don't install something we can't register.
    let meta_file = require_meta_file(config)?;

    let spec = PluginSpec::from_args(name, from, version)?;
    println!(
        "\n  {} {} ({})",
        "Installing".cyan().bold(),
        name,
        spec.source_label()
    );

    let stored = install_from_spec(name, &spec)?;

    let mut cfg = MetaConfig::load_from_file(&meta_file)?;
    cfg.plugins
        .get_or_insert_with(HashMap::new)
        .insert(name.clone(), stored.to_spec_string());
    cfg.save_to_file(&meta_file)
        .with_context(|| format!("Failed to update {}", meta_file.display()))?;

    println!(
        "  {} Registered '{}' in {} — available on next run",
        "✓".green(),
        name,
        meta_file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| meta_file.display().to_string())
    );
    Ok(())
}

fn handle_list(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let meta_file = match active_meta_file(config) {
        Some(p) => p,
        None => {
            println!("  {} No metarepo config found.", "·".bright_black());
            return Ok(());
        }
    };

    let (_, plugins) = load_plugins(&meta_file)?;
    if plugins.is_empty() {
        println!(
            "  {} No plugins registered in {}",
            "·".bright_black(),
            meta_file.display()
        );
        return Ok(());
    }

    println!("\n  {}", "Registered plugins".bold());
    let mut names: Vec<&String> = plugins.keys().collect();
    names.sort();
    for name in names {
        let spec_str = &plugins[name];
        match PluginSpec::parse(name, spec_str) {
            Ok(spec) => print_plugin_status(name, &spec),
            Err(e) => println!("  {} {}  ({})", "✗".red(), name, e),
        }
    }
    Ok(())
}

fn print_plugin_status(name: &str, spec: &PluginSpec) {
    let path = match resolved_binary_path(name, spec) {
        Ok(p) => p,
        Err(e) => {
            println!("  {} {}  (cannot resolve: {})", "✗".red(), name, e);
            return;
        }
    };

    if !path.exists() {
        println!(
            "  {} {}  [{}]  missing — run 'meta plugin install {}'",
            "✗".red(),
            name.bold(),
            spec.source_label(),
            name
        );
        return;
    }

    // Manifest plugins: read the version from the manifest instead of probing
    // (the binary may be a script that doesn't speak the protocol).
    if PluginManifest::is_manifest_path(&path) {
        let version = PluginManifest::from_file_auto(&path)
            .map(|m| m.plugin.version)
            .unwrap_or_else(|_| "?".to_string());
        println!(
            "  {} {}  [manifest]  installed (v{})",
            "✓".green(),
            name.bold(),
            version
        );
        return;
    }

    // Installed: try to probe the reported version for a mismatch check.
    let declared = spec.declared_version();
    match ExternalPlugin::probe(&path) {
        Ok((_, installed)) => {
            if let Some(declared) = declared {
                if declared != installed {
                    println!(
                        "  {} {}  [{}]  version mismatch (declared {}, installed {})",
                        "⚠".yellow(),
                        name.bold(),
                        spec.source_label(),
                        declared,
                        installed
                    );
                    return;
                }
            }
            println!(
                "  {} {}  [{}]  installed (v{})",
                "✓".green(),
                name.bold(),
                spec.source_label(),
                installed
            );
        }
        Err(_) => {
            // Binary present but not probeable (e.g. outside the allowed path
            // policy or not protocol-speaking). Report presence only.
            println!(
                "  {} {}  [{}]  installed at {}",
                "✓".green(),
                name.bold(),
                spec.source_label(),
                path.display()
            );
        }
    }
}

fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Plugin name is required"))?;
    let purge = matches.get_flag("purge");

    let meta_file = require_meta_file(config)?;
    let mut cfg = MetaConfig::load_from_file(&meta_file)?;

    let removed_spec = cfg
        .plugins
        .as_mut()
        .and_then(|plugins| plugins.remove(name));

    let Some(spec_str) = removed_spec else {
        return Err(anyhow!(
            "Plugin '{}' is not registered in {}",
            name,
            meta_file.display()
        ));
    };

    cfg.save_to_file(&meta_file)
        .with_context(|| format!("Failed to update {}", meta_file.display()))?;
    println!(
        "  {} Removed '{}' from {}",
        "✓".green(),
        name,
        meta_file.display()
    );

    if purge {
        if let Ok(spec) = PluginSpec::parse(name, &spec_str) {
            if let Ok(path) = resolved_binary_path(name, &spec) {
                if PluginManifest::is_manifest_path(&path) {
                    // Manifest plugins live in a per-plugin directory; remove it.
                    if let Some(dir) = path.parent() {
                        if dir.exists() {
                            std::fs::remove_dir_all(dir)
                                .with_context(|| format!("Failed to delete {}", dir.display()))?;
                            println!("  {} Deleted {}", "✓".yellow(), dir.display());
                        }
                    }
                } else if path.exists() {
                    std::fs::remove_file(&path)
                        .with_context(|| format!("Failed to delete {}", path.display()))?;
                    println!("  {} Deleted binary {}", "✓".yellow(), path.display());
                }
            }
        }
    }
    Ok(())
}

fn handle_update(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let meta_file = require_meta_file(config)?;
    let (_, plugins) = load_plugins(&meta_file)?;

    if let Some(name) = matches.get_one::<String>("name") {
        let spec_str = plugins
            .get(name)
            .ok_or_else(|| anyhow!("Plugin '{}' is not registered", name))?;
        let spec = PluginSpec::parse(name, spec_str)?;
        // file: sources record the install destination, so reinstalling would
        // copy the file onto itself and truncate it. They have no upstream to
        // pull from — reinstall from the original source instead.
        if matches!(spec, PluginSpec::File { .. }) {
            println!(
                "\n  {} '{}' was installed from a file source — nothing to update.",
                "·".bright_black(),
                name
            );
            println!(
                "    To refresh it, reinstall from the original source: meta plugin install {name} --from file:<path>"
            );
            return Ok(());
        }
        println!("\n  {} {}", "Updating".cyan().bold(), name);
        install_from_spec(name, &spec)?;
        println!("  {} Updated '{}'", "✓".green(), name);
        return Ok(());
    }

    if plugins.is_empty() {
        println!("  {} No plugins to update.", "·".bright_black());
        return Ok(());
    }

    let mut names: Vec<&String> = plugins.keys().collect();
    names.sort();
    for name in names {
        let spec = match PluginSpec::parse(name, &plugins[name]) {
            Ok(s) => s,
            Err(e) => {
                println!("  {} {} skipped ({})", "✗".red(), name, e);
                continue;
            }
        };
        // file: sources have no upstream to pull from; skip in bulk update.
        if matches!(spec, PluginSpec::File { .. }) {
            println!("  {} {} skipped (file source)", "·".bright_black(), name);
            continue;
        }
        println!("  {} {}", "Updating".cyan(), name);
        match install_from_spec(name, &spec) {
            Ok(_) => println!("  {} {}", "✓".green(), name),
            Err(e) => println!("  {} {} failed: {}", "✗".red(), name, e),
        }
    }
    Ok(())
}

impl MetaPlugin for PluginManagerPlugin {
    fn name(&self) -> &str {
        "plugin"
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        Self::create_plugin().register_commands(app)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        Self::create_plugin().handle_command(matches, config)
    }
}

impl BasePlugin for PluginManagerPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Manage metarepo plugins")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for PluginManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}
