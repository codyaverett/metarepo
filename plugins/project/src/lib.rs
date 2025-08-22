use anyhow::Result;
use git2::Repository;
use meta_core::MetaConfig;
use std::path::Path;

pub use crate::plugin::ProjectPlugin;

mod plugin;

pub fn create_project(project_path: &str, repo_url: &str, base_path: &Path) -> Result<()> {
    println!("Creating project '{}' from {}", project_path, repo_url);
    
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
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
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
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