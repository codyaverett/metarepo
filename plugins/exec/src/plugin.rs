use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig, MetaConfig};
use crate::{execute_in_all_projects, execute_in_specific_projects, execute_with_iterator, ProjectIterator};

pub struct ExecPlugin;

impl ExecPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("meta exec")
            .about("Execute commands across multiple repositories")
            .arg(
                Arg::new("command")
                    .value_name("COMMAND")
                    .help("Command to execute in each project directory")
                    .required(true)
            )
            .arg(
                Arg::new("projects")
                    .long("projects")
                    .short('p')
                    .value_name("PROJECTS")
                    .help("Comma-separated list of specific projects to run command in")
                    .value_delimiter(',')
            )
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
                    .help("Only iterate over existing projects")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("git-only")
                    .long("git-only")
                    .help("Only iterate over git repositories")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("parallel")
                    .long("parallel")
                    .help("Execute commands in parallel")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("include-main")
                    .long("include-main")
                    .help("Include the main meta repository")
                    .action(clap::ArgAction::SetTrue)
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for ExecPlugin {
    fn name(&self) -> &str {
        "exec"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("exec")
                .visible_aliases(["e", "x"])
                .about("Execute commands across multiple repositories")
                .disable_help_subcommand(true)
                .allow_external_subcommands(true)
                .arg(
                    Arg::new("projects")
                        .long("projects")
                        .short('p')
                        .value_name("PROJECTS")
                        .help("Comma-separated list of specific projects")
                        .value_delimiter(',')
                )
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
                        .help("Only iterate over existing projects")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("git-only")
                        .long("git-only")
                        .help("Only iterate over git repositories")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("parallel")
                        .long("parallel")
                        .help("Execute commands in parallel")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("include-main")
                        .long("include-main")
                        .help("Include the main meta repository")
                        .action(clap::ArgAction::SetTrue)
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some((command, sub_matches)) => {
                // Parse remaining arguments
                let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                    Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                    None => Vec::new()
                };
                
                // Load meta configuration
                let meta_file = MetaConfig::find_meta_file()
                    .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
                let config = MetaConfig::load_from_file(&meta_file)?;
                let base_path = meta_file.parent().unwrap();
                
                // Check if specific projects were specified (old style)
                if let Some(projects) = matches.get_many::<String>("projects") {
                    let project_list: Vec<&str> = projects.map(|s| s.as_str()).collect();
                    execute_in_specific_projects(command, &args, &project_list)?;
                    return Ok(());
                }
                
                // Build iterator with filters
                let mut iterator = ProjectIterator::new(&config, base_path);
                
                // Apply include patterns
                if let Some(patterns) = matches.get_many::<String>("include-only") {
                    let pattern_vec: Vec<String> = patterns.map(|s| s.to_string()).collect();
                    iterator = iterator.with_include_patterns(pattern_vec);
                }
                
                // Apply exclude patterns
                if let Some(patterns) = matches.get_many::<String>("exclude") {
                    let pattern_vec: Vec<String> = patterns.map(|s| s.to_string()).collect();
                    iterator = iterator.with_exclude_patterns(pattern_vec);
                }
                
                // Apply filters
                if matches.get_flag("existing-only") {
                    iterator = iterator.filter_existing();
                }
                
                if matches.get_flag("git-only") {
                    iterator = iterator.filter_git_repos();
                }
                
                let parallel = matches.get_flag("parallel");
                let include_main = matches.get_flag("include-main");
                
                execute_with_iterator(command, &args, iterator, include_main, parallel)?;
                
                Ok(())
            }
            None => self.show_help()
        }
    }
}

impl Default for ExecPlugin {
    fn default() -> Self {
        Self::new()
    }
}