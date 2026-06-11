//! Install a skill from the skills.sh registry by its `owner/repo/slug` id.
//!
//! Two resolution paths, chosen automatically:
//!   - **Keyed** (when `SKILLS_SH_API_KEY` is set): fetch exact file contents
//!     from the authenticated `/api/v1/skills/{id}` endpoint. Reliable.
//!   - **Keyless** (default): shallow-clone the skill's source GitHub repo and
//!     locate the matching skill directory. The registry slug does not map 1:1
//!     to the repo path (skills.sh munges names), so we fuzzy-match against the
//!     repo's skill directories and frontmatter names.
//!
//! Either way we resolve to a local skill directory and then hand off to
//! `steal::run`, which audits the skill and refuses HIGH-severity findings
//! unless `--force` is given before copying it into a skills destination.

use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use walkdir::WalkDir;

use super::http;
use super::skill_file::Skill;
use super::source;
use super::steal;

/// Default skills.sh skill-detail endpoint, used when `[skill] detail-url` is unset.
pub const DEFAULT_DETAIL_URL: &str = "https://skills.sh/api/v1/skills";

/// A parsed registry id: `owner/repo` source plus the skill `slug`.
struct ParsedId {
    /// `owner/repo`
    source: String,
    /// The skills.sh slug (last path segment).
    slug: String,
    /// Full original id, used for the authenticated detail endpoint.
    id: String,
}

impl ParsedId {
    fn parse(id: &str) -> Result<Self> {
        let id = id.trim().trim_matches('/');
        let parts: Vec<&str> = id.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() < 3 {
            return Err(anyhow!(
                "expected a skill id of the form owner/repo/skill (got {id}). Find one with: meta skill search <query>"
            ));
        }
        Ok(ParsedId {
            source: format!("{}/{}", parts[0], parts[1]),
            slug: parts[parts.len() - 1].to_string(),
            id: id.to_string(),
        })
    }

    /// The leading owner token used by skills.sh as a slug prefix, e.g.
    /// `vercel-labs` -> `vercel`.
    fn owner_token(&self) -> String {
        let owner = self.source.split('/').next().unwrap_or("");
        owner.split('-').next().unwrap_or(owner).to_lowercase()
    }
}

#[derive(Deserialize)]
struct DetailResponse {
    #[serde(default)]
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    path: String,
    contents: String,
}

/// `meta skill add <id>` — install a skill from skills.sh.
///
/// `detail_url` is the resolved skill-detail endpoint and `api_key` the resolved
/// key (env > config), already chosen by the caller. When `api_key` is set the
/// keyed path is used, otherwise resolution falls back to GitHub. A `git_ref`
/// (branch, tag, or commit SHA) forces the GitHub path, since the skills.sh
/// API only serves the latest registry copy.
pub fn run(
    id: &str,
    dest_root: Option<&str>,
    force: bool,
    overwrite: bool,
    git_ref: Option<&str>,
    detail_url: &str,
    api_key: Option<&str>,
) -> Result<()> {
    let parsed = ParsedId::parse(id)?;
    let tmp = TempDir::new().context("creating temp working dir")?;

    let skill_dir = match api_key {
        Some(key) if !key.trim().is_empty() && git_ref.is_none() => {
            println!("  {} Fetching {} from skills.sh", "↓".cyan(), parsed.id);
            resolve_via_api(&parsed, key.trim(), tmp.path(), detail_url)?
        }
        _ => {
            match git_ref {
                Some(r) => println!(
                    "  {} Resolving {} via GitHub ({} at {})",
                    "↓".cyan(),
                    parsed.slug,
                    parsed.source,
                    r
                ),
                None => println!(
                    "  {} Resolving {} via GitHub ({})",
                    "↓".cyan(),
                    parsed.slug,
                    parsed.source
                ),
            }
            resolve_via_github(&parsed, tmp.path(), git_ref)?
        }
    };

    let dir = skill_dir
        .to_str()
        .ok_or_else(|| anyhow!("resolved skill path is not valid UTF-8"))?;
    // The registry resolves to exactly one skill dir, so steal copies it
    // directly; provenance derives from the clone, with the ref passed along.
    steal::run(
        dir,
        dest_root,
        force,
        overwrite,
        git_ref,
        steal::SelectOpts::default(),
        metarepo_core::NonInteractiveMode::Defaults,
    )
}

/// Keyed path: pull exact files from the authenticated detail endpoint.
fn resolve_via_api(parsed: &ParsedId, key: &str, tmp: &Path, detail_url: &str) -> Result<PathBuf> {
    let url = format!("{detail_url}/{}", parsed.id);
    let body = http::get(&url, Some(key))?;
    let detail: DetailResponse =
        serde_json::from_str(&body).context("parsing skills.sh skill detail response")?;
    if detail.files.is_empty() {
        return Err(anyhow!("skills.sh returned no files for {}", parsed.id));
    }
    let dir = tmp.join(&parsed.slug);
    write_files(&dir, &detail.files)?;
    if !dir.join("SKILL.md").exists() {
        return Err(anyhow!(
            "skills.sh payload for {} did not include a SKILL.md",
            parsed.id
        ));
    }
    Ok(dir)
}

/// Write registry file entries under `dir`, guarding against path traversal in
/// the supplied relative paths.
fn write_files(dir: &Path, files: &[FileEntry]) -> Result<()> {
    for f in files {
        let rel = Path::new(&f.path);
        if rel.is_absolute() || rel.components().any(|c| c.as_os_str() == "..") {
            return Err(anyhow!(
                "refusing unsafe file path from registry: {}",
                f.path
            ));
        }
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&target, &f.contents)
            .with_context(|| format!("writing {}", target.display()))?;
    }
    Ok(())
}

/// Keyless path: shallow-clone the source repo (at `git_ref` when given) and
/// find the matching skill dir.
fn resolve_via_github(parsed: &ParsedId, tmp: &Path, git_ref: Option<&str>) -> Result<PathBuf> {
    let repo_dir = tmp.join("repo");
    let repo_url = format!("https://github.com/{}.git", parsed.source);
    source::shallow_clone_ref(&repo_url, &repo_dir, git_ref)?;

    let mut skills = collect_skill_dirs(&repo_dir);
    if skills.is_empty() {
        return Err(anyhow!(
            "no SKILL.md found in {} — set SKILLS_SH_API_KEY to install via the skills.sh API instead",
            parsed.source
        ));
    }

    // Score each skill dir against the requested slug; keep the best.
    let owner_token = parsed.owner_token();
    skills.sort_by_key(|d| std::cmp::Reverse(score_match(&parsed.slug, &owner_token, d)));
    let best = &skills[0];
    if score_match(&parsed.slug, &owner_token, best) == 0 {
        let avail: Vec<String> = skills.iter().map(|d| d.label()).collect();
        return Err(anyhow!(
            "could not match {} in {}. Available skills: {}.\nInstall by exact id, or set SKILLS_SH_API_KEY to use the skills.sh API.",
            parsed.slug,
            parsed.source,
            avail.join(", ")
        ));
    }
    Ok(best.dir.clone())
}

/// A skill directory found in a cloned repo, with its frontmatter name.
struct SkillDir {
    dir: PathBuf,
    dirname: String,
    fm_name: Option<String>,
}

impl SkillDir {
    fn label(&self) -> String {
        match &self.fm_name {
            Some(n) if n != &self.dirname => format!("{} ({})", self.dirname, n),
            _ => self.dirname.clone(),
        }
    }
}

/// Find every directory containing a SKILL.md within `root`.
fn collect_skill_dirs(root: &Path) -> Vec<SkillDir> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "SKILL.md" && entry.path().is_file() {
            let dir = match entry.path().parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            let dirname = dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let fm_name = Skill::load(&dir).ok().and_then(|s| s.frontmatter.name);
            out.push(SkillDir {
                dir,
                dirname,
                fm_name,
            });
        }
    }
    out
}

/// Score how well a skill dir matches the requested slug. Higher is better;
/// 0 means no match. Handles the skills.sh owner-prefix munging
/// (`vercel-react-best-practices` -> repo dir `react-best-practices`).
fn score_match(slug: &str, owner_token: &str, sd: &SkillDir) -> u32 {
    let slug = slug.to_lowercase();
    let dirname = sd.dirname.to_lowercase();
    let fm = sd.fm_name.as_deref().map(slugify);

    let stripped = slug
        .strip_prefix(&format!("{owner_token}-"))
        .unwrap_or(&slug)
        .to_string();

    if dirname == slug || fm.as_deref() == Some(slug.as_str()) {
        4
    } else if dirname == stripped || fm.as_deref() == Some(stripped.as_str()) {
        3
    } else if slug.ends_with(&dirname) || dirname.ends_with(&stripped) {
        2
    } else if dirname.contains(&stripped) || stripped.contains(&dirname) {
        1
    } else {
        0
    }
}

/// Lowercase, replace runs of non-alphanumerics with single hyphens.
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_three_segment_id() {
        let p = ParsedId::parse("vercel-labs/agent-skills/vercel-react-best-practices").unwrap();
        assert_eq!(p.source, "vercel-labs/agent-skills");
        assert_eq!(p.slug, "vercel-react-best-practices");
        assert_eq!(p.owner_token(), "vercel");
    }

    #[test]
    fn rejects_short_id() {
        assert!(ParsedId::parse("owner/repo").is_err());
    }

    #[test]
    fn matches_owner_prefixed_slug_to_repo_dir() {
        // skills.sh slug carries an owner prefix the repo dir does not.
        let sd = SkillDir {
            dir: PathBuf::from("/x/react-best-practices"),
            dirname: "react-best-practices".into(),
            fm_name: Some("react-best-practices".into()),
        };
        assert!(score_match("vercel-react-best-practices", "vercel", &sd) >= 3);
    }

    #[test]
    fn exact_dirname_scores_highest() {
        let sd = SkillDir {
            dir: PathBuf::from("/x/deploy-to-vercel"),
            dirname: "deploy-to-vercel".into(),
            fm_name: None,
        };
        assert_eq!(score_match("deploy-to-vercel", "vercel", &sd), 4);
    }

    #[test]
    fn unrelated_slug_scores_zero() {
        let sd = SkillDir {
            dir: PathBuf::from("/x/web-design-guidelines"),
            dirname: "web-design-guidelines".into(),
            fm_name: None,
        };
        assert_eq!(score_match("kubernetes-operator", "vercel", &sd), 0);
    }

    #[test]
    fn slugify_normalizes() {
        assert_eq!(slugify("React Best Practices"), "react-best-practices");
        assert_eq!(slugify("a__b  c"), "a-b-c");
    }

    #[test]
    fn write_files_rejects_traversal() {
        let tmp = tempdir().unwrap();
        let bad = vec![FileEntry {
            path: "../escape.md".into(),
            contents: "x".into(),
        }];
        assert!(write_files(&tmp.path().join("s"), &bad).is_err());
    }

    #[test]
    fn write_files_writes_nested() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("s");
        let files = vec![
            FileEntry {
                path: "SKILL.md".into(),
                contents: "---\nname: s\n---\nbody".into(),
            },
            FileEntry {
                path: "refs/a.md".into(),
                contents: "a".into(),
            },
        ];
        write_files(&dir, &files).unwrap();
        assert!(dir.join("SKILL.md").exists());
        assert_eq!(fs::read_to_string(dir.join("refs/a.md")).unwrap(), "a");
    }

    #[test]
    fn collect_finds_skill_dirs() {
        let tmp = tempdir().unwrap();
        let s = tmp.path().join("skills/demo");
        fs::create_dir_all(&s).unwrap();
        fs::write(s.join("SKILL.md"), "---\nname: demo\n---\nb").unwrap();
        let found = collect_skill_dirs(tmp.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dirname, "demo");
        assert_eq!(found[0].fm_name.as_deref(), Some("demo"));
    }
}
