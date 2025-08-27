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
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub resources: bool,
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub prompts: bool,
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
            "protocolVersion": "0.1.0",
            "capabilities": {
                "roots": {
                    "listChanged": true
                },
                "sampling": {}
            },
            "clientInfo": {
                "name": "gestalt-mcp-client",
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

        // Read response
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        
        let response: JsonRpcResponse = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse response: {}", line))?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("RPC error: {} - {}", error.code, error.message));
        }

        Ok(response)
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
            let resources: Vec<Resource> = serde_json::from_value(
                result.get("resources").unwrap_or(&json!([])).clone()
            )?;
            Ok(resources)
        } else {
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
        let response = self.send_request("tools/list", json!({})).await?;
        
        if let Some(result) = response.result {
            let tools: Vec<Tool> = serde_json::from_value(
                result.get("tools").unwrap_or(&json!([])).clone()
            )?;
            Ok(tools)
        } else {
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