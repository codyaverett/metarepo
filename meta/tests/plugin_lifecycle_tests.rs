// End-to-end tests for the `meta plugin` lifecycle: install / list / update /
// remove for both manifest plugins and plain file plugins.
//
// These drive the real `meta` binary (via CARGO_BIN_EXE_meta) with a per-process
// HOME and a throwaway workspace, so each test is fully isolated and parallel-safe
// — no global env mutation, no shared on-disk config.
//
// The update tests are regression coverage for a bug where `meta plugin update
// <name>` reinstalled file: source plugins from their recorded install
// destination, copying each file onto itself and truncating it to 0 bytes so the
// plugin could no longer load. The bulk-update path already skipped file: sources;
// the targeted path now does too.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const META_BIN: &str = env!("CARGO_BIN_EXE_meta");

/// An isolated workspace + config home with `.metarepo` already initialized.
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

    /// Run the `meta` binary with this fixture's HOME and workspace cwd.
    fn meta(&self, args: &[&str]) -> Output {
        Command::new(META_BIN)
            .args(args)
            .current_dir(&self.ws)
            .env("HOME", &self.home)
            .env("NO_COLOR", "1")
            .env_remove("XDG_CONFIG_HOME")
            .output()
            .expect("failed to spawn meta binary")
    }

    /// Per-plugin / per-binary install location under the config home.
    fn installed_path(&self, segment: &str) -> PathBuf {
        self.home
            .join(".config")
            .join("metarepo")
            .join("plugins")
            .join(segment)
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn ok(out: Output) -> Output {
    assert!(
        out.status.success(),
        "command failed (status {:?})\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Write a minimal shell manifest plugin into `dir` and return that dir.
fn write_manifest_plugin(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("plugin.manifest.toml"),
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

[[commands.args]]
name = "loud"
long = "loud"
help = "Shout the greeting"

[config.execution]
binary = "./greet.sh"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("greet.sh"),
        r#"#!/usr/bin/env bash
set -euo pipefail
sub="${1:-}"
case "$sub" in
  hello)
    name="${METAREPO_ARG_NAME:-world}"
    greeting="Hello, ${name}!"
    if [ "${METAREPO_ARG_LOUD:-}" = "1" ]; then
      greeting="$(printf '%s' "$greeting" | tr '[:lower:]' '[:upper:]')"
    fi
    echo "$greeting"
    ;;
  *)
    echo "usage: greet hello <name> [--loud]" >&2
    exit 1
    ;;
esac
"#,
    )
    .unwrap();
}

/// Write a dummy executable to install as a plain (non-manifest) file plugin.
/// It need not speak the protocol — these tests only install/update it.
fn write_dummy_file_plugin(path: &Path) {
    fs::write(path, "#!/usr/bin/env bash\necho dummy-plugin-payload\n").unwrap();
}

#[test]
fn manifest_plugin_installs_and_runs() {
    let f = Fixture::new();
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);

    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));

    let out = ok(f.meta(&["greet", "hello", "Ada"]));
    assert!(
        stdout(&out).contains("Hello, Ada!"),
        "expected greeting, got: {}",
        stdout(&out)
    );

    let out = ok(f.meta(&["greet", "hello", "Bob", "--loud"]));
    assert!(
        stdout(&out).contains("HELLO, BOB!"),
        "expected shouted greeting, got: {}",
        stdout(&out)
    );
}

#[test]
fn plugin_list_reports_installed_manifest_plugin() {
    let f = Fixture::new();
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));

    let out = ok(f.meta(&["plugin", "list"]));
    let s = stdout(&out);
    assert!(s.contains("greet"), "list should mention greet: {s}");
    assert!(s.contains("manifest"), "list should tag it [manifest]: {s}");
    assert!(s.contains("0.1.0"), "list should show the version: {s}");
}

/// Regression: targeted `update` of a manifest plugin must not truncate the
/// installed manifest or its script, and the plugin must still run afterwards.
#[test]
fn manifest_plugin_update_does_not_truncate() {
    let f = Fixture::new();
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));

    let manifest = f.installed_path("greet/plugin.manifest.toml");
    let script = f.installed_path("greet/greet.sh");
    let manifest_before = fs::metadata(&manifest).unwrap().len();
    let script_before = fs::metadata(&script).unwrap().len();
    assert!(manifest_before > 0 && script_before > 0);

    let out = ok(f.meta(&["plugin", "update", "greet"]));
    assert!(
        stdout(&out).contains("nothing to update"),
        "update should be a no-op for file sources, got: {}",
        stdout(&out)
    );

    assert_eq!(
        fs::metadata(&manifest).unwrap().len(),
        manifest_before,
        "manifest was modified by update"
    );
    assert_eq!(
        fs::metadata(&script).unwrap().len(),
        script_before,
        "script was modified by update"
    );

    let out = ok(f.meta(&["greet", "hello", "Ada"]));
    assert!(
        stdout(&out).contains("Hello, Ada!"),
        "plugin should still run after update, got: {}",
        stdout(&out)
    );
}

/// Regression: targeted `update` of a plain file plugin must not truncate the
/// installed binary.
#[test]
fn file_plugin_update_does_not_truncate() {
    let f = Fixture::new();
    let src = f.ws.join("dummy-plugin");
    write_dummy_file_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "dummy", "--from", &from]));

    let installed = f.installed_path("metarepo-plugin-dummy");
    let before = fs::read(&installed).unwrap();
    assert!(!before.is_empty());

    let out = ok(f.meta(&["plugin", "update", "dummy"]));
    assert!(
        stdout(&out).contains("nothing to update"),
        "update should be a no-op for file sources, got: {}",
        stdout(&out)
    );

    let after = fs::read(&installed).unwrap();
    assert_eq!(before, after, "installed file was modified by update");
}

/// Bulk `update` (no name) must skip file sources without touching them.
#[test]
fn bulk_update_skips_file_sources_without_truncating() {
    let f = Fixture::new();
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));

    let manifest = f.installed_path("greet/plugin.manifest.toml");
    let before = fs::metadata(&manifest).unwrap().len();

    let out = ok(f.meta(&["plugin", "update"]));
    assert!(
        stdout(&out).contains("skipped"),
        "bulk update should report skipping file sources, got: {}",
        stdout(&out)
    );
    assert_eq!(
        fs::metadata(&manifest).unwrap().len(),
        before,
        "manifest was modified by bulk update"
    );
}

/// `remove --purge` unregisters the plugin and deletes its install directory.
#[test]
fn remove_purge_unregisters_and_deletes_files() {
    let f = Fixture::new();
    let src = f.ws.join("src-greet");
    write_manifest_plugin(&src);
    let from = format!("file:{}", src.display());
    ok(f.meta(&["plugin", "install", "greet", "--from", &from]));

    let plugin_dir = f.installed_path("greet");
    assert!(plugin_dir.exists());

    ok(f.meta(&["plugin", "remove", "greet", "--purge"]));

    assert!(
        !plugin_dir.exists(),
        "purge should delete the per-plugin install directory"
    );
    let out = ok(f.meta(&["plugin", "list"]));
    assert!(
        !stdout(&out).contains("greet"),
        "removed plugin should not appear in list: {}",
        stdout(&out)
    );
}
