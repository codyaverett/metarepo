use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::{clone_repository, get_git_status, clone_missing_repos};

pub struct GitPlugin;

impl GitPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl MetaPlugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("git")
                .about("Git operations across multiple repositories")
                .subcommand(
                    Command::new("clone")
                        .about("Clone meta repository and all child repositories")
                        .arg(
                            Arg::new("url")
                                .value_name("REPO_URL")
                                .help("Repository URL to clone")
                                .required(true)
                        )
                )
                .subcommand(
                    Command::new("status")
                        .about("Show git status across all repositories")
                )
                .subcommand(
                    Command::new("update")
                        .about("Clone missing repositories")
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("clone", sub_matches)) => {
                let url = sub_matches.get_one::<String>("url").unwrap();
                println!("Cloning meta repository from: {}", url);
                
                // Extract repo name from URL for directory name
                let repo_name = url.split('/').last()
                    .unwrap_or("meta-repo")
                    .trim_end_matches(".git");
                
                let target_path = config.working_dir.join(repo_name);
                clone_repository(url, &target_path)?;
                
                // After cloning, look for .meta file and clone child repos
                let meta_file = target_path.join(".meta");
                if meta_file.exists() {
                    std::env::set_current_dir(&target_path)?;
                    clone_missing_repos()?;
                }
                
                Ok(())
            }
            Some(("status", _)) => {
                println!("Git status across all repositories:");
                println!("================================");
                
                // Show status for main repo
                println!("\nMain repository:");
                match get_git_status(&config.working_dir) {
                    Ok(status) => println!("{}", status),
                    Err(e) => println!("Error: {}", e),
                }
                
                // Show status for each project
                for (project_path, _repo_url) in &config.meta_config.projects {
                    let full_path = if config.meta_root().is_some() {
                        config.meta_root().unwrap().join(project_path)
                    } else {
                        config.working_dir.join(project_path)
                    };
                    
                    if full_path.exists() {
                        println!("\n{}:", project_path);
                        match get_git_status(&full_path) {
                            Ok(status) => println!("{}", status),
                            Err(e) => println!("Error: {}", e),
                        }
                    } else {
                        println!("\n{}: (not cloned)", project_path);
                    }
                }
                
                Ok(())
            }
            Some(("update", _)) => {
                println!("Cloning missing repositories...");
                clone_missing_repos()?;
                Ok(())
            }
            _ => {
                println!("Unknown git subcommand. Use --help to see available commands.");
                Ok(())
            }
        }
    }
}

impl Default for GitPlugin {
    fn default() -> Self {
        Self::new()
    }
}