use anyhow::Result;
use clap::{ArgMatches, Command};

pub use crate::plugin::ProjectPlugin;

mod plugin;

pub fn create_project(name: &str, repo_url: &str, base_path: &std::path::Path) -> Result<()> {
    println!("Creating project '{}' from {}", name, repo_url);
    // TODO: Implement actual project creation
    Ok(())
}

pub fn import_project(name: &str, repo_url: &str, base_path: &std::path::Path) -> Result<()> {
    println!("Importing project '{}' from {}", name, repo_url);
    // TODO: Implement actual project import
    Ok(())
}