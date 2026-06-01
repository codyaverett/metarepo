// End-to-end tests for `meta worktree clean` (clean_worktrees).
//
// These build real git repositories under an isolated workspace and call
// clean_worktrees directly with assume_yes, so no interactive prompt is needed.
// They verify the safety contract: merged worktrees are removed (and their
// branches deleted), while unmerged and dirty worktrees are left untouched.

use metarepo::plugins::worktree::{clean_worktrees, CleanOptions};
use metarepo_core::{MetaConfig, NonInteractiveMode, ProjectEntry};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// A workspace containing a single git project "alpha" on branch `main`.
struct Workspace {
    _tmp: TempDir,
    root: PathBuf,
    project: PathBuf,
}

fn setup() -> Workspace {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let project = root.join("alpha");
    std::fs::create_dir(&project).unwrap();

    let mut config = MetaConfig::default();
    config.projects.insert(
        "alpha".to_string(),
        ProjectEntry::Url("https://example.com/alpha.git".to_string()),
    );
    config.save_to_file(root.join(".meta")).unwrap();

    run_git(&project, &["init", "-q", "-b", "main"]);
    std::fs::write(project.join("README.md"), "hello").unwrap();
    run_git(&project, &["add", "."]);
    run_git(&project, &["commit", "-q", "-m", "init"]);

    Workspace {
        _tmp: tmp,
        root,
        project,
    }
}

/// Add a worktree on a new branch, then add a commit on that branch inside the
/// worktree. Returns the worktree path.
fn add_worktree_with_commit(project: &Path, branch: &str) -> PathBuf {
    let path = project.join(".worktrees").join(branch);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    run_git(
        project,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            path.to_str().unwrap(),
        ],
    );
    std::fs::write(path.join("work.txt"), branch).unwrap();
    run_git(&path, &["add", "."]);
    run_git(&path, &["commit", "-q", "-m", "work"]);
    path
}

fn opts(dry_run: bool) -> CleanOptions {
    CleanOptions {
        dry_run,
        assume_yes: true,
        keep_branches: false,
    }
}

#[test]
fn merged_worktree_is_removed_and_branch_deleted() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let ws = setup();
    let wt = add_worktree_with_commit(&ws.project, "feature");
    // Fast-forward main to include feature, making it fully merged.
    run_git(&ws.project, &["merge", "-q", "feature"]);

    clean_worktrees(
        &ws.root,
        &["alpha".to_string()],
        opts(false),
        NonInteractiveMode::Defaults,
    )
    .expect("clean must succeed");

    assert!(!wt.exists(), "merged worktree should have been removed");
    assert!(
        !branch_exists(&ws.project, "feature"),
        "merged branch should have been deleted"
    );
}

#[test]
fn unmerged_worktree_is_kept() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let ws = setup();
    let wt = add_worktree_with_commit(&ws.project, "wip");
    // Do NOT merge: `wip` has a unique commit, so it must be preserved.

    clean_worktrees(
        &ws.root,
        &["alpha".to_string()],
        opts(false),
        NonInteractiveMode::Defaults,
    )
    .expect("clean must succeed");

    assert!(wt.exists(), "unmerged worktree must be kept");
    assert!(
        branch_exists(&ws.project, "wip"),
        "unmerged branch must be kept"
    );
}

#[test]
fn dirty_merged_worktree_is_skipped() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let ws = setup();
    let wt = add_worktree_with_commit(&ws.project, "feature");
    run_git(&ws.project, &["merge", "-q", "feature"]);
    // Introduce an uncommitted (untracked) file — the worktree is now dirty.
    std::fs::write(wt.join("scratch.txt"), "uncommitted").unwrap();

    clean_worktrees(
        &ws.root,
        &["alpha".to_string()],
        opts(false),
        NonInteractiveMode::Defaults,
    )
    .expect("clean must succeed");

    assert!(
        wt.exists(),
        "a merged-but-dirty worktree must be skipped, not removed"
    );
}

#[test]
fn dry_run_removes_nothing() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }
    let ws = setup();
    let wt = add_worktree_with_commit(&ws.project, "feature");
    run_git(&ws.project, &["merge", "-q", "feature"]);

    clean_worktrees(
        &ws.root,
        &["alpha".to_string()],
        opts(true), // dry-run
        NonInteractiveMode::Defaults,
    )
    .expect("dry-run clean must succeed");

    assert!(wt.exists(), "dry-run must not remove worktrees");
    assert!(
        branch_exists(&ws.project, "feature"),
        "dry-run must not delete branches"
    );
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn branch_exists(repo: &Path, name: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", name),
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(["-c", "user.name=Test"])
        .args(["-c", "user.email=test@example.com"])
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("git {:?} failed to spawn: {}", args, e));
    assert!(status.success(), "git {:?} failed", args);
}
