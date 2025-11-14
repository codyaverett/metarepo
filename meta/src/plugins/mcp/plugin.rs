use super::client::McpClient;
use super::config::McpConfig;
use super::mcp_server::{print_vscode_config, MetarepoMcpServer};
use super::server::McpServerConfig;
use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{arg, command, plugin, BasePlugin, MetaPlugin, RuntimeConfig};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

/// McpPlugin using the new simplified plugin architecture
pub struct McpPlugin;

impl McpPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("mcp")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Manage MCP (Model Context Protocol) servers")
            .author("Metarepo Contributors")
            .experimental(true)
            .command(
                command("add")
                    .about("Add a saved MCP server configuration")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Server name")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("command")
                            .help("Command to run")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("args")
                            .help("Arguments for the command")
                            .takes_value(true),
                    )
                    .arg(
                        arg("workdir")
                            .short('w')
                            .long("workdir")
                            .help("Working directory for the server")
                            .takes_value(true),
                    )
                    .arg(
                        arg("env")
                            .short('e')
                            .long("env")
                            .help("Environment variables")
                            .takes_value(true),
                    ),
            )
            .command(
                command("list")
                    .about("List saved MCP server configurations")
                    .with_help_formatting(),
            )
            .command(
                command("remove")
                    .about("Remove a saved MCP server configuration")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Server name")
                            .required(true)
                            .takes_value(true),
                    ),
            )
            .command(
                command("connect")
                    .about("Connect to an MCP server and show its info")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Saved server name OR command to run")
                            .takes_value(true),
                    )
                    .arg(
                        arg("args")
                            .help("Arguments (if using command directly)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("list-resources")
                    .about("List resources from an MCP server")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Saved server name OR command to run")
                            .takes_value(true),
                    )
                    .arg(
                        arg("args")
                            .help("Arguments (if using command directly)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("list-tools")
                    .about("List tools from an MCP server")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Saved server name OR command to run")
                            .takes_value(true),
                    )
                    .arg(
                        arg("args")
                            .help("Arguments (if using command directly)")
                            .takes_value(true),
                    ),
            )
            .command(
                command("call-tool")
                    .about("Call a tool on an MCP server")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Saved server name OR command to run")
                            .takes_value(true),
                    )
                    .arg(
                        arg("server-args")
                            .help("Server arguments (if using command directly)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("tool")
                            .help("Tool name to call")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("tool-args")
                            .long("args")
                            .help("Tool arguments as JSON")
                            .takes_value(true),
                    ),
            )
            .command(
                command("serve")
                    .about("Run Metarepo as an MCP server exposing CLI tools")
                    .with_help_formatting(),
            )
            .command(
                command("config")
                    .about("Print MCP configuration for VS Code or Claude Desktop")
                    .with_help_formatting(),
            )
            .handler("add", handle_add)
            .handler("list", handle_list)
            .handler("remove", handle_remove)
            .handler("connect", handle_connect)
            .handler("list-resources", handle_list_resources)
            .handler("list-tools", handle_list_tools)
            .handler("call-tool", handle_call_tool)
            .handler("serve", handle_serve)
            .handler("config", handle_config)
            .build()
    }
}

/// Handler for the add command
fn handle_add(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_add_async(matches).await })
}

async fn handle_add_async(matches: &ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    let command = matches.get_one::<String>("command").unwrap();
    let args: Vec<String> = matches
        .get_one::<String>("args")
        .map(|s| s.split_whitespace().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let working_dir = matches
        .get_one::<String>("workdir")
        .map(PathBuf::from);

    let env = matches.get_one::<String>("env").map(|s| {
        let mut map = HashMap::new();
        for env_var in s.split(',') {
            if let Some((key, value)) = env_var.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        map
    });

    let config = McpServerConfig {
        name: name.clone(),
        command: command.clone(),
        args,
        working_dir,
        env,
    };

    // Save to persistent configuration
    let mut saved_config = McpConfig::load()?;
    saved_config.add_server(config)?;

    println!("Added MCP server configuration '{}'", name);
    println!("Use 'meta mcp connect {}' to test the connection", name);
    println!("Use 'meta mcp list-tools {}' to see available tools", name);
    Ok(())
}

/// Handler for the list command
fn handle_list(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_list_async().await })
}

async fn handle_list_async() -> Result<()> {
    let config = McpConfig::load()?;
    let servers = config.list_servers();

    if servers.is_empty() {
        println!("No configured MCP servers");
        println!("\nAdd a server with: meta mcp add <name> <command> [args]");
        println!(
            "Example: meta mcp add playwright npx -- --yes @modelcontextprotocol/server-playwright"
        );
        return Ok(());
    }

    println!("Configured MCP servers:");
    println!("{:<20} {:<30} Args", "Name", "Command");
    println!("{}", "-".repeat(70));

    for server in servers {
        let args_str = if server.args.is_empty() {
            "-".to_string()
        } else {
            server.args.join(" ")
        };

        println!("{:<20} {:<30} {}", server.name, server.command, args_str);

        if let Some(ref workdir) = server.working_dir {
            println!("  Working dir: {}", workdir.display());
        }

        if let Some(ref env) = server.env {
            if !env.is_empty() {
                println!("  Environment: {:?}", env);
            }
        }
    }

    println!("\nUsage:");
    println!("  meta mcp connect <name>     - Test connection");
    println!("  meta mcp list-tools <name>  - List available tools");
    println!("  meta mcp call-tool <name> <tool> --args '{{}}'");

    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_remove_async(matches).await })
}

async fn handle_remove_async(matches: &ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();

    // Remove from persistent configuration
    let mut config = McpConfig::load()?;
    config.remove_server(name)?;

    println!("Removed MCP server configuration '{}'", name);
    Ok(())
}

/// Get server info helper function
async fn get_server_info(name: &str, args_str: Option<&str>) -> Result<(String, Vec<String>)> {
    // Check if this is a saved configuration
    let config = McpConfig::load()?;
    if let Some(server) = config.get_server(name) {
        Ok((server.command.clone(), server.args.clone()))
    } else {
        // Treat name as the command itself
        let args = args_str
            .map(|s| s.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        Ok((name.to_string(), args))
    }
}

/// Handler for the connect command
fn handle_connect(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_connect_async(matches).await })
}

async fn handle_connect_async(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
    let args = matches.get_one::<String>("args");

    let (command, server_args) = get_server_info(name, args.map(|s| s.as_str())).await?;

    println!("Connecting to MCP server: {} {:?}", command, server_args);
    let client = McpClient::connect(&command, &server_args).await?;

    if let Some(info) = client.server_info() {
        println!("\nServer Info:");
        println!("  Name: {}", info.name);
        println!("  Version: {}", info.version);
        println!("  Protocol: {}", info.protocol_version);
        println!("  Capabilities:");
        let res_enabled = !info.capabilities.resources.is_null()
            && (info.capabilities.resources.as_bool().unwrap_or(false)
                || info.capabilities.resources.is_object());
        let tools_enabled = !info.capabilities.tools.is_null()
            && (info.capabilities.tools.as_bool().unwrap_or(false)
                || info.capabilities.tools.is_object());
        let prompts_enabled = !info.capabilities.prompts.is_null()
            && (info.capabilities.prompts.as_bool().unwrap_or(false)
                || info.capabilities.prompts.is_object());
        println!("    Resources: {}", res_enabled);
        println!("    Tools: {}", tools_enabled);
        println!("    Prompts: {}", prompts_enabled);
    }

    println!("\nServer connected successfully!");

    client.close().await?;
    Ok(())
}

/// Handler for the list-resources command
fn handle_list_resources(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_list_resources_async(matches).await })
}

async fn handle_list_resources_async(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
    let args = matches.get_one::<String>("args");

    let (command, server_args) = get_server_info(name, args.map(|s| s.as_str())).await?;

    let mut client = McpClient::connect(&command, &server_args).await?;
    let resources = client.list_resources().await?;

    if resources.is_empty() {
        println!("No resources available");
    } else {
        println!("Available resources:");
        for resource in resources {
            println!("\n  URI: {}", resource.uri);
            println!("  Name: {}", resource.name);
            if let Some(desc) = resource.description {
                println!("  Description: {}", desc);
            }
            if let Some(mime) = resource.mime_type {
                println!("  MIME Type: {}", mime);
            }
        }
    }

    client.close().await?;
    Ok(())
}

/// Handler for the list-tools command
fn handle_list_tools(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_list_tools_async(matches).await })
}

async fn handle_list_tools_async(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
    let args = matches.get_one::<String>("args");

    let (command, server_args) = get_server_info(name, args.map(|s| s.as_str())).await?;

    let mut client = McpClient::connect(&command, &server_args).await?;
    let tools = client.list_tools().await?;

    if tools.is_empty() {
        println!("No tools available");
    } else {
        println!("Available tools:");
        for tool in tools {
            println!("\n  Name: {}", tool.name);
            if let Some(desc) = tool.description {
                println!("  Description: {}", desc);
            }
            println!(
                "  Input Schema: {}",
                serde_json::to_string_pretty(&tool.input_schema)?
            );
        }
    }

    client.close().await?;
    Ok(())
}

/// Handler for the call-tool command
fn handle_call_tool(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { handle_call_tool_async(matches).await })
}

async fn handle_call_tool_async(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
    let server_args = matches.get_one::<String>("server-args");
    let tool_name = matches.get_one::<String>("tool").unwrap();
    let tool_args = matches
        .get_one::<String>("tool-args")
        .map(|s| serde_json::from_str(s))
        .transpose()?
        .unwrap_or(json!({}));

    let (command, args) = get_server_info(name, server_args.map(|s| s.as_str())).await?;

    let mut client = McpClient::connect(&command, &args).await?;

    println!("Calling tool '{}' with args: {}", tool_name, tool_args);
    let result = client.call_tool(tool_name, tool_args).await?;

    println!("\nResult:");
    println!("{}", serde_json::to_string_pretty(&result)?);

    client.close().await?;
    Ok(())
}

/// Handler for the serve command
fn handle_serve(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let mut server = MetarepoMcpServer::new();
    server.run()?;
    Ok(())
}

/// Handler for the config command
fn handle_config(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    print_vscode_config();
    Ok(())
}

// Traditional implementation for backward compatibility
impl MetaPlugin for McpPlugin {
    fn name(&self) -> &str {
        "mcp"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.handle_command(matches, config)
    }
}

impl BasePlugin for McpPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Manage MCP (Model Context Protocol) servers")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for McpPlugin {
    fn default() -> Self {
        Self::new()
    }
}
