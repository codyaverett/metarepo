use anyhow::Result;
use clap::{ArgMatches, Command};

pub use crate::plugin::GitPlugin;

mod plugin;
mod operations;

pub fn clone_repository(repo_url: &str, target_path: &std::path::Path) -> Result<()> {
    use git2::Repository;
    
    if target_path.exists() {
        return Err(anyhow::anyhow!("Target directory already exists: {:?}", target_path));
    }
    
    println!("Cloning {} to {:?}...", repo_url, target_path);
    Repository::clone(repo_url, target_path)?;
    println!("Successfully cloned {}", repo_url);
    
    Ok(())
}