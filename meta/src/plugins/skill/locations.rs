//! Candidate skill destination directories, in resolution order.
//! Adapted from galaxy-gateway/steal-skill.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

/// Print the candidate skill destinations and whether each exists.
///
/// `roots` is the configured `[skill] dest-roots` list, if any.
pub fn run(roots: Option<&[String]>) -> Result<()> {
    let candidates = candidate_dests_with(roots);
    println!("{}", "Skill destinations (resolution order):".bold());
    for (label, path) in candidates {
        let exists = path.exists();
        let marker = if exists { "✓".green() } else { "·".dimmed() };
        println!("  {} {:<24} {}", marker, label, path.display());
    }
    Ok(())
}

/// Where skills can be installed, highest precedence first. `$CLAUDE_SKILLS_HOME`
/// always leads; after it come the configured `[skill] dest-roots`
/// (tilde-expanded, in the order given) when `roots` is a non-empty list, else
/// the built-in `./.claude/skills` then `~/.claude/skills`.
pub fn candidate_dests_with(roots: Option<&[String]>) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("CLAUDE_SKILLS_HOME") {
        out.push(("$CLAUDE_SKILLS_HOME".into(), PathBuf::from(p)));
    }
    match roots.filter(|r| !r.is_empty()) {
        // Configured roots replace the built-in chain outright, so someone who
        // lists only a shared location does not silently fall back to ~/.claude.
        Some(roots) => {
            for r in roots {
                out.push((r.clone(), PathBuf::from(expand_tilde(r))));
            }
        }
        None => {
            out.push((
                "./.claude/skills".into(),
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".claude/skills"),
            ));
            if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
                out.push(("~/.claude/skills".into(), home.join(".claude/skills")));
            }
        }
    }
    out
}

/// The default destination directory for a freshly stolen skill, honoring the
/// configured `[skill] dest-roots`: the first candidate that exists, else the
/// first candidate, else the workspace-local `./.claude/skills`.
pub fn default_dest_root_with(roots: Option<&[String]>) -> PathBuf {
    let candidates = candidate_dests_with(roots);
    candidates
        .iter()
        .find(|(_, p)| p.exists())
        .or_else(|| candidates.first())
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| PathBuf::from(".claude/skills"))
}

/// Expand a leading `~/` to `$HOME`.
pub fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return format!("{home}/{rest}");
        }
    }
    p.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Labels minus the env entry, which depends on the ambient environment.
    fn labels(roots: Option<&[String]>) -> Vec<String> {
        candidate_dests_with(roots)
            .into_iter()
            .map(|(l, _)| l)
            .filter(|l| l != "$CLAUDE_SKILLS_HOME")
            .collect()
    }

    #[test]
    fn configured_roots_replace_the_builtin_chain() {
        let roots = vec!["/opt/team/skills".to_string(), "/srv/skills".to_string()];
        assert_eq!(labels(Some(&roots)), roots);
    }

    #[test]
    fn no_roots_uses_the_builtin_chain() {
        assert_eq!(
            labels(None).first().map(String::as_str),
            Some("./.claude/skills")
        );
    }

    #[test]
    fn empty_roots_fall_back_to_builtins() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(
            labels(Some(&empty)).first().map(String::as_str),
            Some("./.claude/skills")
        );
    }

    #[test]
    fn default_dest_root_picks_the_first_existing_configured_root() {
        // Only meaningful when the env override is absent.
        if std::env::var("CLAUDE_SKILLS_HOME").is_ok() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let roots = vec![
            tmp.path().join("missing").to_string_lossy().into_owned(),
            real.to_string_lossy().into_owned(),
        ];
        assert_eq!(default_dest_root_with(Some(&roots)), real);
    }

    #[test]
    fn expand_tilde_leaves_other_paths_alone() {
        assert_eq!(expand_tilde("/opt/skills"), "/opt/skills");
        assert_eq!(expand_tilde("relative/skills"), "relative/skills");
    }
}
