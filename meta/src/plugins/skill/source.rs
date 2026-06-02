//! Resolving where skills come from: detecting git URLs, shallow-cloning them,
//! and discovering every `SKILL.md` in a tree. Shared by `steal` (browse/pick)
//! and `registry` (skills.sh clone).

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::scan;
use super::skill_file::Skill;

/// Whether `s` looks like a git URL we should clone rather than a local path.
///
/// Matches the common transports (`https://`, `http://`, `ssh://`, `git://`),
/// the SCP-style `git@host:owner/repo` form, or any string ending in `.git`.
/// A bare `owner/repo` is intentionally NOT a git URL here — it stays a local
/// path; the skills.sh shorthand is handled by `meta skill add`.
pub fn is_git_url(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("ssh://")
        || s.starts_with("git://")
        || s.ends_with(".git")
        || is_scp_like(s)
}

/// `git@github.com:owner/repo` (and friends): `user@host:path`, no scheme, no
/// leading slash, the part before `:` contains an `@`.
fn is_scp_like(s: &str) -> bool {
    if s.contains("://") {
        return false;
    }
    match s.split_once(':') {
        Some((host, path)) => host.contains('@') && !host.contains('/') && !path.is_empty(),
        None => false,
    }
}

/// Shallow `git clone --depth 1` into `dest`.
pub fn shallow_clone(url: &str, dest: &Path) -> Result<()> {
    let out = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet", url])
        .arg(dest)
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow!(
                "git is required to clone skills from a remote repository but was not found on PATH"
            ));
        }
        Err(e) => return Err(e).context("running git clone"),
    };
    if !out.status.success() {
        return Err(anyhow!(
            "git clone of {} failed: {}",
            url,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// A skill found while scanning a tree, with display metadata pulled from its
/// frontmatter.
pub struct FoundSkill {
    /// Directory containing the skill (parent of its `SKILL.md`).
    pub dir: PathBuf,
    /// Frontmatter `name`, else the directory name.
    pub name: String,
    /// Frontmatter `description`, if any.
    pub description: Option<String>,
}

/// Discover every `SKILL.md` under `root`, returning labeled entries sorted by
/// name. Skills that fail to load are skipped.
pub fn discover_skills(root: &Path) -> Vec<FoundSkill> {
    let mut out: Vec<FoundSkill> = scan::find_skills(root)
        .into_iter()
        .filter_map(|skill_md| {
            let dir = skill_md.parent()?.to_path_buf();
            let skill = Skill::load(&dir).ok()?;
            Some(FoundSkill {
                name: skill.display_name(),
                description: skill.frontmatter.description.clone(),
                dir,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_git_urls() {
        assert!(is_git_url("https://github.com/owner/repo.git"));
        assert!(is_git_url("https://github.com/owner/repo"));
        assert!(is_git_url("git@github.com:owner/repo.git"));
        assert!(is_git_url("ssh://git@host/owner/repo.git"));
        assert!(is_git_url("git://host/repo.git"));
    }

    #[test]
    fn rejects_local_paths() {
        assert!(!is_git_url("./local/skill"));
        assert!(!is_git_url("/abs/path"));
        assert!(!is_git_url("owner/repo")); // skills.sh shorthand, not a git URL
        assert!(!is_git_url("SKILL.md"));
    }

    #[test]
    fn discovers_and_labels_skills() {
        let tmp = tempdir().unwrap();
        for (dir, name, desc) in [("a", "alpha", "first"), ("b", "bravo", "second")] {
            let d = tmp.path().join(dir);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: {desc}\n---\nbody\n"),
            )
            .unwrap();
        }
        let found = discover_skills(tmp.path());
        assert_eq!(found.len(), 2);
        // Sorted by name.
        assert_eq!(found[0].name, "alpha");
        assert_eq!(found[0].description.as_deref(), Some("first"));
        assert_eq!(found[1].name, "bravo");
    }
}
