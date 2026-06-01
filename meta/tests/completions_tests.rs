// End-to-end tests for shell-completion installation via `meta init`.
//
// Completions are no longer a standalone command — `meta init --with-completions`
// installs them for the detected shell. These tests drive the real `meta` binary
// (via CARGO_BIN_EXE_meta) with an isolated per-process HOME and workspace, so
// they never touch the developer's real shell config. `ZSH` is removed from the
// environment so the zsh branch resolves oh-my-zsh under the temp HOME rather
// than inheriting the host's oh-my-zsh.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

const META_BIN: &str = env!("CARGO_BIN_EXE_meta");

struct Fixture {
    _home: TempDir,
    _ws: TempDir,
    home: PathBuf,
    ws: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        Fixture {
            home: home.path().to_path_buf(),
            ws: ws.path().to_path_buf(),
            _home: home,
            _ws: ws,
        }
    }

    /// Run `meta` with this fixture's HOME/workspace, a fixed zsh `$SHELL`, and a
    /// clean environment (no inherited `ZSH`/`XDG_CONFIG_HOME`).
    fn meta(&self, args: &[&str]) -> Output {
        Command::new(META_BIN)
            .args(args)
            .current_dir(&self.ws)
            .env("HOME", &self.home)
            .env("SHELL", "/bin/zsh")
            .env("NO_COLOR", "1")
            .env_remove("ZSH")
            .env_remove("XDG_CONFIG_HOME")
            .output()
            .expect("failed to spawn meta binary")
    }
}

fn assert_ok(out: &Output) {
    assert!(
        out.status.success(),
        "meta failed (status {:?})\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn init_with_completions_installs_for_oh_my_zsh() {
    let f = Fixture::new();
    // Simulate an oh-my-zsh install under the isolated HOME.
    fs::create_dir_all(f.home.join(".oh-my-zsh")).unwrap();

    let out = f.meta(&[
        "init",
        "--with-completions",
        "--non-interactive",
        "defaults",
    ]);
    assert_ok(&out);

    let script = f.home.join(".oh-my-zsh/completions/_meta");
    assert!(
        script.is_file(),
        "expected completion script at {}",
        script.display()
    );
    let contents = fs::read_to_string(&script).unwrap();
    assert!(
        !contents.is_empty(),
        "completion script should not be empty"
    );
    assert!(
        contents.contains("project") && contents.contains("worktree"),
        "completion script should reference known subcommands"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Installed zsh completions"),
        "init should report the completion install; got:\n{stdout}"
    );
}

#[test]
fn init_with_completions_falls_back_without_oh_my_zsh() {
    let f = Fixture::new();
    // No ~/.oh-my-zsh: should fall back to ~/.zsh/completions and print an
    // $fpath note.
    let out = f.meta(&[
        "init",
        "--with-completions",
        "--non-interactive",
        "defaults",
    ]);
    assert_ok(&out);

    let script = f.home.join(".zsh/completions/_meta");
    assert!(
        script.is_file(),
        "expected fallback completion script at {}",
        script.display()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fpath"),
        "fallback should print an fpath instruction; got:\n{stdout}"
    );
}

#[test]
fn plain_init_does_not_touch_home_completions() {
    let f = Fixture::new();
    fs::create_dir_all(f.home.join(".oh-my-zsh")).unwrap();

    // Non-interactive, no --with-completions: must not write any completion file.
    let out = f.meta(&["init", "--non-interactive", "defaults"]);
    assert_ok(&out);

    assert!(
        !f.home.join(".oh-my-zsh/completions/_meta").exists(),
        "plain init must not install completions"
    );
    assert!(
        !f.home.join(".zsh/completions/_meta").exists(),
        "plain init must not install completions"
    );
}
