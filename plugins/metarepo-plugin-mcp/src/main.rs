//! Wire-protocol entry point for the mcp external plugin.
//!
//! Every command is declared `takeover` (protocol 1.3): the host execs this
//! binary with the command and raw arguments as argv instead of dispatching
//! over the plugin wire protocol. That keeps the plugin's full clap surface
//! (option flags, subcommand help) intact, and lets `serve` own stdin/stdout
//! for the MCP stdio transport — which the wire protocol would otherwise
//! occupy.

use anyhow::Result;
use metarepo_core::{MetaConfig, MetaPlugin, RuntimeConfig};
use metarepo_plugin_mcp::McpPlugin;
use metarepo_plugin_sdk::{
    serve_or_takeover, CommandInfo, Plugin, RuntimeConfigDto, TakeoverInvocation,
};

struct McpWirePlugin;

/// (name, about) for each subcommand, mirrored from the clap definitions in
/// `plugin.rs`. Only used for the host's `meta --help`; real parsing happens
/// in this binary after takeover.
const SUBCOMMANDS: &[(&str, &str)] = &[
    ("add", "Add a saved MCP server configuration"),
    ("list", "List saved MCP server configurations"),
    ("remove", "Remove a saved MCP server configuration"),
    ("connect", "Connect to an MCP server and show its info"),
    ("list-resources", "List resources from an MCP server"),
    ("list-tools", "List tools from an MCP server"),
    ("call-tool", "Call a tool on an MCP server"),
    ("serve", "Run Metarepo as an MCP server over stdio"),
    (
        "config",
        "Print Claude Desktop MCP configuration for Metarepo",
    ),
];

impl Plugin for McpWirePlugin {
    fn name(&self) -> &str {
        "mcp"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mut root = CommandInfo::new("mcp", "Manage MCP (Model Context Protocol) servers");
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
            "mcp command '{command}' requires takeover dispatch (plugin protocol 1.3); \
             upgrade metarepo"
        )
    }
}

/// Build the runtime config for a takeover run: the host's snapshot when
/// launched via `meta`, or discovery from the working directory when the
/// binary is invoked directly (e.g. by an MCP client config).
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
    let plugin = McpPlugin::new();

    // Reuse the plugin's own clap tree: register_commands adds the `mcp`
    // subcommand, so parse argv with the command name spliced in front.
    let app = plugin.register_commands(clap::Command::new("metarepo-plugin-mcp"));
    let mut argv = vec!["metarepo-plugin-mcp".to_string(), "mcp".to_string()];
    argv.extend(invocation.args);
    let matches = app.try_get_matches_from(argv)?;
    let (_, mcp_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow::anyhow!("No mcp command given. Try --help."))?;
    plugin.handle_command(mcp_matches, &config)
}

fn main() -> Result<()> {
    serve_or_takeover(McpWirePlugin, run_takeover)
}
