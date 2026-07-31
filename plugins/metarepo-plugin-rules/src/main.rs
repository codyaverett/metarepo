//! Wire-protocol entry point for the rules external plugin.
//!
//! Every command is declared `takeover` (protocol 1.3): the host execs this
//! binary with the command and raw arguments as argv instead of dispatching
//! over the plugin wire protocol, so the plugin's full clap surface (option
//! flags like --project and --fix, nested create subcommands) stays intact.

use anyhow::Result;
use metarepo_core::{MetaConfig, MetaPlugin, RuntimeConfig};
use metarepo_plugin_rules::RulesPlugin;
use metarepo_plugin_sdk::{
    serve_or_takeover, CommandInfo, Plugin, RuntimeConfigDto, TakeoverInvocation,
};

struct RulesWirePlugin;

/// (name, about) for each subcommand, mirrored from the clap definitions in
/// `plugin.rs`. Only used for the host's `meta --help`; real parsing happens
/// in this binary after takeover.
const SUBCOMMANDS: &[(&str, &str)] = &[
    ("check", "Check project structure against configured rules"),
    ("init", "Initialize rules configuration file"),
    ("list", "List all configured rules"),
    ("docs", "Show documentation for creating and using rules"),
    ("create", "Create a new rule"),
    ("status", "Show rules status for all projects"),
    ("copy", "Copy workspace rules to a specific project"),
];

impl Plugin for RulesWirePlugin {
    fn name(&self) -> &str {
        "rules"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mut root = CommandInfo::new("rules", "Manage and enforce project structure rules");
        for (name, about) in SUBCOMMANDS {
            root = root.subcommand(CommandInfo::new(*name, *about).takeover());
        }
        vec![root]
    }

    fn handle(
        &self,
        command: &str,
        _args: &[String],
        _config: &RuntimeConfigDto,
    ) -> Result<Option<String>> {
        // All commands are takeover; a host that dispatches over the wire
        // predates protocol 1.3 and cannot run this plugin's commands.
        anyhow::bail!(
            "rules command '{command}' requires takeover dispatch (plugin protocol 1.3); \
             upgrade metarepo"
        )
    }
}

/// Build the runtime config for a takeover run: the host's snapshot when
/// launched via `meta`, or discovery from the working directory when the
/// binary is invoked directly.
fn runtime_config(config: Option<RuntimeConfigDto>) -> Result<RuntimeConfig> {
    if let Some(dto) = config {
        return Ok(dto.into());
    }
    let working_dir = std::env::current_dir()?;
    let (meta_config, meta_file_path) = match MetaConfig::discover_from(&working_dir) {
        Ok(Some(found)) => {
            let config = MetaConfig::load_from_file_with_format(&found.path, found.format)?;
            (config, Some(found.path))
        }
        Ok(None) => (MetaConfig::default(), None),
        Err(e) => return Err(anyhow::anyhow!("{}", e)),
    };
    Ok(RuntimeConfig {
        meta_config,
        working_dir,
        meta_file_path,
        experimental: false,
        non_interactive: None,
        scope_workspace: false,
        settings_catalog: Vec::new(),
    })
}

fn run_takeover(invocation: TakeoverInvocation) -> Result<()> {
    let config = runtime_config(invocation.config)?;
    let plugin = RulesPlugin::new();

    // Reuse the plugin's own clap tree: register_commands adds the `rules`
    // subcommand, so parse argv with the command name spliced in front.
    let app = plugin.register_commands(clap::Command::new("metarepo-plugin-rules"));
    let mut argv = vec!["metarepo-plugin-rules".to_string(), "rules".to_string()];
    argv.extend(invocation.args);
    let matches = app.try_get_matches_from(argv)?;
    let (_, rules_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow::anyhow!("No rules command given. Try --help."))?;
    plugin.handle_command(rules_matches, &config)
}

fn main() -> Result<()> {
    serve_or_takeover(RulesWirePlugin, run_takeover)
}
