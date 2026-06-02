//! Candidate skill destination directories, in resolution order.
//! Adapted from galaxy-gateway/steal-skill.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

/// Print the candidate skill destinations and whether each exists.
pub fn run() -> Result<()> {
    let candidates = candidate_dests();
    println!("{}", "Skill destinations (resolution order):".bold());
    for (label, path) in candidates {
        let exists = path.exists();
        let marker = if exists { "✓".green() } else { "·".dimmed() };
        println!("  {} {:<24} {}", marker, label, path.display());
    }
    Ok(())
}

/// Where skills can be installed, highest precedence first:
/// `$CLAUDE_SKILLS_HOME`, then the workspace `./.claude/skills`, then
/// `~/.claude/skills`.
pub fn candidate_dests() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("CLAUDE_SKILLS_HOME") {
        out.push(("$CLAUDE_SKILLS_HOME".into(), PathBuf::from(p)));
    }
    out.push((
        "./.claude/skills".into(),
        std::env::current_dir()
            .unwrap_or_default()
            .join(".claude/skills"),
    ));
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        out.push(("~/.claude/skills".into(), home.join(".claude/skills")));
    }
    out
}

/// The default destination directory for a freshly stolen skill: the first
/// candidate that exists, else the workspace-local `./.claude/skills`.
pub fn default_dest_root() -> PathBuf {
    let candidates = candidate_dests();
    candidates
        .iter()
        .find(|(_, p)| p.exists())
        .or_else(|| candidates.first())
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| PathBuf::from(".claude/skills"))
}
