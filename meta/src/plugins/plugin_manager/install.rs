//! Install plugins from their resolved [`PluginSpec`] and locate the resulting
//! binary on disk.

use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::spec::{default_crate_name, PluginSpec};

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

/// The on-disk binary path a spec resolves to once installed. Used by `list`
/// (to report installed/missing) and `update`/`remove`.
pub fn resolved_binary_path(plugin_name: &str, spec: &PluginSpec) -> Result<PathBuf> {
    match spec {
        PluginSpec::Crates { crate_name, .. } => Ok(cargo_bin_dir()?.join(crate_name)),
        PluginSpec::File { path } => Ok(expand_tilde(path)),
        // git+ builds are copied into the plugin dir under the conventional name.
        PluginSpec::Git { .. } => Ok(plugin_dir()?.join(default_crate_name(plugin_name))),
    }
}

/// Install a plugin from its spec. Returns the canonical spec to persist in
/// `.metarepo` (which may be more specific than what the user typed — e.g. a
/// `file:` source is rewritten to point at the copied binary).
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
            let dest = install_file(plugin_name, &expand_tilde(path))?;
            Ok(PluginSpec::File {
                path: dest.to_string_lossy().into_owned(),
            })
        }
        PluginSpec::Git { url } => {
            install_git(plugin_name, url)?;
            Ok(spec.clone())
        }
    }
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

fn install_git(plugin_name: &str, url: &str) -> Result<()> {
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
    Ok(())
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
