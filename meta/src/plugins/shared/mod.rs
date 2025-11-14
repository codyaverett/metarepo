pub mod git_operations;
pub mod output_manager;

pub use git_operations::{clone_with_auth, create_default_worktree, detect_default_branch};
pub use output_manager::{JobStatus, OutputManager, ProgressIndicator, ProjectOutput};
