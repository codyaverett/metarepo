//! Install plugins from their resolved [`PluginSpec`] and locate the resulting
//! binary on disk.

use anyhow::{anyhow, bail, Context, Result};
use metarepo_core::PluginManifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::lockfile::{LockEntry, Lockfile};
use super::spec::{default_crate_name, PluginSpec};
use super::verify::{integrity_target, sha256_file};

/// `~/.config/metarepo/plugins`, created if missing. Preferred home for plugins
/// installed from `file:` and `git+` sources.
pub fn plugin_dir() -> Result<PathBuf> {
    let dir = home_dir()?.join(".config").join("metarepo").join("plugins");
    if !dir.exists() {
        fs::create_dir_all(&dir).context("Failed to create plugin directory")?;
    }
    Ok(dir)
}

/// `~/.cargo/bin`, where `cargo install` places binaries.
pub fn cargo_bin_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cargo").join("bin"))
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("Could not determine home directory")
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// The on-disk path a spec resolves to once installed (used by `list`/`remove`).
/// For manifest plugins this is the manifest file; otherwise the binary.
pub fn resolved_binary_path(plugin_name: &str, spec: &PluginSpec) -> Result<PathBuf> {
    match spec {
        PluginSpec::Crates { crate_name, .. } => Ok(cargo_bin_dir()?.join(crate_name)),
        PluginSpec::File { path } => Ok(expand_tilde(path)),
        // git+ builds are copied into the plugin dir under the conventional name.
        PluginSpec::Git { .. } => Ok(plugin_dir()?.join(default_crate_name(plugin_name))),
    }
}

/// Per-plugin directory for manifest plugins: `~/.config/metarepo/plugins/<name>`.
pub fn manifest_plugin_dir(plugin_name: &str) -> Result<PathBuf> {
    Ok(plugin_dir()?.join(plugin_name))
}

/// Install a plugin from its spec. Returns the canonical spec to persist in
/// `.metarepo` (which may be more specific than what the user typed — e.g. a
/// `file:` source is rewritten to point at the installed manifest or binary).
pub fn install_from_spec(plugin_name: &str, spec: &PluginSpec) -> Result<PluginSpec> {
    match spec {
        PluginSpec::Crates {
            crate_name,
            version,
        } => {
            install_crates(crate_name, version.as_deref())?;
            Ok(spec.clone())
        }
        PluginSpec::File { path } => {
            let source = expand_tilde(path);
            // A manifest source (a plugin.manifest.* file or a directory
            // containing one) installs as a manifest plugin.
            if let Some(manifest_path) = locate_manifest(&source) {
                let dest = install_manifest_files(plugin_name, &manifest_path)?;
                return Ok(PluginSpec::File {
                    path: dest.to_string_lossy().into_owned(),
                });
            }
            let dest = install_file(plugin_name, &source)?;
            Ok(PluginSpec::File {
                path: dest.to_string_lossy().into_owned(),
            })
        }
        PluginSpec::Git { url } => install_git(plugin_name, url),
    }
}

/// Locate a manifest given a `file:` source that is either a manifest file or a
/// directory containing one.
fn locate_manifest(source: &Path) -> Option<PathBuf> {
    if source.is_file() && PluginManifest::is_manifest_path(source) {
        Some(source.to_path_buf())
    } else if source.is_dir() {
        PluginManifest::find_in_dir(source)
    } else {
        None
    }
}

/// Copy a manifest and its referenced binary into a per-plugin directory under
/// the plugins dir, preserving the relative binary path so the loader resolves
/// it. Returns the installed manifest path.
fn install_manifest_files(plugin_name: &str, manifest_path: &Path) -> Result<PathBuf> {
    let manifest = PluginManifest::from_file_auto(manifest_path)?;
    let binary_src = manifest.resolve_binary(manifest_path)?;
    if !binary_src.exists() {
        bail!(
            "manifest references a binary that does not exist: {}",
            binary_src.display()
        );
    }

    let dest_dir = manifest_plugin_dir(plugin_name)?;
    fs::create_dir_all(&dest_dir)?;

    // Copy the manifest, keeping its filename (preserves the format extension).
    let manifest_name = manifest_path
        .file_name()
        .ok_or_else(|| anyhow!("invalid manifest path"))?;
    let manifest_dest = dest_dir.join(manifest_name);
    fs::copy(manifest_path, &manifest_dest)?;

    // Copy the binary to the same relative location the manifest declares.
    let binary_dest = manifest.resolve_binary(&manifest_dest)?;
    if let Some(parent) = binary_dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&binary_src, &binary_dest)?;
    make_executable(&binary_dest)?;

    println!("  Installed manifest plugin to {}", dest_dir.display());
    Ok(manifest_dest)
}

/// The version to record in the lockfile for a stored spec. Informational only
/// (the checksum is the real guard): the declared crates version when present,
/// the manifest version for manifest plugins, otherwise `*`.
fn lock_version(spec: &PluginSpec) -> String {
    if let Some(v) = spec.declared_version() {
        return v.to_string();
    }
    if let PluginSpec::File { path } = spec {
        let p = expand_tilde(path);
        if PluginManifest::is_manifest_path(&p) {
            if let Ok(m) = PluginManifest::from_file_auto(&p) {
                return m.plugin.version;
            }
        }
    }
    "*".to_string()
}

/// Record (or refresh) a plugin's checksum entry in the lockfile beside the
/// active config. Best-effort by design: integrity is only *enforced* when the
/// workspace opts in via `plugins-integrity = "required"`, but the digest is
/// always recorded so opting in later needs no reinstall.
pub fn record_lock_entry(meta_file: &Path, plugin_name: &str, stored: &PluginSpec) -> Result<()> {
    let resolved = resolved_binary_path(plugin_name, stored)?;
    let target = integrity_target(&resolved)?;
    let sha256 = sha256_file(&target)?;

    let dir = meta_file.parent().unwrap_or_else(|| Path::new("."));
    let lock_path = Lockfile::path_for(dir);
    let mut lock = Lockfile::load(&lock_path)?;
    lock.upsert(
        plugin_name,
        LockEntry {
            version: lock_version(stored),
            source: stored.to_spec_string(),
            sha256,
        },
    );
    lock.save(&lock_path)
}

/// Outcome of comparing an installed plugin's binary against the digest
/// recorded in `.metarepo.lock`.
pub enum IntegrityStatus {
    /// The binary matches the recorded digest.
    Ok,
    /// The binary differs from the recorded digest (possible tampering).
    Mismatch,
    /// No digest is recorded for this plugin yet.
    NotRecorded,
    /// The binary could not be resolved or hashed.
    Unreadable(String),
}

/// Compare a plugin's current binary against the digest recorded in the
/// lockfile beside `meta_file`. Used by `meta plugin verify` and `list`; the
/// load-time enforcement in the loader applies the same comparison.
pub fn integrity_status(meta_file: &Path, plugin_name: &str, spec: &PluginSpec) -> IntegrityStatus {
    let dir = meta_file.parent().unwrap_or_else(|| Path::new("."));
    let lock = match Lockfile::load(&Lockfile::path_for(dir)) {
        Ok(lock) => lock,
        Err(e) => return IntegrityStatus::Unreadable(e.to_string()),
    };
    let Some(entry) = lock.get(plugin_name) else {
        return IntegrityStatus::NotRecorded;
    };
    let resolved = match resolved_binary_path(plugin_name, spec) {
        Ok(p) => p,
        Err(e) => return IntegrityStatus::Unreadable(e.to_string()),
    };
    let target = match integrity_target(&resolved) {
        Ok(p) => p,
        Err(e) => return IntegrityStatus::Unreadable(e.to_string()),
    };
    match sha256_file(&target) {
        Ok(actual) if actual == entry.sha256 => IntegrityStatus::Ok,
        Ok(_) => IntegrityStatus::Mismatch,
        Err(e) => IntegrityStatus::Unreadable(e.to_string()),
    }
}

/// Drop a plugin from the lockfile if present. No-op when the lockfile or entry
/// is absent.
pub fn remove_lock_entry(meta_file: &Path, plugin_name: &str) -> Result<()> {
    let dir = meta_file.parent().unwrap_or_else(|| Path::new("."));
    let lock_path = Lockfile::path_for(dir);
    if !lock_path.exists() {
        return Ok(());
    }
    let mut lock = Lockfile::load(&lock_path)?;
    if lock.remove(plugin_name) {
        lock.save(&lock_path)?;
    }
    Ok(())
}

fn install_crates(crate_name: &str, version: Option<&str>) -> Result<()> {
    println!("  Installing {crate_name} from crates.io...");
    let mut cmd = Command::new("cargo");
    cmd.arg("install").arg(crate_name).arg("--force");
    if let Some(v) = version {
        cmd.arg("--version").arg(v);
    }
    let status = cmd.status().context("Failed to run cargo install")?;
    if !status.success() {
        bail!("cargo install {crate_name} failed");
    }
    Ok(())
}

fn install_file(plugin_name: &str, source: &Path) -> Result<PathBuf> {
    if !source.exists() {
        bail!("Plugin path does not exist: {}", source.display());
    }
    let dest = plugin_dir()?.join(default_crate_name(plugin_name));
    fs::copy(source, &dest)
        .with_context(|| format!("Failed to copy {} to {}", source.display(), dest.display()))?;
    make_executable(&dest)?;
    println!("  Installed to {}", dest.display());
    Ok(dest)
}

fn install_git(plugin_name: &str, url: &str) -> Result<PluginSpec> {
    let work = std::env::temp_dir().join(format!(
        "metarepo-plugin-build-{}-{}",
        std::process::id(),
        plugin_name
    ));
    if work.exists() {
        let _ = fs::remove_dir_all(&work);
    }
    // Best-effort cleanup guard.
    let _guard = CleanupDir(work.clone());

    println!("  Cloning {url}...");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(&work)
        .status()
        .context("Failed to run git clone (is git installed?)")?;
    if !status.success() {
        bail!("git clone {url} failed");
    }

    // Manifest plugin: a manifest at the repo root means no protocol build is
    // required (the binary may be a checked-in script). Build only if the
    // referenced binary is missing and the repo is a cargo project.
    if let Some(manifest_path) = PluginManifest::find_in_dir(&work) {
        let manifest = PluginManifest::from_file_auto(&manifest_path)?;
        let binary = manifest.resolve_binary(&manifest_path)?;
        if !binary.exists() && work.join("Cargo.toml").exists() {
            println!("  Building (cargo build --release)...");
            let status = Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&work)
                .status()
                .context("Failed to run cargo build")?;
            if !status.success() {
                bail!("cargo build failed for {url}");
            }
        }
        let dest = install_manifest_files(plugin_name, &manifest_path)?;
        return Ok(PluginSpec::File {
            path: dest.to_string_lossy().into_owned(),
        });
    }

    // Protocol plugin: build and copy the binary into the plugin dir.
    println!("  Building (cargo build --release)...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&work)
        .status()
        .context("Failed to run cargo build")?;
    if !status.success() {
        bail!("cargo build failed for {url}");
    }

    let built = find_built_binary(&work.join("target").join("release"), plugin_name)?;
    let dest = plugin_dir()?.join(default_crate_name(plugin_name));
    fs::copy(&built, &dest)
        .with_context(|| format!("Failed to copy built binary to {}", dest.display()))?;
    make_executable(&dest)?;
    println!("  Installed to {}", dest.display());
    Ok(PluginSpec::Git {
        url: url.to_string(),
    })
}

/// Find the plugin executable in a `target/release` directory. Prefer the
/// conventional `metarepo-plugin-<name>`, then any `metarepo-plugin-*`.
fn find_built_binary(release_dir: &Path, plugin_name: &str) -> Result<PathBuf> {
    let conventional = release_dir.join(default_crate_name(plugin_name));
    if conventional.is_file() {
        return Ok(conventional);
    }
    let entries = fs::read_dir(release_dir)
        .with_context(|| format!("No build output in {}", release_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_file() && name.starts_with("metarepo-plugin-") && !name.contains('.') {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "Could not find a metarepo-plugin-* binary in {}. Does the crate produce one?",
        release_dir.display()
    ))
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    let _ = path;
    Ok(())
}

struct CleanupDir(PathBuf);

impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crates_resolves_to_cargo_bin() {
        let spec = PluginSpec::Crates {
            crate_name: "metarepo-plugin-hello".into(),
            version: None,
        };
        let path = resolved_binary_path("hello", &spec).unwrap();
        assert!(path.ends_with(".cargo/bin/metarepo-plugin-hello"));
    }

    #[test]
    fn git_resolves_to_plugin_dir_conventional_name() {
        let spec = PluginSpec::Git {
            url: "https://example.com/p.git".into(),
        };
        let path = resolved_binary_path("hello", &spec).unwrap();
        assert!(path.ends_with("metarepo/plugins/metarepo-plugin-hello"));
    }

    #[test]
    fn file_resolves_to_its_path() {
        let spec = PluginSpec::File {
            path: "/opt/bin/thing".into(),
        };
        let path = resolved_binary_path("hello", &spec).unwrap();
        assert_eq!(path, PathBuf::from("/opt/bin/thing"));
    }
}
