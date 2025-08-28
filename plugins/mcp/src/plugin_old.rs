use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::server::{McpServerManager, McpServerConfig};
use crate::client::McpClient;
use crate::mcp_server::{GestaltMcpServer, print_vscode_config};
use crate::config::McpConfig;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use serde_json::json;

pub struct McpPlugin {
    manager: Arc<Mutex<McpServerManager>>,
}

impl McpPlugin {
    pub fn new() -> Self {
        let manager = Arc::new(Mutex::new(McpServerManager::new()));
        
        // Load existing configurations on startup
        if let Ok(config) = McpConfig::load() {
            let mut mgr = manager.blocking_lock();
            for server_config in config.list_servers() {
                // Silently ignore errors for servers that may already exist
                let _ = mgr.add_server(server_config.clone());
            }
        }
        
        Self { manager }
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest mcp")
            .about("Manage MCP (Model Context Protocol) servers")
            .subcommand(
                Command::new("start")
                    .about("Start an MCP server")
                    .arg(Arg::new("name").required(true).help("Server name"))
                    .arg(Arg::new("command").help("Command to run (optional if server is already configured)"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
            )
            .subcommand(
                Command::new("stop")
                    .about("Stop an MCP server")
                    .arg(Arg::new("name").required(true).help("Server name"))
            )
            .subcommand(
                Command::new("restart")
                    .about("Restart an MCP server")
                    .arg(Arg::new("name").required(true).help("Server name"))
            )
            .subcommand(
                Command::new("status")
                    .about("Show status of MCP servers")
                    .arg(Arg::new("name").help("Server name (shows all if not specified)"))
            )
            .subcommand(
                Command::new("logs")
                    .about("Show recent output from an MCP server")
                    .arg(Arg::new("name").required(true).help("Server name"))
                    .arg(Arg::new("lines")
                        .short('n')
                        .long("lines")
                        .default_value("50")
                        .help("Number of lines to show"))
            )
            .subcommand(
                Command::new("connect")
                    .about("Connect to an MCP server and interact with it")
                    .arg(Arg::new("command").required(true).help("Command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
            )
            .subcommand(
                Command::new("list-resources")
                    .about("List resources from an MCP server")
                    .arg(Arg::new("command").required(true).help("Command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
            )
            .subcommand(
                Command::new("list-tools")
                    .about("List tools from an MCP server")
                    .arg(Arg::new("command").required(true).help("Command to run"))
                    .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
            )
            .subcommand(
                Command::new("call-tool")
                    .about("Call a tool on an MCP server")
                    .arg(Arg::new("command").required(true).help("Command to run"))
                    .arg(Arg::new("server-args").num_args(0..).help("Arguments for the server"))
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
            .subcommand(
                Command::new("list")
                    .about("List configured MCP servers")
            )
            .subcommand(
                Command::new("remove")
                    .about("Remove a configured MCP server")
                    .arg(Arg::new("name").required(true).help("Server name"))
            )
            .subcommand(
                Command::new("add")
                    .about("Add a new MCP server configuration")
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
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }

    async fn handle_start(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        let command = matches.get_one::<String>("command");
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let mut manager = self.manager.lock().await;
        
        // Check if server already exists in manager
        if let Err(_) = manager.start_server(name).await {
            // Server not in manager, need to add it
            let config = if let Some(cmd) = command {
                // If command is provided, create a new config (temporary, not saved)
                McpServerConfig {
                    name: name.clone(),
                    command: cmd.clone(),
                    args,
                    working_dir: None,
                    env: None,
                }
            } else {
                // Try to load from saved configuration
                let saved_config = McpConfig::load()?;
                saved_config.get_server(name)
                    .ok_or_else(|| anyhow::anyhow!(
                        "Server '{}' not found in configuration. Use 'gest mcp add' to add it first, or provide a command directly.",
                        name
                    ))?
                    .clone()
            };
            
            manager.add_server(config)?;
            manager.start_server(name).await?;
        }
        
        println!("Started MCP server '{}'", name);
        Ok(())
    }

    async fn handle_stop(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        let mut manager = self.manager.lock().await;
        manager.stop_server(name).await?;
        println!("Stopped MCP server '{}'", name);
        Ok(())
    }

    async fn handle_restart(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        let mut manager = self.manager.lock().await;
        manager.restart_server(name).await?;
        println!("Restarted MCP server '{}'", name);
        Ok(())
    }

    async fn handle_status(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").map(|s| s.as_str());
        let manager = self.manager.lock().await;
        let statuses = manager.get_status(name);

        if statuses.is_empty() {
            println!("No MCP servers found");
            return Ok(());
        }

        println!("{:<20} {:<10} {:<10} {:<15}", "Name", "Status", "PID", "Uptime");
        println!("{}", "-".repeat(60));

        for status in statuses {
            let status_str = if status.running { "Running" } else { "Stopped" };
            let pid_str = status.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            let uptime_str = status.uptime_seconds
                .map(|s| format!("{}s", s))
                .unwrap_or_else(|| "-".to_string());

            println!("{:<20} {:<10} {:<10} {:<15}", 
                status.name, 
                status_str, 
                pid_str, 
                uptime_str
            );

            if let Some(output) = status.last_output {
                println!("  Last output: {}", output);
            }
        }

        Ok(())
    }

    async fn handle_logs(&self, matches: &ArgMatches) -> Result<()> {
        let name = matches.get_one::<String>("name").unwrap();
        let lines = matches.get_one::<String>("lines")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50);

        let manager = self.manager.lock().await;
        let output = manager.get_server_output(name, lines)?;

        if output.is_empty() {
            println!("No output available for server '{}'", name);
        } else {
            println!("Recent output from '{}' (last {} lines):", name, lines);
            println!("{}", "-".repeat(60));
            for line in output {
                println!("{}", line);
            }
        }

        Ok(())
    }

    async fn handle_connect(&self, matches: &ArgMatches) -> Result<()> {
        let command = matches.get_one::<String>("command").unwrap();
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        println!("Connecting to MCP server: {} {:?}", command, args);
        let client = McpClient::connect(command, &args).await?;
        
        if let Some(info) = client.server_info() {
            println!("\nServer Info:");
            println!("  Name: {}", info.name);
            println!("  Version: {}", info.version);
            println!("  Protocol: {}", info.protocol_version);
            println!("  Capabilities:");
            println!("    Resources: {}", info.capabilities.resources);
            println!("    Tools: {}", info.capabilities.tools);
            println!("    Prompts: {}", info.capabilities.prompts);
        }

        println!("\nServer connected successfully!");
        println!("Use 'gest mcp list-resources' or 'gest mcp list-tools' to explore capabilities");
        
        client.close().await?;
        Ok(())
    }

    async fn handle_list_resources(&self, matches: &ArgMatches) -> Result<()> {
        let command = matches.get_one::<String>("command").unwrap();
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let mut client = McpClient::connect(command, &args).await?;
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
        let command = matches.get_one::<String>("command").unwrap();
        let args: Vec<String> = matches.get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();

        let mut client = McpClient::connect(command, &args).await?;
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
        let command = matches.get_one::<String>("command").unwrap();
        let args: Vec<String> = matches.get_many::<String>("server-args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();
        let tool_name = matches.get_one::<String>("tool").unwrap();
        let tool_args = matches.get_one::<String>("tool-args")
            .map(|s| serde_json::from_str(s))
            .transpose()?
            .unwrap_or(json!({}));

        let mut client = McpClient::connect(command, &args).await?;
        
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
        saved_config.add_server(config.clone())?;
        
        // Also add to in-memory manager for immediate use
        let mut manager = self.manager.lock().await;
        manager.add_server(config)?;
        
        println!("Added MCP server configuration '{}'", name);
        println!("Use 'gest mcp start {}' to start the server", name);
        Ok(())
    }
    
    async fn handle_list(&self) -> Result<()> {
        let config = McpConfig::load()?;
        let servers = config.list_servers();
        
        if servers.is_empty() {
            println!("No configured MCP servers");
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
                    Command::new("start")
                        .about("Start an MCP server")
                        .arg(Arg::new("name").required(true).help("Server name"))
                        .arg(Arg::new("command").help("Command to run (optional if server is already configured)"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                )
                .subcommand(
                    Command::new("stop")
                        .about("Stop an MCP server")
                        .arg(Arg::new("name").required(true).help("Server name"))
                )
                .subcommand(
                    Command::new("restart")
                        .about("Restart an MCP server")
                        .arg(Arg::new("name").required(true).help("Server name"))
                )
                .subcommand(
                    Command::new("status")
                        .about("Show status of MCP servers")
                        .arg(Arg::new("name").help("Server name (shows all if not specified)"))
                )
                .subcommand(
                    Command::new("logs")
                        .about("Show recent output from an MCP server")
                        .arg(Arg::new("name").required(true).help("Server name"))
                        .arg(Arg::new("lines")
                            .short('n')
                            .long("lines")
                            .default_value("50")
                            .help("Number of lines to show"))
                )
                .subcommand(
                    Command::new("connect")
                        .about("Connect to an MCP server and interact with it")
                        .arg(Arg::new("command").required(true).help("Command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                )
                .subcommand(
                    Command::new("list-resources")
                        .about("List resources from an MCP server")
                        .arg(Arg::new("command").required(true).help("Command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                )
                .subcommand(
                    Command::new("list-tools")
                        .about("List tools from an MCP server")
                        .arg(Arg::new("command").required(true).help("Command to run"))
                        .arg(Arg::new("args").num_args(0..).help("Arguments for the command"))
                )
                .subcommand(
                    Command::new("call-tool")
                        .about("Call a tool on an MCP server")
                        .arg(Arg::new("command").required(true).help("Command to run"))
                        .arg(Arg::new("server-args").num_args(0..).help("Arguments for the server"))
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
                .subcommand(
                    Command::new("list")
                        .about("List configured MCP servers")
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a configured MCP server")
                        .arg(Arg::new("name").required(true).help("Server name"))
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a new MCP server configuration")
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
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        
        runtime.block_on(async {
            match matches.subcommand() {
                Some(("start", sub_matches)) => self.handle_start(sub_matches).await,
                Some(("stop", sub_matches)) => self.handle_stop(sub_matches).await,
                Some(("restart", sub_matches)) => self.handle_restart(sub_matches).await,
                Some(("status", sub_matches)) => self.handle_status(sub_matches).await,
                Some(("logs", sub_matches)) => self.handle_logs(sub_matches).await,
                Some(("add", sub_matches)) => self.handle_add(sub_matches).await,
                Some(("list", _)) => self.handle_list().await,
                Some(("remove", sub_matches)) => self.handle_remove(sub_matches).await,
                Some(("connect", sub_matches)) => self.handle_connect(sub_matches).await,
                Some(("list-resources", sub_matches)) => self.handle_list_resources(sub_matches).await,
                Some(("list-tools", sub_matches)) => self.handle_list_tools(sub_matches).await,
                Some(("call-tool", sub_matches)) => self.handle_call_tool(sub_matches).await,
                Some(("serve", _)) => return self.handle_serve(),
                Some(("config", _)) => return self.handle_config(),
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