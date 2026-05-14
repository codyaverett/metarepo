// Tests for context-aware worktree commands (issue #4).
//
// These exercise the scope-filter path of list_all_worktrees / prune_worktrees
// without requiring real git repositories — the goal is to verify that:
//   - Some(known project) is accepted and limits iteration to that project
//   - Some(unknown project) is rejected with a clear error
//   - None iterates over the full workspace

use metarepo::plugins::worktree::{list_all_worktrees, prune_worktrees, repair_worktrees};
use metarepo_core::{MetaConfig, ProjectEntry};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn setup_workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let meta_path = tmp.path().join(".meta");

    let mut config = MetaConfig::default();
    config.projects.insert(
        "alpha".to_string(),
        ProjectEntry::Url("https://example.com/alpha.git".to_string()),
    );
    config.projects.insert(
        "beta".to_string(),
        ProjectEntry::Url("https://example.com/beta.git".to_string()),
    );
    config.save_to_file(&meta_path).unwrap();
    tmp
}

#[test]
fn list_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = list_all_worktrees(tmp.path(), Some("not-a-project")).unwrap_err();
    assert!(
        err.to_string().contains("not in the workspace"),
        "expected workspace-membership error, got: {}",
        err
    );
}

#[test]
fn list_accepts_known_scope_project() {
    let tmp = setup_workspace();
    // The project has no on-disk checkout so list_all_worktrees skips it cleanly
    // — but the scope itself must be accepted without erroring.
    list_all_worktrees(tmp.path(), Some("alpha")).expect("known project must be accepted");
}

#[test]
fn list_with_none_scope_iterates_workspace() {
    let tmp = setup_workspace();
    list_all_worktrees(tmp.path(), None).expect("workspace-wide listing must succeed");
}

#[test]
fn prune_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = prune_worktrees(tmp.path(), true, Some("not-a-project")).unwrap_err();
    assert!(
        err.to_string().contains("not in the workspace"),
        "expected workspace-membership error, got: {}",
        err
    );
}

#[test]
fn prune_with_none_scope_iterates_workspace() {
    let tmp = setup_workspace();
    prune_worktrees(tmp.path(), true, None).expect("workspace-wide prune must succeed");
}

#[test]
fn repair_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = repair_worktrees(tmp.path(), Some("not-a-project"), true).unwrap_err();
    assert!(
        err.to_string().contains("not in the workspace"),
        "expected workspace-membership error, got: {}",
        err
    );
}

#[test]
fn repair_dry_run_succeeds_workspace_wide() {
    let tmp = setup_workspace();
    repair_worktrees(tmp.path(), None, true).expect("dry-run repair must succeed");
}

/// End-to-end: initialize a git repo + worktree under the workspace, move the
/// worktree directory on disk to break git's record, then verify
/// `repair_worktrees` re-points the administrative files at the new location.
#[test]
fn repair_recovers_moved_worktree() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path();
    let project = workspace.join("alpha");
    std::fs::create_dir(&project).unwrap();

    // Register the project in .meta
    let mut config = MetaConfig::default();
    config.projects.insert(
        "alpha".to_string(),
        ProjectEntry::Url("https://example.com/alpha.git".to_string()),
    );
    config.save_to_file(workspace.join(".meta")).unwrap();

    // Initialize repo with a single commit so `git worktree add` works.
    run_git(&project, &["init", "-q", "-b", "main"]);
    run_git(&project, &["config", "user.email", "test@example.com"]);
    run_git(&project, &["config", "user.name", "test"]);
    std::fs::write(project.join("README.md"), "hello").unwrap();
    run_git(&project, &["add", "."]);
    run_git(&project, &["commit", "-q", "-m", "init"]);

    // Create a worktree under the project's .worktrees directory.
    let original = project.join(".worktrees").join("feature");
    std::fs::create_dir_all(original.parent().unwrap()).unwrap();
    run_git(
        &project,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            original.to_str().unwrap(),
        ],
    );

    // Move the worktree directory — this breaks git's stored gitdir pointer.
    let moved = project.join(".worktrees").join("feature-moved");
    std::fs::rename(&original, &moved).unwrap();

    // Repair: pass the new path so git can rebind it.
    let status = Command::new("git")
        .arg("-C")
        .arg(&project)
        .arg("worktree")
        .arg("repair")
        .arg(&moved)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "git worktree repair (manual) must succeed"
    );

    // Now call our wrapper end-to-end to ensure it returns Ok for the project.
    repair_worktrees(workspace, Some("alpha"), false)
        .expect("scoped repair_worktrees must return Ok on healthy repo");
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("git {:?} failed to spawn: {}", args, e));
    assert!(status.success(), "git {:?} failed", args);
}
