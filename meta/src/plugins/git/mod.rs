use anyhow::Result;
use colored::*;
use metarepo_core::MetaConfig;
use std::path::Path;

// Export the main plugin
pub use self::plugin::GitPlugin;

mod operations;
mod plugin;

pub use operations::get_git_status;

// Import shared git operations
use crate::plugins::shared::{clone_with_auth, create_default_worktree};

pub fn clone_repository(repo_url: &str, target_path: &Path, bare: bool) -> Result<()> {
    if target_path.exists() {
        return Err(anyhow::anyhow!(
            "Target directory already exists: {:?}",
            target_path
        ));
    }

    // Extract repo name from URL for cleaner display
    let repo_name = repo_url
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(repo_url);

    if bare {
        println!("Cloning {} as bare repository...", repo_name.bright_white());

        // Clone as bare repo to <project>/.git/
        let bare_path = target_path.join(".git");
        clone_with_auth(repo_url, &bare_path, true)?;

        // Create the project directory
        std::fs::create_dir_all(target_path)?;

        // Create default worktree at <project>/<default-branch>/
        println!("Creating default worktree...");
        create_default_worktree(&bare_path, target_path)?;

        println!("{} Complete\n", "✓".green());
    } else {
        println!("Cloning {}...", repo_name.bright_white());

        // Use shared clone_with_auth for consistent cloning behavior
        clone_with_auth(repo_url, target_path, false)?;

        println!("{} Complete\n", "✓".green());
    }

    Ok(())
}

pub fn clone_missing_repos() -> Result<()> {
    let meta_file =
        MetaConfig::find_meta_file().ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;

    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    // Collect missing projects first to show count
    let missing_projects: Vec<(String, String, std::path::PathBuf, bool)> = config
        .projects
        .keys()
        .filter_map(|project_path| {
            let full_path = base_path.join(project_path);
            if !full_path.exists() {
                config.get_project_url(project_path).map(|url| {
                    let is_bare = config.is_bare_repo(project_path);
                    (project_path.clone(), url, full_path, is_bare)
                })
            } else {
                None
            }
        })
        .collect();

    if missing_projects.is_empty() {
        println!("All projects already exist");
        return Ok(());
    }

    let total = missing_projects.len();
    println!(
        "Cloning {} missing project{}\n",
        total,
        if total == 1 { "" } else { "s" }
    );

    let mut success_count = 0;
    let mut failed_count = 0;

    for (i, (project_path, repo_url, full_path, is_bare)) in missing_projects.iter().enumerate() {
        let project_name = project_path.rsplit('/').next().unwrap_or(project_path);
        println!(
            "[{}/{}] Cloning {}",
            (i + 1).to_string().cyan(),
            total.to_string().cyan(),
            project_name.bright_white()
        );

        match clone_repository(repo_url, full_path, *is_bare) {
            Ok(_) => success_count += 1,
            Err(e) => {
                eprintln!("{} Failed: {}\n", "✗".red(), e);
                failed_count += 1;
            }
        }
    }

    println!(
        "Summary: {} cloned, {} failed",
        success_count.to_string().green(),
        if failed_count > 0 {
            failed_count.to_string().red()
        } else {
            "0".bright_black()
        }
    );

    Ok(())
}
