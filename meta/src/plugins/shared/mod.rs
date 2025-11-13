pub mod output_manager;
pub mod git_operations;

pub use output_manager::{OutputManager, ProjectOutput, JobStatus, ProgressIndicator};
pub use git_operations::{clone_with_auth, create_default_worktree, detect_default_branch};