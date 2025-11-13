use anyhow::{Context, Result};
use colored::*;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use std::path::Path;
use std::process::Command;

/// Clone a repository with authentication support
pub fn clone_with_auth(url: &str, path: &Path, bare: bool) -> Result<Repository> {
    // Check if this is an SSH URL
    if url.starts_with("git@") || url.starts_with("ssh://") {
        // Set up authentication callbacks for SSH
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            // Get the username (default to "git" for GitHub/GitLab/etc)
            let username = username_from_url.unwrap_or("git");

            // If SSH agent is requested, try it first
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                // Try to find SSH keys in standard locations
                if let Ok(home) = std::env::var("HOME") {
                    let ssh_dir = Path::new(&home).join(".ssh");

                    // Try common SSH key names in order of preference
                    let key_names = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];

                    for key_name in &key_names {
                        let private_key = ssh_dir.join(key_name);
                        if private_key.exists() {
                            // Check if there's a public key as well
                            let public_key = ssh_dir.join(format!("{}.pub", key_name));
                            let public_key_path = if public_key.exists() {
                                Some(public_key.as_path())
                            } else {
                                None
                            };

                            if let Ok(cred) = Cred::ssh_key(
                                username,
                                public_key_path,
                                private_key.as_path(),
                                None, // No passphrase for now
                            ) {
                                return Ok(cred);
                            }
                        }
                    }
                }

                // Try SSH agent as fallback
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }
            }

            // If we couldn't authenticate, return an error
            Err(git2::Error::from_str("SSH authentication failed. Please ensure your SSH keys are set up correctly."))
        });

        // Configure fetch options with our callbacks
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        // Build the repository with authentication
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);

        if bare {
            builder.bare(true);
        }

        // Clone the repository
        builder.clone(url, path).map_err(|e| {
            if e.to_string().contains("authentication") || e.to_string().contains("SSH") {
                anyhow::anyhow!("SSH authentication failed. Please ensure:\n  1. Your SSH key is set up correctly (~/.ssh/id_ed25519 or ~/.ssh/id_rsa)\n  2. The key is added to your GitHub/GitLab account\n  3. You have access to the repository\n\nOriginal error: {}", e)
            } else {
                anyhow::anyhow!("Failed to clone repository: {}", e)
            }
        })
    } else {
        // For HTTPS URLs, use standard clone without authentication callbacks
        if bare {
            let mut builder = git2::build::RepoBuilder::new();
            builder.bare(true);
            builder.clone(url, path).map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))
        } else {
            Repository::clone(url, path).map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))
        }
    }
}

/// Create a default worktree for a bare repository
pub fn create_default_worktree(bare_repo_path: &Path, project_path: &Path) -> Result<()> {
    // Try to detect the default branch
    let default_branch = detect_default_branch(bare_repo_path)?;

    // Create worktree at <project>/<default-branch>/
    let worktree_path = project_path.join(&default_branch);

    let output = Command::new("git")
        .arg("-C")
        .arg(bare_repo_path)
        .arg("worktree")
        .arg("add")
        .arg(&worktree_path)
        .arg(&default_branch)
        .output()
        .context("Failed to create default worktree")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to create default worktree: {}", stderr));
    }

    println!("     {} {}", "✅".green(), format!("Created default worktree: {}", worktree_path.display()).green());

    Ok(())
}

/// Detect the default branch of a repository
pub fn detect_default_branch(repo_path: &Path) -> Result<String> {
    // Try to get the default branch from the remote HEAD
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Output format: "refs/remotes/origin/main"
            if let Some(branch) = stdout.trim().strip_prefix("refs/remotes/origin/") {
                return Ok(branch.to_string());
            }
        }
    }

    // Fallback: try common default branch names
    for branch in &["main", "master", "develop"] {
        let check_output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("rev-parse")
            .arg("--verify")
            .arg(format!("refs/remotes/origin/{}", branch))
            .output();

        if let Ok(output) = check_output {
            if output.status.success() {
                return Ok(branch.to_string());
            }
        }
    }

    // Ultimate fallback
    Ok("main".to_string())
}
