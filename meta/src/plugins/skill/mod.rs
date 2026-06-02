use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

mod plugin;
pub use self::plugin::SkillPlugin;

// Discover/audit/copy external skills (adapted from galaxy-gateway/steal-skill).
pub mod audit;
pub mod locations;
pub mod scan;
pub mod skill_file;
pub mod steal;

// Bundled meta-tool Claude Code skill. Canonical copy lives inside the crate at
// src/plugins/skill/assets/meta-tool/ so it is packaged by `cargo publish`; the
// workspace `.claude/skills/meta-tool/` symlinks to it. Included at compile time
// so installing the skill works in fresh checkouts without the source repo on
// disk.
pub const SKILL_MD: &str = include_str!("assets/meta-tool/SKILL.md");
pub const SKILL_CHANGELOG: &str = include_str!("assets/meta-tool/references/CHANGELOG_NOTES.md");

/// Relative location of the installed skill under a workspace root.
fn skill_root(workspace: &Path) -> PathBuf {
    workspace.join(".claude").join("skills").join("meta-tool")
}

/// Parse the `version:` field from a SKILL.md frontmatter block. Returns None if
/// the file has no recognizable version line (older or hand-edited installs).
fn parse_version(skill_md: &str) -> Option<String> {
    let mut in_frontmatter = false;
    for line in skill_md.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break; // end of frontmatter
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(rest) = trimmed.strip_prefix("version:") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

/// Version of the skill compiled into this binary.
pub fn bundled_version() -> Option<String> {
    parse_version(SKILL_MD)
}

/// Version of the skill currently installed in `workspace`, if any.
pub fn installed_version(workspace: &Path) -> Option<String> {
    let path = skill_root(workspace).join("SKILL.md");
    let contents = fs::read_to_string(path).ok()?;
    parse_version(&contents)
}

/// Whether a skill is installed at all (presence of SKILL.md).
pub fn is_installed(workspace: &Path) -> bool {
    skill_root(workspace).join("SKILL.md").exists()
}

/// Write the bundled skill files into `workspace`, overwriting whatever is there.
pub fn write_skill(workspace: &Path) -> Result<()> {
    let root = skill_root(workspace);
    fs::create_dir_all(root.join("references"))?;
    fs::write(root.join("SKILL.md"), SKILL_MD)?;
    fs::write(
        root.join("references").join("CHANGELOG_NOTES.md"),
        SKILL_CHANGELOG,
    )?;
    Ok(())
}

/// Outcome of an install/update operation, so callers can report precisely.
#[derive(Debug, PartialEq, Eq)]
pub enum SkillAction {
    Installed,
    Updated {
        from: Option<String>,
        to: Option<String>,
    },
    AlreadyCurrent,
}

/// Install the skill. With `force`, always rewrite. Otherwise install only when
/// absent; if already present, leave it untouched (use `update` to refresh).
pub fn install(workspace: &Path, force: bool) -> Result<SkillAction> {
    if !is_installed(workspace) {
        write_skill(workspace)?;
        return Ok(SkillAction::Installed);
    }
    if force {
        let from = installed_version(workspace);
        write_skill(workspace)?;
        return Ok(SkillAction::Updated {
            from,
            to: bundled_version(),
        });
    }
    Ok(SkillAction::AlreadyCurrent)
}

/// Refresh an installed skill when the bundled version differs. Installs if
/// absent. Compares version strings; a missing version on either side is treated
/// as "unknown" and triggers a rewrite to be safe.
pub fn update(workspace: &Path) -> Result<SkillAction> {
    if !is_installed(workspace) {
        write_skill(workspace)?;
        return Ok(SkillAction::Installed);
    }
    let installed = installed_version(workspace);
    let bundled = bundled_version();
    if installed.is_some() && installed == bundled {
        return Ok(SkillAction::AlreadyCurrent);
    }
    write_skill(workspace)?;
    Ok(SkillAction::Updated {
        from: installed,
        to: bundled,
    })
}

/// Remove an installed skill directory. Returns true if something was removed.
pub fn remove(workspace: &Path) -> Result<bool> {
    let root = skill_root(workspace);
    if root.exists() {
        fs::remove_dir_all(&root)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn bundled_version_is_present() {
        assert!(bundled_version().is_some());
    }

    #[test]
    fn install_then_status_reports_installed() {
        let tmp = tempdir().unwrap();
        assert!(!is_installed(tmp.path()));
        assert_eq!(install(tmp.path(), false).unwrap(), SkillAction::Installed);
        assert!(is_installed(tmp.path()));
        assert!(tmp
            .path()
            .join(".claude/skills/meta-tool/SKILL.md")
            .exists());
        assert!(tmp
            .path()
            .join(".claude/skills/meta-tool/references/CHANGELOG_NOTES.md")
            .exists());
    }

    #[test]
    fn install_is_idempotent_without_force() {
        let tmp = tempdir().unwrap();
        install(tmp.path(), false).unwrap();
        assert_eq!(
            install(tmp.path(), false).unwrap(),
            SkillAction::AlreadyCurrent
        );
    }

    #[test]
    fn force_install_rewrites() {
        let tmp = tempdir().unwrap();
        install(tmp.path(), false).unwrap();
        match install(tmp.path(), true).unwrap() {
            SkillAction::Updated { .. } => {}
            other => panic!("expected Updated, got {other:?}"),
        }
    }

    #[test]
    fn update_refreshes_when_version_differs() {
        let tmp = tempdir().unwrap();
        // Install a stale version by hand.
        let root = tmp.path().join(".claude/skills/meta-tool");
        fs::create_dir_all(root.join("references")).unwrap();
        fs::write(
            root.join("SKILL.md"),
            "---\nname: meta-cli\nversion: 0.0.1\n---\nold\n",
        )
        .unwrap();
        match update(tmp.path()).unwrap() {
            SkillAction::Updated { from, to } => {
                assert_eq!(from, Some("0.0.1".to_string()));
                assert_eq!(to, bundled_version());
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        assert_eq!(installed_version(tmp.path()), bundled_version());
    }

    #[test]
    fn update_is_noop_when_current() {
        let tmp = tempdir().unwrap();
        install(tmp.path(), false).unwrap();
        assert_eq!(update(tmp.path()).unwrap(), SkillAction::AlreadyCurrent);
    }

    #[test]
    fn remove_deletes_installed_skill() {
        let tmp = tempdir().unwrap();
        install(tmp.path(), false).unwrap();
        assert!(remove(tmp.path()).unwrap());
        assert!(!is_installed(tmp.path()));
        assert!(!remove(tmp.path()).unwrap());
    }
}
