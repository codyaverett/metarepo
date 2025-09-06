use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::process::Command;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerInfo {
    name: String,
    version: String,
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerCapabilities {
    tools: Option<Value>,
    resources: Option<Value>,
    prompts: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct Resource {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
}

pub struct MetarepoMcpServer {
    tools: Vec<Tool>,
    resources: Vec<Resource>,
    metarepo_path: PathBuf,
}

impl MetarepoMcpServer {
    pub fn new() -> Self {
        let metarepo_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("meta"));
        
        Self {
            tools: Self::build_tools(),
            resources: Vec::new(),
            metarepo_path,
        }
    }

    fn build_tools() -> Vec<Tool> {
        vec![
            // Help tool
            Tool {
                name: "help".to_string(),
                description: "Get help and list available commands".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "plugin": {
                            "type": "string",
                            "description": "Specific plugin to get help for (git, project, exec, mcp)"
                        }
                    }
                }),
            },
            // Git plugin tools
            Tool {
                name: "git_status".to_string(),
                description: "Show git status for all repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "verbose": {
                            "type": "boolean",
                            "description": "Show verbose output"
                        }
                    }
                }),
            },
            Tool {
                name: "git_diff".to_string(),
                description: "Show git diff across repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "staged": {
                            "type": "boolean",
                            "description": "Show staged changes"
                        }
                    }
                }),
            },
            Tool {
                name: "git_commit".to_string(),
                description: "Commit changes across repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["message"],
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Commit message"
                        },
                        "all": {
                            "type": "boolean",
                            "description": "Stage all changes before committing"
                        }
                    }
                }),
            },
            Tool {
                name: "git_pull".to_string(),
                description: "Pull changes from remote repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "git_push".to_string(),
                description: "Push changes to remote repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            
            // Project plugin tools
            Tool {
                name: "project_list".to_string(),
                description: "List all projects in the workspace".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "project_add".to_string(),
                description: "Add a new project to the workspace".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the project directory"
                        },
                        "name": {
                            "type": "string",
                            "description": "Optional project name"
                        }
                    }
                }),
            },
            Tool {
                name: "project_remove".to_string(),
                description: "Remove a project from the workspace".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Project name to remove"
                        }
                    }
                }),
            },
            
            // Exec plugin tools
            Tool {
                name: "exec".to_string(),
                description: "Execute a command across multiple projects".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["command"],
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Command to execute"
                        },
                        "projects": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Specific projects to run in (all if not specified)"
                        }
                    }
                }),
            },
            
            // MCP plugin tools
            Tool {
                name: "mcp_add_server".to_string(),
                description: "Add an MCP server configuration".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["name", "command"],
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Server name"
                        },
                        "command": {
                            "type": "string",
                            "description": "Command to run"
                        },
                        "args": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Command arguments"
                        }
                    }
                }),
            },
            Tool {
                name: "mcp_list_servers".to_string(),
                description: "List configured MCP servers".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "mcp_remove_server".to_string(),
                description: "Remove an MCP server configuration".to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Server name"
                        }
                    }
                }),
            },
        ]
    }

    pub fn run(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        eprintln!("Metarepo MCP Server started. Listening for JSON-RPC requests...");

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse request: {}", line))?;

            // Only send response if the request has an ID (not a notification)
            if request.id.is_some() {
                let response = self.handle_request(request);
                let response_str = serde_json::to_string(&response)?;
                
                writeln!(stdout, "{}", response_str)?;
                stdout.flush()?;
            } else {
                // It's a notification, just handle it without responding
                self.handle_request(request);
            }
        }

        eprintln!("Metarepo MCP Server shutting down (stdin closed)");
        Ok(())
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone().unwrap_or(json!(null));
        
        // Handle notifications (no response needed for notifications without id)
        if request.id.is_none() && request.method == "notifications/initialized" {
            // Return a dummy response that won't be sent
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: json!(null),
                result: Some(json!({})),
                error: None,
            };
        }
        
        match request.method.as_str() {
            "initialize" => self.handle_initialize(id, request.params),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tool_call(id, request.params),
            "resources/list" => self.handle_resources_list(id),
            "prompts/list" => self.handle_prompts_list(id),
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            },
        }
    }

    fn handle_initialize(&self, id: Value, _params: Option<Value>) -> JsonRpcResponse {
        let server_info = ServerInfo {
            name: "metarepo-mcp-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2025-06-18".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(json!({})),
                resources: None,
                prompts: None,
            },
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(server_info).unwrap()),
            error: None,
        }
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": self.tools
            })),
            error: None,
        }
    }

    fn handle_tool_call(&self, id: Value, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                }
            }
        };

        let tool_name = params.get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        
        let arguments = params.get("arguments")
            .cloned()
            .unwrap_or(json!({}));

        match self.execute_tool(tool_name, arguments) {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": result
                    }]
                })),
                error: None,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Tool execution failed: {}", e),
                    data: None,
                }),
            },
        }
    }

    fn handle_resources_list(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "resources": self.resources
            })),
            error: None,
        }
    }

    fn handle_prompts_list(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "prompts": []
            })),
            error: None,
        }
    }

    fn execute_tool(&self, name: &str, arguments: Value) -> Result<String> {
        let mut cmd = Command::new(&self.metarepo_path);
        
        match name {
            "help" => {
                if let Some(plugin) = arguments.get("plugin").and_then(|v| v.as_str()) {
                    cmd.args(&[plugin, "--help"]);
                } else {
                    cmd.arg("--help");
                }
            }
            "git_status" => {
                cmd.args(&["git", "status"]);
                if arguments.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false) {
                    cmd.arg("--verbose");
                }
            }
            "git_diff" => {
                cmd.args(&["git", "diff"]);
                if arguments.get("staged").and_then(|v| v.as_bool()).unwrap_or(false) {
                    cmd.arg("--staged");
                }
            }
            "git_commit" => {
                cmd.args(&["git", "commit"]);
                if let Some(message) = arguments.get("message").and_then(|v| v.as_str()) {
                    cmd.args(&["-m", message]);
                }
                if arguments.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
                    cmd.arg("-a");
                }
            }
            "git_pull" => {
                cmd.args(&["git", "pull"]);
            }
            "git_push" => {
                cmd.args(&["git", "push"]);
            }
            "project_list" => {
                cmd.args(&["project", "list"]);
            }
            "project_add" => {
                cmd.args(&["project", "add"]);
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    cmd.arg(path);
                }
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.args(&["--name", name]);
                }
            }
            "project_remove" => {
                cmd.args(&["project", "remove"]);
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.arg(name);
                }
            }
            "exec" => {
                cmd.arg("exec");
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    cmd.arg(command);
                }
                if let Some(projects) = arguments.get("projects").and_then(|v| v.as_array()) {
                    let project_list: Vec<String> = projects.iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect();
                    if !project_list.is_empty() {
                        cmd.arg("--projects");
                        cmd.arg(project_list.join(","));
                    }
                }
            }
            "mcp_add_server" => {
                cmd.args(&["mcp", "add"]);
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.arg(name);
                }
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    cmd.arg(command);
                }
                if let Some(args) = arguments.get("args").and_then(|v| v.as_array()) {
                    for arg in args {
                        if let Some(arg_str) = arg.as_str() {
                            cmd.arg(arg_str);
                        }
                    }
                }
            }
            "mcp_list_servers" => {
                cmd.args(&["mcp", "list"]);
            }
            "mcp_remove_server" => {
                cmd.args(&["mcp", "remove"]);
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.arg(name);
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }

        let output = cmd.output()
            .with_context(|| format!("Failed to execute tool: {}", name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        let result = if !stdout.is_empty() {
            stdout.to_string()
        } else if !stderr.is_empty() {
            stderr.to_string()
        } else {
            format!("Command executed successfully: {}", name)
        };

        Ok(result)
    }
}

pub fn print_vscode_config() {
    let meta_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("meta"))
        .to_string_lossy()
        .to_string();
    
    let claude_config = json!({
        "mcpServers": {
            "metarepo": {
                "command": meta_path,
                "args": ["mcp", "serve"],
                "env": {}
            }
        }
    });

    println!("=== Claude Desktop Configuration ===");
    println!();
    println!("Add this to your Claude Desktop config file:");
    println!("  macOS: ~/Library/Application Support/Claude/claude_desktop_config.json");
    println!("  Windows: %APPDATA%\\Claude\\claude_desktop_config.json");
    println!();
    println!("{}", serde_json::to_string_pretty(&claude_config).unwrap());
    println!();
    println!("=== Available Tools ===");
    println!();
    println!("The Metarepo MCP server exposes 13 tools:");
    println!("  • help - Get help and list available commands");
    println!("  • git_status, git_diff, git_commit, git_pull, git_push");
    println!("  • project_list, project_add, project_remove");
    println!("  • exec - Execute commands across projects");
    println!("  • mcp_add_server, mcp_list_servers, mcp_remove_server");
    println!();
    println!("=== Testing ===");
    println!();
    println!("Test the server directly:");
    println!("  meta mcp serve");
    println!();
    println!("Then send JSON-RPC commands via stdin");
}