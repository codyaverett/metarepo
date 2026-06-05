use super::client::McpClient;
use super::config::McpConfig;
use super::mcp_server::{print_vscode_config, MetarepoMcpServer, ServePolicy, WorkspaceTarget};
use super::server::McpServerConfig;
use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    arg, command, plugin, BasePlugin, ConfigSetting, ConfigValueType, MetaConfig, MetaPlugin,
    RuntimeConfig,
};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
            .help_description(
                "Run Metarepo as an MCP server and manage connections to other MCP servers.\n\
                 \n\
                 This plugin has two sides. The 'serve' command turns Metarepo itself into\n\
                 a stdio MCP server, exposing its git, project, exec, and mcp commands as\n\
                 tools that Claude Desktop, VS Code, or any MCP client can call. The other\n\
                 commands act as an MCP client: they save server definitions, connect to\n\
                 them, and list or invoke their resources and tools.\n\
                 \n\
                 Saved servers are stored in ~/.config/meta/mcp/servers.json. Commands that\n\
                 take a NAME accept either a saved server name or a raw command to launch\n\
                 directly. Use 'config' to print ready-to-paste client configuration.\n\
                 \n\
                 Examples:\n\
                 \n\
                   meta mcp serve                                          expose Metarepo as an MCP server\n\
                   meta mcp add playwright npx '@playwright/mcp@latest'    save a server definition\n\
                   meta mcp list-tools playwright                          inspect a server's tools",
            )
            .command(
                command("add")
                    .about("Add a saved MCP server configuration")
                    .help_description(
                        "Save an MCP server definition to ~/.config/meta/mcp/servers.json.\n\
                         \n\
                         Records the launch command, arguments, optional working directory,\n\
                         and environment variables under a name you can reuse with connect,\n\
                         list-tools, list-resources, and call-tool. Fails if a server with the\n\
                         same name already exists; remove it first to redefine it.\n\
                         \n\
                         Pass extra arguments as a single quoted string with --workdir to set\n\
                         the working directory and --env KEY=VALUE,KEY2=VALUE2 for environment.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta mcp add playwright npx '@playwright/mcp@latest'\n\
                           meta mcp add local ./server --env DEBUG=1,PORT=8080",
                    )
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
                        // No `-w` short: that's reserved for the global
                        // `--workspace` scope flag.
                        arg("workdir")
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
                    .help_description(
                        "Show every MCP server saved in ~/.config/meta/mcp/servers.json.\n\
                         \n\
                         Prints a table of each server's name, launch command, and arguments,\n\
                         plus any working directory and environment variables. When no servers\n\
                         are configured, prints hints for adding one. This reads only saved\n\
                         definitions; it does not connect to the servers.\n\
                         \n\
                         Example:\n\
                         \n\
                           meta mcp list",
                    )
                    .with_help_formatting(),
            )
            .command(
                command("remove")
                    .about("Remove a saved MCP server configuration")
                    .help_description(
                        "Delete a saved MCP server definition by name.\n\
                         \n\
                         Removes the named entry from ~/.config/meta/mcp/servers.json. Fails if\n\
                         no server with that name exists. This only forgets the saved\n\
                         definition; it does not stop any running server process.\n\
                         \n\
                         Example:\n\
                         \n\
                           meta mcp remove playwright",
                    )
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
                    .help_description(
                        "Connect to an MCP server and print its handshake details.\n\
                         \n\
                         Launches the server, performs the MCP initialize handshake, and\n\
                         reports its name, version, protocol version, and which capabilities\n\
                         (resources, tools, prompts) it advertises, then disconnects. Use it\n\
                         as a quick health check that a server starts and speaks MCP.\n\
                         \n\
                         NAME is either a saved server or a raw command; when passing a command\n\
                         directly, supply its arguments as a single quoted ARGS string.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta mcp connect playwright\n\
                           meta mcp connect npx '@playwright/mcp@latest'",
                    )
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
                    .help_description(
                        "Connect to an MCP server and list the resources it exposes.\n\
                         \n\
                         Performs the handshake, queries resources/list, and prints each\n\
                         resource's URI, name, and (when provided) description and MIME type,\n\
                         then disconnects. Reports when the server exposes no resources.\n\
                         \n\
                         NAME is either a saved server or a raw command; when passing a command\n\
                         directly, supply its arguments as a single quoted ARGS string.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta mcp list-resources filesystem\n\
                           meta mcp list-resources npx '@modelcontextprotocol/server-filesystem'",
                    )
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
                    .help_description(
                        "Connect to an MCP server and list the tools it offers.\n\
                         \n\
                         Performs the handshake, queries tools/list, and prints each tool's\n\
                         name, description, and full JSON input schema, then disconnects. Use\n\
                         this to discover tool names and argument shapes before call-tool.\n\
                         \n\
                         NAME is either a saved server or a raw command; when passing a command\n\
                         directly, supply its arguments as a single quoted ARGS string.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta mcp list-tools playwright\n\
                           meta mcp list-tools npx '@playwright/mcp@latest'",
                    )
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
                    .help_description(
                        "Invoke a single tool on an MCP server and print the result.\n\
                         \n\
                         Connects, calls the named tool with the supplied JSON arguments, prints\n\
                         the pretty-printed result, then disconnects. Arguments default to an\n\
                         empty object when --args is omitted. Run list-tools first to find the\n\
                         tool name and its expected input schema.\n\
                         \n\
                         NAME is either a saved server or a raw command. When launching a\n\
                         command directly, pass its launch arguments as SERVER-ARGS before the\n\
                         TOOL name; tool arguments always go in the --args JSON string.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta mcp call-tool playwright browser_navigate --args '{\"url\":\"https://example.com\"}'\n\
                           meta mcp call-tool filesystem read_file --args '{\"path\":\"README.md\"}'",
                    )
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
                        // Not marked required at the clap level: it follows the
                        // optional `server-args` positional, and clap forbids a
                        // required positional after an optional one. Presence is
                        // enforced at runtime in the handler instead.
                        arg("tool").help("Tool name to call").takes_value(true),
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
                    .about("Run Metarepo as an MCP server over stdio")
                    .arg(
                        arg("meta")
                            .long("meta")
                            .help(
                                "Pin the server to this workspace (a .meta file or its \
                                 directory). Tools run with --config set to it. Defaults to \
                                 discovery from the launch directory.",
                            )
                            .takes_value(true),
                    )
                    .arg(
                        arg("allow-workspaces")
                            .long("allow-workspaces")
                            .help(
                                "Comma-separated workspaces to host (allowlist mode). Tools take \
                                 a 'workspace' argument selecting one; each workspace's own \
                                 [mcp.serve] policy applies. Overrides --meta.",
                            )
                            .takes_value(true),
                    )
                    .help_description(
                        "Run Metarepo itself as an MCP server, speaking JSON-RPC over stdio.\n\
                         \n\
                         Reads MCP requests from stdin and writes responses to stdout, exposing\n\
                         Metarepo's own commands as tools: help, git_status, git_diff,\n\
                         git_commit, git_pull, git_push, project_list, project_add,\n\
                         project_remove, exec, and mcp_add_server / mcp_list_servers /\n\
                         mcp_remove_server. Each tool shells out to this same binary.\n\
                         \n\
                         You normally do not run this by hand; an MCP client (Claude Desktop,\n\
                         VS Code) launches it. Run 'meta mcp config' to print the client\n\
                         configuration that points at this command. The server runs until\n\
                         stdin closes.\n\
                         \n\
                         Example:\n\
                         \n\
                           meta mcp serve",
                    )
                    .with_help_formatting(),
            )
            .command(
                command("config")
                    .about("Print Claude Desktop MCP configuration for Metarepo")
                    .help_description(
                        "Print ready-to-paste client configuration for the Metarepo MCP server.\n\
                         \n\
                         Emits a Claude Desktop mcpServers JSON block wired to run 'meta mcp\n\
                         serve' with the absolute path of this binary, along with the config\n\
                         file locations for macOS and Windows, a summary of the exposed tools,\n\
                         and testing instructions. Copy the JSON into your client config to\n\
                         register Metarepo as an MCP server.\n\
                         \n\
                         Example:\n\
                         \n\
                           meta mcp config",
                    )
                    .with_help_formatting()
                    .arg(
                        arg("meta")
                            .long("meta")
                            .help(
                                "Comma-separated workspaces to emit entries for (one pinned \
                                 entry each). Defaults to the current directory.",
                            )
                            .takes_value(true),
                    )
                    .arg(
                        arg("allow-workspaces")
                            .long("allow-workspaces")
                            .help("Emit a single allowlist entry hosting all --meta workspaces"),
                    ),
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

    let working_dir = matches.get_one::<String>("workdir").map(PathBuf::from);

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
    let tool_name = matches
        .get_one::<String>("tool")
        .ok_or_else(|| anyhow::anyhow!("Tool name required"))?;
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

/// Resolve the workspace this server should be pinned to: returns the `.meta`
/// file to inject as `--config` and the directory to run tool subprocesses in.
///
/// `--meta` may point at a `.meta` file or a directory containing one; without
/// it we fall back to whatever was discovered from the launch directory.
fn resolve_workspace(
    matches: &ArgMatches,
    config: &RuntimeConfig,
) -> (Option<PathBuf>, Option<PathBuf>) {
    if let Some(raw) = matches.get_one::<String>("meta") {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            // Discover the .meta inside the directory.
            match MetaConfig::discover_from(&path) {
                Ok(Some(found)) => {
                    let root = found
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| path.clone());
                    return (Some(found.path), Some(root));
                }
                _ => return (None, Some(path)),
            }
        }
        // Treat as a .meta file path.
        let root = path.parent().map(Path::to_path_buf);
        return (Some(path), root);
    }

    // No --meta: use whatever the launch directory discovered.
    let root = config
        .meta_file_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf);
    (config.meta_file_path.clone(), root)
}

/// Build a workspace target from a `.meta` file/dir path, loading its policy.
fn target_from_path(raw: &str) -> WorkspaceTarget {
    let path = PathBuf::from(raw.trim());
    let (config, root) = if path.is_dir() {
        match MetaConfig::discover_from(&path) {
            Ok(Some(found)) => {
                let root = found
                    .path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| path.clone());
                (Some(found.path), Some(root))
            }
            _ => (None, Some(path.clone())),
        }
    } else {
        (Some(path.clone()), path.parent().map(Path::to_path_buf))
    };
    let pinned = match &config {
        Some(p) => MetaConfig::load_from_file(p).unwrap_or_default(),
        None => MetaConfig::default(),
    };
    let policy = ServePolicy::from_settings(pinned.mcp.as_ref().and_then(|m| m.serve.as_ref()));
    WorkspaceTarget {
        name: WorkspaceTarget::derive_name(root.as_deref()),
        config,
        root,
        policy,
    }
}

/// Handler for the serve command
fn handle_serve(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    // Allowlist mode: host several workspaces, selected per call.
    if let Some(list) = matches.get_one::<String>("allow-workspaces") {
        let targets: Vec<WorkspaceTarget> = list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(target_from_path)
            .collect();
        if targets.is_empty() {
            return Err(anyhow::anyhow!(
                "--allow-workspaces listed no valid workspaces"
            ));
        }
        let mut server = MetarepoMcpServer::with_targets(targets, true);
        server.run()?;
        return Ok(());
    }

    // Pinned mode: a single workspace (from --meta or launch-dir discovery).
    let (meta_file, root) = resolve_workspace(matches, config);
    let pinned = match &meta_file {
        Some(p) => MetaConfig::load_from_file(p).unwrap_or_else(|_| config.meta_config.clone()),
        None => config.meta_config.clone(),
    };
    let policy = ServePolicy::from_settings(pinned.mcp.as_ref().and_then(|m| m.serve.as_ref()));

    let mut server = MetarepoMcpServer::with_options(meta_file, root, policy);
    server.run()?;
    Ok(())
}

/// Handler for the config command
fn handle_config(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let workspaces: Vec<String> = matches
        .get_one::<String>("meta")
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let allowlist = matches.get_flag("allow-workspaces");
    print_vscode_config(&workspaces, allowlist);
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

    fn settings(&self) -> Vec<ConfigSetting> {
        vec![
            ConfigSetting::new(
                "mcp.serve.mode",
                "Policy when served via mcp serve: full (default), read-write, or read-only",
                ConfigValueType::String,
            )
            .with_default("full"),
            ConfigSetting::new(
                "mcp.serve.allow-exec",
                "Allow the arbitrary-shell exec tool when serving (default: true)",
                ConfigValueType::Bool,
            )
            .with_default("true"),
            ConfigSetting::new(
                "mcp.serve.tools",
                "Optional allowlist of tool names exposed when serving",
                ConfigValueType::StringList,
            ),
            ConfigSetting::new(
                "mcp.serve.projects",
                "Optional allowlist of projects the exec tool may target when serving",
                ConfigValueType::StringList,
            ),
        ]
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
