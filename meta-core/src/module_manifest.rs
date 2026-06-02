//! Parsing for `meta.module.toml` — the manifest that turns a single repository
//! into a **meta module**: a self-contained bundle of the plugin(s) it provides
//! and the Claude Code skill(s) that drive them.
//!
//! A module manifest is a thin index. It does not redefine the plugin or skill
//! formats — its `plugins` entries point at existing `plugin.manifest.*` files
//! (or protocol-plugin binaries) and its `skills` entries point at `SKILL.md`
//! directories. See `docs/MODULES.md` for the design.
//!
//! This mirrors the structure and multi-format loader of [`crate::PluginManifest`].

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Module manifest filenames the loader recognizes, in priority order.
pub const MODULE_MANIFEST_FILENAMES: &[&str] = &[
    "meta.module.toml",
    "meta.module.yaml",
    "meta.module.yml",
    "meta.module.json",
];

/// Top-level `meta.module.*` structure. Everything lives under the `[module]`
/// table, so the manifest reads as `[module]` + `[[module.plugins]]` +
/// `[[module.skills]]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaModuleManifest {
    /// Module metadata and contributions.
    pub module: ModuleInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub repository: String,
    /// Minimum `meta` version required to wire this module up. Enforced by the
    /// host at enable time (semantics live in the `meta` crate's verify module).
    #[serde(default)]
    pub min_meta_version: Option<String>,

    /// Plugin(s) this module provides. May be empty for a skill-only module.
    #[serde(default)]
    pub plugins: Vec<ModulePluginRef>,

    /// Skill(s) this module ships. May be empty for a plugin-only module.
    #[serde(default)]
    pub skills: Vec<ModuleSkillRef>,
}

/// A reference to one plugin provided by the module. Exactly one of `manifest`
/// or `binary` must be set:
/// - `manifest` — path (relative to the repo root) to a `plugin.manifest.*`
///   (a manifest plugin, executed via argv dispatch).
/// - `binary` — path to an executable; with `protocol = true` it speaks the
///   stdio plugin protocol, otherwise it is treated as a manifest-less binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePluginRef {
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(default)]
    pub protocol: bool,
}

/// A reference to one skill shipped by the module: a directory (relative to the
/// repo root) containing a `SKILL.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSkillRef {
    pub path: String,
}

impl ModulePluginRef {
    /// The repo-relative source path the host should read for this plugin
    /// (the manifest path when set, otherwise the binary path).
    pub fn source(&self) -> Option<&str> {
        self.manifest.as_deref().or(self.binary.as_deref())
    }
}

impl MetaModuleManifest {
    /// Load a manifest, choosing the parser by file extension
    /// (`.toml`, `.yaml`/`.yml`, `.json`). Defaults to TOML for unknown
    /// extensions. Validates before returning.
    pub fn from_file_auto(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read module manifest {}", path.display()))?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let manifest: MetaModuleManifest = match ext.as_str() {
            "json" => serde_json::from_str(&content)
                .with_context(|| format!("Invalid JSON module manifest {}", path.display()))?,
            "yaml" | "yml" => serde_yaml::from_str(&content)
                .with_context(|| format!("Invalid YAML module manifest {}", path.display()))?,
            _ => toml::from_str(&content)
                .with_context(|| format!("Invalid TOML module manifest {}", path.display()))?,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Find a `meta.module.*` file directly inside `dir`, if one exists.
    pub fn find_in_dir(dir: &Path) -> Option<PathBuf> {
        MODULE_MANIFEST_FILENAMES
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.is_file())
    }

    /// Whether a path is a recognized module-manifest filename.
    pub fn is_manifest_path(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| MODULE_MANIFEST_FILENAMES.contains(&n))
            .unwrap_or(false)
    }

    /// Validate the manifest's internal consistency.
    pub fn validate(&self) -> Result<()> {
        if self.module.name.is_empty() {
            return Err(anyhow::anyhow!("Module name cannot be empty"));
        }
        if self.module.version.is_empty() {
            return Err(anyhow::anyhow!("Module version cannot be empty"));
        }
        if self.module.plugins.is_empty() && self.module.skills.is_empty() {
            return Err(anyhow::anyhow!(
                "Module '{}' contributes nothing: declare at least one [[module.plugins]] or [[module.skills]]",
                self.module.name
            ));
        }
        for (i, p) in self.module.plugins.iter().enumerate() {
            match (p.manifest.is_some(), p.binary.is_some()) {
                (true, true) => {
                    return Err(anyhow::anyhow!(
                        "Module '{}' plugin #{} sets both 'manifest' and 'binary' (set exactly one)",
                        self.module.name,
                        i + 1
                    ));
                }
                (false, false) => {
                    return Err(anyhow::anyhow!(
                        "Module '{}' plugin #{} sets neither 'manifest' nor 'binary' (set exactly one)",
                        self.module.name,
                        i + 1
                    ));
                }
                _ => {}
            }
        }
        for (i, s) in self.module.skills.iter().enumerate() {
            if s.path.is_empty() {
                return Err(anyhow::anyhow!(
                    "Module '{}' skill #{} has an empty path",
                    self.module.name,
                    i + 1
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const TOML_SRC: &str = r#"
[module]
name = "example"
version = "0.1.0"
description = "An example module"

[[module.plugins]]
manifest = "plugin/plugin.manifest.toml"

[[module.skills]]
path = "skills/example-skill"
"#;

    const YAML_SRC: &str = r#"
module:
  name: example
  version: 0.1.0
  description: An example module
  plugins:
    - manifest: plugin/plugin.manifest.toml
  skills:
    - path: skills/example-skill
"#;

    const JSON_SRC: &str = r#"
{
  "module": {
    "name": "example", "version": "0.1.0", "description": "An example module",
    "plugins": [ { "manifest": "plugin/plugin.manifest.toml" } ],
    "skills": [ { "path": "skills/example-skill" } ]
  }
}
"#;

    fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn loads_all_three_formats_equivalently() {
        let dir = tempdir().unwrap();
        for (file, src) in [
            ("meta.module.toml", TOML_SRC),
            ("meta.module.yaml", YAML_SRC),
            ("meta.module.json", JSON_SRC),
        ] {
            let path = write(dir.path(), file, src);
            let m = MetaModuleManifest::from_file_auto(&path).unwrap();
            assert_eq!(m.module.name, "example");
            assert_eq!(m.module.plugins.len(), 1);
            assert_eq!(
                m.module.plugins[0].source(),
                Some("plugin/plugin.manifest.toml")
            );
            assert_eq!(m.module.skills.len(), 1);
            assert_eq!(m.module.skills[0].path, "skills/example-skill");
        }
    }

    #[test]
    fn find_in_dir_prefers_toml() {
        let dir = tempdir().unwrap();
        write(dir.path(), "meta.module.json", JSON_SRC);
        assert!(MetaModuleManifest::find_in_dir(dir.path())
            .unwrap()
            .ends_with("meta.module.json"));
        write(dir.path(), "meta.module.toml", TOML_SRC);
        assert!(MetaModuleManifest::find_in_dir(dir.path())
            .unwrap()
            .ends_with("meta.module.toml"));
    }

    #[test]
    fn rejects_empty_contribution() {
        let src = "[module]\nname = \"x\"\nversion = \"0.1.0\"\n";
        let err = MetaModuleManifest::from_toml_for_test(src).unwrap_err();
        assert!(err.to_string().contains("contributes nothing"));
    }

    #[test]
    fn rejects_plugin_with_both_manifest_and_binary() {
        let src = "[module]\nname = \"x\"\nversion = \"0.1.0\"\n\
                   [[module.plugins]]\nmanifest = \"a\"\nbinary = \"b\"\n";
        let err = MetaModuleManifest::from_toml_for_test(src).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn rejects_plugin_with_neither_manifest_nor_binary() {
        let src = "[module]\nname = \"x\"\nversion = \"0.1.0\"\n\
                   [[module.plugins]]\nprotocol = true\n";
        let err = MetaModuleManifest::from_toml_for_test(src).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    impl MetaModuleManifest {
        /// Parse-and-validate from a TOML string (test helper).
        fn from_toml_for_test(content: &str) -> Result<Self> {
            let m: MetaModuleManifest = toml::from_str(content)?;
            m.validate()?;
            Ok(m)
        }
    }
}
