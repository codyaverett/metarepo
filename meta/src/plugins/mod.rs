// Built-in plugins for metarepo
// These are compiled directly into the binary rather than as separate crates

pub mod init;
pub mod git;
pub mod project;
pub mod config;
pub mod exec;
pub mod mcp;
pub mod rules;
pub mod worktree;
pub mod run;
pub mod plugin_loader;
pub mod plugin_manager;
pub mod shared;

// Re-export plugin structs for convenience
pub use init::InitPlugin;
pub use git::GitPlugin;
pub use project::ProjectPlugin;
pub use config::ConfigPlugin;
pub use exec::ExecPlugin;
pub use mcp::McpPlugin;
pub use rules::RulesPlugin;
pub use worktree::WorktreePlugin;
pub use run::RunPlugin;
pub use plugin_manager::PluginManagerPlugin;

// Re-export plugin loader
pub use plugin_loader::{PluginLoader, ExternalPlugin};