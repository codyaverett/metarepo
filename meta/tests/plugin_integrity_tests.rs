// End-to-end tests for plugin version pinning + checksum integrity (issue #25).
//
// These drive the real `meta` binary with a per-process HOME and a throwaway
// workspace so each test is isolated and parallel-safe. They cover:
//   - a `.metarepo.lock` entry is recorded on install,
//   - a plugin loads normally under `plugins-integrity = "required"`,
//   - a binary tampered after install is refused under integrity,
//   - a version pin that the plugin does not satisfy refuses to load,
//   - `--allow-version-mismatch` downgrades that refusal to a warning.

use std::fs;
use std::path::{Path, PathBuf};
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
        let f = Fixture {
            home: home.path().to_path_buf(),
            ws: ws.path().to_path_buf(),
            _home: home,
            _ws: ws,
        };
        let out = f.meta(&["init", "--non-interactive", "defaults"]);
        assert!(
            out.status.success(),
            "meta init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        f
    }

    fn meta(&self, args: &[&str]) -> Output {
        Command::new(META_BIN)
            .args(args)
            .current_dir(&self.ws)
            .env("HOME", &self.home)
            .env("NO_COLOR", "1")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("METAREPO_ALLOW_VERSION_MISMATCH")
            .output()
            .expect("failed to spawn meta binary")
    }

    fn config_path(&self) -> PathBuf {
        self.ws.join(".metarepo")
    }

    fn installed_path(&self, segment: &str) -> PathBuf {
        self.home
            .join(".config")
            .join("metarepo")
            .join("plugins")
            .join(segment)
    }

    /// Turn on checksum integrity. The JSON config already carries a
    /// `"plugins-integrity": null` field, so flip its value rather than
    /// inserting a second key (serde rejects duplicate fields).
    fn enable_integrity(&self) {
        let path = self.config_path();
        let s = fs::read_to_string(&path).unwrap();
        let replaced = s.replace(
            "\"plugins-integrity\": null",
            "\"plugins-integrity\": \"required\"",
        );
        assert_ne!(replaced, s, "expected a plugins-integrity field to flip");
        fs::write(&path, replaced).unwrap();
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn ok(out: Output) -> Output {
    assert!(
        out.status.success(),
        "command failed (status {:?})\nstdout: {}\nstderr: {}",
        out.status.code(),
        stdout(&out),
        stderr(&out)
    );
    out
}

/// Name of the greet plugin's script: a bash script on unix, a batch file on
/// Windows (which cannot execute `.sh` files as processes).
const GREET_SCRIPT: &str = if cfg!(windows) {
    "greet.cmd"
} else {
    "greet.sh"
};

#[cfg(not(windows))]
fn greet_script_body() -> String {
    r#"#!/usr/bin/env bash
set -euo pipefail
sub="${1:-}"
case "$sub" in
  hello)
    echo "Hello, ${METAREPO_ARG_NAME:-world}!"
    ;;
  *)
    echo "usage: greet hello <name>" >&2
    exit 1
    ;;
esac
"#
    .to_string()
}

#[cfg(windows)]
fn greet_script_body() -> String {
    // cmd.exe is unreliable with bare-LF batch files; emit CRLF.
    [
        "@echo off",
        "if \"%1\"==\"hello\" goto hello",
        "echo usage: greet hello NAME 1>&2",
        "exit /b 1",
        ":hello",
        "if defined METAREPO_ARG_NAME (echo Hello, %METAREPO_ARG_NAME%!) else (echo Hello, world!)",
        "exit /b 0",
        "",
    ]
    .join("\r\n")
}

/// Write a minimal shell manifest plugin (the `greet` example) into `dir`.
fn write_manifest_plugin(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("plugin.manifest.toml"),
        format!(
            r#"[plugin]
name = "greet"
version = "0.1.0"
description = "test manifest plugin"

[[commands]]
name = "hello"
description = "Print a greeting"

[[commands.args]]
name = "name"
help = "Who to greet"
required = true
takes_value = true

[config.execution]
binary = "./{GREET_SCRIPT}"
"#
        ),
    )
    .unwrap();
    fs::write(dir.join(GREET_SCRIPT), greet_script_body()).unwrap();
}

fn install_greet(f: &Fixture) {
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));
}

#[test]
fn install_records_lockfile_entry() {
    let f = Fixture::new();
    install_greet(&f);

    let lock = f.ws.join(".metarepo.lock");
    assert!(lock.exists(), ".metarepo.lock should be created on install");
    let body = fs::read_to_string(&lock).unwrap();
    assert!(body.contains("greet"), "lockfile should list greet: {body}");
    assert!(
        body.contains("sha256"),
        "lockfile should record a sha256: {body}"
    );
    assert!(
        body.contains("0.1.0"),
        "lockfile should record the version: {body}"
    );
}

#[test]
fn untampered_plugin_loads_under_integrity() {
    let f = Fixture::new();
    install_greet(&f);
    f.enable_integrity();

    let out = ok(f.meta(&["greet", "hello", "Ada"]));
    assert!(
        stdout(&out).contains("Hello, Ada!"),
        "plugin should still run when its checksum matches: {}",
        stdout(&out)
    );
}

#[test]
fn tampered_binary_is_refused_under_integrity() {
    let f = Fixture::new();
    install_greet(&f);
    f.enable_integrity();

    // Mutate the installed binary after install (append a harmless comment so
    // the bytes — and thus the digest — change without breaking the script).
    let script = f.installed_path(&format!("greet/{GREET_SCRIPT}"));
    let mut contents = fs::read_to_string(&script).unwrap();
    contents.push_str("\n# tampered\n");
    fs::write(&script, contents).unwrap();

    let out = f.meta(&["greet", "hello", "Ada"]);
    assert!(
        !out.status.success(),
        "tampered plugin must not run under integrity"
    );
    assert!(
        stderr(&out).contains("checksum mismatch"),
        "expected a checksum mismatch error, got stderr: {}",
        stderr(&out)
    );
}

#[test]
fn verify_passes_for_untampered_plugin() {
    let f = Fixture::new();
    install_greet(&f);

    let out = ok(f.meta(&["plugin", "verify"]));
    let s = stdout(&out);
    assert!(s.contains("greet"), "verify should mention greet: {s}");
    assert!(
        s.contains("matches") || s.contains("verified"),
        "verify should report success: {s}"
    );
}

#[test]
fn verify_fails_after_tamper() {
    let f = Fixture::new();
    install_greet(&f);

    let script = f.installed_path(&format!("greet/{GREET_SCRIPT}"));
    let mut contents = fs::read_to_string(&script).unwrap();
    contents.push_str("\n# tampered\n");
    fs::write(&script, contents).unwrap();

    let out = f.meta(&["plugin", "verify"]);
    assert!(
        !out.status.success(),
        "verify must exit non-zero when a checksum does not match"
    );
    assert!(
        stdout(&out).contains("MISMATCH"),
        "verify should flag the mismatch: {}",
        stdout(&out)
    );
}

#[test]
fn list_flags_tampered_plugin_even_without_integrity_mode() {
    let f = Fixture::new();
    install_greet(&f);

    let script = f.installed_path(&format!("greet/{GREET_SCRIPT}"));
    let mut contents = fs::read_to_string(&script).unwrap();
    contents.push_str("\n# tampered\n");
    fs::write(&script, contents).unwrap();

    // No `enable_integrity()`: a mismatch must still be surfaced in list.
    let out = ok(f.meta(&["plugin", "list"]));
    assert!(
        stdout(&out).contains("MISMATCH"),
        "list should warn about the tampered plugin: {}",
        stdout(&out)
    );
}

#[test]
fn list_shows_integrity_ok_when_required() {
    let f = Fixture::new();
    install_greet(&f);
    f.enable_integrity();

    let out = ok(f.meta(&["plugin", "list"]));
    assert!(
        stdout(&out).contains("integrity: ok"),
        "list should confirm integrity under required mode: {}",
        stdout(&out)
    );
}

/// Write an executable script at `base` that speaks the plugin protocol and
/// reports name `fake` at version `reports`. On Windows the script is a `.cmd`
/// batch file (bash scripts cannot be spawned there); the actual path written
/// is returned.
fn write_protocol_script(base: &Path, reports: &str) -> PathBuf {
    let path = if cfg!(windows) {
        base.with_extension("cmd")
    } else {
        base.to_path_buf()
    };
    fs::write(&path, protocol_script_body(reports)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

#[cfg(not(windows))]
fn protocol_script_body(reports: &str) -> String {
    format!(
        r#"#!/usr/bin/env bash
while IFS= read -r line; do
  case "$line" in
    *GetInfo*)
      printf '%s\n' '{{"type":"Info","name":"fake","version":"{reports}","experimental":false,"protocol_version":"1.0"}}'
      ;;
    *RegisterCommands*)
      printf '%s\n' '{{"type":"Commands","commands":[{{"name":"fake","about":"fake plugin","subcommands":[],"args":[]}}]}}'
      ;;
    *GetSettings*)
      printf '%s\n' '{{"type":"Settings","settings":[]}}'
      ;;
    *HandleCommand*)
      printf '%s\n' '{{"type":"Success","message":"fake ran"}}'
      ;;
    *)
      printf '%s\n' '{{"type":"Error","message":"unknown request"}}'
      ;;
  esac
done
"#
    )
}

#[cfg(windows)]
fn protocol_script_body(reports: &str) -> String {
    // Batch read-loop over stdin: one JSON request per line, one JSON response
    // per line. CRLF endings — cmd.exe is unreliable with bare-LF batch files.
    // The request is matched by writing it to a scratch file and running
    // findstr over that file: piping `echo !line!` into findstr would not work,
    // because each side of a cmd.exe pipe runs in a child without the parent's
    // variable context.
    [
        "@echo off",
        "setlocal",
        ":loop",
        "set \"line=\"",
        "set /p \"line=\"",
        "if not defined line exit /b 0",
        ">\"%~dp0req.txt\" echo(%line%",
        "findstr /c:\"GetInfo\" \"%~dp0req.txt\" >nul",
        "if not errorlevel 1 goto info",
        "findstr /c:\"RegisterCommands\" \"%~dp0req.txt\" >nul",
        "if not errorlevel 1 goto commands",
        "findstr /c:\"GetSettings\" \"%~dp0req.txt\" >nul",
        "if not errorlevel 1 goto settings",
        "findstr /c:\"HandleCommand\" \"%~dp0req.txt\" >nul",
        "if not errorlevel 1 goto handle",
        "echo {\"type\":\"Error\",\"message\":\"unknown request\"}",
        "goto loop",
        ":info",
        "echo {\"type\":\"Info\",\"name\":\"fake\",\"version\":\"@VERSION@\",\"experimental\":false,\"protocol_version\":\"1.0\"}",
        "goto loop",
        ":commands",
        "echo {\"type\":\"Commands\",\"commands\":[{\"name\":\"fake\",\"about\":\"fake plugin\",\"subcommands\":[],\"args\":[]}]}",
        "goto loop",
        ":settings",
        "echo {\"type\":\"Settings\",\"settings\":[]}",
        "goto loop",
        ":handle",
        "echo {\"type\":\"Success\",\"message\":\"fake ran\"}",
        "goto loop",
        "",
    ]
    .join("\r\n")
    .replace("@VERSION@", reports)
}

/// Place a fake protocol plugin in the fixture's `~/.cargo/bin` (where crates
/// plugins are loaded from) and pin it in `.metarepo` with `pin`. The plugin
/// reports version `reports`.
fn install_fake_crates_plugin(f: &Fixture, reports: &str, pin: &str) {
    let bin_dir = f.home.join(".cargo").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // On Windows this writes metarepo-plugin-fake.cmd; resolution falls back
    // to extension candidates for the conventional extension-less name.
    write_protocol_script(&bin_dir.join("metarepo-plugin-fake"), reports);
    fs::write(
        f.config_path(),
        format!(r#"{{"projects":{{}},"plugins":{{"fake":"crates:metarepo-plugin-fake@{pin}"}}}}"#),
    )
    .unwrap();
}

#[test]
fn install_records_probed_version_for_unpinned_binary() {
    let f = Fixture::new();
    // A protocol plugin reporting 2.0.0, installed from a file source with no
    // version pin: the lockfile should capture the probed version, not "*".
    let script = write_protocol_script(&f.ws.join("metarepo-plugin-probe"), "2.0.0");
    let from = format!("file:{}", script.display());
    ok(f.meta(&["plugin", "install", "probe", "--from", &from]));

    let lock = fs::read_to_string(f.ws.join(".metarepo.lock")).unwrap();
    assert!(
        lock.contains("2.0.0"),
        "lockfile should record the probed version: {lock}"
    );
    assert!(
        !lock.contains("\"*\""),
        "lockfile should not fall back to * for a probeable binary: {lock}"
    );
}

#[test]
fn list_does_not_falsely_flag_semver_satisfied_pin() {
    let f = Fixture::new();
    // Pin ^1.0.0, plugin reports 1.4.2 — satisfied, so list must NOT warn.
    install_fake_crates_plugin(&f, "1.4.2", "1.0.0");

    let out = ok(f.meta(&["plugin", "list"]));
    let s = stdout(&out);
    assert!(
        !s.contains("version mismatch"),
        "a semver-satisfied pin must not be flagged as a mismatch: {s}"
    );
    assert!(
        s.contains("1.4.2"),
        "list should show the installed version: {s}"
    );
}

#[test]
fn list_flags_semver_violating_pin() {
    let f = Fixture::new();
    // Pin ^1.0.0, plugin reports 2.0.0 — violates the pin.
    install_fake_crates_plugin(&f, "2.0.0", "1.0.0");

    let out = ok(f.meta(&["plugin", "list"]));
    assert!(
        stdout(&out).contains("version mismatch"),
        "a pin-violating version should be flagged in list: {}",
        stdout(&out)
    );
}

#[test]
fn update_version_rejects_non_crates_plugin() {
    let f = Fixture::new();
    install_greet(&f); // installed as a file:/manifest source

    let out = f.meta(&["plugin", "update", "greet", "--version", "9.9.9"]);
    assert!(
        !out.status.success(),
        "re-pinning a non-crates plugin should fail"
    );
    assert!(
        stderr(&out).contains("crates.io"),
        "expected a crates-only re-pin error, got: {}",
        stderr(&out)
    );
}

#[test]
fn version_pin_mismatch_refuses_to_load() {
    let f = Fixture::new();
    install_fake_crates_plugin(&f, "2.0.0", "1.0.0");

    let out = f.meta(&["fake"]);
    assert!(
        !out.status.success(),
        "a plugin that violates its version pin must not load"
    );
    assert!(
        stderr(&out).contains("version mismatch"),
        "expected a version mismatch error, got stderr: {}",
        stderr(&out)
    );
}

#[test]
fn allow_version_mismatch_loads_with_warning() {
    let f = Fixture::new();
    install_fake_crates_plugin(&f, "2.0.0", "1.0.0");

    let out = ok(f.meta(&["--allow-version-mismatch", "fake"]));
    assert!(
        stdout(&out).contains("fake ran"),
        "plugin should run when the mismatch is explicitly allowed: stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
}

#[test]
fn version_pin_satisfied_loads() {
    let f = Fixture::new();
    // Pin ^1.0.0; plugin reports 1.4.2 which satisfies it.
    install_fake_crates_plugin(&f, "1.4.2", "1.0.0");

    let out = ok(f.meta(&["fake"]));
    assert!(
        stdout(&out).contains("fake ran"),
        "plugin satisfying its pin should load and run: stdout={} stderr={}",
        stdout(&out),
        stderr(&out)
    );
}
