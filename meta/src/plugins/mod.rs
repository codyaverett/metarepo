// Built-in plugins for metarepo
// These are compiled directly into the binary rather than as separate crates

pub mod config;
pub mod exec;
pub mod git;
pub mod init;
pub mod mcp;
pub mod plugin_loader;
pub mod plugin_manager;
pub mod project;
pub mod rules;
pub mod run;
pub mod shared;
pub mod worktree;

// Re-export plugin structs for convenience
pub use config::ConfigPlugin;
pub use exec::ExecPlugin;
pub use git::GitPlugin;
pub use init::InitPlugin;
pub use mcp::McpPlugin;
pub use plugin_manager::PluginManagerPlugin;
pub use project::ProjectPlugin;
pub use rules::RulesPlugin;
pub use run::RunPlugin;
pub use worktree::WorktreePlugin;

// Re-export plugin loader
pub use plugin_loader::{ExternalPlugin, PluginLoader};
