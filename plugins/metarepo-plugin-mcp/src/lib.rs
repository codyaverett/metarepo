//! MCP server and gateway for metarepo, packaged as an external plugin.
//!
//! Extracted from the `meta` binary's built-in (experimental) `mcp` plugin.
//! The module layout is unchanged; `main.rs` wires it to the plugin wire
//! protocol via `metarepo-plugin-sdk`, declaring every command as a takeover
//! command so the binary parses its own clap surface and `serve` can own
//! stdin/stdout for the MCP stdio transport.

pub mod client;
pub mod config;
pub mod mcp_server;
pub mod plugin;
pub mod server; // Keep for McpServerConfig type only

pub use mcp_server::{print_vscode_config, MetarepoMcpServer};
pub use plugin::McpPlugin;
