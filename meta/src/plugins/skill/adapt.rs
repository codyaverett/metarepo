//! Optional headless-Claude adaptation of a freshly stolen skill.
//!
//! Gated behind `meta skill steal --adapt [purpose]`. After a skill is installed
//! we back it up, then run a headless `claude -p … --permission-mode acceptEdits`
//! with the working directory set to the skill so Claude can edit the skill's
//! files in place to fit this repo (and an optional free-text purpose). The
//! adapted skill is re-audited afterward.

use anyhow::{Context, Result};
use colored::Colorize;
use metarepo_core::SkillSettings;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::audit::{audit_skill, has_high, print_findings};

/// The external AI command used by `--adapt`, with a `{prompt}` placeholder in
/// its args. Defaults to `claude -p {prompt} --permission-mode acceptEdits`;
/// override via the `[skill]` block in `.meta` to use codex / opencode / etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl Default for AdaptCommand {
    fn default() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec![
                "-p".to_string(),
                "{prompt}".to_string(),
                "--permission-mode".to_string(),
                "acceptEdits".to_string(),
            ],
        }
    }
}

impl AdaptCommand {
    /// Resolve the adapt command from `[skill]` config, falling back to the
    /// claude default for any field left unset.
    pub fn from_settings(settings: Option<&SkillSettings>) -> Self {
        let default = Self::default();
        match settings {
            Some(s) => Self {
                command: s.adapt_command.clone().unwrap_or(default.command),
                args: s.adapt_args.clone().unwrap_or(default.args),
            },
            None => default,
        }
    }
}

/// Substitute the built `prompt` for every `{prompt}` placeholder in `args`.
fn render_args(args: &[String], prompt: &str) -> Vec<String> {
    args.iter().map(|a| a.replace("{prompt}", prompt)).collect()
}

/// Lightweight description of the repo a skill is being adapted for.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepoContext {
    pub name: String,
    pub languages: Vec<String>,
    pub top_level: Vec<String>,
}

/// Adapt the installed skill at `skill_dir` to `repo_root` (+ optional purpose)
/// using the configured headless AI command. No-op (with a notice) when that
/// command is not on PATH.
pub fn adapt_skill(
    skill_dir: &Path,
    repo_root: &Path,
    purpose: Option<&str>,
    cmd: &AdaptCommand,
) -> Result<()> {
    if which(&cmd.command).is_none() {
        println!(
            "  {} '{}' not found on PATH — skipping adaptation",
            "·".bright_black(),
            cmd.command
        );
        return Ok(());
    }

    let skill_md = skill_dir.join("SKILL.md");
    let body = std::fs::read_to_string(&skill_md)
        .with_context(|| format!("reading {}", skill_md.display()))?;
    let ctx = repo_context(repo_root);
    let prompt = build_prompt(&body, &ctx, purpose);

    // Audit baseline so we can tell if adaptation introduces a NEW high finding.
    let had_high_before = audit_skill(skill_dir)
        .map(|(_, f)| has_high(&f))
        .unwrap_or(false);

    // Back up the skill before letting Claude edit it.
    let backup = backup_dir(skill_dir);
    if backup.exists() {
        std::fs::remove_dir_all(&backup).ok();
    }
    copy_tree(skill_dir, &backup)
        .with_context(|| format!("backing up skill to {}", backup.display()))?;

    println!(
        "  {} Adapting '{}' with {}…",
        "✦".cyan(),
        skill_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        cmd.command
    );

    // Run the configured AI command, allowing it to edit files within the skill
    // dir (cwd). Args carry the prompt via the `{prompt}` placeholder.
    let status = Command::new(&cmd.command)
        .current_dir(skill_dir)
        .args(render_args(&cmd.args, &prompt))
        .status()
        .with_context(|| format!("running {}", cmd.command))?;
    if !status.success() {
        println!(
            "  {} {} exited with {} — skill left as installed (backup at {})",
            "⚠".yellow(),
            cmd.command,
            status,
            backup.display()
        );
        return Ok(());
    }

    // Re-audit the adapted skill and surface any newly introduced risk.
    match audit_skill(skill_dir) {
        Ok((_, findings)) => {
            print_findings(&findings);
            if has_high(&findings) && !had_high_before {
                println!(
                    "  {} adaptation introduced a HIGH-severity pattern — review carefully; original saved at {}",
                    "⚠".red(),
                    backup.display()
                );
            } else {
                println!(
                    "  {} Adapted (original saved at {})",
                    "✓".green(),
                    backup.display()
                );
            }
        }
        Err(e) => println!("  {} could not re-audit adapted skill: {}", "!".yellow(), e),
    }
    Ok(())
}

/// The backup location for a skill dir: under the OS temp dir, NOT beside the
/// installed skill — a sibling `<name>.orig` inside `.claude/skills/` would be
/// picked up as a duplicate skill.
fn backup_dir(skill_dir: &Path) -> PathBuf {
    let name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "skill".to_string());
    std::env::temp_dir().join("meta-skill-backups").join(name)
}

/// Locate `cmd` on `PATH`, returning its full path. An absolute/relative path
/// that exists is accepted as-is.
fn which(cmd: &str) -> Option<PathBuf> {
    let p = Path::new(cmd);
    if p.is_absolute() || cmd.contains('/') {
        return p.is_file().then(|| p.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|cand| cand.is_file())
}

/// Gather a light description of `root`: its name, detected languages, and a few
/// top-level entries.
pub fn repo_context(root: &Path) -> RepoContext {
    let name = root
        .canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().to_string())
        .or_else(|| root.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "repo".to_string());

    let markers = [
        ("Cargo.toml", "Rust"),
        ("package.json", "JavaScript/TypeScript"),
        ("go.mod", "Go"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("Gemfile", "Ruby"),
        ("pom.xml", "Java"),
        ("composer.json", "PHP"),
    ];
    let mut languages: Vec<String> = Vec::new();
    for (file, lang) in markers {
        if root.join(file).exists() && !languages.iter().any(|l| l == lang) {
            languages.push(lang.to_string());
        }
    }

    let mut top_level: Vec<String> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter(|n| !n.starts_with('.') && n != "target" && n != "node_modules")
                .collect()
        })
        .unwrap_or_default();
    top_level.sort();
    top_level.truncate(20);

    RepoContext {
        name,
        languages,
        top_level,
    }
}

/// Build the headless-Claude prompt.
pub fn build_prompt(skill_md: &str, ctx: &RepoContext, purpose: Option<&str>) -> String {
    let langs = if ctx.languages.is_empty() {
        "unknown".to_string()
    } else {
        ctx.languages.join(", ")
    };
    let layout = if ctx.top_level.is_empty() {
        "(empty)".to_string()
    } else {
        ctx.top_level.join(", ")
    };
    let purpose_line = match purpose {
        Some(p) if !p.trim().is_empty() => format!("\nAdditional purpose from the user: {p}\n"),
        _ => String::new(),
    };

    format!(
        "You are adapting a Claude Code skill so it fits a specific repository. \
         The skill's files are in the current working directory; edit them in place \
         (SKILL.md and any references/ or scripts/). Keep the SKILL.md frontmatter valid \
         (name, description). Do not add network-fetch-and-execute patterns, rm -rf, or \
         wildcard allowed-tools.\n\n\
         Target repository:\n\
         - name: {name}\n\
         - languages: {langs}\n\
         - top-level entries: {layout}\n{purpose_line}\n\
         Current SKILL.md:\n---\n{skill_md}\n---\n\n\
         Edit the skill files now to tailor the skill to this repository.",
        name = ctx.name,
    )
}

/// Recursively copy a directory (used for the pre-adapt backup), skipping the
/// existing `.orig` backups and VCS/build noise.
fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    use walkdir::WalkDir;
    for entry in WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !matches!(n.as_ref(), ".git" | "node_modules" | "target")
        })
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        let rel = p.strip_prefix(src)?;
        let target = dest.join(rel);
        if p.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if p.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(p, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn repo_context_detects_languages_and_layout() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap(); // skipped
        let ctx = repo_context(root);
        assert!(ctx.languages.contains(&"Rust".to_string()));
        assert!(ctx.top_level.contains(&"src".to_string()));
        assert!(ctx.top_level.contains(&"docs".to_string()));
        assert!(!ctx.top_level.contains(&"target".to_string()));
    }

    #[test]
    fn build_prompt_includes_context_and_purpose() {
        let ctx = RepoContext {
            name: "myrepo".into(),
            languages: vec!["Rust".into()],
            top_level: vec!["src".into()],
        };
        let p = build_prompt(
            "---\nname: demo\n---\nbody",
            &ctx,
            Some("make it CI-friendly"),
        );
        assert!(p.contains("myrepo"));
        assert!(p.contains("Rust"));
        assert!(p.contains("name: demo"));
        assert!(p.contains("make it CI-friendly"));
    }

    #[test]
    fn build_prompt_omits_empty_purpose() {
        let ctx = RepoContext::default();
        let p = build_prompt("body", &ctx, Some("   "));
        assert!(!p.contains("Additional purpose"));
    }

    #[test]
    fn adapt_skips_when_command_missing() {
        // Point at a command that cannot exist; adapt_skill must be a no-op that
        // leaves the skill untouched and writes no backup.
        let tmp = tempdir().unwrap();
        let skill = tmp.path().join("demo");
        fs::create_dir_all(&skill).unwrap();
        let original = "---\nname: demo\n---\nbody";
        fs::write(skill.join("SKILL.md"), original).unwrap();
        let cmd = AdaptCommand {
            command: "definitely-not-a-real-binary-xyz".into(),
            args: vec!["{prompt}".into()],
        };
        adapt_skill(&skill, tmp.path(), None, &cmd).unwrap();
        assert_eq!(
            fs::read_to_string(skill.join("SKILL.md")).unwrap(),
            original
        );
        assert!(!backup_dir(&skill).exists());
    }

    #[test]
    fn render_args_substitutes_prompt() {
        let args = vec![
            "-p".to_string(),
            "{prompt}".to_string(),
            "--flag".to_string(),
        ];
        let out = render_args(&args, "hello world");
        assert_eq!(out, vec!["-p", "hello world", "--flag"]);
    }

    #[test]
    fn from_settings_overrides_and_defaults() {
        // None → claude defaults.
        let d = AdaptCommand::from_settings(None);
        assert_eq!(d.command, "claude");
        assert!(d.args.contains(&"{prompt}".to_string()));

        // Partial override: command set, args fall back to default.
        let s = SkillSettings {
            dest: None,
            adapt_command: Some("codex".into()),
            adapt_args: None,
        };
        let c = AdaptCommand::from_settings(Some(&s));
        assert_eq!(c.command, "codex");
        assert_eq!(c.args, AdaptCommand::default().args);

        // Full override.
        let s2 = SkillSettings {
            dest: None,
            adapt_command: Some("opencode".into()),
            adapt_args: Some(vec!["run".into(), "{prompt}".into()]),
        };
        let c2 = AdaptCommand::from_settings(Some(&s2));
        assert_eq!(c2.command, "opencode");
        assert_eq!(c2.args, vec!["run", "{prompt}"]);
    }

    #[test]
    fn which_finds_known_and_misses_bogus() {
        assert!(which("sh").is_some());
        assert!(which("definitely-not-a-real-binary-xyz").is_none());
    }
}
