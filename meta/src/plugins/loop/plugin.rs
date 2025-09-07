use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};
use std::process::Command as ProcessCommand;
use std::path::Path;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaConfig {
    #[serde(default)]
    pub projects: HashMap<String, String>,
}

pub struct LoopPlugin;

impl LoopPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn execute_in_projects(&self, command: &str, args: &[&str], projects: Vec<(String, String)>, filters: &LoopFilters) -> Result<()> {
        let mut filtered_projects = projects;
        
        // Apply filters
        if !filters.include_patterns.is_empty() {
            filtered_projects.retain(|(project_path, _)| {
                let project_name = Path::new(project_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(project_path);
                
                filters.include_patterns.iter().any(|pattern| {
                    // Support exact match or substring match
                    project_name == pattern || project_path == pattern || 
                    project_name.contains(pattern) || project_path.contains(pattern)
                })
            });
        }
        
        if !filters.exclude_patterns.is_empty() {
            filtered_projects.retain(|(project_path, _)| {
                let project_name = Path::new(project_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(project_path);
                
                !filters.exclude_patterns.iter().any(|pattern| {
                    // Support exact match or substring match
                    project_name == pattern || project_path == pattern || 
                    project_name.contains(pattern) || project_path.contains(pattern)
                })
            });
        }
        
        if filters.existing_only {
            filtered_projects.retain(|(name, _)| Path::new(name).exists());
        }
        
        if filters.git_only {
            filtered_projects.retain(|(name, _)| Path::new(name).join(".git").exists());
        }
        
        if filtered_projects.is_empty() {
            println!("No projects matched the criteria");
            return Ok(());
        }
        
        println!("Executing command in {} projects", filtered_projects.len());
        println!("Command: {} {}", command, args.join(" "));
        println!();
        
        for (idx, (project_path, repo_url)) in filtered_projects.iter().enumerate() {
            let project_name = Path::new(project_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(project_path);
            
            println!("[{}/{}] {}", idx + 1, filtered_projects.len(), project_name);
            println!("  Path: {}", project_path);
            
            let path = Path::new(project_path);
            if !path.exists() {
                println!("  ⚠️  Project directory does not exist, skipping");
                continue;
            }
            
            let mut cmd = ProcessCommand::new(command);
            cmd.args(args)
               .current_dir(path)
               .env("META_PROJECT_NAME", project_name)
               .env("META_PROJECT_PATH", project_path)
               .env("META_PROJECT_REPO", repo_url);
            
            match cmd.status() {
                Ok(status) if status.success() => {
                    println!("  ✅ Success");
                }
                Ok(status) => {
                    eprintln!("  ❌ Failed with exit code: {:?}", status.code());
                }
                Err(e) => {
                    eprintln!("  ❌ Error: {}", e);
                }
            }
            println!();
        }
        
        Ok(())
    }
    
    fn list_projects(&self, projects: Vec<(String, String)>, filters: &LoopFilters) -> Result<()> {
        let mut filtered_projects = projects;
        
        // Apply filters
        if !filters.include_patterns.is_empty() {
            filtered_projects.retain(|(project_path, _)| {
                let project_name = Path::new(project_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(project_path);
                
                filters.include_patterns.iter().any(|pattern| {
                    // Support exact match or substring match
                    project_name == pattern || project_path == pattern || 
                    project_name.contains(pattern) || project_path.contains(pattern)
                })
            });
        }
        
        if !filters.exclude_patterns.is_empty() {
            filtered_projects.retain(|(project_path, _)| {
                let project_name = Path::new(project_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(project_path);
                
                !filters.exclude_patterns.iter().any(|pattern| {
                    // Support exact match or substring match
                    project_name == pattern || project_path == pattern || 
                    project_name.contains(pattern) || project_path.contains(pattern)
                })
            });
        }
        
        if filters.existing_only {
            filtered_projects.retain(|(name, _)| Path::new(name).exists());
        }
        
        if filters.git_only {
            filtered_projects.retain(|(name, _)| Path::new(name).join(".git").exists());
        }
        
        if filtered_projects.is_empty() {
            println!("No projects found");
            return Ok(());
        }
        
        println!("Projects ({}):", filtered_projects.len());
        for (project_path, _) in filtered_projects {
            let path = Path::new(&project_path);
            let status = if !path.exists() {
                " [missing]"
            } else if path.join(".git").exists() {
                " [git]"
            } else {
                ""
            };
            
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&project_path);
            
            println!("  {} - {}{}", name, project_path, status);
        }
        
        Ok(())
    }
}

struct LoopFilters {
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    existing_only: bool,
    git_only: bool,
}

impl MetaPlugin for LoopPlugin {
    fn name(&self) -> &str {
        "loop"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("loop")
                .about("Execute commands across all projects in meta repository")
                .version(env!("CARGO_PKG_VERSION"))
                .disable_help_subcommand(true)
                .allow_external_subcommands(true)
                .arg(
                    Arg::new("include-only")
                        .long("include-only")
                        .value_name("PATTERNS")
                        .help("Only include projects matching these patterns (comma-separated)")
                        .value_delimiter(',')
                )
                .arg(
                    Arg::new("exclude")
                        .long("exclude")
                        .value_name("PATTERNS")
                        .help("Exclude projects matching these patterns (comma-separated)")
                        .value_delimiter(',')
                )
                .arg(
                    Arg::new("existing-only")
                        .long("existing-only")
                        .action(clap::ArgAction::SetTrue)
                        .help("Only iterate over existing projects")
                )
                .arg(
                    Arg::new("git-only")
                        .long("git-only")
                        .action(clap::ArgAction::SetTrue)
                        .help("Only iterate over git repositories")
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        // Load meta config to get project list - try different file names
        let projects = if Path::new(".meta.json").exists() {
            let content = std::fs::read_to_string(".meta.json")?;
            let config: MetaConfig = serde_json::from_str(&content)?;
            config.projects.into_iter().collect()
        } else if Path::new(".meta").exists() {
            let content = std::fs::read_to_string(".meta")?;
            let config: MetaConfig = serde_json::from_str(&content)?;
            config.projects.into_iter().collect()
        } else if Path::new(".meta.yaml").exists() {
            // For now, return empty if YAML parsing fails
            println!("Note: YAML config support is limited, using .meta.json is recommended");
            Vec::new()
        } else {
            return Err(anyhow::anyhow!("No .meta or .meta.json found in current directory"));
        };
        
        // Build filters
        let filters = LoopFilters {
            include_patterns: matches.get_many::<String>("include-only")
                .map(|vals| vals.map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            exclude_patterns: matches.get_many::<String>("exclude")
                .map(|vals| vals.map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            existing_only: matches.get_flag("existing-only"),
            git_only: matches.get_flag("git-only"),
        };
        
        // Handle external subcommands for commands to execute
        match matches.subcommand() {
            Some((command, sub_matches)) => {
                // Parse remaining arguments - external subcommands store args differently
                let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                    Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                    None => Vec::new()
                };
                
                self.execute_in_projects(command, &args, projects, &filters)
            }
            None => {
                // No command provided, list projects
                self.list_projects(projects, &filters)
            }
        }
    }
}

impl Default for LoopPlugin {
    fn default() -> Self {
        Self::new()
    }
}