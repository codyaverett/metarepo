// Integration test for the `--bare --depth N` combination in `meta project add`.
//
// This exercises the highest-risk interaction flagged in review: cloning a
// remote as a shallow bare repo and then running `git worktree add` against
// it. It uses a local file:// remote (no network required) so it runs in CI,
// but drives the exact same code path (`clone_with_auth` +
// `create_default_worktree` + `detect_default_branch`) used by
// `meta project add --bare --depth N`.

use metarepo::plugins::shared::{clone_with_auth, create_default_worktree, detect_default_branch};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;
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

/// Create a small local source repository with a few commits on `main`, so a
/// shallow clone (`--depth 1`) is meaningfully different from a full clone.
fn setup_source_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();

    run_git(repo, &["init", "-q", "-b", "main"]);
    run_git(repo, &["config", "user.email", "test@example.com"]);
    run_git(repo, &["config", "user.name", "Test"]);

    for i in 1..=3 {
        std::fs::write(repo.join("file.txt"), format!("commit {}", i)).unwrap();
        run_git(repo, &["add", "."]);
        run_git(repo, &["commit", "-q", "-m", &format!("commit {}", i)]);
    }

    tmp
}

/// Wraps a `git daemon` child process serving `base_dir` over the `git://`
/// protocol, killed automatically on drop.
///
/// `clone_with_auth` uses libgit2's shallow fetch support, which (as
/// discovered while writing this test) is rejected by libgit2's *local*
/// (file://) transport with "shallow fetch is not supported by the local
/// transport". Real remotes are accessed over a network transport
/// (http(s)/ssh), so a `git daemon` (git://) server gives an honest,
/// network-transport exercise of the shallow-clone path without requiring
/// external network access in CI.
struct GitDaemon {
    child: Child,
    port: u16,
}

impl GitDaemon {
    fn start(base_dir: &Path) -> Self {
        // Reserve a free port, then hand it to `git daemon`. There is a small
        // race between releasing the listener and `git daemon` binding it,
        // but that's acceptable for a local test.
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };

        let child = Command::new("git")
            .arg("daemon")
            .arg("--reuseaddr")
            .arg("--export-all")
            .arg(format!("--base-path={}", base_dir.display()))
            .arg(format!("--port={}", port))
            .arg("--listen=127.0.0.1")
            .arg(base_dir)
            // Do not inherit this test's stdout/stderr: `git daemon` forks a
            // long-lived process that would otherwise keep the harness's
            // output pipe open after the test binary itself exits, hanging
            // any `cargo test | ...` pipeline.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to start git daemon");

        let daemon = GitDaemon { child, port };
        daemon.wait_until_ready();
        daemon
    }

    /// Poll for the daemon's listening socket to accept connections. A plain
    /// TCP probe (rather than an `ls-remote` against a specific repo) keeps
    /// this decoupled from any particular repo name under `base_dir`.
    fn wait_until_ready(&self) {
        for _ in 0..50 {
            if std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("git daemon did not become ready on port {}", self.port);
    }

    fn url(&self, repo_name: &str) -> String {
        format!("git://127.0.0.1:{}/{}", self.port, repo_name)
    }
}

impl Drop for GitDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `meta project add --bare --depth 1` against a real remote: clone as a
/// shallow bare repo, then create the default worktree from it. Verifies
/// that `detect_default_branch` resolves HEAD and `git worktree add`
/// succeeds against a depth-limited bare repo.
#[test]
fn bare_and_shallow_clone_combination_supports_default_worktree() {
    let source = setup_source_repo();
    std::fs::write(source.path().join("git-daemon-export-ok"), "").unwrap();

    // Serve the source repo over git:// (a real network transport, unlike
    // file://) so the shallow-fetch path libgit2 uses for actual remotes is
    // exercised.
    let daemon_base = source.path().parent().unwrap();
    let daemon = GitDaemon::start(daemon_base);
    let repo_name = source.path().file_name().unwrap().to_str().unwrap();
    let source_url = daemon.url(repo_name);

    let workspace = TempDir::new().unwrap();
    let project_path = workspace.path().join("project");
    let bare_path = project_path.join(".git");

    std::fs::create_dir_all(&project_path).unwrap();

    // Shallow bare clone, mirroring `meta project add --bare --depth 1`.
    clone_with_auth(&source_url, &bare_path, true, Some(1))
        .expect("shallow bare clone must succeed");

    // Shallow clones via git2 do not always populate refs/remotes/origin/HEAD,
    // so mirror the layout `meta project add` relies on and confirm
    // detect_default_branch still resolves a usable branch name.
    let default_branch = detect_default_branch(&bare_path).expect("must detect default branch");
    assert!(
        !default_branch.is_empty(),
        "default branch detection must return a non-empty branch name on a shallow bare repo"
    );

    // `git worktree add` against the depth-limited bare repo must succeed.
    create_default_worktree(&bare_path, &project_path)
        .expect("worktree add must succeed on a shallow bare repo");

    let worktree_path = project_path.join(&default_branch);
    assert!(
        worktree_path.join("file.txt").exists(),
        "checked-out worktree must contain the tracked file"
    );
    assert_eq!(
        std::fs::read_to_string(worktree_path.join("file.txt")).unwrap(),
        "commit 3",
        "worktree must check out the latest commit from the shallow clone"
    );

    // Confirm the clone really is shallow (depth 1): only one commit reachable.
    let log_output = Command::new("git")
        .arg("-C")
        .arg(&worktree_path)
        .args(["log", "--oneline"])
        .output()
        .expect("git log must run");
    assert!(log_output.status.success());
    let commit_count = String::from_utf8_lossy(&log_output.stdout).lines().count();
    assert_eq!(
        commit_count, 1,
        "shallow clone with depth 1 must expose exactly one commit"
    );
}
