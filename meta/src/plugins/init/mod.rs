use anyhow::Result;
use colored::Colorize;
use metarepo_core::{ConfigFormat, MetaConfig, KNOWN_FILENAMES};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Export the plugin
pub use self::plugin::InitPlugin;

mod plugin;

use crate::plugins::skill;

/// User-selected options for `meta init`.
///
/// - `force`: overwrite the existing config with fresh defaults.
/// - `repair`: never write the config file; only restore missing artifacts
///   (gitignore, skill if requested). Useful after a partial setup.
/// - `with_skill`: install the bundled Claude Code skill under
///   `.claude/skills/meta-tool/`.
/// - `all`: shorthand that implies `with_skill` (and any future optional
///   artifacts).
/// - `format`: on-disk format used when creating a fresh config. Defaults to
///   JSON (writing `.metarepo`). Existing configs in any format are detected
///   automatically — `format` only affects the new-file path.
#[derive(Debug, Clone, Copy)]
pub struct InitOptions {
    pub force: bool,
    pub repair: bool,
    pub with_skill: bool,
    pub all: bool,
    pub format: ConfigFormat,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            force: false,
            repair: false,
            with_skill: false,
            all: false,
            format: ConfigFormat::Json,
        }
    }
}

impl InitOptions {
    fn want_skill(&self) -> bool {
        self.with_skill || self.all
    }
}

/// Locate any pre-existing recognized config file in `root`. Returns the
/// matching path so reporting and overwrite logic can name it specifically.
fn find_existing_config(root: &Path) -> Option<PathBuf> {
    KNOWN_FILENAMES
        .iter()
        .map(|name| root.join(name))
        .find(|p| p.exists())
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
    /// Path of the config file we created, overwrote, or detected. None when
    /// no .meta-style file exists and we didn't create one (e.g. `--repair`
    /// against an empty dir, which errors before reaching the report).
    pub config_path: Option<PathBuf>,
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
        modules: None,
        nested: None,
        aliases: None,
        scripts: None,
        worktree_init: None,
        default_bare: None,
        plugins_integrity: None,
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
///
/// Existing config files in any recognized format (`.metarepo`, `.meta`,
/// `.metarepo.yaml`, etc.) are detected so repeated runs don't fight each
/// other. Fresh inits use `options.format` to choose the new filename.
pub fn initialize_meta_repo_with_options<P: AsRef<Path>>(
    path: P,
    options: InitOptions,
) -> Result<InitReport> {
    let root = path.as_ref();
    let existing = find_existing_config(root);
    let mut report = InitReport::default();

    if options.repair {
        let existing = existing.ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot repair: no metarepo config file present in {}. Run 'meta init' first.",
                root.display()
            )
        })?;
        report.meta_skipped_existing = true;
        report.config_path = Some(existing);
    } else if let Some(existing_path) = existing {
        if options.force {
            // Overwrite the existing file in place, preserving its format so
            // we don't accidentally migrate the user without asking.
            let format = ConfigFormat::from_path(&existing_path).unwrap_or(ConfigFormat::Json);
            let config = create_default_config();
            config.save_to_file_with_format(&existing_path, format)?;
            report.meta_overwritten = true;
            report.config_path = Some(existing_path);
        } else {
            report.meta_skipped_existing = true;
            report.config_path = Some(existing_path);
        }
    } else {
        // Fresh init: create a new file in the requested format.
        let new_path = root.join(options.format.canonical_filename());
        let config = create_default_config();
        config.save_to_file_with_format(&new_path, options.format)?;
        report.meta_created = true;
        report.config_path = Some(new_path);
    }

    // --- .gitignore ---
    report.gitignore_updated = update_gitignore(root)?;

    // --- optional skill ---
    if options.want_skill() {
        if skill::is_installed(root) {
            report.skill_already_present = true;
        } else {
            skill::write_skill(root)?;
            report.skill_installed = true;
        }
    }

    Ok(report)
}

fn print_report(report: &InitReport) {
    let path_label = report
        .config_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or(".metarepo");

    if report.meta_created {
        println!(
            "  {} Created {} with default configuration",
            "✓".green(),
            path_label
        );
    } else if report.meta_overwritten {
        println!(
            "  {} Overwrote {} with default configuration (--force)",
            "✓".yellow(),
            path_label
        );
    } else if report.meta_skipped_existing {
        println!(
            "  {} {} already present (use --force to overwrite)",
            "·".bright_black(),
            path_label
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
    fn fresh_init_creates_metarepo_and_gitignore() {
        let tmp = tempdir().unwrap();
        let report = initialize_meta_repo_with_options(tmp.path(), InitOptions::default()).unwrap();
        assert!(report.meta_created);
        assert!(report.gitignore_updated);
        // Fresh inits write the new canonical filename, not the legacy one.
        assert!(tmp.path().join(".metarepo").exists());
        assert!(!tmp.path().join(".meta").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn fresh_init_legacy_meta_is_detected_and_preserved() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        std::fs::write(path.join(".meta"), "{}").unwrap();
        let report = initialize_meta_repo_with_options(path, InitOptions::default()).unwrap();
        assert!(report.meta_skipped_existing);
        // Did not silently create a parallel .metarepo file.
        assert!(!path.join(".metarepo").exists());
        assert!(path.join(".meta").exists());
    }

    #[test]
    fn fresh_init_with_yaml_format_writes_yaml_file() {
        let tmp = tempdir().unwrap();
        let options = InitOptions {
            format: ConfigFormat::Yaml,
            ..InitOptions::default()
        };
        let report = initialize_meta_repo_with_options(tmp.path(), options).unwrap();
        assert!(report.meta_created);
        let written = report.config_path.unwrap();
        assert_eq!(written.file_name().unwrap(), ".metarepo.yaml");
        let raw = fs::read_to_string(&written).unwrap();
        assert!(raw.contains("ignore"), "yaml output should include keys");
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
