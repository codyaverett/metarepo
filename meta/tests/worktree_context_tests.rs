// Tests for context-aware worktree commands (issue #4).
//
// These exercise the scope-filter path of list_all_worktrees / prune_worktrees
// without requiring real git repositories — the goal is to verify that:
//   - Some(known project) is accepted and limits iteration to that project
//   - Some(unknown project) is rejected with a clear error
//   - None iterates over the full workspace

use metarepo::plugins::worktree::{list_all_worktrees, prune_worktrees};
use metarepo_core::{MetaConfig, ProjectEntry};
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
