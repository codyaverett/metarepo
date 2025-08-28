pub mod plugin;
pub mod client;
pub mod mcp_server;
pub mod config;
pub mod server; // Keep for McpServerConfig type only

pub use plugin::McpPlugin;
pub use mcp_server::{GestaltMcpServer, print_vscode_config};