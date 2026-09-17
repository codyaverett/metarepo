use anyhow::Result;
use git2::{BranchType, ErrorCode, Repository};
use std::path::Path;

pub fn get_git_status(repo_path: &Path) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let statuses = repo.statuses(None)?;

    if statuses.is_empty() {
        Ok("Clean working directory".to_string())
    } else {
        let mut status_lines = Vec::new();

        for entry in statuses.iter() {
            if let Ok(path) = entry.path() {
                let status = entry.status();
                let mut status_str = String::new();

                if status.is_wt_new() {
                    status_str.push('?');
                } else {
                    status_str.push(' ');
                }
                if status.is_wt_modified() {
                    status_str.push('M');
                } else {
                    status_str.push(' ');
                }
                if status.is_wt_deleted() {
                    status_str.push('D');
                } else {
                    status_str.push(' ');
                }
                if status.is_index_new() {
                    status_str.push('A');
                } else {
                    status_str.push(' ');
                }
                if status.is_index_modified() {
                    status_str.push('M');
                } else {
                    status_str.push(' ');
                }
                if status.is_index_deleted() {
                    status_str.push('D');
                } else {
                    status_str.push(' ');
                }

                status_lines.push(format!("{} {}", status_str, path));
            }
        }

        Ok(status_lines.join("\n"))
    }
}

/// Render the current branch of the repository at `repo_path` as a single line
/// for `meta git branch`.
///
/// Every state a repository can honestly be in gets a rendering instead of an
/// error: a detached HEAD reports its short SHA, a freshly initialized repo
/// reports its unborn branch, and a directory that is not a repository at all
/// says so. With `verbose`, a branch that tracks an upstream also carries its
/// ahead/behind counts; a branch without one is simply marked as such.
///
/// ponytail: bare projects report the bare repo's own HEAD rather than
/// expanding into their managed worktrees. Expand via list_worktrees if a
/// per-worktree branch listing is ever wanted.
pub fn get_branch_info(repo_path: &Path, verbose: bool) -> Result<String> {
    let repo = match Repository::open(repo_path) {
        Ok(repo) => repo,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok("(not a git repo)".to_string()),
        Err(e) => return Err(e.into()),
    };
    let bare = if repo.is_bare() { " (bare)" } else { "" };

    let head = match repo.head() {
        Ok(head) => head,
        // A repository with no commits yet: HEAD is a symbolic ref pointing at
        // a branch that does not exist. Report the branch it will become.
        Err(e) if e.code() == ErrorCode::UnbornBranch => {
            let name = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|r| r.symbolic_target().ok().flatten().map(|t| t.to_string()))
                .unwrap_or_else(|| "HEAD".to_string());
            let name = name
                .strip_prefix("refs/heads/")
                .unwrap_or(&name)
                .to_string();
            return Ok(format!("{} (no commits yet){}", name, bare));
        }
        Err(e) => return Err(e.into()),
    };

    if !head.is_branch() {
        let sha = head
            .target()
            .map(|oid| oid.to_string()[..7].to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Ok(format!("{} (detached HEAD){}", sha, bare));
    }

    let branch = head.shorthand().unwrap_or("HEAD").to_string();
    if !verbose {
        return Ok(format!("{}{}", branch, bare));
    }

    let upstream = repo
        .find_branch(&branch, BranchType::Local)
        .and_then(|b| b.upstream());
    let (Ok(upstream), Some(local)) = (upstream, head.target()) else {
        return Ok(format!("{}{} (no upstream)", branch, bare));
    };
    let Some(remote) = upstream.get().target() else {
        return Ok(format!("{}{} (no upstream)", branch, bare));
    };

    let (ahead, behind) = repo.graph_ahead_behind(local, remote)?;
    let name = upstream
        .name()
        .ok()
        .flatten()
        .unwrap_or("upstream")
        .to_string();
    Ok(format!(
        "{}{} (ahead {}, behind {} of {})",
        branch, bare, ahead, behind, name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, Signature};

    /// Initialize a repo with one empty commit and return (tempdir, commit oid).
    fn repo_with_commit() -> (tempfile::TempDir, Oid) {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = Repository::init(dir.path()).expect("init");
        let sig = Signature::now("Test", "test@example.com").expect("signature");
        let tree_oid = repo
            .treebuilder(None)
            .expect("treebuilder")
            .write()
            .unwrap();
        let tree = repo.find_tree(tree_oid).expect("tree");
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .expect("commit");
        (dir, oid)
    }

    #[test]
    fn detached_head_renders_short_sha() {
        let (dir, oid) = repo_with_commit();
        let repo = Repository::open(dir.path()).expect("open");
        repo.set_head_detached(oid).expect("detach");

        let info = get_branch_info(dir.path(), false).expect("branch info");
        assert!(
            info.starts_with(&oid.to_string()[..7]),
            "expected short sha prefix, got {:?}",
            info
        );
        assert!(info.contains("detached"), "got {:?}", info);
    }

    #[test]
    fn branch_without_upstream_renders_cleanly_in_verbose() {
        let (dir, _) = repo_with_commit();
        let info = get_branch_info(dir.path(), true).expect("branch info");
        assert!(info.contains("no upstream"), "got {:?}", info);
        assert!(!info.contains("ahead"), "got {:?}", info);
        // Plain mode is just the branch name (master or main, depending on the
        // host's init.defaultBranch), with no annotation.
        let plain = get_branch_info(dir.path(), false).unwrap();
        assert!(!plain.is_empty() && !plain.contains('('), "got {:?}", plain);
    }

    #[test]
    fn verbose_reports_ahead_behind_against_upstream() {
        let (origin, _) = repo_with_commit();
        let dest = tempfile::tempdir().expect("tempdir");
        let clone_path = dest.path().join("clone");
        let repo = Repository::clone(origin.path().to_str().unwrap(), &clone_path).expect("clone");

        // One local commit on top of the cloned upstream.
        let sig = Signature::now("Test", "test@example.com").expect("signature");
        let head = repo.head().expect("head").peel_to_commit().expect("commit");
        let tree = head.tree().expect("tree");
        repo.commit(Some("HEAD"), &sig, &sig, "local", &tree, &[&head])
            .expect("commit");

        let info = get_branch_info(&clone_path, true).expect("branch info");
        assert!(
            info.contains("ahead 1, behind 0"),
            "expected ahead/behind counts, got {:?}",
            info
        );
        assert!(info.contains("origin/"), "got {:?}", info);
    }

    #[test]
    fn unborn_branch_and_non_repo_do_not_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        Repository::init(dir.path()).expect("init");
        let info = get_branch_info(dir.path(), true).expect("branch info");
        assert!(info.contains("no commits yet"), "got {:?}", info);

        let plain = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            get_branch_info(plain.path(), false).unwrap(),
            "(not a git repo)"
        );
    }
}
