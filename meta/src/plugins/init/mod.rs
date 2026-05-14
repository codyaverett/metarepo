use anyhow::Result;
use colored::Colorize;
use metarepo_core::MetaConfig;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Export the plugin
pub use self::plugin::InitPlugin;

mod plugin;

// Bundled meta-tool Claude Code skill. Lives in the workspace at
// .claude/skills/meta-tool/ and is included at compile time so `meta init
// --with-skill` works in fresh checkouts that don't have the source repo
// available on disk.
const SKILL_MD: &str = include_str!("../../../../.claude/skills/meta-tool/SKILL.md");
const SKILL_CHANGELOG: &str =
    include_str!("../../../../.claude/skills/meta-tool/references/CHANGELOG_NOTES.md");

/// User-selected options for `meta init`.
///
/// - `force`: overwrite the existing `.meta` with fresh defaults.
/// - `repair`: never write `.meta`; only restore missing artifacts (gitignore,
///   skill if requested). Useful after a partial setup.
/// - `with_skill`: install the bundled Claude Code skill under
///   `.claude/skills/meta-tool/`.
/// - `all`: shorthand that implies `with_skill` (and any future optional
///   artifacts).
#[derive(Debug, Default, Clone, Copy)]
pub struct InitOptions {
    pub force: bool,
    pub repair: bool,
    pub with_skill: bool,
    pub all: bool,
}

impl InitOptions {
    fn want_skill(&self) -> bool {
        self.with_skill || self.all
    }
}

/// Per-artifact outcome from running init.
#[derive(Debug, Default)]
pub struct InitReport {
    pub meta_created: bool,
    pub meta_overwritten: bool,
    pub meta_skipped_existing: bool,
    pub gitignore_updated: bool,
    pub skill_installed: bool,
    pub skill_already_present: bool,
}

fn create_default_config() -> MetaConfig {
    MetaConfig {
        ignore: vec![
            ".git".to_string(),
            ".vscode".to_string(),
            "node_modules".to_string(),
            "target".to_string(),
            ".DS_Store".to_string(),
        ],
        projects: HashMap::new(),
        plugins: None,
        nested: None,
        aliases: None,
        scripts: None,
        worktree_init: None,
        default_bare: None,
    }
}

/// Backwards-compatible entry point: idempotent init with no optional extras.
pub fn initialize_meta_repo<P: AsRef<Path>>(path: P) -> Result<()> {
    let report = initialize_meta_repo_with_options(path, InitOptions::default())?;
    print_report(&report);
    Ok(())
}

/// Idempotent init with explicit options. Returns a report so callers (and
/// tests) can assert which artifacts were created vs skipped.
pub fn initialize_meta_repo_with_options<P: AsRef<Path>>(
    path: P,
    options: InitOptions,
) -> Result<InitReport> {
    let root = path.as_ref();
    let meta_file_path = root.join(".meta");
    let mut report = InitReport::default();

    let meta_exists = meta_file_path.exists();

    // --- .meta ---
    if options.repair {
        // Repair mode never rewrites .meta — it only ensures sibling artifacts.
        if !meta_exists {
            return Err(anyhow::anyhow!(
                "Cannot repair: no .meta file present. Run 'meta init' first."
            ));
        }
        report.meta_skipped_existing = true;
    } else if !meta_exists {
        let config = create_default_config();
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&meta_file_path, content)?;
        report.meta_created = true;
    } else if options.force {
        let config = create_default_config();
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&meta_file_path, content)?;
        report.meta_overwritten = true;
    } else {
        report.meta_skipped_existing = true;
    }

    // --- .gitignore ---
    report.gitignore_updated = update_gitignore(root)?;

    // --- optional skill ---
    if options.want_skill() {
        let skill_root = root.join(".claude").join("skills").join("meta-tool");
        if skill_root.join("SKILL.md").exists() {
            report.skill_already_present = true;
        } else {
            install_skill(&skill_root)?;
            report.skill_installed = true;
        }
    }

    Ok(report)
}

fn print_report(report: &InitReport) {
    if report.meta_created {
        println!("  {} Created .meta with default configuration", "✓".green());
    } else if report.meta_overwritten {
        println!(
            "  {} Overwrote .meta with default configuration (--force)",
            "✓".yellow()
        );
    } else if report.meta_skipped_existing {
        println!(
            "  {} .meta already present (use --force to overwrite)",
            "·".bright_black()
        );
    }

    if report.gitignore_updated {
        println!("  {} Updated .gitignore", "✓".green());
    } else {
        println!("  {} .gitignore already current", "·".bright_black());
    }

    if report.skill_installed {
        println!(
            "  {} Installed Claude Code skill at .claude/skills/meta-tool/",
            "✓".green()
        );
    } else if report.skill_already_present {
        println!(
            "  {} Claude Code skill already present at .claude/skills/meta-tool/",
            "·".bright_black()
        );
    }
}

fn install_skill(skill_root: &Path) -> Result<()> {
    fs::create_dir_all(skill_root.join("references"))?;
    fs::write(skill_root.join("SKILL.md"), SKILL_MD)?;
    fs::write(
        skill_root.join("references").join("CHANGELOG_NOTES.md"),
        SKILL_CHANGELOG,
    )?;
    Ok(())
}

/// Append meta-specific ignore patterns if missing. Returns true if any change
/// was written so the caller can report it.
fn update_gitignore<P: AsRef<Path>>(path: P) -> Result<bool> {
    let gitignore_path: PathBuf = path.as_ref().join(".gitignore");

    let mut existing_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let meta_ignores = [
        "# Meta repository ignores",
        ".DS_Store",
        "*.log",
        "node_modules/",
        "target/",
    ];

    let mut updated = false;
    for ignore_line in meta_ignores {
        if !existing_content.contains(ignore_line) {
            if !existing_content.ends_with('\n') && !existing_content.is_empty() {
                existing_content.push('\n');
            }
            existing_content.push_str(ignore_line);
            existing_content.push('\n');
            updated = true;
        }
    }

    if updated {
        fs::write(&gitignore_path, existing_content)?;
    }

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fresh_init_creates_meta_and_gitignore() {
        let tmp = tempdir().unwrap();
        let report = initialize_meta_repo_with_options(tmp.path(), InitOptions::default()).unwrap();
        assert!(report.meta_created);
        assert!(report.gitignore_updated);
        assert!(tmp.path().join(".meta").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn reinit_is_idempotent_and_preserves_meta() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let meta_path = path.join(".meta");

        // Write a hand-tweaked .meta the user might have committed.
        let mut config = MetaConfig::default();
        config.projects.insert(
            "my-project".to_string(),
            metarepo_core::ProjectEntry::Url("https://example.com/x.git".to_string()),
        );
        config.save_to_file(&meta_path).unwrap();

        let report = initialize_meta_repo_with_options(path, InitOptions::default()).unwrap();
        assert!(!report.meta_created);
        assert!(report.meta_skipped_existing);

        // Original user config still intact.
        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();
        assert!(loaded.projects.contains_key("my-project"));
    }

    #[test]
    fn force_overwrites_existing_meta() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let meta_path = path.join(".meta");

        let mut config = MetaConfig::default();
        config.projects.insert(
            "doomed".to_string(),
            metarepo_core::ProjectEntry::Url("https://example.com/x.git".to_string()),
        );
        config.save_to_file(&meta_path).unwrap();

        let options = InitOptions {
            force: true,
            ..InitOptions::default()
        };
        let report = initialize_meta_repo_with_options(path, options).unwrap();
        assert!(report.meta_overwritten);

        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();
        assert!(
            !loaded.projects.contains_key("doomed"),
            "--force must replace existing config with defaults"
        );
    }

    #[test]
    fn repair_refuses_without_existing_meta() {
        let tmp = tempdir().unwrap();
        let options = InitOptions {
            repair: true,
            ..InitOptions::default()
        };
        let err = initialize_meta_repo_with_options(tmp.path(), options).unwrap_err();
        assert!(err.to_string().contains("Cannot repair"));
    }

    #[test]
    fn repair_restores_gitignore_without_touching_meta() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let meta_path = path.join(".meta");

        let mut config = MetaConfig::default();
        config.projects.insert(
            "keep-me".to_string(),
            metarepo_core::ProjectEntry::Url("https://example.com/x.git".to_string()),
        );
        config.save_to_file(&meta_path).unwrap();
        // gitignore intentionally missing

        let options = InitOptions {
            repair: true,
            ..InitOptions::default()
        };
        let report = initialize_meta_repo_with_options(path, options).unwrap();
        assert!(report.meta_skipped_existing);
        assert!(report.gitignore_updated);
        assert!(path.join(".gitignore").exists());

        let loaded = MetaConfig::load_from_file(&meta_path).unwrap();
        assert!(loaded.projects.contains_key("keep-me"));
    }

    #[test]
    fn with_skill_installs_bundled_skill() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let options = InitOptions {
            with_skill: true,
            ..InitOptions::default()
        };
        let report = initialize_meta_repo_with_options(path, options).unwrap();
        assert!(report.skill_installed);
        let skill_md = path.join(".claude/skills/meta-tool/SKILL.md");
        assert!(skill_md.exists());
        let content = fs::read_to_string(&skill_md).unwrap();
        assert!(!content.is_empty(), "embedded SKILL.md must not be empty");
    }

    #[test]
    fn with_skill_is_idempotent_when_already_present() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let options = InitOptions {
            with_skill: true,
            ..InitOptions::default()
        };
        initialize_meta_repo_with_options(path, options).unwrap();
        let report = initialize_meta_repo_with_options(path, options).unwrap();
        assert!(report.skill_already_present);
        assert!(!report.skill_installed);
    }

    #[test]
    fn update_gitignore_appends_missing_lines() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let gitignore_path = path.join(".gitignore");
        fs::write(&gitignore_path, "*.tmp\n").unwrap();

        let changed = update_gitignore(path).unwrap();
        assert!(changed);

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("*.tmp"));
        assert!(content.contains(".DS_Store"));
        assert!(content.contains("node_modules/"));

        // Second call is a no-op.
        let changed_again = update_gitignore(path).unwrap();
        assert!(!changed_again);
    }
}
