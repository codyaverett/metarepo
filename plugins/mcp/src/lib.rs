pub mod client;
pub mod config;
pub mod mcp_server;
pub mod plugin;
pub mod server; // Keep for McpServerConfig type only

pub use mcp_server::{print_vscode_config, GestaltMcpServer};
pub use plugin::McpPlugin;
