use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::client::McpClient;
use crate::mcp_server::{GestaltMcpServer, print_vscode_config};
use crate::config::McpConfig;
use crate::server::McpServerConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use serde_json::json;

pub struct McpPlugin;

impl McpPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest mcp")
            .about("Manage MCP (Model Context Protocol) servers")
            .subcommand(
                Command::new("add")
                    .about("Add a saved MCP server configuration")
                    .arg(Arg::new("name").required(true).help("Server name"))
                    .arg(Arg::new("command").required(true).help("Command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                    .arg(Arg::new("workdir")
                        .short('w')
                        .long("workdir")
                        .value_name("PATH")
                        .help("Working directory for the server"))
                    .arg(Arg::new("env")
                        .short('e')
                        .long("env")
                        .value_name("KEY=VALUE")
                        .num_args(0..)
                        .help("Environment variables"))
            )
            .subcommand(
                Command::new("list")
                    .about("List saved MCP server configurations")
            )
            .subcommand(
                Command::new("remove")
                    .about("Remove a saved MCP server configuration")
                    .arg(Arg::new("name").required(true).help("Server name"))
            )
            .subcommand(
                Command::new("connect")
                    .about("Connect to an MCP server and show its info")
                    .arg(Arg::new("name").help("Saved server name OR command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
            )
            .subcommand(
                Command::new("list-resources")
                    .about("List resources from an MCP server")
                    .arg(Arg::new("name").help("Saved server name OR command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
            )
            .subcommand(
                Command::new("list-tools")
                    .about("List tools from an MCP server")
                    .arg(Arg::new("name").help("Saved server name OR command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
            )
            .subcommand(
                Command::new("call-tool")
                    .about("Call a tool on an MCP server")
                    .arg(Arg::new("name").help("Saved server name OR command to run"))
                    .arg(Arg::new("server-args").num_args(0..).help("Server arguments (if using command directly)"))
                    .arg(Arg::new("tool").required(true).help("Tool name to call"))
                    .arg(Arg::new("tool-args").long("args").help("Tool arguments as JSON"))
            )
            .subcommand(
                Command::new("serve")
                    .about("Run Gestalt as an MCP server exposing CLI tools")
            )
            .subcommand(
                Command::new("config")
                    .about("Print MCP configuration for VS Code or Claude Desktop")
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }

    async fn handle_add(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        let command = matches.get_one::<String>("command").unwrap();
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let working_dir = matches.get_one::<String>("workdir")
            .map(|s| PathBuf::from(s));

        let env = matches.get_many::<String>("env")
            .map(|vals| {
                let mut map = HashMap::new();
                for val in vals {
                    if let Some((key, value)) = val.split_once('=') {
                        map.insert(key.to_string(), value.to_string());
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
        println!("Use 'gest mcp connect {}' to test the connection", name);
        println!("Use 'gest mcp list-tools {}' to see available tools", name);
        Ok(())
    }
    
    async fn handle_list(&self) -> Result<()> {
        let config = McpConfig::load()?;
        let servers = config.list_servers();
        
        if servers.is_empty() {
            println!("No configured MCP servers");
            println!("\nAdd a server with: gest mcp add <name> <command> [args]");
            println!("Example: gest mcp add playwright npx -- --yes @modelcontextprotocol/server-playwright");
            return Ok(());
        }
        
        println!("Configured MCP servers:");
        println!("{:<20} {:<30} {}", "Name", "Command", "Args");
        println!("{}", "-".repeat(70));
        
        for server in servers {
            let args_str = if server.args.is_empty() {
                "-".to_string()
            } else {
                server.args.join(" ")
            };
            
            println!("{:<20} {:<30} {}", 
                server.name, 
                server.command,
                args_str
            );
            
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
        println!("  gest mcp connect <name>     - Test connection");
        println!("  gest mcp list-tools <name>  - List available tools");
        println!("  gest mcp call-tool <name> <tool> --args '{{}}'");
        
        Ok(())
    }
    
    async fn handle_remove(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        
        // Remove from persistent configuration
        let mut config = McpConfig::load()?;
        config.remove_server(name)?;
        
        println!("Removed MCP server configuration '{}'", name);
        Ok(())
    }

    async fn get_server_info(&self, name: &str, args: Vec<String>) -> Result<(String, Vec<String>)> {
        // Check if this is a saved configuration
        let config = McpConfig::load()?;
        if let Some(server) = config.get_server(name) {
            Ok((server.command.clone(), server.args.clone()))
        } else {
            // Treat name as the command itself
            Ok((name.to_string(), args))
        }
    }

    async fn handle_connect(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name")
            .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let (command, server_args) = self.get_server_info(name, args).await?;

        println!("Connecting to MCP server: {} {:?}", command, server_args);
        let client = McpClient::connect(&command, &server_args).await?;
        
        if let Some(info) = client.server_info() {
            println!("\nServer Info:");
            println!("  Name: {}", info.name);
            println!("  Version: {}", info.version);
            println!("  Protocol: {}", info.protocol_version);
            println!("  Capabilities:");
            let res_enabled = !info.capabilities.resources.is_null() && 
                             (info.capabilities.resources.as_bool().unwrap_or(false) || 
                              info.capabilities.resources.is_object());
            let tools_enabled = !info.capabilities.tools.is_null() && 
                               (info.capabilities.tools.as_bool().unwrap_or(false) || 
                                info.capabilities.tools.is_object());
            let prompts_enabled = !info.capabilities.prompts.is_null() && 
                                 (info.capabilities.prompts.as_bool().unwrap_or(false) || 
                                  info.capabilities.prompts.is_object());
            println!("    Resources: {}", res_enabled);
            println!("    Tools: {}", tools_enabled);
            println!("    Prompts: {}", prompts_enabled);
        }

        println!("\nServer connected successfully!");
        
        client.close().await?;
        Ok(())
    }

    async fn handle_list_resources(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name")
            .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let (command, server_args) = self.get_server_info(name, args).await?;

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

    async fn handle_list_tools(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name")
            .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let (command, server_args) = self.get_server_info(name, args).await?;

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
                println!("  Input Schema: {}", serde_json::to_string_pretty(&tool.input_schema)?);
            }
        }

        client.close().await?;
        Ok(())
    }

    async fn handle_call_tool(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name")
            .ok_or_else(|| anyhow::anyhow!("Server name or command required"))?;
        let server_args: Vec<String> = matches.get_many::<String>("server-args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();
        let tool_name = matches.get_one::<String>("tool").unwrap();
        let tool_args = matches.get_one::<String>("tool-args")
            .map(|s| serde_json::from_str(s))
            .transpose()?
            .unwrap_or(json!({}));

        let (command, args) = self.get_server_info(name, server_args).await?;

        let mut client = McpClient::connect(&command, &args).await?;
        
        println!("Calling tool '{}' with args: {}", tool_name, tool_args);
        let result = client.call_tool(tool_name, tool_args).await?;
        
        println!("\nResult:");
        println!("{}", serde_json::to_string_pretty(&result)?);

        client.close().await?;
        Ok(())
    }

    fn handle_serve(&self) -> Result<()> {
        let mut server = GestaltMcpServer::new();
        server.run()?;
        Ok(())
    }

    fn handle_config(&self) -> Result<()> {
        print_vscode_config();
        Ok(())
    }
}

impl MetaPlugin for McpPlugin {
    fn name(&self) -> &str {
        "mcp"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("mcp")
                .about("Manage MCP (Model Context Protocol) servers")
                .subcommand(
                    Command::new("add")
                        .about("Add a saved MCP server configuration")
                        .arg(Arg::new("name").required(true).help("Server name"))
                        .arg(Arg::new("command").required(true).help("Command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                        .arg(Arg::new("workdir")
                            .short('w')
                            .long("workdir")
                            .value_name("PATH")
                            .help("Working directory for the server"))
                        .arg(Arg::new("env")
                            .short('e')
                            .long("env")
                            .value_name("KEY=VALUE")
                            .num_args(0..)
                            .help("Environment variables"))
                )
                .subcommand(
                    Command::new("list")
                        .about("List saved MCP server configurations")
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a saved MCP server configuration")
                        .arg(Arg::new("name").required(true).help("Server name"))
                )
                .subcommand(
                    Command::new("connect")
                        .about("Connect to an MCP server and show its info")
                        .arg(Arg::new("name").help("Saved server name OR command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
                )
                .subcommand(
                    Command::new("list-resources")
                        .about("List resources from an MCP server")
                        .arg(Arg::new("name").help("Saved server name OR command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
                )
                .subcommand(
                    Command::new("list-tools")
                        .about("List tools from an MCP server")
                        .arg(Arg::new("name").help("Saved server name OR command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments (if using command directly)"))
                )
                .subcommand(
                    Command::new("call-tool")
                        .about("Call a tool on an MCP server")
                        .arg(Arg::new("name").help("Saved server name OR command to run"))
                        .arg(Arg::new("server-args").num_args(0..).help("Server arguments (if using command directly)"))
                        .arg(Arg::new("tool").required(true).help("Tool name to call"))
                        .arg(Arg::new("tool-args").long("args").help("Tool arguments as JSON"))
                )
                .subcommand(
                    Command::new("serve")
                        .about("Run Gestalt as an MCP server exposing CLI tools")
                )
                .subcommand(
                    Command::new("config")
                        .about("Print MCP configuration for VS Code or Claude Desktop")
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        
        runtime.block_on(async {
            match matches.subcommand() {
                Some(("add", sub_matches)) => self.handle_add(sub_matches).await,
                Some(("list", _)) => self.handle_list().await,
                Some(("remove", sub_matches)) => self.handle_remove(sub_matches).await,
                Some(("connect", sub_matches)) => self.handle_connect(sub_matches).await,
                Some(("list-resources", sub_matches)) => self.handle_list_resources(sub_matches).await,
                Some(("list-tools", sub_matches)) => self.handle_list_tools(sub_matches).await,
                Some(("call-tool", sub_matches)) => self.handle_call_tool(sub_matches).await,
                Some(("serve", _)) => self.handle_serve(),
                Some(("config", _)) => self.handle_config(),
                _ => self.show_help(),
            }
        })
    }
}

impl Default for McpPlugin {
    fn default() -> Self {
        Self::new()
    }
}