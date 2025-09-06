use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub resources: serde_json::Value,  // Can be bool or object
    #[serde(default)]
    pub tools: serde_json::Value,      // Can be bool or object
    #[serde(default)]
    pub prompts: serde_json::Value,    // Can be bool or object
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

pub struct McpClient {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    server_info: Option<ServerInfo>,
}

impl McpClient {
    pub async fn connect(command: &str, args: &[String]) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut process = cmd.spawn()
            .with_context(|| format!("Failed to spawn MCP server: {}", command))?;

        let stdin = process.stdin.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
        let stdout = process.stdout.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
        let stdout = BufReader::new(stdout);

        let mut client = Self {
            process,
            stdin,
            stdout,
            server_info: None,
        };

        // Initialize connection
        client.initialize().await?;

        Ok(client)
    }

    async fn initialize(&mut self) -> Result<()> {
        let response = self.send_request("initialize", json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {
                "roots": {
                    "listChanged": true
                },
                "sampling": {}
            },
            "clientInfo": {
                "name": "metarepo-mcp-client",
                "version": "0.1.0"
            }
        })).await?;

        if let Some(result) = response.result {
            self.server_info = Some(serde_json::from_value(result)?);
        }

        // Send initialized notification
        self.send_notification("notifications/initialized", json!({})).await?;

        Ok(())
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<JsonRpcResponse> {
        let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let request_str = serde_json::to_string(&request)?;
        self.stdin.write_all(request_str.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        // Keep reading until we get our response (handle server requests in between)
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).await?;
            
            // eprintln!("DEBUG: Received from server while waiting for {} response: {}", method, line.trim());
            
            // Try to parse as a generic JSON value first
            let json_value: Value = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse JSON: {}", line))?;
            
            // Check if this is a request from the server (has method but no result/error)
            if json_value.get("method").is_some() && 
               json_value.get("result").is_none() && 
               json_value.get("error").is_none() {
                // This is a request from the server, handle it
                // eprintln!("DEBUG: Received server request: {}", json_value.get("method").unwrap());
                
                // Handle roots/list request
                if json_value.get("method") == Some(&json!("roots/list")) {
                    let server_id = json_value.get("id").cloned().unwrap_or(json!(null));
                    let roots_response = json!({
                        "jsonrpc": "2.0",
                        "id": server_id,
                        "result": {
                            "roots": []
                        }
                    });
                    // eprintln!("DEBUG: Sending roots/list response: {}", serde_json::to_string(&roots_response)?);
                    let response_str = serde_json::to_string(&roots_response)?;
                    self.stdin.write_all(response_str.as_bytes()).await?;
                    self.stdin.write_all(b"\n").await?;
                    self.stdin.flush().await?;
                    // eprintln!("DEBUG: Sent roots/list response");
                }
                
                // Continue waiting for our actual response
                continue;
            }
            
            // This should be a response
            let response: JsonRpcResponse = serde_json::from_value(json_value)
                .with_context(|| format!("Failed to parse response: {}", line))?;
            
            // Check if this is the response to our request
            if response.id == id {
                return Ok(response);
            }
            
            // If not our response, continue waiting
            // eprintln!("DEBUG: Response ID {} doesn't match our request ID {}, continuing...", response.id, id);
        }
    }

    async fn send_notification(&mut self, method: &str, params: Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let notification_str = serde_json::to_string(&notification)?;
        self.stdin.write_all(notification_str.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        Ok(())
    }

    pub async fn list_resources(&mut self) -> Result<Vec<Resource>> {
        let response = self.send_request("resources/list", json!({})).await?;
        
        if let Some(result) = response.result {
            // Debug: Print the raw result
            // eprintln!("DEBUG: Raw resources/list result: {}", serde_json::to_string_pretty(&result).unwrap_or_default());
            
            // The result might be an array directly, or wrapped in a "resources" field
            let resources_value = if result.is_array() {
                result
            } else {
                result.get("resources").unwrap_or(&json!([])).clone()
            };
            
            let resources: Vec<Resource> = serde_json::from_value(resources_value)?;
            Ok(resources)
        } else {
            // eprintln!("DEBUG: No result from resources/list");
            Ok(vec![])
        }
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<Value> {
        let response = self.send_request("resources/read", json!({
            "uri": uri
        })).await?;
        
        response.result
            .ok_or_else(|| anyhow::anyhow!("No result from resource read"))
    }

    pub async fn list_tools(&mut self) -> Result<Vec<Tool>> {
        // eprintln!("DEBUG: Sending tools/list request");
        let response = self.send_request("tools/list", json!({})).await?;
        
        // Check for error first
        if let Some(error) = response.error {
            // eprintln!("DEBUG: Error from tools/list: {:?}", error);
            return Err(anyhow::anyhow!("MCP error: {}", error.message));
        }
        
        if let Some(result) = response.result {
            // Debug: Print the raw result
            // eprintln!("DEBUG: Raw tools/list result: {}", serde_json::to_string_pretty(&result).unwrap_or_default());
            
            // The result might be an array directly, or wrapped in a "tools" field
            let tools_value = if result.is_array() {
                result
            } else {
                result.get("tools").unwrap_or(&json!([])).clone()
            };
            
            let tools: Vec<Tool> = serde_json::from_value(tools_value)?;
            Ok(tools)
        } else {
            // eprintln!("DEBUG: No result from tools/list");
            Ok(vec![])
        }
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let response = self.send_request("tools/call", json!({
            "name": name,
            "arguments": arguments
        })).await?;
        
        response.result
            .ok_or_else(|| anyhow::anyhow!("No result from tool call"))
    }

    pub async fn list_prompts(&mut self) -> Result<Vec<Prompt>> {
        let response = self.send_request("prompts/list", json!({})).await?;
        
        if let Some(result) = response.result {
            let prompts: Vec<Prompt> = serde_json::from_value(
                result.get("prompts").unwrap_or(&json!([])).clone()
            )?;
            Ok(prompts)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_prompt(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let response = self.send_request("prompts/get", json!({
            "name": name,
            "arguments": arguments
        })).await?;
        
        response.result
            .ok_or_else(|| anyhow::anyhow!("No result from prompt get"))
    }

    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    pub async fn close(mut self) -> Result<()> {
        self.send_notification("notifications/cancelled", json!({})).await.ok();
        self.process.kill().await?;
        Ok(())
    }
}