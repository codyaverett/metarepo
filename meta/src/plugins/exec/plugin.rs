use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat, MetaConfig,
    plugin, command, arg,
};
use super::{execute_in_specific_projects, execute_with_iterator, ProjectIterator};

/// ExecPlugin using the new simplified plugin architecture
pub struct ExecPlugin;

impl ExecPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("exec")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Execute commands across multiple repositories")
            .author("Metarepo Contributors")
            .command(
                command("exec")
                    .about("Execute commands across multiple repositories")
                    .aliases(vec!["e".to_string(), "x".to_string()])
                    .allow_external_subcommands(true)
                    .arg(
                        arg("projects")
                            .short('p')
                            .long("projects")
                            .help("Comma-separated list of specific projects")
                            .takes_value(true)
                    )
                    .arg(
                        arg("include-only")
                            .long("include-only")
                            .help("Only include projects matching these patterns (comma-separated)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("exclude")
                            .long("exclude")
                            .help("Exclude projects matching these patterns (comma-separated)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("existing-only")
                            .long("existing-only")
                            .help("Only iterate over existing projects")
                    )
                    .arg(
                        arg("git-only")
                            .long("git-only")
                            .help("Only iterate over git repositories")
                    )
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Execute commands in parallel")
                    )
                    .arg(
                        arg("include-main")
                            .long("include-main")
                            .help("Include the main meta repository")
                    )
            )
            .handler("exec", handle_exec)
            .build()
    }
}

/// Handler for the exec command
fn handle_exec(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    // Load meta configuration
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    match matches.subcommand() {
        Some((command, sub_matches)) => {
            // Parse remaining arguments
            let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                None => Vec::new()
            };
            
            // Check if specific projects were specified
            if let Some(projects_str) = matches.get_one::<String>("projects") {
                let project_list: Vec<&str> = projects_str.split(',').collect();
                execute_in_specific_projects(command, &args, &project_list)?;
                return Ok(());
            }
            
            // Build iterator with filters
            let mut iterator = ProjectIterator::new(&config, base_path);
            
            // Apply include patterns
            if let Some(patterns_str) = matches.get_one::<String>("include-only") {
                let pattern_vec: Vec<String> = patterns_str.split(',').map(|s| s.to_string()).collect();
                iterator = iterator.with_include_patterns(pattern_vec);
            }
            
            // Apply exclude patterns
            if let Some(patterns_str) = matches.get_one::<String>("exclude") {
                let pattern_vec: Vec<String> = patterns_str.split(',').map(|s| s.to_string()).collect();
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
        None => {
            // Show help using the v2 system
            let plugin = ExecPlugin::create_plugin();
            let app = plugin.register_commands(clap::Command::new("meta"));
            let mut help_app = app.clone();
            help_app.print_help()?;
            Ok(())
        }
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for ExecPlugin {
    fn name(&self) -> &str {
        "exec"
    }
    
    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Check for output format flag
        if let Some(format_str) = matches.get_one::<String>("output-format") {
            if let Some(format) = HelpFormat::from_str(format_str) {
                return self.show_help(format);
            }
        }
        
        // Check for AI help flag
        if matches.get_flag("ai") {
            return self.show_ai_help();
        }
        
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.handle_command(matches, config)
    }
}

impl BasePlugin for ExecPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }
    
    fn description(&self) -> Option<&str> {
        Some("Execute commands across multiple repositories")
    }
    
    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for ExecPlugin {
    fn default() -> Self {
        Self::new()
    }
}