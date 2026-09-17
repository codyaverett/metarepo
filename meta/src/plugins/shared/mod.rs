pub mod git_operations;
pub mod output_manager;

pub use git_operations::{
    clone_with_auth, create_default_worktree, detect_default_branch, parse_depth_arg,
    refetch_shallow,
};
pub use output_manager::{JobStatus, OutputManager, ProgressIndicator, ProjectOutput};

/// Render a relative path as a portable identifier: always `/`-separated, so a
/// string written into config or a provenance file on Windows still resolves
/// and compares equal on unix. `Path::join` accepts `/` on Windows, so readers
/// need no matching change.
///
/// No-op on unix, where `\` is a legal filename character and must be kept.
pub fn portable_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}
