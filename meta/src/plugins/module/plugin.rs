use super::{discover, enable as ops, scan};
use anyhow::{anyhow, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use metarepo_core::{BasePlugin, MetaConfig, MetaPlugin, NonInteractiveMode, RuntimeConfig};
use std::path::PathBuf;

/// Manage meta modules: repos that bundle a plugin and/or skills as one unit.
pub struct ModulePlugin;

impl ModulePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ModulePlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the active config file, erroring with guidance when none exists.
fn require_meta_file(config: &RuntimeConfig) -> Result<PathBuf> {
    config
        .meta_file_path
        .clone()
        .or_else(MetaConfig::find_meta_file)
        .ok_or_else(|| {
            anyhow!("No metarepo config found. Run 'meta init' first, or pass --config <path>.")
        })
}

impl MetaPlugin for ModulePlugin {
    fn name(&self) -> &str {
        "module"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(module_command())
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("enable", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(PathBuf::from)
                    .expect("path is required");
                let meta_file = require_meta_file(config)?;
                ops::enable(
                    &path,
                    &meta_file,
                    m.get_flag("force"),
                    m.get_flag("overwrite"),
                )
            }
            Some(("disable", m)) => {
                let name = m
                    .get_one::<String>("name")
                    .map(String::as_str)
                    .expect("name is required");
                let meta_file = require_meta_file(config)?;
                ops::disable(name, &meta_file)
            }
            Some(("list", _)) => {
                let meta_file = require_meta_file(config)?;
                ops::list(&meta_file)
            }
            Some(("status", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(PathBuf::from)
                    .expect("path is required");
                ops::status(&path)
            }
            Some(("scan", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(String::as_str)
                    .unwrap_or(".");
                scan::run(path)
            }
            _ => {
                module_command().print_help()?;
                println!();
                Ok(())
            }
        }
    }
}

/// Surface a module manifest discovered in `repo` and, in a TTY, offer to enable
/// it. Called by `meta project add`. Never wires up without explicit consent.
pub fn offer_enable_after_add(
    repo: &std::path::Path,
    config: &RuntimeConfig,
    non_interactive: NonInteractiveMode,
) {
    if let Some(meta_file) = config
        .meta_file_path
        .clone()
        .or_else(MetaConfig::find_meta_file)
    {
        if let Err(e) = discover::offer_enable(repo, &meta_file, non_interactive) {
            eprintln!("  ! module discovery: {}", e);
        }
    }
}

fn module_command() -> Command {
    Command::new("module")
        .about("Manage meta modules (repos bundling a plugin and/or skills)")
        .version(env!("CARGO_PKG_VERSION"))
        .long_about(
            "Enable, disable, and inspect meta modules. A module is a repo with a\n\
             meta.module.* manifest that bundles the plugin it provides and the\n\
             Claude Code skills that drive it. Enabling stages the plugin into\n\
             .meta-modules/ and installs the skills (audit-gated).\n\n\
             Examples:\n  \
               meta module                       Show this help\n  \
               meta module status <repo>         Preview what a module would wire up\n  \
               meta module enable <repo>         Stage the plugin and install the skills\n  \
               meta module enable <repo> -f      Enable despite HIGH skill findings\n  \
               meta module list                  List enabled modules\n  \
               meta module disable <name>        Reverse an enable\n  \
               meta module scan <dir>            List module manifests under a path",
        )
        .subcommand_required(false)
        .subcommand(
            Command::new("enable")
                .about("Stage a module's plugin and install its skills")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("Path to the module repo (containing meta.module.*)")
                        .required(true),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .action(ArgAction::SetTrue)
                        .help("Install skills even with HIGH-severity audit findings"),
                )
                .arg(
                    Arg::new("overwrite")
                        .long("overwrite")
                        .action(ArgAction::SetTrue)
                        .help("Replace already-registered plugins/skills of the same name"),
                ),
        )
        .subcommand(
            Command::new("disable")
                .about("Remove an enabled module's plugin, skills, and config entry")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("name")
                        .help("Module name (as shown by 'meta module list')")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List enabled modules")
                .version(env!("CARGO_PKG_VERSION")),
        )
        .subcommand(
            Command::new("status")
                .about("Preview what enabling a module would wire up")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("Path to the module repo")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("scan")
                .about("Walk a directory and list the module manifests found")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("Directory to scan (defaults to current dir)")
                        .default_value("."),
                ),
        )
}

impl BasePlugin for ModulePlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Manage meta modules (plugin + skill bundles)")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}
