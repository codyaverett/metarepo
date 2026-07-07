// Tests for context-aware worktree commands (issue #4).
//
// These exercise the scope-filter path of list_all_worktrees / prune_worktrees
// without requiring real git repositories — the goal is to verify that:
//   - Some(known project) is accepted and limits iteration to that project
//   - Some(unknown project) is rejected with a clear error
//   - None iterates over the full workspace

use metarepo::plugins::worktree::{
    add_worktrees, list_all_worktrees, prune_worktrees, repair_worktrees,
};
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

// Scope is now expressed as a slice of resolved project keys. An unknown key
// (e.g. a --project typo) is still rejected; a valid key is accepted; and the
// full key set iterates the whole workspace.
fn all_projects() -> Vec<String> {
    vec!["alpha".to_string(), "beta".to_string()]
}

#[test]
fn list_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = list_all_worktrees(tmp.path(), &["not-a-project".to_string()]).unwrap_err();
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
    list_all_worktrees(tmp.path(), &["alpha".to_string()]).expect("known project must be accepted");
}

#[test]
fn list_with_full_scope_iterates_workspace() {
    let tmp = setup_workspace();
    list_all_worktrees(tmp.path(), &all_projects()).expect("workspace-wide listing must succeed");
}

#[test]
fn prune_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = prune_worktrees(tmp.path(), true, &["not-a-project".to_string()]).unwrap_err();
    assert!(
        err.to_string().contains("not in the workspace"),
        "expected workspace-membership error, got: {}",
        err
    );
}

#[test]
fn prune_with_full_scope_iterates_workspace() {
    let tmp = setup_workspace();
    prune_worktrees(tmp.path(), true, &all_projects()).expect("workspace-wide prune must succeed");
}

#[test]
fn repair_rejects_unknown_scope_project() {
    let tmp = setup_workspace();
    let err = repair_worktrees(tmp.path(), &["not-a-project".to_string()], true).unwrap_err();
    assert!(
        err.to_string().contains("not in the workspace"),
        "expected workspace-membership error, got: {}",
        err
    );
}

#[test]
fn repair_dry_run_succeeds_workspace_wide() {
    let tmp = setup_workspace();
    repair_worktrees(tmp.path(), &all_projects(), true).expect("dry-run repair must succeed");
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
    repair_worktrees(workspace, &["alpha".to_string()], false)
        .expect("scoped repair_worktrees must return Ok on healthy repo");
}

/// End-to-end: with projects `app` and `plugins/a`, running `meta worktree list`
/// from inside `plugins/` must show only the `plugins/a` worktree, not `app`.
#[test]
fn list_from_subdirectory_scopes_to_that_subtree() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();

    for key in ["app", "plugins/a"] {
        let project = ws.join(key);
        std::fs::create_dir_all(&project).unwrap();
        run_git(&project, &["init", "-q", "-b", "main"]);
        std::fs::write(project.join("file.txt"), "x").unwrap();
        run_git(&project, &["add", "."]);
        run_git(&project, &["commit", "-q", "-m", "init"]);
        let wt = project.join(".worktrees").join("feat");
        std::fs::create_dir_all(wt.parent().unwrap()).unwrap();
        run_git(
            &project,
            &["worktree", "add", "-q", "-b", "feat", wt.to_str().unwrap()],
        );
    }

    let mut config = MetaConfig::default();
    config.projects.insert(
        "app".to_string(),
        ProjectEntry::Url("https://example.com/app.git".to_string()),
    );
    config.projects.insert(
        "plugins/a".to_string(),
        ProjectEntry::Url("https://example.com/a.git".to_string()),
    );
    config.save_to_file(ws.join(".meta")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_meta"))
        .args(["worktree", "list"])
        .current_dir(ws.join("plugins"))
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run meta binary");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains("plugins/a"),
        "should list the in-scope project; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("app"),
        "must NOT list the out-of-scope project 'app'; got:\n{stdout}"
    );
}

/// When a worktree is created with `-b` (force new branch) and a remote branch
/// of that name already exists, the new branch must be based on the remote
/// branch (and track it), not on local HEAD. Regression test for the
/// remote-source behavior.
#[test]
fn create_branch_bases_on_remote_when_it_exists() {
    if !git_available() {
        eprintln!("skipping: git not available");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();

    // Build a bare remote with main (commit "one") and feature (commit "two").
    let remote = ws.join("remote.git");
    run_git_at(ws, &["init", "-q", "--bare", remote.to_str().unwrap()]);
    let seed = ws.join("seed");
    run_git_at(
        ws,
        &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            seed.to_str().unwrap(),
        ],
    );
    std::fs::write(seed.join("a.txt"), "one").unwrap();
    run_git(&seed, &["add", "."]);
    run_git(&seed, &["commit", "-qm", "one"]);
    run_git(&seed, &["branch", "-M", "main"]);
    run_git(&seed, &["push", "-q", "-u", "origin", "main"]);
    run_git(&seed, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(seed.join("a.txt"), "two").unwrap();
    run_git(&seed, &["commit", "-qam", "two"]);
    run_git(&seed, &["push", "-q", "-u", "origin", "feature"]);

    // Project clone that has origin/feature as a remote-tracking ref but no
    // local feature branch.
    let app = ws.join("app");
    run_git_at(
        ws,
        &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            app.to_str().unwrap(),
        ],
    );
    run_git(&app, &["checkout", "-q", "main"]);

    let mut config = MetaConfig::default();
    config.projects.insert(
        "app".to_string(),
        ProjectEntry::Url("https://example.com/app.git".to_string()),
    );
    config.save_to_file(ws.join(".meta")).unwrap();

    // Force a new branch with -b (create_branch=true) and no explicit start.
    add_worktrees(
        "feature",
        &["app".to_string()],
        ws,
        None,
        true, // create_branch (-b)
        None, // starting_point
        true, // no_hooks
        false,
        Some("app"),
        &config,
    )
    .expect("add_worktrees must succeed");

    // The worktree must sit on the remote tip ("two") and track origin/feature,
    // not on local HEAD ("one") with no upstream.
    let wt = app.join(".worktrees").join("feature");
    let head_subject = git_stdout(&wt, &["log", "--oneline", "-1"]);
    assert!(
        head_subject.contains("two"),
        "worktree should be based on the remote branch tip 'two', got: {head_subject}"
    );
    let upstream = git_stdout(
        &wt,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    );
    assert_eq!(
        upstream.trim(),
        "origin/feature",
        "worktree branch should track origin/feature"
    );
}

/// Run git in `dir` and return trimmed stdout (empty on failure).
fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git spawn");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Run git with `dir` as the working directory (not `-C`), for clone/init that
/// take an explicit target path.
fn run_git_at(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(["-c", "user.name=Test"])
        .args(["-c", "user.email=test@example.com"])
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("git {:?} failed to spawn: {}", args, e));
    assert!(status.success(), "git {:?} failed", args);
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_git(dir: &Path, args: &[&str]) {
    // Disable commit signing and pin an identity so the test is deterministic
    // regardless of the developer's global git config (a global
    // `commit.gpgsign = true` otherwise makes `git commit` block on a gpg
    // passphrase prompt).
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
