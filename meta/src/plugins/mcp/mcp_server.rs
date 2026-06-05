use super::client::McpClient;
use super::config::McpConfig;
use super::server::McpServerConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// How long to wait for a downstream MCP server to connect/respond.
const GATEWAY_TIMEOUT: Duration = Duration::from_secs(30);

/// Gateway meta-tools fronting the saved downstream MCP servers. They keep the
/// top-level tool surface small (progressive disclosure): browse the catalog,
/// open one server's tools on demand, search across all, then proxy a call.
const GATEWAY_TOOLS: [&str; 7] = [
    "mcp_catalog",
    "mcp_list_tools",
    "mcp_search_tools",
    "mcp_call",
    "mcp_workspaces",
    "mcp_enable",
    "mcp_disable",
];

/// Write capability a tool needs, used to gate it against the serve policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    /// Read-only inspection (always allowed).
    Read,
    /// Mutates repos or workspace config.
    Write,
    /// The arbitrary-shell `exec` tool (gated separately).
    Exec,
}

/// Classify a tool name by the capability it needs.
fn tool_kind(name: &str) -> ToolKind {
    match name {
        "help" | "git_status" | "git_diff" | "project_list" | "mcp_list_servers"
        | "mcp_catalog" | "mcp_list_tools" | "mcp_search_tools" | "mcp_workspaces" => {
            ToolKind::Read
        }
        "exec" => ToolKind::Exec,
        // mcp_call proxies a downstream tool that could mutate state, so it is a
        // write (blocked in read-only, allowed in read-write/full).
        _ => ToolKind::Write,
    }
}

/// How permissive the served workspace is. Mirrors `mcp.serve.mode` in `.meta`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeMode {
    /// Everything, including `exec` (subject to `allow_exec`). Default.
    Full,
    /// Reads and writes, but never `exec`.
    ReadWrite,
    /// Reads only.
    ReadOnly,
}

/// Resolved permission policy for `meta mcp serve`. Defaults are full access so
/// existing setups are unchanged.
#[derive(Debug, Clone)]
pub struct ServePolicy {
    pub mode: ServeMode,
    pub allow_exec: bool,
    pub tools: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
}

impl Default for ServePolicy {
    fn default() -> Self {
        Self {
            mode: ServeMode::Full,
            allow_exec: true,
            tools: None,
            projects: None,
        }
    }
}

impl ServePolicy {
    /// Build a policy from a workspace's `[mcp.serve]` block (if any).
    pub fn from_settings(settings: Option<&metarepo_core::McpServeSettings>) -> Self {
        let mut policy = ServePolicy::default();
        if let Some(s) = settings {
            if let Some(mode) = s.mode.as_deref() {
                policy.mode = match mode.to_ascii_lowercase().as_str() {
                    "read-only" | "readonly" => ServeMode::ReadOnly,
                    "read-write" | "readwrite" => ServeMode::ReadWrite,
                    _ => ServeMode::Full,
                };
            }
            if let Some(allow) = s.allow_exec {
                policy.allow_exec = allow;
            }
            policy.tools = s.tools.clone();
            policy.projects = s.projects.clone();
        }
        policy
    }

    /// Restrict an `exec` call to the policy's project allowlist. Returns the
    /// effective project list to pass via `--projects`, or an error if the call
    /// requested a project outside the allowlist. `None` means no restriction.
    fn exec_projects(&self, requested: &[String]) -> Result<Option<Vec<String>>> {
        let Some(allowed) = &self.projects else {
            return Ok(None);
        };
        if requested.is_empty() {
            return Ok(Some(allowed.clone()));
        }
        if let Some(bad) = requested.iter().find(|p| !allowed.contains(p)) {
            return Err(anyhow::anyhow!(
                "Project '{}' is outside this server's allowed projects: {}",
                bad,
                allowed.join(", ")
            ));
        }
        Ok(Some(requested.to_vec()))
    }

    /// Whether a tool may be listed and called under this policy.
    fn allows(&self, name: &str) -> bool {
        if let Some(list) = &self.tools {
            if !list.iter().any(|t| t == name) {
                return false;
            }
        }
        match tool_kind(name) {
            ToolKind::Read => true,
            ToolKind::Write => self.mode != ServeMode::ReadOnly,
            ToolKind::Exec => self.mode == ServeMode::Full && self.allow_exec,
        }
    }

    /// One-line human summary for the `initialize` instructions.
    fn summary(&self) -> String {
        let mode = match self.mode {
            ServeMode::Full => "full",
            ServeMode::ReadWrite => "read-write",
            ServeMode::ReadOnly => "read-only",
        };
        let exec = if self.mode == ServeMode::Full && self.allow_exec {
            "exec enabled"
        } else {
            "exec disabled"
        };
        match &self.tools {
            Some(list) => format!("mode={mode}, {exec}, tools=[{}]", list.join(", ")),
            None => format!("mode={mode}, {exec}"),
        }
    }
}

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
    /// MCP `instructions`: surfaced to the client so the model knows which
    /// workspace this server is pinned to and what its policy allows.
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
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

/// One workspace this server may operate on: the `.meta` to inject as `--config`,
/// the directory to run tool subprocesses in, and that workspace's serve policy.
#[derive(Debug, Clone)]
pub struct WorkspaceTarget {
    pub name: String,
    pub config: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub policy: ServePolicy,
}

impl WorkspaceTarget {
    /// Derive a short name (directory basename) for a workspace, falling back to
    /// "default" when unpinned.
    pub fn derive_name(root: Option<&std::path::Path>) -> String {
        root.and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "default".to_string())
    }
}

pub struct MetarepoMcpServer {
    tools: Vec<Tool>,
    resources: Vec<Resource>,
    metarepo_path: PathBuf,
    /// Workspaces this server may serve. One entry = pinned/single mode; several
    /// = allowlist mode, where each tool call selects one by a `workspace` arg.
    targets: Vec<WorkspaceTarget>,
    /// True when more than one workspace is allowed (allowlist mode).
    multi: bool,
    /// Policy gating the workspace-independent gateway meta-tools. The single
    /// target's policy when pinned; full access in allowlist mode.
    gateway_policy: ServePolicy,
    /// Cache of downstream `(tool name, description)` lists keyed by server name,
    /// so repeated list/search calls don't respawn a server. Single-threaded.
    tool_cache: RefCell<HashMap<String, Vec<(String, String)>>>,
    /// Downstream tools promoted (via `mcp_enable`) into the top-level tools/list.
    promoted: RefCell<Vec<PromotedTool>>,
    /// Server-initiated JSON-RPC notification lines queued during request
    /// handling, flushed by the run loop (e.g. `tools/list_changed`).
    pending_notifications: RefCell<Vec<String>>,
}

/// A downstream tool surfaced at the gateway's top level under a namespaced name
/// (`server__tool`), so a client that honors `tools/list_changed` can call it
/// directly after `mcp_enable`.
#[derive(Debug, Clone)]
struct PromotedTool {
    qualified_name: String,
    server: String,
    tool: String,
    description: String,
    input_schema: Value,
}

impl Default for MetarepoMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MetarepoMcpServer {
    /// Unpinned, full-access server (backward-compatible default).
    pub fn new() -> Self {
        Self::with_options(None, None, ServePolicy::default())
    }

    /// Server pinned to a single workspace with an explicit policy.
    pub fn with_options(
        workspace_config: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
        policy: ServePolicy,
    ) -> Self {
        let name = WorkspaceTarget::derive_name(workspace_root.as_deref());
        let target = WorkspaceTarget {
            name,
            config: workspace_config,
            root: workspace_root,
            policy,
        };
        Self::with_targets(vec![target], false)
    }

    /// Server allowed to operate on several workspaces (allowlist mode when
    /// `multi` is true). The first target is the default when a call omits the
    /// `workspace` argument and only one is configured.
    pub fn with_targets(targets: Vec<WorkspaceTarget>, multi: bool) -> Self {
        let metarepo_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("meta"));
        let gateway_policy = if multi {
            ServePolicy::default()
        } else {
            targets
                .first()
                .map(|t| t.policy.clone())
                .unwrap_or_default()
        };

        Self {
            tools: Self::build_tools(),
            resources: Vec::new(),
            metarepo_path,
            targets,
            multi,
            gateway_policy,
            tool_cache: RefCell::new(HashMap::new()),
            promoted: RefCell::new(Vec::new()),
            pending_notifications: RefCell::new(Vec::new()),
        }
    }

    /// Queue a `tools/list_changed` notification for the run loop to flush.
    fn notify_tools_changed(&self) {
        self.pending_notifications.borrow_mut().push(
            json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }).to_string(),
        );
    }

    /// Resolve which workspace a tool call targets. In pinned mode the single
    /// target is always used; in allowlist mode the call's `workspace` argument
    /// selects one (by name, config path, or root), defaulting to the sole entry.
    fn resolve_target(&self, arguments: &Value) -> Result<&WorkspaceTarget> {
        let requested = arguments.get("workspace").and_then(|v| v.as_str());
        if !self.multi {
            return self
                .targets
                .first()
                .ok_or_else(|| anyhow::anyhow!("No workspace configured"));
        }
        match requested {
            Some(name) => self
                .targets
                .iter()
                .find(|t| {
                    t.name == name
                        || t.config.as_deref().map(|p| p.to_string_lossy()) == Some(name.into())
                        || t.root.as_deref().map(|p| p.to_string_lossy()) == Some(name.into())
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Workspace '{}' is not allowed. Allowed: {}",
                        name,
                        self.workspace_names().join(", ")
                    )
                }),
            None if self.targets.len() == 1 => Ok(&self.targets[0]),
            None => Err(anyhow::anyhow!(
                "This server hosts multiple workspaces; pass a 'workspace' argument. Allowed: {}",
                self.workspace_names().join(", ")
            )),
        }
    }

    fn workspace_names(&self) -> Vec<String> {
        self.targets.iter().map(|t| t.name.clone()).collect()
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
            // Gateway meta-tools: progressive disclosure over saved downstream
            // MCP servers. Browse the catalog, open one server's tools on demand,
            // search across all, then proxy a call — instead of surfacing every
            // downstream tool at the top level.
            Tool {
                name: "mcp_catalog".to_string(),
                description: "List the saved downstream MCP servers this gateway can reach"
                    .to_string(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            Tool {
                name: "mcp_list_tools".to_string(),
                description:
                    "List the tools a saved downstream MCP server exposes (connects on demand)"
                        .to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["server"],
                    "properties": {
                        "server": { "type": "string", "description": "Saved server name" }
                    }
                }),
            },
            Tool {
                name: "mcp_search_tools".to_string(),
                description:
                    "Search tool names and descriptions across all saved downstream servers"
                        .to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string", "description": "Case-insensitive substring" }
                    }
                }),
            },
            Tool {
                name: "mcp_workspaces".to_string(),
                description: "List the workspaces this server may operate on".to_string(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            Tool {
                name: "mcp_enable".to_string(),
                description: "Promote a downstream server's tools to this server's top-level \
                              tool list (callable directly as server__tool)"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["server"],
                    "properties": {
                        "server": { "type": "string", "description": "Saved server name" },
                        "tool": { "type": "string", "description": "One tool to enable (default: all)" }
                    }
                }),
            },
            Tool {
                name: "mcp_disable".to_string(),
                description: "Remove previously promoted downstream tools from the top-level list"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "server": { "type": "string", "description": "Server to disable (default: all)" },
                        "tool": { "type": "string", "description": "One tool to disable (default: all for the server)" }
                    }
                }),
            },
            Tool {
                name: "mcp_call".to_string(),
                description: "Call a tool on a saved downstream MCP server and return its result"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "required": ["server", "tool"],
                    "properties": {
                        "server": { "type": "string", "description": "Saved server name" },
                        "tool": { "type": "string", "description": "Tool name on that server" },
                        "arguments": { "type": "object", "description": "Tool arguments (default {})" }
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

            // Flush any server-initiated notifications queued while handling the
            // request (e.g. tools/list_changed after mcp_enable/mcp_disable).
            for note in self.pending_notifications.borrow_mut().drain(..) {
                writeln!(stdout, "{}", note)?;
                stdout.flush()?;
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
        let instructions = if self.multi {
            let lines: Vec<String> = self
                .targets
                .iter()
                .map(|t| format!("  - {} ({})", t.name, t.policy.summary()))
                .collect();
            format!(
                "Metarepo MCP server hosting {} workspaces; pass a 'workspace' argument to \
                 workspace tools (or call mcp_workspaces).\n{}",
                self.targets.len(),
                lines.join("\n")
            )
        } else {
            let t = &self.targets[0];
            let workspace = match &t.config {
                Some(p) => p.display().to_string(),
                None => "discovered from the launch directory".to_string(),
            };
            format!(
                "Metarepo MCP server. Workspace: {workspace}. Policy: {}.",
                t.policy.summary()
            )
        };

        let server_info = ServerInfo {
            name: "metarepo-mcp-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2025-06-18".to_string(),
            capabilities: ServerCapabilities {
                // Advertise that the tool list can change so clients re-fetch
                // after mcp_enable/mcp_disable.
                tools: Some(json!({ "listChanged": true })),
                resources: None,
                prompts: None,
            },
            instructions: Some(instructions),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(server_info).unwrap()),
            error: None,
        }
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let mut tools: Vec<Value> = if self.multi {
            // Allowlist mode: which workspace tools are permitted varies per call,
            // so advertise them all and add a `workspace` selector to each
            // workspace tool's schema. Gateway tools stay workspace-independent.
            self.tools
                .iter()
                .map(|t| {
                    let mut v = serde_json::to_value(t).unwrap_or(json!({}));
                    if !GATEWAY_TOOLS.contains(&t.name.as_str()) {
                        if let Some(props) = v
                            .get_mut("inputSchema")
                            .and_then(|s| s.get_mut("properties"))
                            .and_then(|p| p.as_object_mut())
                        {
                            props.insert(
                                "workspace".to_string(),
                                json!({
                                    "type": "string",
                                    "description": "Which hosted workspace to act on (see mcp_workspaces)"
                                }),
                            );
                        }
                    }
                    v
                })
                .collect()
        } else {
            // Pinned mode: only advertise tools the policy permits, so a
            // restricted server does not offer tools it will then reject. Gateway
            // tools use the gateway policy; workspace tools use the workspace's.
            let policy = &self.targets[0].policy;
            self.tools
                .iter()
                .filter(|t| {
                    if GATEWAY_TOOLS.contains(&t.name.as_str()) {
                        self.gateway_policy.allows(&t.name)
                    } else {
                        policy.allows(&t.name)
                    }
                })
                .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
                .collect()
        };

        // Append promoted downstream tools. They proxy a downstream call, so they
        // are only advertised when the gateway policy permits proxying.
        if self.gateway_policy.allows("mcp_call") {
            for p in self.promoted.borrow().iter() {
                tools.push(json!({
                    "name": p.qualified_name,
                    "description": format!("[{}] {}", p.server, p.description),
                    "inputSchema": p.input_schema,
                }));
            }
        }

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({ "tools": tools })),
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

        let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

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
        // Gateway meta-tools are workspace-independent; gate them with the
        // gateway policy and dispatch to downstream MCP servers.
        if GATEWAY_TOOLS.contains(&name) {
            if !self.gateway_policy.allows(name) {
                return Err(anyhow::anyhow!(
                    "Tool '{}' is not permitted by this server's policy ({})",
                    name,
                    self.gateway_policy.summary()
                ));
            }
            return self.execute_gateway_tool(name, &arguments);
        }

        // A promoted downstream tool (server__tool): proxy it like mcp_call.
        let promoted = self
            .promoted
            .borrow()
            .iter()
            .find(|p| p.qualified_name == name)
            .cloned();
        if let Some(p) = promoted {
            if !self.gateway_policy.allows("mcp_call") {
                return Err(anyhow::anyhow!(
                    "Proxying downstream tools is not permitted by this server's policy ({})",
                    self.gateway_policy.summary()
                ));
            }
            let config = McpConfig::load().context("Failed to load saved MCP servers")?;
            let cfg = resolve_server(&config, &p.server)?;
            let tool = p.tool.clone();
            let result = run_async(async move {
                let mut client = connect_downstream(&cfg).await?;
                let r = client.call_tool(&tool, arguments).await;
                client.close().await.ok();
                r
            })?;
            return Ok(serde_json::to_string_pretty(&result)?);
        }

        // Workspace tools: resolve which workspace this call targets, then gate
        // it against that workspace's policy.
        let target = self.resolve_target(&arguments)?;
        if !target.policy.allows(name) {
            return Err(anyhow::anyhow!(
                "Tool '{}' is not permitted by workspace '{}' ({})",
                name,
                target.name,
                target.policy.summary()
            ));
        }

        let mut cmd = Command::new(&self.metarepo_path);

        // Pin spawned subprocesses to the resolved workspace and enable
        // experimental subcommands (mcp_* tools need `-x`). `--config` forces the
        // child to use this workspace's .meta instead of re-discovering from cwd.
        cmd.arg("--experimental");
        if let Some(config) = &target.config {
            cmd.arg("--config").arg(config);
        }
        if let Some(root) = &target.root {
            cmd.current_dir(root);
        }

        match name {
            "help" => {
                if let Some(plugin) = arguments.get("plugin").and_then(|v| v.as_str()) {
                    cmd.args([plugin, "--help"]);
                } else {
                    cmd.arg("--help");
                }
            }
            "git_status" => {
                cmd.args(["git", "status"]);
                if arguments
                    .get("verbose")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cmd.arg("--verbose");
                }
            }
            "git_diff" => {
                cmd.args(["git", "diff"]);
                if arguments
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cmd.arg("--staged");
                }
            }
            "git_commit" => {
                cmd.args(["git", "commit"]);
                if let Some(message) = arguments.get("message").and_then(|v| v.as_str()) {
                    cmd.args(["-m", message]);
                }
                if arguments
                    .get("all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cmd.arg("-a");
                }
            }
            "git_pull" => {
                cmd.args(["git", "pull"]);
            }
            "git_push" => {
                cmd.args(["git", "push"]);
            }
            "project_list" => {
                cmd.args(["project", "list"]);
            }
            "project_add" => {
                cmd.args(["project", "add"]);
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    cmd.arg(path);
                }
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.args(["--name", name]);
                }
            }
            "project_remove" => {
                cmd.args(["project", "remove"]);
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.arg(name);
                }
            }
            "exec" => {
                cmd.arg("exec");
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    cmd.arg(command);
                }
                let requested: Vec<String> = arguments
                    .get("projects")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| p.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                // Apply the workspace's project allowlist (if any): defaults to the
                // allowed set and rejects anything outside it.
                match target.policy.exec_projects(&requested)? {
                    Some(list) if !list.is_empty() => {
                        cmd.arg("--projects").arg(list.join(","));
                    }
                    _ => {
                        if !requested.is_empty() {
                            cmd.arg("--projects").arg(requested.join(","));
                        }
                    }
                }
            }
            "mcp_add_server" => {
                cmd.args(["mcp", "add"]);
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
                cmd.args(["mcp", "list"]);
            }
            "mcp_remove_server" => {
                cmd.args(["mcp", "remove"]);
                if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                    cmd.arg(name);
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }

        let output = cmd
            .output()
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

    /// Dispatch a gateway meta-tool against the saved downstream servers.
    fn execute_gateway_tool(&self, name: &str, arguments: &Value) -> Result<String> {
        // mcp_workspaces describes this server, not a downstream one.
        if name == "mcp_workspaces" {
            let workspaces: Vec<Value> = self
                .targets
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "config": t.config.as_ref().map(|p| p.display().to_string()),
                        "policy": t.policy.summary(),
                    })
                })
                .collect();
            return Ok(serde_json::to_string_pretty(&json!({
                "mode": if self.multi { "allowlist" } else { "pinned" },
                "workspaces": workspaces,
            }))?);
        }

        let config = McpConfig::load().context("Failed to load saved MCP servers")?;
        match name {
            "mcp_catalog" => self.gateway_catalog(&config),
            "mcp_list_tools" => {
                let server = arguments
                    .get("server")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("mcp_list_tools requires a 'server' argument")
                    })?;
                let tools = self.downstream_tools(&config, server)?;
                Ok(serde_json::to_string_pretty(&json!({
                    "server": server,
                    "tools": tools.iter().map(|(n, d)| json!({ "name": n, "description": d }))
                        .collect::<Vec<_>>(),
                }))?)
            }
            "mcp_search_tools" => {
                let query = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("mcp_search_tools requires a 'query' argument"))?
                    .to_lowercase();
                let mut matches = Vec::new();
                let mut server_names: Vec<&String> = config.servers.keys().collect();
                server_names.sort();
                for server in server_names {
                    // A failing server should not abort the whole search.
                    let tools = match self.downstream_tools(&config, server) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    for (tool, desc) in tools {
                        if tool.to_lowercase().contains(&query)
                            || desc.to_lowercase().contains(&query)
                        {
                            matches.push(json!({
                                "server": server, "tool": tool, "description": desc
                            }));
                        }
                    }
                }
                Ok(serde_json::to_string_pretty(&json!({
                    "query": query, "matches": matches
                }))?)
            }
            "mcp_call" => {
                let server = arguments
                    .get("server")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("mcp_call requires a 'server' argument"))?;
                let tool = arguments
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("mcp_call requires a 'tool' argument"))?;
                let tool_args = arguments.get("arguments").cloned().unwrap_or(json!({}));
                let cfg = resolve_server(&config, server)?;
                let result = run_async(async move {
                    let mut client = connect_downstream(&cfg).await?;
                    let r = client.call_tool(tool, tool_args).await;
                    client.close().await.ok();
                    r
                })?;
                Ok(serde_json::to_string_pretty(&result)?)
            }
            "mcp_enable" => {
                let server = arguments
                    .get("server")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("mcp_enable requires a 'server' argument"))?;
                let only = arguments.get("tool").and_then(|v| v.as_str());
                let full = self.downstream_full_tools(&config, server)?;
                let mut enabled = Vec::new();
                for (tname, desc, schema) in full {
                    if let Some(want) = only {
                        if tname != want {
                            continue;
                        }
                    }
                    let qualified = format!("{server}__{tname}");
                    let mut promoted = self.promoted.borrow_mut();
                    if !promoted.iter().any(|p| p.qualified_name == qualified) {
                        promoted.push(PromotedTool {
                            qualified_name: qualified.clone(),
                            server: server.to_string(),
                            tool: tname.clone(),
                            description: desc,
                            input_schema: schema,
                        });
                    }
                    enabled.push(qualified);
                }
                if let Some(want) = only {
                    if enabled.is_empty() {
                        return Err(anyhow::anyhow!(
                            "Server '{}' has no tool named '{}'",
                            server,
                            want
                        ));
                    }
                }
                self.notify_tools_changed();
                Ok(serde_json::to_string_pretty(&json!({
                    "enabled": enabled,
                    "note": "Promoted tools are callable as server__tool; clients honoring \
                             tools/list_changed will see them after refreshing.",
                }))?)
            }
            "mcp_disable" => {
                let server = arguments.get("server").and_then(|v| v.as_str());
                let tool = arguments.get("tool").and_then(|v| v.as_str());
                let before = self.promoted.borrow().len();
                self.promoted.borrow_mut().retain(|p| match (server, tool) {
                    (Some(s), Some(t)) => !(p.server == s && p.tool == t),
                    (Some(s), None) => p.server != s,
                    _ => false, // neither given: disable all
                });
                let removed = before - self.promoted.borrow().len();
                if removed > 0 {
                    self.notify_tools_changed();
                }
                Ok(serde_json::to_string_pretty(
                    &json!({ "disabled": removed }),
                )?)
            }
            _ => Err(anyhow::anyhow!("Unknown gateway tool: {}", name)),
        }
    }

    /// Full tool descriptors (name, description, input schema) for one downstream
    /// server, fetched by connecting on demand.
    fn downstream_full_tools(
        &self,
        config: &McpConfig,
        server: &str,
    ) -> Result<Vec<(String, String, Value)>> {
        let cfg = resolve_server(config, server)?;
        let tools = run_async(async move {
            let mut client = connect_downstream(&cfg).await?;
            let t = client.list_tools().await;
            client.close().await.ok();
            t
        })?;
        Ok(tools
            .into_iter()
            .map(|t| (t.name, t.description.unwrap_or_default(), t.input_schema))
            .collect())
    }

    /// List the saved downstream servers without connecting to any of them.
    fn gateway_catalog(&self, config: &McpConfig) -> Result<String> {
        let mut names: Vec<&String> = config.servers.keys().collect();
        names.sort();
        let servers: Vec<Value> = names
            .into_iter()
            .map(|n| {
                let s = &config.servers[n];
                json!({ "name": n, "command": s.command, "args": s.args })
            })
            .collect();
        Ok(serde_json::to_string_pretty(
            &json!({ "servers": servers }),
        )?)
    }

    /// Tools for one downstream server, using the cache or connecting on demand.
    fn downstream_tools(&self, config: &McpConfig, server: &str) -> Result<Vec<(String, String)>> {
        if let Some(cached) = self.tool_cache.borrow().get(server) {
            return Ok(cached.clone());
        }
        let cfg = resolve_server(config, server)?;
        let tools = run_async(async move {
            let mut client = connect_downstream(&cfg).await?;
            let t = client.list_tools().await;
            client.close().await.ok();
            t
        })?;
        let list: Vec<(String, String)> = tools
            .into_iter()
            .map(|t| (t.name, t.description.unwrap_or_default()))
            .collect();
        self.tool_cache
            .borrow_mut()
            .insert(server.to_string(), list.clone());
        Ok(list)
    }
}

/// Resolve a saved server by name, erroring if it is unknown.
fn resolve_server(config: &McpConfig, name: &str) -> Result<McpServerConfig> {
    config.servers.get(name).cloned().ok_or_else(|| {
        anyhow::anyhow!("No saved MCP server named '{}'. Add it with mcp add.", name)
    })
}

/// Connect to a downstream server with a timeout. Note: working_dir/env on the
/// saved config are not yet applied (same limitation as the `mcp connect` CLI).
async fn connect_downstream(cfg: &McpServerConfig) -> Result<McpClient> {
    tokio::time::timeout(GATEWAY_TIMEOUT, McpClient::connect(&cfg.command, &cfg.args))
        .await
        .map_err(|_| anyhow::anyhow!("Timed out connecting to MCP server '{}'", cfg.name))?
}

/// Run an async gateway operation from the synchronous server loop.
fn run_async<F, T>(fut: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to start async runtime")?
        .block_on(fut)
}

/// Print ready-to-paste Claude Desktop config.
///
/// With no `workspaces`, pins to the current directory. With several and
/// `allowlist=false`, emits one pinned entry per workspace. With `allowlist=true`,
/// emits a single entry hosting all of them (per-call `workspace` selection).
pub fn print_vscode_config(workspaces: &[String], allowlist: bool) {
    let meta_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("meta"))
        .to_string_lossy()
        .to_string();

    let basename = |p: &str| {
        std::path::Path::new(p)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string())
    };

    // `mcp` is experimental, so the client must launch `meta -x mcp serve`.
    let mut servers = serde_json::Map::new();
    if allowlist && !workspaces.is_empty() {
        servers.insert(
            "metarepo".to_string(),
            json!({
                "command": meta_path,
                "args": ["-x", "mcp", "serve", "--allow-workspaces", workspaces.join(",")],
                "env": {}
            }),
        );
    } else if !workspaces.is_empty() {
        for ws in workspaces {
            servers.insert(
                format!("metarepo-{}", basename(ws)),
                json!({
                    "command": meta_path,
                    "args": ["-x", "mcp", "serve", "--meta", ws],
                    "env": {}
                }),
            );
        }
    } else {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        servers.insert(
            "metarepo".to_string(),
            json!({
                "command": meta_path,
                "args": ["-x", "mcp", "serve", "--meta", cwd],
                "env": {}
            }),
        );
    }
    let claude_config = json!({ "mcpServers": servers });

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
    println!("The Metarepo MCP server exposes these tools (subject to the serve policy):");
    println!("  • help - Get help and list available commands");
    println!("  • git_status, git_diff, git_commit, git_pull, git_push");
    println!("  • project_list, project_add, project_remove");
    println!("  • exec - Execute commands across projects");
    println!("  • mcp_add_server, mcp_list_servers, mcp_remove_server");
    println!("  • mcp_catalog, mcp_list_tools, mcp_search_tools, mcp_call (gateway)");
    println!("  • mcp_workspaces, mcp_enable, mcp_disable (gateway)");
    println!();
    println!("=== Testing ===");
    println!();
    println!("Test the server directly:");
    println!("  meta mcp serve");
    println!();
    println!("Then send JSON-RPC commands via stdin");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(mode: ServeMode, allow_exec: bool, tools: Option<Vec<String>>) -> ServePolicy {
        ServePolicy {
            mode,
            allow_exec,
            tools,
            projects: None,
        }
    }

    #[test]
    fn full_default_allows_everything() {
        let p = policy(ServeMode::Full, true, None);
        assert!(p.allows("git_status"));
        assert!(p.allows("git_commit"));
        assert!(p.allows("exec"));
    }

    #[test]
    fn read_only_denies_writes_and_exec() {
        let p = policy(ServeMode::ReadOnly, true, None);
        assert!(p.allows("git_status"));
        assert!(p.allows("project_list"));
        assert!(!p.allows("git_commit"));
        assert!(!p.allows("project_add"));
        assert!(!p.allows("exec"));
    }

    #[test]
    fn read_write_allows_writes_but_not_exec() {
        let p = policy(ServeMode::ReadWrite, true, None);
        assert!(p.allows("git_commit"));
        assert!(!p.allows("exec")); // exec only in Full
    }

    #[test]
    fn allow_exec_false_disables_exec_in_full() {
        let p = policy(ServeMode::Full, false, None);
        assert!(p.allows("git_commit"));
        assert!(!p.allows("exec"));
    }

    #[test]
    fn tools_allowlist_intersects_with_mode() {
        let p = policy(ServeMode::Full, true, Some(vec!["git_status".into()]));
        assert!(p.allows("git_status"));
        assert!(!p.allows("git_commit")); // not in allowlist
        assert!(!p.allows("exec")); // not in allowlist
    }

    #[test]
    fn gateway_browse_tools_are_reads_call_is_write() {
        // Browsing the gateway is read-only; proxying a call is a write.
        assert_eq!(tool_kind("mcp_catalog"), ToolKind::Read);
        assert_eq!(tool_kind("mcp_list_tools"), ToolKind::Read);
        assert_eq!(tool_kind("mcp_search_tools"), ToolKind::Read);
        assert_eq!(tool_kind("mcp_call"), ToolKind::Write);

        let ro = policy(ServeMode::ReadOnly, true, None);
        assert!(ro.allows("mcp_catalog"));
        assert!(ro.allows("mcp_search_tools"));
        assert!(!ro.allows("mcp_call")); // read-only cannot proxy a call
    }

    #[test]
    fn from_settings_parses_mode_and_exec() {
        let s = metarepo_core::McpServeSettings {
            mode: Some("read-only".to_string()),
            allow_exec: Some(false),
            tools: None,
            projects: None,
        };
        let p = ServePolicy::from_settings(Some(&s));
        assert_eq!(p.mode, ServeMode::ReadOnly);
        assert!(!p.allow_exec);
    }

    #[test]
    fn exec_projects_defaults_and_rejects_outside_allowlist() {
        let mut p = ServePolicy::default();
        // No allowlist: no restriction.
        assert_eq!(p.exec_projects(&[]).unwrap(), None);

        p.projects = Some(vec!["web".into(), "api".into()]);
        // Empty request → defaults to the allowlist.
        assert_eq!(
            p.exec_projects(&[]).unwrap(),
            Some(vec!["web".into(), "api".into()])
        );
        // Subset request is kept.
        assert_eq!(
            p.exec_projects(&["web".into()]).unwrap(),
            Some(vec!["web".into()])
        );
        // Outside the allowlist is rejected.
        assert!(p.exec_projects(&["secret".into()]).is_err());
    }

    fn target(name: &str, mode: ServeMode) -> WorkspaceTarget {
        WorkspaceTarget {
            name: name.to_string(),
            config: None,
            root: None,
            policy: policy(mode, true, None),
        }
    }

    #[test]
    fn notify_tools_changed_queues_a_list_changed_notification() {
        let s = MetarepoMcpServer::new();
        assert!(s.promoted.borrow().is_empty());
        assert!(s.pending_notifications.borrow().is_empty());
        s.notify_tools_changed();
        let queued = s.pending_notifications.borrow();
        assert_eq!(queued.len(), 1);
        assert!(queued[0].contains("notifications/tools/list_changed"));
        // enable/disable are gateway tools classified as writes.
        assert!(GATEWAY_TOOLS.contains(&"mcp_enable"));
        assert!(GATEWAY_TOOLS.contains(&"mcp_disable"));
        assert_eq!(tool_kind("mcp_enable"), ToolKind::Write);
    }

    #[test]
    fn resolve_target_pinned_ignores_workspace_arg() {
        let s = MetarepoMcpServer::with_targets(vec![target("only", ServeMode::Full)], false);
        let t = s.resolve_target(&json!({ "workspace": "nope" })).unwrap();
        assert_eq!(t.name, "only");
    }

    #[test]
    fn resolve_target_allowlist_requires_and_validates_workspace() {
        let s = MetarepoMcpServer::with_targets(
            vec![
                target("acme", ServeMode::Full),
                target("personal", ServeMode::ReadOnly),
            ],
            true,
        );
        // Missing workspace with several targets is an error.
        assert!(s.resolve_target(&json!({})).is_err());
        // Unknown workspace is rejected.
        assert!(s.resolve_target(&json!({ "workspace": "ghost" })).is_err());
        // Known workspace resolves, carrying its own policy.
        let t = s
            .resolve_target(&json!({ "workspace": "personal" }))
            .unwrap();
        assert_eq!(t.name, "personal");
        assert_eq!(t.policy.mode, ServeMode::ReadOnly);
    }
}
