use anyhow::Result;
use git2::Repository;
use meta_core::MetaConfig;
use std::path::Path;

pub use crate::plugin::GitPlugin;

mod plugin;
mod operations;

pub use operations::get_git_status;

pub fn clone_repository(repo_url: &str, target_path: &Path) -> Result<()> {
    if target_path.exists() {
        return Err(anyhow::anyhow!("Target directory already exists: {:?}", target_path));
    }
    
    println!("Cloning {} to {:?}...", repo_url, target_path);
    Repository::clone(repo_url, target_path)?;
    println!("Successfully cloned {}", repo_url);
    
    Ok(())
}

pub fn clone_missing_repos() -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;
    
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    for (project_path, repo_url) in &config.projects {
        let full_path = base_path.join(project_path);
        
        if !full_path.exists() {
            println!("Cloning missing project: {}", project_path);
            if let Err(e) = clone_repository(repo_url, &full_path) {
                eprintln!("Failed to clone {}: {}", project_path, e);
            }
        } else {
            println!("Project {} already exists, skipping", project_path);
        }
    }
    
    Ok(())
}