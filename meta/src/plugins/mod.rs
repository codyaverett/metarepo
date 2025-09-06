// Built-in plugins for metarepo
// These are compiled directly into the binary rather than as separate crates

pub mod init;
pub mod git;
pub mod project;
pub mod exec;
pub mod mcp;
pub mod rules;

// Re-export plugin structs for convenience
pub use init::InitPlugin;
pub use git::GitPlugin;
pub use project::ProjectPlugin;
pub use exec::ExecPlugin;
pub use mcp::McpPlugin;
pub use rules::RulesPlugin;