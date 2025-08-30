use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig, output_format_arg, OutputFormat, format_header};
use serde_json;
use crate::{clone_repository, get_git_status, clone_missing_repos};

pub struct GitPlugin;

impl GitPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest git")
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
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("git")
                .visible_alias("g")
                .about("Git operations across multiple repositories")
                .disable_help_subcommand(true)
                .allow_external_subcommands(true) // This allows unknown subcommands to pass through
                .subcommand(
                    Command::new("clone")
                        .visible_alias("c")
                        .about("Clone meta repository and all child repositories")
                        .arg(
                            Arg::new("url")
                                .value_name("REPO_URL")
                                .help("Repository URL to clone")
                                .required(true)
                        )
                        .arg(output_format_arg())
                )
                .subcommand(
                    Command::new("status")
                        .visible_aliases(["st", "s"])
                        .about("Show git status across all repositories")
                        .arg(output_format_arg())
                )
                .subcommand(
                    Command::new("update")
                        .visible_aliases(["up", "u"])
                        .about("Clone missing repositories")
                        .arg(output_format_arg())
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // If no subcommand is provided, show help
        if matches.subcommand().is_none() {
            return self.show_help();
        }
        
        match matches.subcommand() {
            Some(("clone", sub_matches)) => {
                let url = sub_matches.get_one::<String>("url").unwrap();
                let output_format = self.get_output_format(sub_matches);
                
                match output_format {
                    OutputFormat::Human => println!("Cloning meta repository from: {}", url),
                    OutputFormat::Ai => println!("## Cloning Meta Repository\n\n- **Source**: `{}`", url),
                    _ => {},
                }
                
                // Extract repo name from URL for directory name
                let repo_name = url.split('/').last()
                    .unwrap_or("meta-repo")
                    .trim_end_matches(".git");
                
                let target_path = config.working_dir.join(repo_name);
                clone_repository(url, &target_path, output_format)?;
                
                // After cloning, look for .meta file and clone child repos
                let meta_file = target_path.join(".meta");
                if meta_file.exists() {
                    std::env::set_current_dir(&target_path)?;
                    clone_missing_repos(output_format)?;
                }
                
                Ok(())
            }
            Some(("status", sub_matches)) => {
                let output_format = self.get_output_format(sub_matches);
                
                match output_format {
                    OutputFormat::Human => {
                        println!("{}", format_header("Git Status Across All Repositories", output_format));
                        println!();
                    },
                    OutputFormat::Ai => println!("## Git Status Across All Repositories\n"),
                    _ => {},
                }
                
                let mut all_statuses = Vec::new();
                
                // Show status for main repo
                if output_format == OutputFormat::Human {
                    println!("Main repository:");
                }
                
                match get_git_status(&config.working_dir, output_format) {
                    Ok(status) => {
                        match output_format {
                            OutputFormat::Human => println!("{}", status),
                            OutputFormat::Ai => println!("### Main Repository\n\n{}", status),
                            OutputFormat::Json => {
                                let parsed: serde_json::Value = serde_json::from_str(&status).unwrap_or_default();
                                all_statuses.push(serde_json::json!({
                                    "project": ".",
                                    "status": parsed
                                }));
                            },
                        }
                    },
                    Err(e) => {
                        match output_format {
                            OutputFormat::Human => println!("Error: {}", e),
                            OutputFormat::Ai => println!("### Main Repository\n\n✗ **Error**: {}", e),
                            OutputFormat::Json => {
                                all_statuses.push(serde_json::json!({
                                    "project": ".",
                                    "error": e.to_string()
                                }));
                            },
                        }
                    },
                }
                
                // Show status for each project
                for (project_path, _repo_url) in &config.meta_config.projects {
                    let full_path = if config.meta_root().is_some() {
                        config.meta_root().unwrap().join(project_path)
                    } else {
                        config.working_dir.join(project_path)
                    };
                    
                    if full_path.exists() {
                        if output_format == OutputFormat::Human {
                            println!("\n{}:", project_path);
                        }
                        
                        match get_git_status(&full_path, output_format) {
                            Ok(status) => {
                                match output_format {
                                    OutputFormat::Human => println!("{}", status),
                                    OutputFormat::Ai => println!("\n### {}\n\n{}", project_path, status),
                                    OutputFormat::Json => {
                                        let parsed: serde_json::Value = serde_json::from_str(&status).unwrap_or_default();
                                        all_statuses.push(serde_json::json!({
                                            "project": project_path,
                                            "status": parsed
                                        }));
                                    },
                                }
                            },
                            Err(e) => {
                                match output_format {
                                    OutputFormat::Human => println!("Error: {}", e),
                                    OutputFormat::Ai => println!("\n### {}\n\n✗ **Error**: {}", project_path, e),
                                    OutputFormat::Json => {
                                        all_statuses.push(serde_json::json!({
                                            "project": project_path,
                                            "error": e.to_string()
                                        }));
                                    },
                                }
                            },
                        }
                    } else {
                        match output_format {
                            OutputFormat::Human => println!("\n{}: (not cloned)", project_path),
                            OutputFormat::Ai => println!("\n### {}\n\n⚠ **Warning**: Not cloned", project_path),
                            OutputFormat::Json => {
                                all_statuses.push(serde_json::json!({
                                    "project": project_path,
                                    "status": "not_cloned"
                                }));
                            },
                        }
                    }
                }
                
                if output_format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&all_statuses)?);
                }
                
                Ok(())
            }
            Some(("update", sub_matches)) => {
                let output_format = self.get_output_format(sub_matches);
                
                match output_format {
                    OutputFormat::Human => println!("Cloning missing repositories..."),
                    OutputFormat::Ai => println!("## Cloning Missing Repositories\n"),
                    _ => {},
                }
                
                clone_missing_repos(output_format)?;
                Ok(())
            }
            Some((external_cmd, _args)) => {
                // Handle unknown/external subcommands by showing help
                println!("Unknown git subcommand: '{}'", external_cmd);
                println!();
                self.show_help()
            }
            None => {
                // This case is already handled above, but keeping for completeness
                self.show_help()
            }
        }
    }
    
    fn supports_output_format(&self) -> bool {
        true
    }
}

impl Default for GitPlugin {
    fn default() -> Self {
        Self::new()
    }
}