// Integration tests for shallow-history re-truncation (`meta git pull --shallow`).
//
// Demonstrates the behavior gap the flag closes: a plain `git pull` on a
// shallow clone accumulates every new upstream commit, while
// `refetch_shallow` + pull keeps history at the configured depth. Uses local
// repositories only (no network); `refetch_shallow` shells out to the git
// CLI, which supports shallow fetches over the local transport.

use metarepo::plugins::shared::refetch_shallow;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit_count(dir: &Path) -> usize {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("git rev-list must run");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}

fn add_commit(repo: &Path, n: usize) {
    std::fs::write(repo.join("file.txt"), format!("commit {}", n)).unwrap();
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-q", "-m", &format!("commit {}", n)]);
}

/// Source repo with one initial commit, plus a depth-1 clone of it.
fn setup_shallow_pair() -> (TempDir, TempDir) {
    let source = TempDir::new().unwrap();
    run_git(source.path(), &["init", "-q", "-b", "main"]);
    run_git(source.path(), &["config", "user.email", "test@example.com"]);
    run_git(source.path(), &["config", "user.name", "Test"]);
    add_commit(source.path(), 1);

    let clone_parent = TempDir::new().unwrap();
    let clone_path = clone_parent.path().join("clone");
    let output = Command::new("git")
        .args(["clone", "-q", "--depth", "1"])
        .arg(source.path())
        .arg(&clone_path)
        .output()
        .expect("git clone must run");
    assert!(
        output.status.success(),
        "shallow clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(commit_count(&clone_path), 1);

    (source, clone_parent)
}

/// Baseline: a plain pull on a shallow clone accumulates upstream commits —
/// this is the behavior --shallow exists to avoid.
#[test]
fn plain_pull_accumulates_history_on_shallow_clone() {
    let (source, clone_parent) = setup_shallow_pair();
    let clone_path = clone_parent.path().join("clone");

    add_commit(source.path(), 2);
    add_commit(source.path(), 3);

    run_git(&clone_path, &["pull", "-q"]);
    assert_eq!(
        commit_count(&clone_path),
        3,
        "plain pull on a shallow clone must accumulate all new commits"
    );
}

/// refetch_shallow before the pull keeps history at the configured depth.
#[test]
fn refetch_shallow_then_pull_keeps_history_at_depth() {
    let (source, clone_parent) = setup_shallow_pair();
    let clone_path = clone_parent.path().join("clone");

    add_commit(source.path(), 2);
    add_commit(source.path(), 3);

    refetch_shallow(&clone_path, 1).expect("refetch_shallow must succeed");
    run_git(&clone_path, &["pull", "-q"]);

    assert_eq!(
        commit_count(&clone_path),
        1,
        "after refetch_shallow the pulled history must stay at depth 1"
    );
    assert_eq!(
        std::fs::read_to_string(clone_path.join("file.txt")).unwrap(),
        "commit 3",
        "the working tree must still fast-forward to the latest commit"
    );
}

#[test]
fn refetch_shallow_rejects_non_positive_depth() {
    let dir = TempDir::new().unwrap();
    for bad in [0, -1] {
        let err = refetch_shallow(dir.path(), bad).unwrap_err();
        assert!(
            err.to_string().contains("positive integer"),
            "depth {} must be rejected with a clear error, got: {}",
            bad,
            err
        );
    }
}
