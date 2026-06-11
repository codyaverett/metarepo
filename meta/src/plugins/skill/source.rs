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
        || s.starts_with("file://")
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

/// Split inline `url#ref` syntax into the URL and the ref. Only applies when
/// the part before the `#` is itself a git URL; anything else (a local path,
/// a URL with no `#`) is returned unchanged with no ref.
pub fn split_url_ref(s: &str) -> (&str, Option<&str>) {
    if let Some((url, r)) = s.rsplit_once('#') {
        if !r.is_empty() && is_git_url(url) {
            return (url, Some(r));
        }
    }
    (s, None)
}

/// Shallow `git clone --depth 1` of the remote default branch into `dest`.
pub fn shallow_clone(url: &str, dest: &Path) -> Result<()> {
    shallow_clone_ref(url, dest, None)
}

/// Shallow-clone `url` at an optional ref (branch, tag, or commit SHA).
///
/// Branches and tags use `git clone --depth 1 --branch <ref>`. A full 40-hex
/// SHA (which `--branch` rejects) is reached by cloning the default branch and
/// fetching the commit; a shorter all-hex ref is tried as a branch/tag first,
/// then as an abbreviated SHA.
pub fn shallow_clone_ref(url: &str, dest: &Path, git_ref: Option<&str>) -> Result<()> {
    let Some(r) = git_ref.map(str::trim).filter(|r| !r.is_empty()) else {
        return clone_cmd(url, dest, None);
    };
    if is_full_sha(r) {
        clone_cmd(url, dest, None)?;
        return checkout_sha(url, dest, r);
    }
    match clone_cmd(url, dest, Some(r)) {
        Ok(()) => Ok(()),
        // An all-hex ref that is not a branch or tag may be an abbreviated
        // commit SHA: retry as a default clone plus a commit checkout.
        Err(e) if is_hexish(r) => {
            let _ = std::fs::remove_dir_all(dest);
            clone_cmd(url, dest, None).map_err(|_| e)?;
            checkout_sha(url, dest, r)
        }
        Err(e) => Err(e),
    }
}

/// `git clone --depth 1 [--branch <ref>] url dest`, with errors that
/// distinguish a missing ref from a missing repo.
fn clone_cmd(url: &str, dest: &Path, branch: Option<&str>) -> Result<()> {
    let mut args = vec!["clone", "--depth", "1", "--quiet"];
    if let Some(b) = branch {
        args.extend(["--branch", b]);
    }
    let out = Command::new("git").args(&args).arg(url).arg(dest).output();
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
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr = stderr.trim();
        if let Some(b) = branch {
            // git's wording for a clone that reached the repo but not the ref.
            if stderr.contains("Remote branch") || stderr.contains("find remote ref") {
                return Err(anyhow!(
                    "ref '{}' not found in {} (no such branch or tag; for a commit use its SHA)",
                    b,
                    url
                ));
            }
        }
        return Err(anyhow!("git clone of {} failed: {}", url, stderr));
    }
    Ok(())
}

/// Check out commit `sha` inside the shallow clone at `dest`: first try a
/// shallow fetch of just that commit, then fall back to deepening the clone.
fn checkout_sha(url: &str, dest: &Path, sha: &str) -> Result<()> {
    if git_in(dest, &["fetch", "--quiet", "--depth", "1", "origin", sha]).is_ok()
        && git_in(dest, &["checkout", "--quiet", "--detach", "FETCH_HEAD"]).is_ok()
    {
        return Ok(());
    }
    let _ = git_in(dest, &["fetch", "--quiet", "--unshallow"]);
    git_in(dest, &["checkout", "--quiet", "--detach", sha])
        .map_err(|_| anyhow!("commit {} not found in {}", sha, url))
}

/// Run `git -C dir <args>`, failing with stderr on a non-zero exit.
fn git_in(dir: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("running git")?;
    if out.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// A full 40-character hex commit SHA.
fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// All-hex and long enough to plausibly be an abbreviated SHA.
fn is_hexish(s: &str) -> bool {
    s.len() >= 7 && s.len() < 40 && s.chars().all(|c| c.is_ascii_hexdigit())
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
        assert!(is_git_url("file:///abs/path/repo"));
    }

    #[test]
    fn rejects_local_paths() {
        assert!(!is_git_url("./local/skill"));
        assert!(!is_git_url("/abs/path"));
        assert!(!is_git_url("owner/repo")); // skills.sh shorthand, not a git URL
        assert!(!is_git_url("SKILL.md"));
    }

    #[test]
    fn splits_inline_url_ref() {
        assert_eq!(
            split_url_ref("https://github.com/o/r#dev"),
            ("https://github.com/o/r", Some("dev"))
        );
        assert_eq!(
            split_url_ref("git@github.com:o/r.git#v1.2.3"),
            ("git@github.com:o/r.git", Some("v1.2.3"))
        );
        // No ref, or not a git URL before the hash: unchanged.
        assert_eq!(
            split_url_ref("https://github.com/o/r"),
            ("https://github.com/o/r", None)
        );
        assert_eq!(split_url_ref("./local#path"), ("./local#path", None));
        assert_eq!(
            split_url_ref("https://github.com/o/r#"),
            ("https://github.com/o/r#", None)
        );
    }

    #[test]
    fn classifies_shas() {
        assert!(is_full_sha(&"a".repeat(40)));
        assert!(!is_full_sha("abc1234"));
        assert!(!is_full_sha(&"g".repeat(40)));
        assert!(is_hexish("abc1234"));
        assert!(!is_hexish("dev"));
        assert!(!is_hexish("main"));
        assert!(!is_hexish("abc")); // too short to be an abbreviated SHA
    }

    /// Initialize a local source repo with a branch, a tag, and distinct files
    /// per ref. Returns None when git is unavailable.
    fn repo_with_refs() -> Option<tempfile::TempDir> {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
        };
        run(&["init", "-q", "-b", "main"])?;
        run(&["config", "user.email", "t@t.t"])?;
        run(&["config", "user.name", "t"])?;
        fs::write(root.join("on-main"), "m").unwrap();
        run(&["add", "-A"])?;
        run(&["commit", "-q", "-m", "main"])?;
        run(&["tag", "v1"])?;
        run(&["checkout", "-q", "-b", "feature"])?;
        fs::write(root.join("on-feature"), "f").unwrap();
        run(&["add", "-A"])?;
        run(&["commit", "-q", "-m", "feature"])?;
        run(&["checkout", "-q", "main"])?;
        Some(tmp)
    }

    #[test]
    fn clones_a_branch_ref() {
        let Some(src) = repo_with_refs() else { return };
        let url = format!("file://{}", src.path().display());
        let out = tempdir().unwrap();
        let dest = out.path().join("clone");
        shallow_clone_ref(&url, &dest, Some("feature")).unwrap();
        assert!(dest.join("on-feature").exists());
    }

    #[test]
    fn clones_a_tag_ref() {
        let Some(src) = repo_with_refs() else { return };
        let url = format!("file://{}", src.path().display());
        let out = tempdir().unwrap();
        let dest = out.path().join("clone");
        shallow_clone_ref(&url, &dest, Some("v1")).unwrap();
        assert!(dest.join("on-main").exists());
        assert!(!dest.join("on-feature").exists());
    }

    #[test]
    fn missing_ref_reports_the_ref() {
        let Some(src) = repo_with_refs() else { return };
        let url = format!("file://{}", src.path().display());
        let out = tempdir().unwrap();
        let dest = out.path().join("clone");
        let err = shallow_clone_ref(&url, &dest, Some("no-such-ref")).unwrap_err();
        assert!(err.to_string().contains("ref 'no-such-ref' not found"));
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
