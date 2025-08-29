use anyhow::Result;
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
    println!("Creating project '{}' from {}", project_path, repo_url);
    
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
    
    println!("Cloning {} to {}...", repo_url, project_path);
    Repository::clone(repo_url, &full_project_path)?;
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), repo_url.to_string());
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore
    update_gitignore(base_path, project_path)?;
    
    println!("Successfully created project '{}'", project_path);
    println!("Updated .meta file and .gitignore");
    
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
                    
                    println!("Creating symlink from {} to {}", project_path, external_path.display());
                    create_symlink(&external_path, &local_project_path)?;
                    
                    let url = if let Some(detected_url) = remote_url {
                        println!("Detected git remote: {}", detected_url);
                        format!("external:{}", detected_url)
                    } else {
                        println!("No git remote detected, marking as external local project");
                        format!("external:local:{}", external_path.display())
                    };
                    
                    (url, true)
                } else {
                    // Internal directory - just use it as is
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;
                    
                    let url = if let Some(detected_url) = remote_url {
                        println!("Detected git remote: {}", detected_url);
                        detected_url
                    } else {
                        println!("No git remote detected, marking as local project");
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
                println!("Detected git remote: {}", detected_url);
                detected_url
            } else {
                println!("No git remote detected, marking as local project");
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
    
    println!("Importing project '{}' with URL: {}", project_path, final_repo_url);
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), final_repo_url);
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore
    update_gitignore(base_path, project_path)?;
    
    println!("Successfully imported project '{}'", project_path);
    if is_external {
        println!("Created symlink to external directory");
    }
    println!("Updated .meta file and .gitignore");
    
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
        println!("Added '{}' to .gitignore", project_path);
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
        println!("No projects found in workspace.");
        return Ok(());
    }
    
    println!("Projects in workspace:");
    println!("─────────────────────");
    
    for (name, url) in &config.projects {
        let project_path = base_path.join(name);
        
        // Check if it's a symlink
        let is_symlink = project_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        
        let status = if project_path.exists() {
            if is_symlink {
                "⬌ External (symlink)"
            } else if project_path.join(".git").exists() {
                "✓ Present"
            } else {
                "⚠ Present (not a git repo)"
            }
        } else {
            "✗ Missing"
        };
        
        println!("  {} [{}]", name, status);
        
        if url.starts_with("external:local:") {
            let path = url.strip_prefix("external:local:").unwrap();
            println!("    Type: External local project (no remote)");
            println!("    Path: {}", path);
        } else if url.starts_with("external:") {
            let remote_url = url.strip_prefix("external:").unwrap();
            println!("    Type: External project");
            println!("    URL: {}", remote_url);
            if is_symlink {
                if let Ok(target) = std::fs::read_link(&project_path) {
                    println!("    Path: {}", target.display());
                }
            }
        } else if url.starts_with("local:") {
            println!("    Type: Local project (no remote)");
        } else {
            println!("    URL: {}", url);
        }
    }
    
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
            eprintln!("⚠️  Warning: Project '{}' has uncommitted changes!", project_name);
            eprintln!("    Use --force to remove anyway (changes will be lost)");
            eprintln!("    Or commit/stash your changes first.");
            return Err(anyhow::anyhow!("Uncommitted changes detected"));
        }
    }
    
    // Remove from .meta file
    config.projects.remove(project_name);
    config.save_to_file(&meta_file_path)?;
    
    // Remove from .gitignore
    remove_from_gitignore(base_path, project_name)?;
    
    println!("✓ Removed project '{}' from .meta file", project_name);
    
    // Optionally remove the directory
    if project_path.exists() {
        if force {
            std::fs::remove_dir_all(&project_path)?;
            println!("✓ Removed project directory '{}'", project_name);
        } else {
            println!("ℹ️  Project directory '{}' still exists on disk", project_name);
            println!("    To remove it, run: rm -rf {}", project_name);
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
    println!("✓ Removed '{}' from .gitignore", project_name);
    
    Ok(())
}