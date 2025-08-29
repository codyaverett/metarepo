use anyhow::Result;
use colored::*;
use git2::{Repository, Status, StatusOptions};
use meta_core::MetaConfig;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs;

#[cfg(windows)]
use std::os::windows::fs;

pub use crate::plugin::ProjectPlugin;

mod plugin;

pub fn create_project(project_path: &str, repo_url: &str, base_path: &Path) -> Result<()> {
    println!("\n  {} {}", "🌱".green(), "Creating new project...".bold());
    println!("     {} {}", "Name:".bright_black(), project_path.bright_white());
    println!("     {} {}", "Source:".bright_black(), repo_url.bright_cyan());
    
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'gest init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project already exists in config
    if config.projects.contains_key(project_path) {
        return Err(anyhow::anyhow!("Project '{}' already exists in .meta file", project_path));
    }
    
    // Clone the repository
    let full_project_path = base_path.join(project_path);
    if full_project_path.exists() {
        return Err(anyhow::anyhow!("Directory '{}' already exists", project_path));
    }
    
    println!("     {} {}", "Status:".bright_black(), "Cloning repository...".yellow());
    Repository::clone(repo_url, &full_project_path)?;
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), repo_url.to_string());
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore
    update_gitignore(base_path, project_path)?;
    
    println!("\n  {} {}", "✅".green(), format!("Successfully created '{}'", project_path).bold().green());
    println!("     {} {}", "└".bright_black(), "Updated .meta file and .gitignore".italic().bright_black());
    println!();
    
    Ok(())
}

pub fn import_project(project_path: &str, source: Option<&str>, base_path: &Path) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'gest init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project already exists in config
    if config.projects.contains_key(project_path) {
        return Err(anyhow::anyhow!("Project '{}' already exists in .meta file", project_path));
    }
    
    let local_project_path = base_path.join(project_path);
    
    // Determine what the source is and how to handle it
    let (final_repo_url, is_external) = if let Some(src) = source {
        if !src.starts_with("http") && !src.starts_with("git@") && !src.starts_with("ssh://") {
            // This is a local path (relative or absolute)
            let external_path = if src.starts_with('/') {
                PathBuf::from(src)
            } else {
                // Resolve relative path from current working directory or base path
                let resolved = base_path.join(src).canonicalize()
                    .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(src).canonicalize()).unwrap_or(Ok(PathBuf::from(src))))
                    .unwrap_or_else(|_| PathBuf::from(src));
                resolved
            };
            
            // Check if this path is outside the workspace (external)
            let is_external_dir = !external_path.starts_with(base_path) || external_path == base_path.join(project_path);
            
            if external_path.exists() && external_path.join(".git").exists() {
                if is_external_dir {
                    // External directory exists and is a git repo - create symlink
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;
                    
                    // Create symlink to external directory
                    if local_project_path.exists() {
                        return Err(anyhow::anyhow!("Directory '{}' already exists", project_path));
                    }
                    
                    println!("\n  {} {}", "🔗".cyan(), "Creating symlink...".bold());
                    println!("     {} {}", "From:".bright_black(), project_path.bright_white());
                    println!("     {} {}", "To:".bright_black(), external_path.display().to_string().bright_magenta());
                    create_symlink(&external_path, &local_project_path)?;
                    
                    let url = if let Some(detected_url) = remote_url {
                        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                        format!("external:{}", detected_url)
                    } else {
                        println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                        format!("external:local:{}", external_path.display())
                    };
                    
                    (url, true)
                } else {
                    // Internal directory - just use it as is
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;
                    
                    let url = if let Some(detected_url) = remote_url {
                        println!("\n  {} {}", "📍".green(), "Using existing directory".bold());
                        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                        detected_url
                    } else {
                        println!("\n  {} {}", "📍".yellow(), "Using existing directory".bold());
                        println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                        format!("local:{}", project_path)
                    };
                    
                    (url, false)
                }
            } else if external_path.exists() {
                return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository", external_path.display()));
            } else {
                // Path doesn't exist - treat as URL for cloning
                (src.to_string(), false)
            }
        } else {
            // Regular git URL
            (src.to_string(), false)
        }
    } else {
        // No URL provided, check if directory exists locally
        if local_project_path.exists() && local_project_path.join(".git").exists() {
            let repo = Repository::open(&local_project_path)?;
            let remote_url = get_remote_url(&repo)?;
            
            let url = if let Some(detected_url) = remote_url {
                println!("\n  {} {}", "📍".green(), "Using existing directory".bold());
                println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                detected_url
            } else {
                println!("\n  {} {}", "📍".yellow(), "Using existing directory".bold());
                println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                format!("local:{}", project_path)
            };
            
            (url, false)
        } else if local_project_path.exists() {
            return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository", project_path));
        } else {
            return Err(anyhow::anyhow!("Directory '{}' doesn't exist and no repository URL provided", project_path));
        }
    };
    
    // If not external and directory doesn't exist, clone it
    if !is_external && !local_project_path.exists() {
        if !final_repo_url.starts_with("local:") && !final_repo_url.starts_with("external:") {
            println!("Cloning {} to {}...", final_repo_url, project_path);
            Repository::clone(&final_repo_url, &local_project_path)?;
        } else {
            return Err(anyhow::anyhow!("Cannot clone a local project URL"));
        }
    }
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), final_repo_url.clone());
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore
    update_gitignore(base_path, project_path)?;
    
    // Success message
    println!("\n  {} {}", "✅".green(), format!("Successfully imported '{}'", project_path).bold().green());
    
    if is_external {
        println!("     {} {}", "└".bright_black(), "Created symlink to external directory".italic().bright_black());
    }
    println!("     {} {}", "└".bright_black(), "Updated .meta file and .gitignore".italic().bright_black());
    println!();
    
    Ok(())
}

fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::symlink(target, link)?;
        Ok(())
    }
    
    #[cfg(windows)]
    {
        // On Windows, try to create a directory symlink
        // This requires admin privileges or developer mode
        if target.is_dir() {
            fs::symlink_dir(target, link)?;
        } else {
            fs::symlink_file(target, link)?;
        }
        Ok(())
    }
    
    #[cfg(not(any(unix, windows)))]
    {
        Err(anyhow::anyhow!("Symbolic links are not supported on this platform"))
    }
}

fn get_remote_url(repo: &Repository) -> Result<Option<String>> {
    // Try to get the 'origin' remote first, then fallback to first available remote
    let remote_names = repo.remotes()?;
    
    // First try 'origin'
    if remote_names.iter().any(|n| n == Some("origin")) {
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }
    
    // Fallback to first available remote
    for name in remote_names.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }
    
    Ok(None)
}

fn update_gitignore(base_path: &Path, project_path: &str) -> Result<()> {
    let gitignore_path = base_path.join(".gitignore");
    
    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };
    
    // Check if project path is already ignored
    if !content.lines().any(|line| line.trim() == project_path) {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(project_path);
        content.push('\n');
        
        std::fs::write(&gitignore_path, content)?;
        // Silent - shown in summary
    }
    
    Ok(())
}

pub fn list_projects(base_path: &Path) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'gest init' first."));
    }
    
    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    if config.projects.is_empty() {
        println!("\n  {} {}", "📦".bright_blue(), "No projects found in workspace".dimmed());
        println!("  {} {}", "".dimmed(), "Use 'gest project import' to add projects".dimmed());
        println!();
        return Ok(());
    }
    
    println!("\n  {} {}", "📦".bright_blue(), "Workspace Projects".bold());
    println!("  {}", "═".repeat(60).bright_black());
    
    for (name, url) in &config.projects {
        let project_path = base_path.join(name);
        
        // Check if it's a symlink
        let is_symlink = project_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        
        let (status_icon, status_text, status_color) = if project_path.exists() {
            if is_symlink {
                ("🔗", "External", "cyan")
            } else if project_path.join(".git").exists() {
                ("✅", "Active", "green")
            } else {
                ("⚠️ ", "No Git", "yellow")
            }
        } else {
            ("❌", "Missing", "red")
        };
        
        // Project name and status
        println!();
        print!("  {} {}", status_icon, name.bold());
        
        match status_color {
            "green" => println!(" {}", format!("[{}]", status_text).green()),
            "cyan" => println!(" {}", format!("[{}]", status_text).cyan()),
            "yellow" => println!(" {}", format!("[{}]", status_text).yellow()),
            "red" => println!(" {}", format!("[{}]", status_text).red()),
            _ => println!(" [{}]", status_text),
        }
        
        // Project details with proper indentation and styling
        if url.starts_with("external:local:") {
            let path = url.strip_prefix("external:local:").unwrap();
            println!("  {}  {} {}", "│".bright_black(), "Type:".bright_black(), "Local (no remote)".italic());
            println!("  {}  {} {}", "│".bright_black(), "Path:".bright_black(), path.bright_white());
        } else if url.starts_with("external:") {
            let remote_url = url.strip_prefix("external:").unwrap();
            println!("  {}  {} {}", "│".bright_black(), "Type:".bright_black(), "External".cyan().italic());
            println!("  {}  {} {}", "│".bright_black(), "Remote:".bright_black(), remote_url.bright_white());
            if is_symlink {
                if let Ok(target) = std::fs::read_link(&project_path) {
                    println!("  {}  {} {}", "└".bright_black(), "Links to:".bright_black(), target.display().to_string().bright_magenta());
                }
            }
        } else if url.starts_with("local:") {
            println!("  {}  {} {}", "└".bright_black(), "Type:".bright_black(), "Local (no remote)".italic());
        } else {
            println!("  {}  {} {}", "└".bright_black(), "Remote:".bright_black(), url.bright_white());
        }
    }
    
    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {} projects total\n", config.projects.len().to_string().cyan().bold(), "workspace".dimmed());
    
    Ok(())
}

pub fn remove_project(project_name: &str, base_path: &Path, force: bool) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'gest init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project exists in config
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!("Project '{}' not found in .meta file", project_name));
    }
    
    let project_path = base_path.join(project_name);
    
    // Check for uncommitted changes if directory exists
    if project_path.exists() && project_path.join(".git").exists() && !force {
        let repo = Repository::open(&project_path)?;
        
        // Check for uncommitted changes
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        status_opts.include_ignored(false);
        
        let statuses = repo.statuses(Some(&mut status_opts))?;
        
        let has_changes = statuses.iter().any(|entry| {
            let status = entry.status();
            status.intersects(
                Status::INDEX_NEW | Status::INDEX_MODIFIED | Status::INDEX_DELETED |
                Status::INDEX_RENAMED | Status::INDEX_TYPECHANGE |
                Status::WT_NEW | Status::WT_MODIFIED | Status::WT_DELETED |
                Status::WT_TYPECHANGE | Status::WT_RENAMED
            )
        });
        
        if has_changes {
            eprintln!("\n  {} {}", "⚠️".yellow(), format!("Project '{}' has uncommitted changes!", project_name).bold().yellow());
            eprintln!("     {} {}", "│".bright_black(), "Use --force to remove anyway (changes will be lost)".bright_red());
            eprintln!("     {} {}", "└".bright_black(), "Or commit/stash your changes first".bright_white());
            eprintln!();
            return Err(anyhow::anyhow!("Uncommitted changes detected"));
        }
    }
    
    // Remove from .meta file
    config.projects.remove(project_name);
    config.save_to_file(&meta_file_path)?;
    
    // Remove from .gitignore
    remove_from_gitignore(base_path, project_name)?;
    
    println!("\n  {} {}", "🗑".red(), format!("Removed project '{}'", project_name).bold());
    println!("     {} {}", "└".bright_black(), "Removed from .meta file".italic().bright_black());
    
    // Optionally remove the directory
    if project_path.exists() {
        if force {
            std::fs::remove_dir_all(&project_path)?;
            println!("     {} {}", "└".bright_black(), format!("Deleted directory '{}'", project_name).italic().bright_red());
        } else {
            println!("     {} {}", "└".bright_black(), format!("Directory '{}' kept on disk", project_name).italic().bright_black());
            println!("     {} {}", " ".bright_black(), format!("To remove: rm -rf {}", project_name).dimmed());
        }
    }
    
    Ok(())
}

fn remove_from_gitignore(base_path: &Path, project_name: &str) -> Result<()> {
    let gitignore_path = base_path.join(".gitignore");
    
    if !gitignore_path.exists() {
        return Ok(());
    }
    
    let content = std::fs::read_to_string(&gitignore_path)?;
    let new_content: Vec<&str> = content
        .lines()
        .filter(|line| line.trim() != project_name)
        .collect();
    
    std::fs::write(&gitignore_path, new_content.join("\n") + "\n")?;
    // Silent - shown in summary
    
    Ok(())
}