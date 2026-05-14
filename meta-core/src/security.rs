//! Security helpers for validating untrusted inputs that flow from the `.meta`
//! config and CLI args into filesystem paths, subprocess spawns, and child
//! environment variables.
//!
//! These helpers are deliberately small and pure so that every plugin can apply
//! the same trust boundary without re-implementing checks.

use anyhow::{anyhow, Result};
use std::path::{Component, Path, PathBuf};

/// Environment variable names that can subvert child processes if attacker-controlled.
/// We refuse to forward these from `.meta` config to subprocesses.
pub const DANGEROUS_ENV_VARS: &[&str] = &[
    // Dynamic linker hijacking
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_FORCE_FLAT_NAMESPACE",
    // Shell startup hijacking
    "BASH_ENV",
    "ENV",
    "PROMPT_COMMAND",
    // Language runtime hijacking
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "NODE_PATH",
    "RUBYOPT",
    "RUBYLIB",
    "PERL5OPT",
    "PERL5LIB",
    // Git command hijacking
    "GIT_SSH_COMMAND",
    "GIT_SSH",
    "GIT_EXEC_PATH",
];

/// Returns true if `key` matches a known dangerous env var (case-insensitive).
pub fn is_dangerous_env_var(key: &str) -> bool {
    DANGEROUS_ENV_VARS
        .iter()
        .any(|d| d.eq_ignore_ascii_case(key))
}

/// Validate a string used as a relative path segment in config-driven path
/// joins (project name, branch name, worktree path suffix, plugin file ref).
/// Rejects empty values, null bytes, absolute paths, and any `..` component.
pub fn validate_path_segment(label: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(anyhow!("{} must not be empty", label));
    }
    if value.contains('\0') {
        return Err(anyhow!("{} must not contain null bytes", label));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(anyhow!(
            "{} must be a relative path (got absolute '{}')",
            label,
            value
        ));
    }
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(anyhow!(
                    "{} must not contain '..' segments (got '{}')",
                    label,
                    value
                ));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(anyhow!(
                    "{} must be a relative path (got '{}')",
                    label,
                    value
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Verify that `joined` (which need not exist on disk yet) resolves inside
/// `base` after canonicalization. Returns the canonical resolved path.
///
/// Walks up `joined` to find an existing ancestor, canonicalizes that, then
/// re-appends the non-existing suffix — this lets us safely check paths that
/// are about to be created.
pub fn ensure_within_base(base: &Path, joined: &Path) -> Result<PathBuf> {
    let canon_base = base
        .canonicalize()
        .map_err(|e| anyhow!("Failed to canonicalize base {:?}: {}", base, e))?;

    let canon_joined = canonicalize_creatable(joined)?;

    if !canon_joined.starts_with(&canon_base) {
        return Err(anyhow!(
            "Path {:?} escapes base directory {:?}",
            joined,
            base
        ));
    }
    Ok(canon_joined)
}

/// Canonicalize a path that may not exist yet by canonicalizing the nearest
/// existing ancestor and appending the remainder.
pub fn canonicalize_creatable(path: &Path) -> Result<PathBuf> {
    if let Ok(canon) = path.canonicalize() {
        return Ok(canon);
    }
    let mut existing = path;
    let mut suffix = PathBuf::new();
    loop {
        if existing.exists() {
            break;
        }
        let file = existing
            .file_name()
            .ok_or_else(|| anyhow!("Cannot canonicalize path {:?}", path))?;
        suffix = Path::new(file).join(&suffix);
        existing = existing
            .parent()
            .ok_or_else(|| anyhow!("Cannot canonicalize path {:?}", path))?;
        if existing.as_os_str().is_empty() {
            return Err(anyhow!("Cannot canonicalize path {:?}", path));
        }
    }
    let canon_existing = existing
        .canonicalize()
        .map_err(|e| anyhow!("Failed to canonicalize {:?}: {}", existing, e))?;
    Ok(canon_existing.join(suffix))
}

/// Schemes we recognize as remote git URLs in `meta project add`.
/// Note: `git://` is unauthenticated/unencrypted but still a real URL scheme;
/// detecting it correctly is preferable to silently falling through to local-path
/// handling. Callers that want to warn can check [`is_unencrypted_git_scheme`].
pub fn is_supported_git_url(src: &str) -> bool {
    src.starts_with("https://")
        || src.starts_with("http://")
        || src.starts_with("ssh://")
        || src.starts_with("git://")
        || src.starts_with("git@")
}

/// True for schemes that don't provide transport-level encryption/authentication.
pub fn is_unencrypted_git_scheme(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("git://")
}

/// Reject obviously malformed URL strings before storing them in config.
/// Allows ssh shorthand (`git@host:repo`); just guards against control chars
/// and empty values.
pub fn validate_project_url(src: &str) -> Result<()> {
    if src.is_empty() {
        return Err(anyhow!("project URL must not be empty"));
    }
    if src.bytes().any(|b| b == 0 || b == b'\n' || b == b'\r') {
        return Err(anyhow!("project URL contains invalid control characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_segment_rejects_traversal() {
        assert!(validate_path_segment("name", "../etc").is_err());
        assert!(validate_path_segment("name", "a/../b").is_err());
        assert!(validate_path_segment("name", "/abs").is_err());
        assert!(validate_path_segment("name", "").is_err());
        assert!(validate_path_segment("name", "with\0null").is_err());
    }

    #[test]
    fn validate_path_segment_accepts_normal_names() {
        assert!(validate_path_segment("name", "my-project").is_ok());
        assert!(validate_path_segment("name", "nested/subdir").is_ok());
        assert!(validate_path_segment("branch", "feature/foo").is_ok());
    }

    #[test]
    fn dangerous_env_detection_is_case_insensitive() {
        assert!(is_dangerous_env_var("LD_PRELOAD"));
        assert!(is_dangerous_env_var("ld_preload"));
        assert!(is_dangerous_env_var("BASH_ENV"));
        assert!(!is_dangerous_env_var("PATH"));
        assert!(!is_dangerous_env_var("HOME"));
    }

    #[test]
    fn url_scheme_detection_handles_git_protocol() {
        assert!(is_supported_git_url("git://github.com/u/r.git"));
        assert!(is_supported_git_url("https://github.com/u/r.git"));
        assert!(is_supported_git_url("git@github.com:u/r.git"));
        assert!(!is_supported_git_url("file:///etc/passwd"));
        assert!(!is_supported_git_url("/local/path"));
        assert!(is_unencrypted_git_scheme("git://x"));
        assert!(is_unencrypted_git_scheme("http://x"));
        assert!(!is_unencrypted_git_scheme("https://x"));
    }

    #[test]
    fn ensure_within_base_blocks_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("base");
        std::fs::create_dir(&base).unwrap();
        let sibling = tmp.path().join("sibling");
        std::fs::create_dir(&sibling).unwrap();

        // Joining base with "../sibling" canonicalizes outside base.
        let attempt = base.join("../sibling");
        assert!(ensure_within_base(&base, &attempt).is_err());

        // A path strictly inside base resolves cleanly.
        let inside = base.join("sub").join("file");
        let resolved = ensure_within_base(&base, &inside).unwrap();
        assert!(resolved.starts_with(base.canonicalize().unwrap()));
    }
}
