//! Integration tests for the bundled meta-tool skill install location (#76).
//!
//! `meta skill install` installs to `<workspace>/.claude/skills/meta-tool` by
//! default, but honors the `[skill] dest` config key so users on a different
//! skills home can relocate it (installing to `<dest>/meta-tool`, matching where
//! stolen skills land).

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Run `meta <args>` in `dir` and return combined success + stdout.
fn run_meta(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_meta"))
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run meta binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

#[test]
fn install_uses_default_location_without_dest() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".meta"), r#"{"projects":{}}"#).unwrap();

    let (ok, _out) = run_meta(tmp.path(), &["skill", "install"]);
    assert!(ok, "install should succeed");
    assert!(
        tmp.path()
            .join(".claude/skills/meta-tool/SKILL.md")
            .exists(),
        "skill should install at the default .claude/skills/meta-tool"
    );
}

#[test]
fn install_honors_skill_dest_override() {
    let tmp = TempDir::new().unwrap();
    // Point the skills home at a custom directory under the workspace.
    fs::write(
        tmp.path().join(".meta"),
        r#"{"projects":{},"skill":{"dest":"./custom-skills"}}"#,
    )
    .unwrap();

    let (ok, out) = run_meta(tmp.path(), &["skill", "install"]);
    assert!(ok, "install should succeed");

    // Installs to <dest>/meta-tool, not the default .claude/skills path.
    assert!(
        tmp.path().join("custom-skills/meta-tool/SKILL.md").exists(),
        "skill should install under the configured dest; stdout:\n{out}"
    );
    assert!(
        !tmp.path().join(".claude/skills/meta-tool").exists(),
        "skill must NOT install at the default location when dest is set"
    );

    // status and remove operate on the same custom location.
    let (ok, status) = run_meta(tmp.path(), &["skill", "status"]);
    assert!(ok && status.contains("up to date"), "status: {status}");

    let (ok, _) = run_meta(tmp.path(), &["skill", "remove"]);
    assert!(ok, "remove should succeed");
    assert!(
        !tmp.path().join("custom-skills/meta-tool").exists(),
        "remove should delete the skill at the configured dest"
    );
}
