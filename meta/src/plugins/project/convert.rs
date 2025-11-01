use anyhow::{Context, Result};
use colored::*;
use metarepo_core::{MetaConfig, ProjectEntry, ProjectMetadata};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Convert a normal repository to a bare repository with worktrees
pub fn convert_to_bare(project_name: &str, base_path: &Path) -> Result<()> {
    // Load configuration
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Check if project exists
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!("Project '{}' not found in workspace", project_name));
    }

    let project_path = base_path.join(project_name);

    // Check if directory exists
    if !project_path.exists() {
        return Err(anyhow::anyhow!("Project directory '{}' does not exist", project_name));
    }

    // Check if it's already a bare repository
    if config.is_bare_repo(project_name) {
        println!("\n  {} {}", "ℹ".cyan(), "Project is already configured as a bare repository".cyan());
        return Ok(());
    }

    // Check if .git exists
    if !project_path.join(".git").exists() {
        return Err(anyhow::anyhow!("Project '{}' is not a git repository", project_name));
    }

    println!("\n  {} {}", "⚠️".yellow(), "Converting to Bare Repository".bold().yellow());
    println!("  {}", "═".repeat(60).bright_black());
    println!("\n  {} This operation will:", "ℹ".cyan());
    println!("     {} Convert {} to a bare repository", "•".bright_black(), project_name.bright_white());
    println!("     {} Create a worktree for the current branch", "•".bright_black());
    println!("     {} Update the .meta configuration", "•".bright_black());
    println!("\n  {} {}", "⚠️".yellow(), "Warning: This operation modifies your repository structure!".yellow());
    println!("  {} {}", "".bright_black(), "Make sure you have committed all changes before proceeding.".bright_black());

    // Check for uncommitted changes
    let status_output = Command::new("git")
        .arg("-C")
        .arg(&project_path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .context("Failed to check git status")?;

    if !status_output.stdout.is_empty() {
        println!("\n  {} {}", "❌".red(), "Uncommitted changes detected!".red());
        println!("     {} {}", "└".bright_black(), "Commit or stash your changes first".bright_black());
        return Err(anyhow::anyhow!("Cannot convert repository with uncommitted changes"));
    }

    // Get current branch
    let branch_output = Command::new("git")
        .arg("-C")
        .arg(&project_path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .context("Failed to get current branch")?;

    let current_branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    if current_branch.is_empty() {
        return Err(anyhow::anyhow!("Could not determine current branch"));
    }

    println!("\n  {} Current branch: {}", "📍".cyan(), current_branch.bright_white());

    // Prompt for confirmation
    use std::io::{self, Write};
    print!("\n  {} Continue with conversion? [y/N]: ", "→".bright_black());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();

    if response != "y" && response != "yes" {
        println!("\n  {} {}", "ℹ".bright_black(), "Conversion cancelled".bright_black());
        return Ok(());
    }

    println!("\n  {} {}", "🔄".blue(), "Starting conversion...".bold());

    // Step 1: Move .git to .git.tmp
    println!("\n  {} Backing up .git directory...", "1️⃣".blue());
    let git_backup = project_path.join(".git.tmp");
    std::fs::rename(project_path.join(".git"), &git_backup)
        .context("Failed to backup .git directory")?;
    println!("     {} {}", "✅".green(), "Backed up to .git.tmp".green());

    // Step 2: Clone as bare repository
    println!("\n  {} Creating bare repository...", "2️⃣".blue());
    let bare_path = project_path.join(".git");

    // Clone from the backup
    let clone_output = Command::new("git")
        .arg("clone")
        .arg("--bare")
        .arg(&git_backup)
        .arg(&bare_path)
        .output();

    match clone_output {
        Ok(output) if output.status.success() => {
            println!("     {} {}", "✅".green(), "Created bare repository".green());
        }
        _ => {
            // Restore on failure
            println!("     {} {}", "❌".red(), "Failed to create bare repository".red());
            println!("     {} Restoring original .git...", "🔄".yellow());
            if git_backup.exists() {
                std::fs::rename(&git_backup, project_path.join(".git")).ok();
            }
            return Err(anyhow::anyhow!("Failed to clone as bare repository"));
        }
    }

    // Step 3: Create worktree for current branch
    println!("\n  {} Creating worktree for '{}'...", "3️⃣".blue(), current_branch.bright_white());
    let worktree_path = project_path.join(&current_branch);

    let worktree_output = Command::new("git")
        .arg("-C")
        .arg(&bare_path)
        .arg("worktree")
        .arg("add")
        .arg(&worktree_path)
        .arg(&current_branch)
        .output()
        .context("Failed to create worktree")?;

    if !worktree_output.status.success() {
        let stderr = String::from_utf8_lossy(&worktree_output.stderr);
        println!("     {} {}", "❌".red(), format!("Failed: {}", stderr.trim()).red());

        // Cleanup on failure
        println!("     {} Cleaning up...", "🔄".yellow());
        std::fs::remove_dir_all(&bare_path).ok();
        if git_backup.exists() {
            std::fs::rename(&git_backup, project_path.join(".git")).ok();
        }

        return Err(anyhow::anyhow!("Failed to create worktree"));
    }

    println!("     {} {}", "✅".green(), format!("Created at {}", worktree_path.display()).green());

    // Step 4: Remove backup
    println!("\n  {} Removing backup...", "4️⃣".blue());
    std::fs::remove_dir_all(&git_backup)
        .context("Failed to remove backup")?;
    println!("     {} {}", "✅".green(), "Backup removed".green());

    // Step 5: Update .meta configuration
    println!("\n  {} Updating .meta configuration...", "5️⃣".blue());

    // Get project URL
    let project_url = config.get_project_url(project_name)
        .ok_or_else(|| anyhow::anyhow!("Could not get project URL"))?;

    // Update to ProjectMetadata format with bare flag
    config.projects.insert(
        project_name.to_string(),
        ProjectEntry::Metadata(ProjectMetadata {
            url: project_url,
            aliases: Vec::new(),
            scripts: HashMap::new(),
            env: HashMap::new(),
            worktree_init: None,
            bare: Some(true),
        }),
    );

    config.save_to_file(&meta_file_path)?;
    println!("     {} {}", "✅".green(), "Configuration updated".green());

    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {}", "✅".green(), "Conversion complete!".bold().green());
    println!("\n  {} Next steps:", "ℹ".cyan());
    println!("     {} Your current branch is now at: {}", "•".bright_black(), worktree_path.display().to_string().bright_white());
    println!("     {} Create new worktrees with: {}", "•".bright_black(), format!("meta worktree add <branch> --project {}", project_name).bright_cyan());
    println!("     {} New worktrees will be created at: {}/", "•".bright_black(), project_path.display().to_string().bright_white());
    println!();

    Ok(())
}
