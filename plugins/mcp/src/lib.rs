pub mod plugin;
pub mod server;
pub mod client;
pub mod mcp_server;

pub use plugin::McpPlugin;
pub use mcp_server::{GestaltMcpServer, print_vscode_config};