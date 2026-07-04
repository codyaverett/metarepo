//! `meta status` — an interactive multi-repo dashboard.
//!
//! Gathers per-project git state (branch, ahead/behind vs upstream, dirty file
//! count) and presents it in a navigable TUI built on the shared tree-shell
//! primitives ([`metarepo_core::tui::tree_shell`]). Read-only in this version:
//! navigate, search, and drill into a repo's detail; refresh with `r`.

use git2::{Repository, StatusOptions};
use std::path::Path;
use std::process::Command;

mod dashboard;
mod plugin;

pub use plugin::StatusPlugin;

/// Fetch the selected repository (`git fetch`). Blocks on the network; the
/// dashboard refreshes ahead/behind counts afterward.
pub(super) fn fetch(path: &Path) -> Result<(), String> {
    run_git(path, &["fetch", "--quiet"])
}

/// Fast-forward pull the selected repository (`git pull --ff-only`). Fails
/// cleanly (surfaced in the status line) for bare or diverged repos rather than
/// creating a merge commit.
pub(super) fn pull(path: &Path) -> Result<(), String> {
    run_git(path, &["pull", "--ff-only", "--quiet"])
}

/// Run `git -C <path> <args>`, returning the trimmed stderr on failure. Uses the
/// git CLI so the user's configured credentials/helpers apply, matching the rest
/// of the codebase.
fn run_git(path: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        let msg = err.trim();
        Err(if msg.is_empty() {
            format!("git exited with {}", output.status)
        } else {
            msg.lines().next().unwrap_or(msg).to_string()
        })
    }
}

/// The git state of one tracked project, as shown in the dashboard.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoStatus {
    /// Project key (its path under the workspace root).
    pub name: String,
    /// Resolved state, or why it could not be read.
    pub state: RepoState,
}

/// The outcome of inspecting a project's directory.
#[derive(Debug, Clone, PartialEq)]
pub enum RepoState {
    /// The project directory does not exist on disk.
    Missing,
    /// The directory exists but is not a git repository.
    NotGit,
    /// Inspecting the repository failed.
    Error(String),
    /// Successfully read git state.
    Ok {
        /// Current branch (or short commit id when detached).
        branch: String,
        /// Commits ahead of the upstream (0 when no upstream).
        ahead: usize,
        /// Commits behind the upstream (0 when no upstream).
        behind: usize,
        /// Number of changed working-tree/index entries (0 = clean).
        dirty: usize,
    },
}

impl RepoState {
    /// A compact one-line summary for the tree row.
    pub fn summary(&self) -> String {
        match self {
            RepoState::Missing => "(missing)".to_string(),
            RepoState::NotGit => "(not a git repo)".to_string(),
            RepoState::Error(e) => format!("(error: {e})"),
            RepoState::Ok {
                branch,
                ahead,
                behind,
                dirty,
            } => {
                let mut parts = vec![branch.clone()];
                if *ahead > 0 {
                    parts.push(format!("+{ahead}"));
                }
                if *behind > 0 {
                    parts.push(format!("-{behind}"));
                }
                parts.push(if *dirty > 0 {
                    format!("*{dirty}")
                } else {
                    "clean".to_string()
                });
                parts.join(" ")
            }
        }
    }
}

/// Gather status for each project under `base_path`, preserving input order.
pub fn gather_all(base_path: &Path, projects: &[String]) -> Vec<RepoStatus> {
    projects
        .iter()
        .map(|name| RepoStatus {
            name: name.clone(),
            state: gather_one(&base_path.join(name)),
        })
        .collect()
}

/// Inspect a single repository directory.
fn gather_one(path: &Path) -> RepoState {
    if !path.exists() {
        return RepoState::Missing;
    }
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(_) => return RepoState::NotGit,
    };

    let branch = match current_branch(&repo) {
        Ok(b) => b,
        Err(e) => return RepoState::Error(e),
    };
    let dirty = match dirty_count(&repo) {
        Ok(n) => n,
        Err(e) => return RepoState::Error(e),
    };
    let (ahead, behind) = ahead_behind(&repo).unwrap_or((0, 0));

    RepoState::Ok {
        branch,
        ahead,
        behind,
        dirty,
    }
}

/// Current branch shorthand, or a short commit id when HEAD is detached.
fn current_branch(repo: &Repository) -> Result<String, String> {
    match repo.head() {
        Ok(head) => {
            if let Ok(name) = head.shorthand() {
                Ok(name.to_string())
            } else if let Some(oid) = head.target() {
                Ok(oid.to_string()[..7].to_string())
            } else {
                Ok("(unknown)".to_string())
            }
        }
        // An unborn branch (fresh repo, no commits) is not an error to surface.
        Err(_) => Ok("(no commits)".to_string()),
    }
}

/// Count changed working-tree/index entries (untracked included, ignored not).
fn dirty_count(repo: &Repository) -> Result<usize, String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).exclude_submodules(true);
    repo.statuses(Some(&mut opts))
        .map(|s| s.len())
        .map_err(|e| e.message().to_string())
}

/// Ahead/behind counts vs the current branch's upstream. Returns `None` when
/// there is no upstream (or HEAD is detached/unborn).
fn ahead_behind(repo: &Repository) -> Option<(usize, usize)> {
    let head = repo.head().ok()?;
    let local_oid = head.target()?;
    let branch_name = head.shorthand().ok()?;
    let branch = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .ok()?;
    let upstream = branch.upstream().ok()?;
    let upstream_oid = upstream.get().target()?;
    repo.graph_ahead_behind(local_oid, upstream_oid).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {:?} failed", args);
    }

    #[test]
    fn missing_and_non_git_dirs() {
        let tmp = tempdir().unwrap();
        assert_eq!(gather_one(&tmp.path().join("nope")), RepoState::Missing);
        std::fs::create_dir(tmp.path().join("plain")).unwrap();
        assert_eq!(gather_one(&tmp.path().join("plain")), RepoState::NotGit);
    }

    #[test]
    fn clean_then_dirty_repo() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("r");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join("a.txt"), "hi").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "init"]);

        match gather_one(&repo) {
            RepoState::Ok {
                branch,
                ahead,
                behind,
                dirty,
            } => {
                assert_eq!(branch, "main");
                assert_eq!((ahead, behind), (0, 0));
                assert_eq!(dirty, 0);
            }
            other => panic!("expected clean Ok, got {other:?}"),
        }

        // An untracked file makes it dirty.
        std::fs::write(repo.join("b.txt"), "new").unwrap();
        match gather_one(&repo) {
            RepoState::Ok { dirty, .. } => assert_eq!(dirty, 1),
            other => panic!("expected dirty Ok, got {other:?}"),
        }
    }

    #[test]
    fn fetch_then_gather_reports_behind() {
        let tmp = tempdir().unwrap();
        let bare = tmp.path().join("remote.git");
        git(
            tmp.path(),
            &["init", "-q", "--bare", bare.to_str().unwrap()],
        );

        // Clone A, push an initial commit on main.
        let a = tmp.path().join("a");
        git(
            tmp.path(),
            &["clone", "-q", bare.to_str().unwrap(), a.to_str().unwrap()],
        );
        std::fs::write(a.join("f.txt"), "one").unwrap();
        git(&a, &["add", "."]);
        git(&a, &["commit", "-qm", "one"]);
        git(&a, &["push", "-q", "-u", "origin", "HEAD:main"]);

        // Clone B (tracks origin/main at "one").
        let b = tmp.path().join("b");
        git(
            tmp.path(),
            &["clone", "-q", bare.to_str().unwrap(), b.to_str().unwrap()],
        );

        // A advances the remote by one commit.
        std::fs::write(a.join("f.txt"), "two").unwrap();
        git(&a, &["commit", "-qam", "two"]);
        git(&a, &["push", "-q"]);

        // Before fetch, B does not know it is behind.
        assert!(matches!(gather_one(&b), RepoState::Ok { behind: 0, .. }));

        // Our fetch updates the tracking ref; gather now reports behind = 1.
        super::fetch(&b).unwrap();
        match gather_one(&b) {
            RepoState::Ok { behind, ahead, .. } => {
                assert_eq!((ahead, behind), (0, 1));
            }
            other => panic!("expected Ok behind=1, got {other:?}"),
        }
    }

    #[test]
    fn summary_formats_states() {
        assert_eq!(RepoState::Missing.summary(), "(missing)");
        assert_eq!(
            RepoState::Ok {
                branch: "main".into(),
                ahead: 2,
                behind: 1,
                dirty: 3,
            }
            .summary(),
            "main +2 -1 *3"
        );
        assert_eq!(
            RepoState::Ok {
                branch: "dev".into(),
                ahead: 0,
                behind: 0,
                dirty: 0,
            }
            .summary(),
            "dev clean"
        );
    }
}
