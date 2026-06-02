//! Discover a meta module inside a freshly-added project repo and, in a TTY,
//! offer to enable it. Activation is always explicit — non-interactive runs only
//! print the command to run.

use anyhow::Result;
use colored::Colorize;
use metarepo_core::{is_interactive, prompt_confirm, MetaModuleManifest, NonInteractiveMode};
use std::path::Path;

use super::enable;

/// If `repo` contains a module manifest, surface it and (interactively) prompt
/// to enable it.
pub fn offer_enable(
    repo: &Path,
    meta_file: &Path,
    non_interactive: NonInteractiveMode,
) -> Result<()> {
    let Some(manifest_path) = MetaModuleManifest::find_in_dir(repo) else {
        return Ok(());
    };
    let manifest = match MetaModuleManifest::from_file_auto(&manifest_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "  {} found {} but it failed to parse: {}",
                "⚠".yellow(),
                manifest_path.display(),
                e
            );
            return Ok(());
        }
    };

    println!(
        "\n  📦 this repo is a meta module: {} v{} ({} plugin(s), {} skill(s))",
        manifest.module.name.cyan().bold(),
        manifest.module.version,
        manifest.module.plugins.len(),
        manifest.module.skills.len(),
    );

    if !is_interactive() {
        println!(
            "  {} run 'meta module enable {}' to wire it up",
            "·".bright_black(),
            repo.display()
        );
        return Ok(());
    }

    let yes = prompt_confirm(
        &format!("Enable module '{}' now?", manifest.module.name),
        false,
        non_interactive,
    )?;
    if yes {
        enable::enable(repo, meta_file, false, false)?;
    } else {
        println!(
            "  {} skipped — run 'meta module enable {}' later",
            "·".bright_black(),
            repo.display()
        );
    }
    Ok(())
}
