//! Loader for arbitrary Claude Code skills discovered on disk.
//!
//! This is distinct from the bundled meta-tool skill managed by the rest of this
//! plugin: it parses *any* `SKILL.md` (its YAML frontmatter + body) so the
//! `scan`, `audit`, and `steal` subcommands can inspect and copy external skills.
//! Adapted from galaxy-gateway/steal-skill.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The recognized frontmatter fields of a `SKILL.md`. Unknown fields are ignored
/// so we stay forward-compatible with skills that carry extra metadata.
#[derive(Debug, Deserialize, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<serde_yaml::Value>,
}

/// A loaded skill: where it lives plus its parsed frontmatter and body.
#[derive(Debug)]
pub struct Skill {
    /// Directory containing the skill (parent of `SKILL.md`).
    pub root: PathBuf,
    /// Path to the `SKILL.md` itself.
    pub skill_md: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl Skill {
    /// Load a skill from either a directory (expects `SKILL.md` inside) or a
    /// direct path to a `SKILL.md` file.
    pub fn load(root: &Path) -> Result<Self> {
        let skill_md = if root.is_file() {
            root.to_path_buf()
        } else {
            root.join("SKILL.md")
        };
        let raw = std::fs::read_to_string(&skill_md)
            .with_context(|| format!("reading {}", skill_md.display()))?;
        let (frontmatter, body) = split_frontmatter(&raw);
        Ok(Skill {
            root: skill_md.parent().unwrap_or(Path::new(".")).to_path_buf(),
            skill_md,
            frontmatter,
            body,
        })
    }

    /// Best-effort skill name: frontmatter `name`, else the directory name.
    pub fn display_name(&self) -> String {
        self.frontmatter
            .name
            .clone()
            .or_else(|| {
                self.root
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "(unnamed)".to_string())
    }
}

/// Split a `---`-delimited YAML frontmatter block from the markdown body. Returns
/// defaults if there is no parseable frontmatter so malformed skills still load.
fn split_frontmatter(raw: &str) -> (Frontmatter, String) {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let fm_str = &rest[..end];
            let body = rest[end + 4..].trim_start_matches('\n').to_string();
            let fm: Frontmatter = serde_yaml::from_str(fm_str).unwrap_or_default();
            return (fm, body);
        }
    }
    (Frontmatter::default(), raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn loads_frontmatter_and_body() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("demo");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: demo\ndescription: a demo\n---\nHello body\n",
        )
        .unwrap();
        let s = Skill::load(&dir).unwrap();
        assert_eq!(s.frontmatter.name.as_deref(), Some("demo"));
        assert_eq!(s.frontmatter.description.as_deref(), Some("a demo"));
        assert_eq!(s.body.trim(), "Hello body");
        assert_eq!(s.display_name(), "demo");
    }

    #[test]
    fn falls_back_to_dir_name_when_unnamed() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("from-dir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "no frontmatter here\n").unwrap();
        let s = Skill::load(&dir).unwrap();
        assert_eq!(s.display_name(), "from-dir");
    }
}
