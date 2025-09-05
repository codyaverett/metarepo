use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatus {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub last_output: Option<String>,
}

pub struct McpServerInstance {
    pub config: McpServerConfig,
    pub process: Option<Child>,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub start_time: Option<std::time::Instant>,
    pub output_buffer: Arc<Mutex<Vec<String>>>,
}

impl McpServerInstance {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            start_time: None,
            output_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Err(anyhow::anyhow!(
                "Server '{}' is already running",
                self.config.name
            ));
        }

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        if let Some(ref env_vars) = self.config.env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start MCP server '{}'", self.config.name))?;

        // Keep stdin to send initialization and keep the server alive
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin for MCP server"))?;

        // Send initialization request to the MCP server
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "0.1.0",
                "capabilities": {},
                "clientInfo": {
                    "name": "gestalt-mcp",
                    "version": "0.1.0"
                }
            }
        });

        let init_str = format!("{}\n", init_request.to_string());
        stdin
            .write_all(init_str.as_bytes())
            .await
            .with_context(|| "Failed to send initialization to MCP server")?;
        stdin.flush().await?;

        // Store stdin to keep the connection alive
        self.stdin = Some(stdin);

        let output_buffer = Arc::clone(&self.output_buffer);

        if let Some(stdout) = child.stdout.take() {
            let buffer_clone = Arc::clone(&output_buffer);
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buffer = buffer_clone.lock().unwrap();
                    buffer.push(format!("[stdout] {}", line));
                    if buffer.len() > 1000 {
                        buffer.drain(0..500);
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buffer = output_buffer.lock().unwrap();
                    buffer.push(format!("[stderr] {}", line));
                    if buffer.len() > 1000 {
                        buffer.drain(0..500);
                    }
                }
            });
        }

        self.process = Some(child);
        self.start_time = Some(std::time::Instant::now());

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        // Drop stdin first to close the connection gracefully
        self.stdin = None;

        if let Some(mut child) = self.process.take() {
            // Give the process a moment to exit gracefully
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Then kill if still running
            let _ = child.kill().await;
            self.start_time = None;
            self.output_buffer.lock().unwrap().clear();
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Server '{}' is not running",
                self.config.name
            ))
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some(ref child) = self.process {
            match child.id() {
                Some(_) => true,
                None => false,
            }
        } else {
            false
        }
    }

    pub fn get_status(&self) -> McpServerStatus {
        let pid = self.process.as_ref().and_then(|p| p.id());
        let uptime = self.start_time.map(|t| t.elapsed().as_secs());
        let last_output = self.output_buffer.lock().unwrap().last().cloned();

        McpServerStatus {
            name: self.config.name.clone(),
            running: self.is_running(),
            pid,
            uptime_seconds: uptime,
            last_output,
        }
    }

    pub fn get_output(&self, lines: usize) -> Vec<String> {
        let buffer = self.output_buffer.lock().unwrap();
        let start = if buffer.len() > lines {
            buffer.len() - lines
        } else {
            0
        };
        buffer[start..].to_vec()
    }
}

pub struct McpServerManager {
    servers: HashMap<String, McpServerInstance>,
}

impl McpServerManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn add_server(&mut self, config: McpServerConfig) -> Result<()> {
        if self.servers.contains_key(&config.name) {
            return Err(anyhow::anyhow!("Server '{}' already exists", config.name));
        }
        let name = config.name.clone();
        self.servers.insert(name, McpServerInstance::new(config));
        Ok(())
    }

    pub async fn start_server(&mut self, name: &str) -> Result<()> {
        let server = self
            .servers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", name))?;
        server.start().await
    }

    pub async fn stop_server(&mut self, name: &str) -> Result<()> {
        let server = self
            .servers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", name))?;
        server.stop().await
    }

    pub async fn restart_server(&mut self, name: &str) -> Result<()> {
        if let Ok(server) = self
            .servers
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", name))
        {
            if server.is_running() {
                server.stop().await?;
            }
            server.start().await?;
        }
        Ok(())
    }

    pub fn get_status(&self, name: Option<&str>) -> Vec<McpServerStatus> {
        match name {
            Some(n) => self
                .servers
                .get(n)
                .map(|s| vec![s.get_status()])
                .unwrap_or_default(),
            None => self.servers.values().map(|s| s.get_status()).collect(),
        }
    }

    pub fn get_server_output(&self, name: &str, lines: usize) -> Result<Vec<String>> {
        let server = self
            .servers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", name))?;
        Ok(server.get_output(lines))
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.stop_server(&name).await {
                eprintln!("Failed to stop server '{}': {}", name, e);
            }
        }
        Ok(())
    }
}
