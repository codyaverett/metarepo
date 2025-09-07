use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};
use std::process::Command as ProcessCommand;
use crate::{ProjectIterator, MetaConfig};
use std::path::Path;

pub struct LoopPlugin;

impl LoopPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn execute_in_projects(&self, command: &str, args: &[&str], iterator: ProjectIterator) -> Result<()> {
        let projects: Vec<_> = iterator.collect();
        
        if projects.is_empty() {
            println!("No projects matched the criteria");
            return Ok(());
        }
        
        println!("Executing command in {} projects", projects.len());
        println!("Command: {} {}", command, args.join(" "));
        println!();
        
        for (idx, project) in projects.iter().enumerate() {
            println!("[{}/{}] {}", idx + 1, projects.len(), project.name);
            println!("  Path: {}", project.path.display());
            
            if !project.exists {
                println!("  ⚠️  Project directory does not exist, skipping");
                continue;
            }
            
            let mut cmd = ProcessCommand::new(command);
            cmd.args(args)
               .current_dir(&project.path)
               .env("META_PROJECT_NAME", &project.name)
               .env("META_PROJECT_PATH", &project.path)
               .env("META_PROJECT_REPO", &project.repo_url);
            
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
    
    fn list_projects(&self, iterator: ProjectIterator) -> Result<()> {
        let projects: Vec<_> = iterator.collect();
        
        if projects.is_empty() {
            println!("No projects found");
            return Ok(());
        }
        
        println!("Projects ({}):", projects.len());
        for project in projects {
            let status = if !project.exists {
                " [missing]"
            } else if project.is_git_repo() {
                " [git]"
            } else {
                ""
            };
            
            println!("  {} - {}{}", project.name, project.path.display(), status);
        }
        
        Ok(())
    }
}

impl MetaPlugin for LoopPlugin {
    fn name(&self) -> &str {
        "loop"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("loop")
                .about("Iterate over projects in meta repository")
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
                .arg(
                    Arg::new("command")
                        .value_name("COMMAND")
                        .help("Command to execute in each project directory")
                        .required(false)
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Load meta config to get project list
        let meta_config_path = Path::new(".meta.json");
        let meta_config: MetaConfig = if meta_config_path.exists() {
            let content = std::fs::read_to_string(meta_config_path)?;
            serde_json::from_str(&content)?
        } else {
            // Try .meta.yaml
            let yaml_path = Path::new(".meta.yaml");
            if yaml_path.exists() {
                let content = std::fs::read_to_string(yaml_path)?;
                serde_json::from_str(&content).unwrap_or_else(|_| {
                    MetaConfig {
                        ignore: vec![],
                        projects: std::collections::HashMap::new(),
                        plugins: None,
                    }
                })
            } else {
                return Err(anyhow::anyhow!("No .meta.json or .meta.yaml found in current directory"));
            }
        };
        
        let base_path = std::env::current_dir()?;
        let mut iterator = ProjectIterator::new(&meta_config, &base_path);
        
        // Apply filters
        if let Some(include_patterns) = matches.get_many::<String>("include-only") {
            let patterns: Vec<String> = include_patterns.map(|s| s.to_string()).collect();
            iterator = iterator.with_include_patterns(patterns);
        }
        
        if let Some(exclude_patterns) = matches.get_many::<String>("exclude") {
            let patterns: Vec<String> = exclude_patterns.map(|s| s.to_string()).collect();
            iterator = iterator.with_exclude_patterns(patterns);
        }
        
        if matches.get_flag("existing-only") {
            iterator = iterator.filter_existing();
        }
        
        if matches.get_flag("git-only") {
            iterator = iterator.filter_git_repos();
        }
        
        // Execute command or list projects
        if let Some(command_str) = matches.get_one::<String>("command") {
            // Parse command and arguments
            let parts: Vec<&str> = command_str.split_whitespace().collect();
            if let Some((command, args)) = parts.split_first() {
                self.execute_in_projects(command, args, iterator)
            } else {
                Err(anyhow::anyhow!("Invalid command"))
            }
        } else {
            self.list_projects(iterator)
        }
    }
}

impl Default for LoopPlugin {
    fn default() -> Self {
        Self::new()
    }
}