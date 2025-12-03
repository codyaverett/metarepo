use super::{execute_in_specific_projects, execute_with_iterator, ProjectIterator};
use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{arg, command, plugin, BasePlugin, MetaConfig, MetaPlugin, RuntimeConfig};

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
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .short('p')
                            .long("project")
                            .help("Single project to run command in")
                            .takes_value(true),
                    )
                    .arg(
                        arg("projects")
                            .long("projects")
                            .help("Comma-separated list of specific projects")
                            .takes_value(true),
                    )
                    .arg(
                        arg("all")
                            .short('a')
                            .long("all")
                            .help("Run command in all projects"),
                    )
                    .arg(
                        arg("include-only")
                            .long("include-only")
                            .help("Only include projects matching these patterns (comma-separated)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("exclude")
                            .long("exclude")
                            .help("Exclude projects matching these patterns (comma-separated)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("include-tags")
                            .long("include-tags")
                            .help("Only include projects with these tags (comma-separated)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("exclude-tags")
                            .long("exclude-tags")
                            .help("Exclude projects with these tags (comma-separated)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("existing-only")
                            .long("existing-only")
                            .help("Only iterate over existing projects"),
                    )
                    .arg(
                        arg("git-only")
                            .long("git-only")
                            .help("Only iterate over git repositories"),
                    )
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Execute commands in parallel"),
                    )
                    .arg(
                        arg("include-main")
                            .long("include-main")
                            .help("Include the main meta repository"),
                    ),
            )
            .handler("exec", handle_exec)
            .build()
    }
}

/// Handler for the exec command
fn handle_exec(matches: &ArgMatches, runtime_config: &RuntimeConfig) -> Result<()> {
    // Load meta configuration
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    // Get the external subcommand (the actual command to run)
    match matches.subcommand() {
        Some((command, sub_matches)) => {
            // Parse remaining arguments from the external subcommand
            let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                None => Vec::new(),
            };

            // Collect selected projects
            let mut selected_projects = Vec::new();

            // Check for --all flag
            if matches.get_flag("all") {
                // Run in all projects
                let mut iterator = ProjectIterator::new(&config, base_path);

                // Apply additional filters if provided
                if let Some(patterns_str) = matches.get_one::<String>("include-only") {
                    let pattern_vec: Vec<String> =
                        patterns_str.split(',').map(|s| s.to_string()).collect();
                    iterator = iterator.with_include_patterns(pattern_vec);
                }

                if let Some(patterns_str) = matches.get_one::<String>("exclude") {
                    let pattern_vec: Vec<String> =
                        patterns_str.split(',').map(|s| s.to_string()).collect();
                    iterator = iterator.with_exclude_patterns(pattern_vec);
                }

                if let Some(tags_str) = matches.get_one::<String>("include-tags") {
                    let tag_vec: Vec<String> =
                        tags_str.split(',').map(|s| s.trim().to_string()).collect();
                    iterator = iterator.with_include_tags(tag_vec);
                }

                if let Some(tags_str) = matches.get_one::<String>("exclude-tags") {
                    let tag_vec: Vec<String> =
                        tags_str.split(',').map(|s| s.trim().to_string()).collect();
                    iterator = iterator.with_exclude_tags(tag_vec);
                }

                if matches.get_flag("existing-only") {
                    iterator = iterator.filter_existing();
                }

                if matches.get_flag("git-only") {
                    iterator = iterator.filter_git_repos();
                }

                let parallel = matches.get_flag("parallel");
                let include_main = matches.get_flag("include-main");
                let no_progress = matches.get_flag("no-progress");
                let streaming = matches.get_flag("streaming");

                execute_with_iterator(
                    command,
                    &args,
                    iterator,
                    include_main,
                    parallel,
                    no_progress,
                    streaming,
                )?;
                return Ok(());
            }

            // Check for single project
            if let Some(project_id) = matches.get_one::<String>("project") {
                // Use resolve_project to handle aliases
                if let Some(resolved) = runtime_config.resolve_project(project_id) {
                    selected_projects.push(resolved);
                } else {
                    selected_projects.push(project_id.clone());
                }
            }

            // Check for multiple projects
            if let Some(projects_str) = matches.get_one::<String>("projects") {
                for p in projects_str.split(',') {
                    let trimmed = p.trim();
                    // Use resolve_project to handle aliases
                    if let Some(resolved) = runtime_config.resolve_project(trimmed) {
                        selected_projects.push(resolved);
                    } else {
                        selected_projects.push(trimmed.to_string());
                    }
                }
            }

            // If no projects specified, check for current project context
            if selected_projects.is_empty() {
                if let Some(current) = runtime_config.current_project() {
                    selected_projects.push(current);
                } else {
                    // No context and no projects specified - show help
                    println!("No project context found. Use --project, --projects, or --all to specify targets.");
                    return Ok(());
                }
            }

            // Execute in selected projects
            if !selected_projects.is_empty() {
                let project_refs: Vec<&str> =
                    selected_projects.iter().map(|s| s.as_str()).collect();
                execute_in_specific_projects(command, &args, &project_refs)?;
                return Ok(());
            }

            // Build iterator with filters (for backward compatibility)
            let mut iterator = ProjectIterator::new(&config, base_path);

            // Apply include patterns
            if let Some(patterns_str) = matches.get_one::<String>("include-only") {
                let pattern_vec: Vec<String> =
                    patterns_str.split(',').map(|s| s.to_string()).collect();
                iterator = iterator.with_include_patterns(pattern_vec);
            }

            // Apply exclude patterns
            if let Some(patterns_str) = matches.get_one::<String>("exclude") {
                let pattern_vec: Vec<String> =
                    patterns_str.split(',').map(|s| s.to_string()).collect();
                iterator = iterator.with_exclude_patterns(pattern_vec);
            }

            // Apply tag filters
            if let Some(tags_str) = matches.get_one::<String>("include-tags") {
                let tag_vec: Vec<String> =
                    tags_str.split(',').map(|s| s.trim().to_string()).collect();
                iterator = iterator.with_include_tags(tag_vec);
            }

            if let Some(tags_str) = matches.get_one::<String>("exclude-tags") {
                let tag_vec: Vec<String> =
                    tags_str.split(',').map(|s| s.trim().to_string()).collect();
                iterator = iterator.with_exclude_tags(tag_vec);
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
            let no_progress = matches.get_flag("no-progress");
            let streaming = matches.get_flag("streaming");

            execute_with_iterator(
                command,
                &args,
                iterator,
                include_main,
                parallel,
                no_progress,
                streaming,
            )?;

            Ok(())
        }
        None => {
            // No command specified - show error
            eprintln!("Error: No command specified");
            eprintln!("Usage: meta exec [OPTIONS] <COMMAND> [ARGS]...");
            eprintln!("\nExamples:");
            eprintln!("  meta exec pwd                    # Run in current project context");
            eprintln!("  meta exec --all git status       # Run in all projects");
            eprintln!("  meta exec -p doop npm install    # Run in specific project");
            std::process::exit(1);
        }
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for ExecPlugin {
    fn name(&self) -> &str {
        "exec"
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Register exec as a direct command with allow_external_subcommands
        let exec_cmd = clap::Command::new("exec")
            .about("Execute commands across multiple repositories")
            .version(env!("CARGO_PKG_VERSION"))
            .allow_external_subcommands(true)
            .arg(
                clap::Arg::new("project")
                    .short('p')
                    .long("project")
                    .help("Single project to run command in")
                    .value_name("PROJECT"),
            )
            .arg(
                clap::Arg::new("projects")
                    .long("projects")
                    .help("Comma-separated list of specific projects")
                    .value_name("PROJECTS"),
            )
            .arg(
                clap::Arg::new("all")
                    .short('a')
                    .long("all")
                    .help("Run command in all projects")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("include-only")
                    .long("include-only")
                    .help("Only include projects matching these patterns (comma-separated)")
                    .value_name("PATTERNS"),
            )
            .arg(
                clap::Arg::new("exclude")
                    .long("exclude")
                    .help("Exclude projects matching these patterns (comma-separated)")
                    .value_name("PATTERNS"),
            )
            .arg(
                clap::Arg::new("include-tags")
                    .long("include-tags")
                    .help("Only include projects with these tags (comma-separated)")
                    .value_name("TAGS"),
            )
            .arg(
                clap::Arg::new("exclude-tags")
                    .long("exclude-tags")
                    .help("Exclude projects with these tags (comma-separated)")
                    .value_name("TAGS"),
            )
            .arg(
                clap::Arg::new("existing-only")
                    .long("existing-only")
                    .help("Only iterate over existing projects")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("git-only")
                    .long("git-only")
                    .help("Only iterate over git repositories")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("parallel")
                    .long("parallel")
                    .help("Execute commands in parallel")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("include-main")
                    .long("include-main")
                    .help("Include the main meta repository")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("no-progress")
                    .long("no-progress")
                    .help("Disable progress indicators (useful for CI environments)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("streaming")
                    .long("streaming")
                    .help("Show output as it happens instead of buffered (legacy behavior)")
                    .action(clap::ArgAction::SetTrue),
            );

        app.subcommand(exec_cmd)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Call the handler directly
        handle_exec(matches, config)
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
