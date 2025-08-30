use anyhow::Result;
use git2::{RemoteCallbacks, FetchOptions, Cred, CredentialType};
use meta_core::{MetaConfig, OutputFormat, format_success, format_info, format_error};
use serde_json;
use std::path::Path;

pub use crate::plugin::GitPlugin;

mod plugin;
mod operations;

pub use operations::get_git_status;

pub fn clone_repository(repo_url: &str, target_path: &Path, output_format: OutputFormat) -> Result<()> {
    if target_path.exists() {
        return Err(anyhow::anyhow!("Target directory already exists: {:?}", target_path));
    }
    
    match output_format {
        OutputFormat::Human => println!("Cloning {} to {:?}...", repo_url, target_path),
        OutputFormat::Ai => println!("- **Cloning**: `{}` → `{:?}`", repo_url, target_path),
        OutputFormat::Json => {},
    }
    
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
    
    match output_format {
        OutputFormat::Human => println!("{}", format_success(&format!("Successfully cloned {}", repo_url), output_format)),
        OutputFormat::Ai => println!("✓ **Cloned**: `{}`", repo_url),
        OutputFormat::Json => {
            let result = serde_json::json!({
                "status": "success",
                "action": "clone",
                "repository": repo_url,
                "path": target_path
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        },
    }
    
    Ok(())
}

pub fn clone_missing_repos(output_format: OutputFormat) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;
    
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    let mut results = Vec::new();
    
    for (project_path, repo_url) in &config.projects {
        let full_path = base_path.join(project_path);
        
        if !full_path.exists() {
            match output_format {
                OutputFormat::Human => println!("Cloning missing project: {}", project_path),
                _ => {},
            }
            
            match clone_repository(repo_url, &full_path, output_format) {
                Ok(_) => results.push((project_path.clone(), "cloned".to_string(), true)),
                Err(e) => {
                    match output_format {
                        OutputFormat::Human => eprintln!("{}", format_error(&format!("Failed to clone {}: {}", project_path, e), output_format)),
                        _ => {},
                    }
                    results.push((project_path.clone(), format!("error: {}", e), false));
                }
            }
        } else {
            match output_format {
                OutputFormat::Human => println!("{}", format_info(&format!("Project {} already exists, skipping", project_path), output_format)),
                _ => {},
            }
            results.push((project_path.clone(), "exists".to_string(), true));
        }
    }
    
    // Output summary for AI and JSON formats
    match output_format {
        OutputFormat::Ai => {
            println!("\n## Clone Summary");
            for (path, status, success) in &results {
                let symbol = if *success { "✓" } else { "✗" };
                println!("{} **{}**: {}", symbol, path, status);
            }
        },
        OutputFormat::Json => {
            let json_results: Vec<_> = results.iter().map(|(path, status, success)| {
                serde_json::json!({
                    "project": path,
                    "status": status,
                    "success": success
                })
            }).collect();
            println!("{}", serde_json::to_string_pretty(&json_results)?);
        },
        _ => {},
    }
    
    Ok(())
}