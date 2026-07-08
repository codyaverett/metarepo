use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

mod plugin;
pub use self::plugin::SkillPlugin;

// Discover/audit/copy external skills (adapted from galaxy-gateway/steal-skill).
pub mod adapt;
pub mod audit;
pub mod git;
pub mod http;
pub mod locations;
pub mod mark;
pub mod picker;
pub mod registry;
pub mod scan;
pub mod search;
pub mod skill_file;
pub mod source;
pub mod steal;

// Bundled meta-tool Claude Code skill. Canonical copy lives inside the crate at
// src/plugins/skill/assets/meta-tool/ so it is packaged by `cargo publish`; the
// workspace `.claude/skills/meta-tool/` symlinks to it. Included at compile time
// so installing the skill works in fresh checkouts without the source repo on
// disk.
pub const SKILL_MD: &str = include_str!("assets/meta-tool/SKILL.md");
pub const SKILL_CHANGELOG: &str = include_str!("assets/meta-tool/references/CHANGELOG_NOTES.md");

/// Default install location of the bundled meta-tool skill under a workspace
/// root: `<workspace>/.claude/skills/meta-tool`. Used when no `[skill] dest`
/// override is configured.
pub fn default_skill_root(workspace: &Path) -> PathBuf {
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

/// Version of the skill currently installed at `root`, if any.
pub fn installed_version(root: &Path) -> Option<String> {
    let path = root.join("SKILL.md");
    let contents = fs::read_to_string(path).ok()?;
    parse_version(&contents)
}

/// Whether a skill is installed at all (presence of SKILL.md).
pub fn is_installed(root: &Path) -> bool {
    root.join("SKILL.md").exists()
}

/// Lock file recording the fingerprint of the files we last wrote, so `update`
/// can tell a pristine install from a locally modified one.
const LOCK_FILE: &str = ".skill-lock.json";

/// Hex sha256 over the skill's content files. Length-prefixed so file
/// boundaries cannot be confused.
fn fingerprint(skill_md: &str, changelog: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [skill_md, changelog] {
        hasher.update(part.len().to_le_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Fingerprint recorded at the last install/update, if any. Legacy installs
/// (written before lock files existed) have none.
fn recorded_fingerprint(root: &Path) -> Option<String> {
    let raw = fs::read_to_string(root.join(LOCK_FILE)).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(parsed.get("sha256")?.as_str()?.to_string())
}

/// Fingerprint of the files currently on disk (missing files hash as empty).
fn installed_fingerprint(root: &Path) -> Option<String> {
    let skill_md = fs::read_to_string(root.join("SKILL.md")).ok()?;
    let changelog =
        fs::read_to_string(root.join("references").join("CHANGELOG_NOTES.md")).unwrap_or_default();
    Some(fingerprint(&skill_md, &changelog))
}

fn write_lock(root: &Path) -> Result<()> {
    let lock = serde_json::json!({
        "version": bundled_version(),
        "sha256": fingerprint(SKILL_MD, SKILL_CHANGELOG),
    });
    fs::write(root.join(LOCK_FILE), serde_json::to_string_pretty(&lock)?)?;
    Ok(())
}

/// Write the bundled skill files into `root`, overwriting whatever is there, and
/// record their fingerprint in the lock file.
pub fn write_skill(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("references"))?;
    fs::write(root.join("SKILL.md"), SKILL_MD)?;
    fs::write(
        root.join("references").join("CHANGELOG_NOTES.md"),
        SKILL_CHANGELOG,
    )?;
    write_lock(root)?;
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
    /// `update` ran but no skill is installed; install is the opt-in step.
    NotInstalled,
    /// `update` declined to overwrite the installed copy (see the reason).
    Refused(UpdateRefusal),
}

/// Why `update` refused to rewrite an installed skill without `--force`.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateRefusal {
    /// The installed files differ from the fingerprint recorded at install
    /// time: the user (or something else) edited them.
    LocallyModified,
    /// The installed version is newer than the bundled one; updating would be
    /// a downgrade.
    InstalledNewer { installed: String, bundled: String },
    /// No fingerprint was recorded (legacy or hand-rolled install), so a
    /// pristine copy cannot be told apart from an edited one.
    UnknownProvenance,
}

/// Install the skill. With `force`, always rewrite. Otherwise install only when
/// absent; if already present, leave it untouched (use `update` to refresh).
pub fn install(root: &Path, force: bool) -> Result<SkillAction> {
    if !is_installed(root) {
        write_skill(root)?;
        return Ok(SkillAction::Installed);
    }
    if force {
        let from = installed_version(root);
        write_skill(root)?;
        return Ok(SkillAction::Updated {
            from,
            to: bundled_version(),
        });
    }
    Ok(SkillAction::AlreadyCurrent)
}

/// Refresh an installed skill to the bundled copy, but only when it is safe:
/// the installed files must match the fingerprint recorded when they were
/// written, and the installed version must not be newer than the bundled one.
/// Anything else is refused (overridable with `force`). Does not install when
/// absent — `install`/`init` are the opt-in entry points.
pub fn update(root: &Path, force: bool) -> Result<SkillAction> {
    if !is_installed(root) {
        return Ok(SkillAction::NotInstalled);
    }
    let installed = installed_version(root);
    let bundled = bundled_version();
    if force {
        write_skill(root)?;
        return Ok(SkillAction::Updated {
            from: installed,
            to: bundled,
        });
    }

    // Content identical to the bundle: nothing to do. Backfill the lock so
    // legacy installs (written before lock files existed) gain a fingerprint.
    let on_disk = installed_fingerprint(root);
    if on_disk.as_deref() == Some(fingerprint(SKILL_MD, SKILL_CHANGELOG).as_str()) {
        if recorded_fingerprint(root).is_none() {
            write_lock(root)?;
        }
        return Ok(SkillAction::AlreadyCurrent);
    }

    // Downgrade guard: never silently replace a newer installed skill.
    if let (Some(inst), Some(bund)) = (
        installed
            .as_deref()
            .and_then(|v| v.parse::<semver::Version>().ok()),
        bundled
            .as_deref()
            .and_then(|v| v.parse::<semver::Version>().ok()),
    ) {
        if inst > bund {
            return Ok(SkillAction::Refused(UpdateRefusal::InstalledNewer {
                installed: inst.to_string(),
                bundled: bund.to_string(),
            }));
        }
    }

    match recorded_fingerprint(root) {
        Some(recorded) if on_disk.as_deref() == Some(recorded.as_str()) => {
            // Pristine copy of an older bundle: safe to refresh.
            write_skill(root)?;
            Ok(SkillAction::Updated {
                from: installed,
                to: bundled,
            })
        }
        Some(_) => Ok(SkillAction::Refused(UpdateRefusal::LocallyModified)),
        None => Ok(SkillAction::Refused(UpdateRefusal::UnknownProvenance)),
    }
}

/// Remove an installed skill directory. Returns true if something was removed.
pub fn remove(root: &Path) -> Result<bool> {
    if root.exists() {
        fs::remove_dir_all(root)?;
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
        let root = default_skill_root(tmp.path());
        assert!(!is_installed(&root));
        assert_eq!(install(&root, false).unwrap(), SkillAction::Installed);
        assert!(is_installed(&root));
        assert!(root.join("SKILL.md").exists());
        assert!(root.join("references/CHANGELOG_NOTES.md").exists());
        // The default layout is <workspace>/.claude/skills/meta-tool.
        assert!(tmp
            .path()
            .join(".claude/skills/meta-tool/SKILL.md")
            .exists());
    }

    #[test]
    fn install_is_idempotent_without_force() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        install(&root, false).unwrap();
        assert_eq!(install(&root, false).unwrap(), SkillAction::AlreadyCurrent);
    }

    #[test]
    fn force_install_rewrites() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        install(&root, false).unwrap();
        match install(&root, true).unwrap() {
            SkillAction::Updated { .. } => {}
            other => panic!("expected Updated, got {other:?}"),
        }
    }

    /// Write an old-version skill by hand at `root`, optionally with a lock that
    /// matches its content (i.e. a pristine install of an older bundle).
    fn write_stale_skill(root: &Path, with_lock: bool) {
        fs::create_dir_all(root.join("references")).unwrap();
        let skill_md = "---\nname: meta-cli\nversion: 0.0.1\n---\nold\n";
        let changelog = "old notes\n";
        fs::write(root.join("SKILL.md"), skill_md).unwrap();
        fs::write(root.join("references/CHANGELOG_NOTES.md"), changelog).unwrap();
        if with_lock {
            let lock = serde_json::json!({
                "version": "0.0.1",
                "sha256": fingerprint(skill_md, changelog),
            });
            fs::write(root.join(LOCK_FILE), lock.to_string()).unwrap();
        }
    }

    #[test]
    fn update_refreshes_pristine_older_install() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        write_stale_skill(&root, true);
        match update(&root, false).unwrap() {
            SkillAction::Updated { from, to } => {
                assert_eq!(from, Some("0.0.1".to_string()));
                assert_eq!(to, bundled_version());
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        assert_eq!(installed_version(&root), bundled_version());
    }

    #[test]
    fn update_refuses_without_recorded_fingerprint() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        write_stale_skill(&root, false);
        assert_eq!(
            update(&root, false).unwrap(),
            SkillAction::Refused(UpdateRefusal::UnknownProvenance)
        );
        // Untouched.
        assert_eq!(installed_version(&root), Some("0.0.1".to_string()));
    }

    #[test]
    fn update_refuses_locally_modified_install() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        install(&root, false).unwrap();
        let md = root.join("SKILL.md");
        let mut contents = fs::read_to_string(&md).unwrap();
        contents.push_str("\nlocal tweak\n");
        fs::write(&md, &contents).unwrap();
        assert_eq!(
            update(&root, false).unwrap(),
            SkillAction::Refused(UpdateRefusal::LocallyModified)
        );
        // The edit survives.
        assert_eq!(fs::read_to_string(&md).unwrap(), contents);
    }

    #[test]
    fn update_force_overwrites_modified_install() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        install(&root, false).unwrap();
        let md = root.join("SKILL.md");
        fs::write(&md, "---\nname: meta-cli\nversion: 0.0.1\n---\nedited\n").unwrap();
        match update(&root, true).unwrap() {
            SkillAction::Updated { to, .. } => assert_eq!(to, bundled_version()),
            other => panic!("expected Updated, got {other:?}"),
        }
        assert_eq!(installed_version(&root), bundled_version());
    }

    #[test]
    fn update_refuses_downgrade_of_newer_install() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        fs::create_dir_all(root.join("references")).unwrap();
        let skill_md = "---\nname: meta-cli\nversion: 99.0.0\n---\nfuture\n";
        fs::write(root.join("SKILL.md"), skill_md).unwrap();
        fs::write(root.join("references/CHANGELOG_NOTES.md"), "notes\n").unwrap();
        match update(&root, false).unwrap() {
            SkillAction::Refused(UpdateRefusal::InstalledNewer { installed, .. }) => {
                assert_eq!(installed, "99.0.0");
            }
            other => panic!("expected InstalledNewer refusal, got {other:?}"),
        }
    }

    #[test]
    fn update_does_not_install_when_absent() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        assert_eq!(update(&root, false).unwrap(), SkillAction::NotInstalled);
        assert!(!is_installed(&root));
    }

    #[test]
    fn update_is_noop_when_current_and_backfills_lock() {
        let tmp = tempdir().unwrap();
        let root = default_skill_root(tmp.path());
        install(&root, false).unwrap();
        // Simulate a legacy install: drop the lock, content still pristine.
        let lock = root.join(LOCK_FILE);
        fs::remove_file(&lock).unwrap();
        assert_eq!(update(&root, false).unwrap(), SkillAction::AlreadyCurrent);
        assert!(lock.exists());
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
