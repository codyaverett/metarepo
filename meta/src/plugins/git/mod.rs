use anyhow::{Context, Result};
use colored::*;
use metarepo_core::MetaConfig;
use std::path::Path;
use std::process::{Command, Stdio};

// Export the main plugin
pub use self::plugin::GitPlugin;

mod plugin;
mod operations;

pub use operations::get_git_status;

pub fn clone_repository(repo_url: &str, target_path: &Path) -> Result<()> {
    if target_path.exists() {
        return Err(anyhow::anyhow!("Target directory already exists: {:?}", target_path));
    }

    // Extract repo name from URL for cleaner display
    let repo_name = repo_url
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(repo_url);

    println!("Cloning {}...", repo_name.bright_white());

    // Use git clone with --progress for real-time progress display
    let status = Command::new("git")
        .arg("clone")
        .arg("--progress")
        .arg(repo_url)
        .arg(target_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute git clone command")?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to clone repository: git clone exited with status {}",
            status
        ));
    }

    println!("{} Complete\n", "✓".green());

    Ok(())
}

pub fn clone_missing_repos() -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;

    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    // Collect missing projects first to show count
    let missing_projects: Vec<(String, String, std::path::PathBuf)> = config.projects.keys()
        .filter_map(|project_path| {
            let full_path = base_path.join(project_path);
            if !full_path.exists() {
                config.get_project_url(project_path).map(|url| {
                    (project_path.clone(), url, full_path)
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
    println!("Cloning {} missing project{}\n", total, if total == 1 { "" } else { "s" });

    let mut success_count = 0;
    let mut failed_count = 0;

    for (i, (project_path, repo_url, full_path)) in missing_projects.iter().enumerate() {
        let project_name = project_path.rsplit('/').next().unwrap_or(project_path);
        println!("[{}/{}] Cloning {}",
            (i + 1).to_string().cyan(),
            total.to_string().cyan(),
            project_name.bright_white()
        );

        match clone_repository(repo_url, full_path) {
            Ok(_) => success_count += 1,
            Err(e) => {
                eprintln!("{} Failed: {}\n", "✗".red(), e);
                failed_count += 1;
            }
        }
    }

    println!("Summary: {} cloned, {} failed",
        success_count.to_string().green(),
        if failed_count > 0 { failed_count.to_string().red() } else { "0".bright_black() }
    );

    Ok(())
}