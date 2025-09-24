use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat,
    plugin, command, arg,
};
use std::collections::HashMap;
use super::{run_script, list_scripts};

/// RunPlugin for executing project scripts
pub struct RunPlugin;

impl RunPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("run")
            .description("Run project-specific scripts defined in .meta")
            .author("Metarepo Contributors")
            .command(
                command("script")
                    .about("Run a named script")
                    .long_about("Run a script defined in the .meta file.\n\n\
                                 Scripts can be defined globally or per-project.\n\
                                 Project scripts override global scripts with the same name.\n\n\
                                 Examples:\n\
                                   meta run test                    # Run in current project or all with 'test' script\n\
                                   meta run test --project foo      # Run in specific project\n\
                                   meta run build --all             # Run in all projects\n\
                                   meta run deploy --parallel       # Run in parallel")
                    .arg(
                        arg("script")
                            .help("Name of the script to run")
                            .required(true)
                            .takes_value(true)
                    )
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Single project to run script in")
                            .takes_value(true)
                    )
                    .arg(
                        arg("projects")
                            .long("projects")
                            .help("Comma-separated list of projects")
                            .takes_value(true)
                    )
                    .arg(
                        arg("all")
                            .long("all")
                            .short('a')
                            .help("Run script in all projects")
                    )
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Run scripts in parallel across projects")
                    )
                    .arg(
                        arg("existing-only")
                            .long("existing-only")
                            .help("Only run in existing project directories")
                    )
                    .arg(
                        arg("git-only")
                            .long("git-only")
                            .help("Only run in git repositories")
                    )
                    .arg(
                        arg("env")
                            .long("env")
                            .short('e')
                            .help("Set environment variable (KEY=VALUE)")
                            .takes_value(true)
                    )
            )
            .command(
                command("list")
                    .about("List available scripts")
                    .aliases(vec!["ls".to_string(), "l".to_string()])
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Show scripts for specific project")
                            .takes_value(true)
                    )
            )
            .handler("script", handle_run_script)
            .handler("list", handle_list)
            .build()
    }
}


/// Handler for the script command
fn handle_run_script(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let script_name = matches.get_one::<String>("script")
        .ok_or_else(|| anyhow::anyhow!("Script name is required"))?;
    
    let parallel = matches.get_flag("parallel");
    let existing_only = matches.get_flag("existing-only");
    let git_only = matches.get_flag("git-only");
    
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    // Get current project context
    let current_project = config.current_project();
    
    // Parse environment variables
    let mut env_vars = HashMap::new();
    if let Some(env_args) = matches.get_many::<String>("env") {
        for env_str in env_args {
            if let Some((key, value)) = env_str.split_once('=') {
                env_vars.insert(key.to_string(), value.to_string());
            }
        }
    }
    
    // Collect selected projects
    let mut projects = Vec::new();
    
    if matches.get_flag("all") {
        projects.push("--all".to_string());
    } else if let Some(project) = matches.get_one::<String>("project") {
        // Use resolve_project to handle aliases
        if let Some(resolved) = config.resolve_project(project) {
            projects.push(resolved);
        } else {
            projects.push(project.clone());
        }
    } else if let Some(project_list) = matches.get_one::<String>("projects") {
        for p in project_list.split(',') {
            let trimmed = p.trim();
            // Use resolve_project to handle aliases
            if let Some(resolved) = config.resolve_project(trimmed) {
                projects.push(resolved);
            } else {
                projects.push(trimmed.to_string());
            }
        }
    }
    // If no projects specified, will use current project or find projects with script
    
    run_script(script_name, &projects, &base_path, current_project.as_deref(), parallel, existing_only, git_only, &env_vars)?;
    Ok(())
}

/// Handler for the list command
fn handle_list(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    let project = if let Some(project_id) = matches.get_one::<String>("project") {
        config.resolve_project(project_id)
    } else {
        config.current_project()
    };
    
    list_scripts(&base_path, project.as_deref())?;
    Ok(())
}

// Traditional implementation for backward compatibility
impl MetaPlugin for RunPlugin {
    fn name(&self) -> &str {
        "run"
    }
    
    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Register the main 'run' command with optional script argument
        let run_cmd = clap::Command::new("run")
            .about("Run project-specific scripts defined in .meta")
            .version(env!("CARGO_PKG_VERSION"))
            .arg(
                clap::Arg::new("script")
                    .help("Name of the script to run")
                    .index(1)
                    .required(false)
            )
            .arg(
                clap::Arg::new("project")
                    .long("project")
                    .short('p')
                    .help("Single project to run script in")
                    .value_name("PROJECT")
            )
            .arg(
                clap::Arg::new("projects")
                    .long("projects")
                    .help("Comma-separated list of projects")
                    .value_name("PROJECTS")
            )
            .arg(
                clap::Arg::new("all")
                    .long("all")
                    .short('a')
                    .help("Run script in all projects")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                clap::Arg::new("parallel")
                    .long("parallel")
                    .help("Run scripts in parallel across projects")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                clap::Arg::new("env")
                    .long("env")
                    .short('e')
                    .help("Set environment variable (KEY=VALUE)")
                    .action(clap::ArgAction::Append)
                    .value_name("KEY=VALUE")
            )
            .arg(
                clap::Arg::new("list")
                    .long("list")
                    .short('l')
                    .help("List available scripts")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                clap::Arg::new("existing-only")
                    .long("existing-only")
                    .help("Only run in existing project directories")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                clap::Arg::new("git-only")
                    .long("git-only")
                    .help("Only run in git repositories")
                    .action(clap::ArgAction::SetTrue)
            );
        
        app.subcommand(run_cmd)
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Check for list flag
        if matches.get_flag("list") {
            return handle_list(matches, config);
        }
        
        // Check if script is provided
        if matches.get_one::<String>("script").is_some() {
            return handle_run_script(matches, config);
        }
        
        // No script provided, list available scripts
        handle_list(matches, config)
    }
}

impl BasePlugin for RunPlugin {
    fn version(&self) -> Option<&str> {
        None
    }
    
    fn description(&self) -> Option<&str> {
        Some("Run project-specific scripts defined in .meta")
    }
    
    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for RunPlugin {
    fn default() -> Self {
        Self::new()
    }
}