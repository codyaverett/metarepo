use anyhow::Result;
use metarepo_core::{MetaConfig, MetaPlugin, RuntimeConfig};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod lib;
use lib::ExamplePlugin;

// Protocol for communication between metarepo and external plugins
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum PluginRequest {
    GetInfo,
    RegisterCommands,
    HandleCommand {
        command: String,
        args: Vec<String>,
        config: RuntimeConfigDto,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum PluginResponse {
    Info {
        name: String,
        version: String,
        experimental: bool,
    },
    Commands {
        commands: Vec<CommandInfo>,
    },
    Success {
        message: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandInfo {
    name: String,
    about: String,
    subcommands: Vec<CommandInfo>,
    args: Vec<ArgInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArgInfo {
    name: String,
    help: String,
    required: bool,
}

// DTO for RuntimeConfig serialization
#[derive(Debug, Serialize, Deserialize)]
struct RuntimeConfigDto {
    meta_config: MetaConfig,
    working_dir: PathBuf,
    meta_file_path: Option<PathBuf>,
    experimental: bool,
}

impl From<RuntimeConfigDto> for RuntimeConfig {
    fn from(dto: RuntimeConfigDto) -> Self {
        RuntimeConfig {
            meta_config: dto.meta_config,
            working_dir: dto.working_dir,
            meta_file_path: dto.meta_file_path,
            experimental: dto.experimental,
        }
    }
}

fn main() -> Result<()> {
    // Check if running in subprocess mode (JSON-RPC over stdio)
    if std::env::var("METAREPO_PLUGIN_MODE").is_ok() {
        run_subprocess_mode()
    } else {
        // Running standalone - show usage information
        println!("Metarepo Example Plugin v0.1.0");
        println!("================================");
        println!();
        println!("This is an external plugin for the metarepo CLI tool.");
        println!();
        println!("Usage modes:");
        println!("1. Subprocess mode (called by metarepo): Set METAREPO_PLUGIN_MODE=1");
        println!("2. Dynamic library: Load the .so/.dll/.dylib file");
        println!();
        println!("To use this plugin with metarepo:");
        println!("  meta plugin add {}", std::env::current_exe()?.display());
        println!();
        println!("Available commands:");
        println!("  meta example hello <name>  - Print a greeting");
        println!("  meta example info         - Show repository information");
        println!("  meta example count        - Count projects in repository");
        Ok(())
    }
}

fn run_subprocess_mode() -> Result<()> {
    let plugin = ExamplePlugin::new();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Read requests line by line
    for line in stdin.lock().lines() {
        let line = line?;
        
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse the request
        let request: PluginRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = PluginResponse::Error {
                    message: format!("Failed to parse request: {}", e),
                };
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        };

        // Handle the request
        let response = match request {
            PluginRequest::GetInfo => PluginResponse::Info {
                name: plugin.name().to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                experimental: plugin.is_experimental(),
            },
            
            PluginRequest::RegisterCommands => {
                // Build command structure
                // Note: In a real implementation, you'd extract this from clap
                let commands = vec![CommandInfo {
                    name: "example".to_string(),
                    about: "Example plugin commands".to_string(),
                    subcommands: vec![
                        CommandInfo {
                            name: "hello".to_string(),
                            about: "Print a greeting message".to_string(),
                            subcommands: vec![],
                            args: vec![ArgInfo {
                                name: "name".to_string(),
                                help: "Name to greet".to_string(),
                                required: true,
                            }],
                        },
                        CommandInfo {
                            name: "info".to_string(),
                            about: "Display repository information".to_string(),
                            subcommands: vec![],
                            args: vec![],
                        },
                        CommandInfo {
                            name: "count".to_string(),
                            about: "Count projects in repository".to_string(),
                            subcommands: vec![],
                            args: vec![],
                        },
                    ],
                    args: vec![],
                }];
                
                PluginResponse::Commands { commands }
            }
            
            PluginRequest::HandleCommand {
                command,
                args,
                config,
            } => {
                // Convert DTO to RuntimeConfig
                let runtime_config: RuntimeConfig = config.into();
                
                // Create a minimal ArgMatches-like structure
                // In a real implementation, you'd properly parse with clap
                let app = clap::Command::new("plugin");
                let app = plugin.register_commands(app);
                
                // Build the full argument list
                let mut full_args = vec!["plugin".to_string(), command];
                full_args.extend(args);
                
                // Parse and handle
                match app.try_get_matches_from(&full_args) {
                    Ok(matches) => {
                        if let Some((cmd, sub_matches)) = matches.subcommand() {
                            match plugin.handle_command(sub_matches, &runtime_config) {
                                Ok(_) => PluginResponse::Success { message: None },
                                Err(e) => PluginResponse::Error {
                                    message: e.to_string(),
                                },
                            }
                        } else {
                            PluginResponse::Error {
                                message: "No subcommand provided".to_string(),
                            }
                        }
                    }
                    Err(e) => PluginResponse::Error {
                        message: format!("Failed to parse arguments: {}", e),
                    },
                }
            }
        };

        // Send the response
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_request_serialization() {
        let request = PluginRequest::GetInfo;
        let json = serde_json::to_string(&request).unwrap();
        let parsed: PluginRequest = serde_json::from_str(&json).unwrap();
        matches!(parsed, PluginRequest::GetInfo);
    }

    #[test]
    fn test_plugin_response_serialization() {
        let response = PluginResponse::Info {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            experimental: false,
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: PluginResponse = serde_json::from_str(&json).unwrap();
        matches!(parsed, PluginResponse::Info { .. });
    }
}