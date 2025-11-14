pub mod plugin;
// pub mod plugin_old;
pub mod client;
pub mod config;
pub mod mcp_server;
pub mod server; // Keep for McpServerConfig type only

// Export the main plugin
pub use plugin::McpPlugin;
// Keep old plugin available for backward compatibility (deprecated)
// #[deprecated(note = "Use McpPlugin instead")]
// pub use plugin_old::McpPlugin as McpPluginV1;
pub use mcp_server::{print_vscode_config, MetarepoMcpServer};
