//! The `.metarepo.lock` integrity lockfile: per-plugin SHA-256 digests recorded
//! at install time and verified at load time. See `docs/PLUGIN_INTEGRITY.md`.

use anyhow::{Context, Result};
use metarepo_core::MetaConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Filename of the lockfile, kept beside the active `.metarepo` config.
pub const LOCKFILE_NAME: &str = ".metarepo.lock";

/// Recorded integrity facts for one installed plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    /// Version resolved at install time (informational).
    pub version: String,
    /// The canonical spec the plugin was installed from.
    pub source: String,
    /// Lowercase hex SHA-256 of the resolved binary.
    pub sha256: String,
}

/// The parsed `.metarepo.lock` contents.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    /// Map of plugin name -> recorded entry. `BTreeMap` keeps the file stable
    /// and diff-friendly when committed to version control.
    #[serde(default)]
    pub plugins: BTreeMap<String, LockEntry>,
}

impl Lockfile {
    /// The lockfile path for a config directory.
    pub fn path_for(config_dir: &Path) -> PathBuf {
        config_dir.join(LOCKFILE_NAME)
    }

    /// Locate the lockfile alongside the active `.metarepo`, walking up from the
    /// current directory the same way config discovery does. Returns `None` when
    /// no metarepo config is found.
    pub fn locate() -> Option<PathBuf> {
        let meta_file = MetaConfig::find_meta_file()?;
        let dir = meta_file.parent()?;
        Some(Self::path_for(dir))
    }

    /// Read the lockfile, returning an empty one if it does not exist yet.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read lockfile {}", path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("Failed to parse lockfile {}", path.display()))
    }

    /// Write the lockfile to disk (pretty TOML).
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).context("Failed to serialize lockfile")?;
        fs::write(path, text)
            .with_context(|| format!("Failed to write lockfile {}", path.display()))
    }

    pub fn get(&self, name: &str) -> Option<&LockEntry> {
        self.plugins.get(name)
    }

    /// Insert or replace a plugin's entry.
    pub fn upsert(&mut self, name: impl Into<String>, entry: LockEntry) {
        self.plugins.insert(name.into(), entry);
    }

    /// Drop a plugin's entry. Returns whether anything was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> LockEntry {
        LockEntry {
            version: "1.2.3".into(),
            source: "crates:metarepo-plugin-foo@1.2.3".into(),
            sha256: "deadbeef".into(),
        }
    }

    #[test]
    fn roundtrips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = Lockfile::path_for(dir.path());

        // Missing file loads as empty.
        let mut lock = Lockfile::load(&path).unwrap();
        assert!(lock.is_empty());

        lock.upsert("foo", entry());
        lock.save(&path).unwrap();

        let reloaded = Lockfile::load(&path).unwrap();
        assert_eq!(reloaded.get("foo"), Some(&entry()));
    }

    #[test]
    fn remove_reports_presence() {
        let mut lock = Lockfile::default();
        lock.upsert("foo", entry());
        assert!(lock.remove("foo"));
        assert!(!lock.remove("foo"));
    }
}
