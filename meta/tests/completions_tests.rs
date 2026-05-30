// End-to-end tests for `meta completions <shell>`.
//
// These drive the real `meta` binary (via CARGO_BIN_EXE_meta) so they exercise
// the full clap tree build that `clap_complete` performs — the path that
// previously tripped clap debug assertions for subcommands lacking a version
// string. Each shell must produce a non-empty script on stdout and exit 0.

use std::process::{Command, Output};

const META_BIN: &str = env!("CARGO_BIN_EXE_meta");

fn run(args: &[&str]) -> Output {
    Command::new(META_BIN)
        .args(args)
        .output()
        .expect("failed to run meta binary")
}

#[test]
fn generates_completions_for_every_supported_shell() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let out = run(&["completions", shell]);
        assert!(
            out.status.success(),
            "`meta completions {shell}` exited with failure: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !out.stdout.is_empty(),
            "`meta completions {shell}` produced an empty script"
        );
    }
}

#[test]
fn completion_script_includes_known_subcommands() {
    let out = run(&["completions", "bash"]);
    let script = String::from_utf8_lossy(&out.stdout);
    for cmd in ["project", "worktree", "completions"] {
        assert!(
            script.contains(cmd),
            "bash completion script should reference the `{cmd}` subcommand"
        );
    }
}

#[test]
fn completions_output_is_stable_with_experimental_flag() {
    // An installed completion script must not depend on whether `-x` was passed.
    let stable = run(&["completions", "bash"]);
    let experimental = run(&["-x", "completions", "bash"]);
    assert!(stable.status.success() && experimental.status.success());
    assert_eq!(
        stable.stdout, experimental.stdout,
        "completion script differed between stable and experimental invocations"
    );
}

#[test]
fn rejects_unknown_shell() {
    let out = run(&["completions", "notashell"]);
    assert!(
        !out.status.success(),
        "an unknown shell should be rejected with a non-zero exit"
    );
}
