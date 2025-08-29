use anyhow::Result;
use git2::{Repository, Status, StatusOptions};
use meta_core::MetaConfig;
use std::path::Path;

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

pub fn import_project(project_path: &str, repo_url: &str, base_path: &Path) -> Result<()> {
    println!("Importing project '{}' from {}", project_path, repo_url);
    
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
    
    let full_project_path = base_path.join(project_path);
    
    // If directory doesn't exist, clone it
    if !full_project_path.exists() {
        println!("Directory doesn't exist, cloning {} to {}...", repo_url, project_path);
        Repository::clone(repo_url, &full_project_path)?;
    } else if !full_project_path.join(".git").exists() {
        return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository", project_path));
    } else {
        println!("Directory '{}' already exists and appears to be a git repository", project_path);
    }
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), repo_url.to_string());
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore
    update_gitignore(base_path, project_path)?;
    
    println!("Successfully imported project '{}'", project_path);
    println!("Updated .meta file and .gitignore");
    
    Ok(())
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
        let status = if project_path.exists() {
            if project_path.join(".git").exists() {
                "✓ Present"
            } else {
                "⚠ Present (not a git repo)"
            }
        } else {
            "✗ Missing"
        };
        
        println!("  {} [{}]", name, status);
        println!("    URL: {}", url);
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