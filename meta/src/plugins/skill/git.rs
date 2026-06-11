//! Derive provenance for a stolen skill: when the source skill lives inside a
//! git checkout (a local clone, or the shallow clone `steal` made from a URL),
//! recover the origin URL, the committed SHA, and the skill's path within the
//! repo. Recorded into the copied skill and reported in the steal output so a
//! stolen skill stays traceable and re-fetchable.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Where a stolen skill came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// Clone URL of the repo's remote (origin, else the first remote).
    pub url: String,
    /// Commit SHA that was checked out when the skill was taken.
    pub commit: String,
    /// Path of the skill directory relative to the repo root (`.` at the root).
    pub subpath: String,
    /// Whether the working tree had uncommitted changes at steal time.
    pub dirty: bool,
    /// The branch, tag, or commit the caller asked for (`--ref` / `url#ref`),
    /// when the steal was pinned to one. The resolved SHA is in `commit`.
    pub git_ref: Option<String>,
}

/// The filename recording provenance inside a stolen skill.
pub const PROVENANCE_FILE: &str = ".meta-source.toml";

/// Run `git -C dir <args>` and return trimmed stdout, or `None` on any failure
/// (git missing, not a repo, non-zero exit).
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Derive provenance for the skill directory `dir`, or `None` when it is not
/// inside a git repository with a usable remote.
pub fn derive(dir: &Path) -> Option<Provenance> {
    let toplevel = git(dir, &["rev-parse", "--show-toplevel"])?;
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    let url = remote_url(dir)?;

    // subpath = dir relative to the repo root.
    let subpath = Path::new(dir)
        .canonicalize()
        .ok()
        .and_then(|d| {
            Path::new(&toplevel)
                .canonicalize()
                .ok()
                .and_then(|top| d.strip_prefix(&top).ok().map(|p| p.to_path_buf()))
        })
        .map(|p| {
            if p.as_os_str().is_empty() {
                ".".to_string()
            } else {
                p.to_string_lossy().to_string()
            }
        })
        .unwrap_or_else(|| ".".to_string());

    let dirty = git(dir, &["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Some(Provenance {
        url,
        commit,
        subpath,
        dirty,
        git_ref: None,
    })
}

/// The remote clone URL: `origin` if present, else the first remote.
fn remote_url(dir: &Path) -> Option<String> {
    if let Some(u) = git(dir, &["remote", "get-url", "origin"]) {
        return Some(u);
    }
    let first = git(dir, &["remote"])?.lines().next().map(str::to_string)?;
    git(dir, &["remote", "get-url", &first])
}

impl Provenance {
    /// A short SHA for display.
    fn short(&self) -> &str {
        if self.commit.len() >= 12 {
            &self.commit[..12]
        } else {
            &self.commit
        }
    }

    /// One-line summary for the steal report.
    pub fn summary(&self) -> String {
        let dirty = if self.dirty {
            " (uncommitted changes)"
        } else {
            ""
        };
        let r = self
            .git_ref
            .as_deref()
            .map(|r| format!("#{r}"))
            .unwrap_or_default();
        format!(
            "{}{}@{} ({}){}",
            self.url,
            r,
            self.short(),
            self.subpath,
            dirty
        )
    }

    /// Write the provenance file into a stolen skill directory.
    pub fn write_file(&self, skill_dir: &Path) -> Result<()> {
        let ref_line = self
            .git_ref
            .as_deref()
            .map(|r| format!("ref = \"{r}\"\n"))
            .unwrap_or_default();
        let body = format!(
            "# Provenance recorded by `meta skill steal`.\n\
             url = \"{}\"\n\
             {}commit = \"{}\"\n\
             subpath = \"{}\"\n\
             dirty = {}\n",
            self.url, ref_line, self.commit, self.subpath, self.dirty
        );
        let path = skill_dir.join(PROVENANCE_FILE);
        std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Initialize a git repo with one remote and a committed skill. Returns the
    /// skill dir, or `None` if git is unavailable.
    fn repo_with_skill() -> Option<(tempfile::TempDir, std::path::PathBuf)> {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
        };
        run(&["init", "-q"])?;
        run(&["config", "user.email", "t@t.t"])?;
        run(&["config", "user.name", "t"])?;
        run(&["remote", "add", "origin", "https://github.com/o/r.git"])?;
        let skill = root.join("skills/demo");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "---\nname: demo\n---\nb\n").unwrap();
        run(&["add", "-A"])?;
        run(&["commit", "-q", "-m", "init"])?;
        Some((tmp, skill))
    }

    #[test]
    fn derives_url_commit_and_subpath() {
        let Some((_tmp, skill)) = repo_with_skill() else {
            return; // git not available
        };
        let p = derive(&skill).expect("provenance");
        assert_eq!(p.url, "https://github.com/o/r.git");
        assert_eq!(p.commit.len(), 40);
        assert_eq!(p.subpath, "skills/demo");
        assert!(!p.dirty);
        assert!(p.summary().starts_with("https://github.com/o/r.git@"));
        assert!(p.summary().contains("(skills/demo)"));
    }

    #[test]
    fn reports_dirty_tree() {
        let Some((_tmp, skill)) = repo_with_skill() else {
            return;
        };
        fs::write(skill.join("SKILL.md"), "---\nname: demo\n---\nchanged\n").unwrap();
        let p = derive(&skill).expect("provenance");
        assert!(p.dirty);
        assert!(p.summary().contains("uncommitted changes"));
    }

    #[test]
    fn none_outside_a_repo() {
        let tmp = tempdir().unwrap();
        assert!(derive(tmp.path()).is_none());
    }

    #[test]
    fn write_file_roundtrips() {
        let tmp = tempdir().unwrap();
        let p = Provenance {
            url: "https://github.com/o/r.git".into(),
            commit: "abc123".into(),
            subpath: "skills/demo".into(),
            dirty: false,
            git_ref: None,
        };
        p.write_file(tmp.path()).unwrap();
        let body = fs::read_to_string(tmp.path().join(PROVENANCE_FILE)).unwrap();
        assert!(body.contains("url = \"https://github.com/o/r.git\""));
        assert!(body.contains("subpath = \"skills/demo\""));
        assert!(!body.contains("ref = "));
    }

    #[test]
    fn write_file_records_ref_when_pinned() {
        let tmp = tempdir().unwrap();
        let p = Provenance {
            url: "https://github.com/o/r.git".into(),
            commit: "abc123".into(),
            subpath: ".".into(),
            dirty: false,
            git_ref: Some("v1.2.3".into()),
        };
        p.write_file(tmp.path()).unwrap();
        let body = fs::read_to_string(tmp.path().join(PROVENANCE_FILE)).unwrap();
        assert!(body.contains("ref = \"v1.2.3\""));
        assert!(p.summary().contains("#v1.2.3@"));
    }
}
