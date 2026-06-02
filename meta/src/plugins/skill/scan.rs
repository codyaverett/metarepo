//! Walk a directory tree and list the Claude Code skills it contains.
//! Adapted from galaxy-gateway/steal-skill.

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::skill_file::Skill;

/// List every skill found under `path`.
pub fn run(path: &str) -> Result<()> {
    let root = Path::new(path);
    let skills = find_skills(root);
    if skills.is_empty() {
        println!("{}", "no skills found".dimmed());
        return Ok(());
    }
    println!(
        "{} in {}",
        format!("found {} skill(s)", skills.len()).bold(),
        root.display()
    );
    for s in skills {
        match Skill::load(&s) {
            Ok(sk) => {
                let name = sk.display_name();
                let desc = sk.frontmatter.description.as_deref().unwrap_or("");
                println!("  {} {} — {}", "•".cyan(), name.bold(), desc);
                println!("    {}", sk.skill_md.display().to_string().dimmed());
            }
            Err(e) => eprintln!("  {} {}: {}", "!".red(), s.display(), e),
        }
    }
    Ok(())
}

/// Locate `SKILL.md` files beneath `root`, skipping noise directories.
pub fn find_skills(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | "node_modules" | "target")
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "SKILL.md")
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_skills_and_skips_noise() {
        let tmp = tempdir().unwrap();
        let good = tmp.path().join("a");
        fs::create_dir_all(&good).unwrap();
        fs::write(good.join("SKILL.md"), "---\nname: a\n---\nbody\n").unwrap();

        // Should be ignored.
        let noise = tmp.path().join("node_modules/pkg");
        fs::create_dir_all(&noise).unwrap();
        fs::write(noise.join("SKILL.md"), "---\nname: noise\n---\n").unwrap();

        let found = find_skills(tmp.path());
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("a/SKILL.md"));
    }
}
