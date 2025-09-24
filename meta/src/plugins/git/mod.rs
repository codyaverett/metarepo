use anyhow::Result;
use git2::{RemoteCallbacks, FetchOptions, Cred, CredentialType};
use metarepo_core::MetaConfig;
use std::path::Path;

// Export the main plugin
pub use self::plugin::GitPlugin;

mod plugin;
mod operations;

pub use operations::get_git_status;

pub fn clone_repository(repo_url: &str, target_path: &Path) -> Result<()> {
    if target_path.exists() {
        return Err(anyhow::anyhow!("Target directory already exists: {:?}", target_path));
    }
    
    println!("Cloning {} to {:?}...", repo_url, target_path);
    
    // Set up authentication callbacks for SSH
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            
            // Try SSH agent first
            if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
            
            // Try default SSH key locations
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            for key_name in &["id_rsa", "id_ed25519", "id_ecdsa"] {
                let private_key = format!("{}/.ssh/{}", home, key_name);
                let public_key = format!("{}.pub", private_key);
                
                if std::path::Path::new(&private_key).exists() {
                    if let Ok(cred) = Cred::ssh_key(username, Some(std::path::Path::new(&public_key)), std::path::Path::new(&private_key), None) {
                        return Ok(cred);
                    }
                }
            }
            
            Err(git2::Error::from_str("No valid SSH credentials found"))
        } else {
            Err(git2::Error::from_str("Unsupported authentication type"))
        }
    });
    
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);
    
    builder.clone(repo_url, target_path)?;
    println!("Successfully cloned {}", repo_url);
    
    Ok(())
}

pub fn clone_missing_repos() -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;
    
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    for (project_path, _entry) in &config.projects {
        let full_path = base_path.join(project_path);
        
        if !full_path.exists() {
            if let Some(repo_url) = config.get_project_url(project_path) {
                println!("Cloning missing project: {}", project_path);
                if let Err(e) = clone_repository(&repo_url, &full_path) {
                    eprintln!("Failed to clone {}: {}", project_path, e);
                }
            }
        } else {
            println!("Project {} already exists, skipping", project_path);
        }
    }
    
    Ok(())
}