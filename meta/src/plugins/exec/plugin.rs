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
                    .about("Run a shell command in each project")
                    .help_description(
                        "Run an arbitrary shell command once in each selected project.\n\
                         \n\
                         Everything after the exec flags is treated as the command to run, so it can\n\
                         be any program plus its arguments (e.g. git, npm, cargo). With no project\n\
                         selection, exec uses the directory-aware scope: inside a project it runs\n\
                         there, inside a subdirectory it runs in the projects beneath it, and at the\n\
                         workspace root it runs everywhere.\n\
                         \n\
                         Use -p/--project or --projects to target specific projects, -a/--all to run\n\
                         across the whole workspace, and --include-only/--exclude to filter by name.\n\
                         --git-only and --existing-only restrict the set further. Projects disabled\n\
                         in the .meta config are skipped unless --include-disabled is passed.\n\
                         --parallel runs the command concurrently and --include-main also runs it in\n\
                         the meta repo itself.\n\
                         \n\
                         Examples:\n  \
                           meta exec --all git status\n  \
                           meta exec -p doop npm install\n  \
                           meta exec --git-only --parallel git pull",
                    )
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
                    )
                    .arg(
                        arg("include-disabled")
                            .long("include-disabled")
                            .help("Also run in projects disabled in the .meta config"),
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

            let include_disabled = matches.get_flag("include-disabled");

            // Check for --all flag
            if matches.get_flag("all") {
                // Run in all projects
                let mut iterator =
                    ProjectIterator::new(&config, base_path).include_disabled(include_disabled);

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

            // Drop explicitly-selected projects that are disabled, unless the
            // user opted in with --include-disabled. Resolution already happened
            // above, so an alias of a disabled project is caught here too.
            if !selected_projects.is_empty() && !include_disabled {
                let disabled = config.disabled_project_keys();
                selected_projects.retain(|key| {
                    if disabled.contains(key) {
                        eprintln!(
                            "Skipping disabled project '{key}' (use --include-disabled to run it)"
                        );
                        false
                    } else {
                        true
                    }
                });
                if selected_projects.is_empty() {
                    return Ok(());
                }
            }

            // If no projects specified, fall back to the directory-aware scope:
            // inside a project -> that project; inside a subdirectory -> the
            // projects beneath it; at the workspace root (or with --workspace)
            // -> all. Explicit --project/--projects above override this.
            if selected_projects.is_empty() {
                selected_projects = runtime_config.scoped_project_keys();
                if selected_projects.is_empty() {
                    println!("No projects in this directory. Use --workspace to run across the whole workspace, or --project/--projects to target specific projects.");
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
            let mut iterator =
                ProjectIterator::new(&config, base_path).include_disabled(include_disabled);

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
            .about("Run a shell command in each project")
            .after_long_help(metarepo_core::format_help_description(
                "Run an arbitrary shell command once in each selected project.\n\
                 \n\
                 Everything after the exec flags is treated as the command to run, so it can\n\
                 be any program plus its arguments (e.g. git, npm, cargo). With no project\n\
                 selection, exec uses the directory-aware scope: inside a project it runs\n\
                 there, inside a subdirectory it runs in the projects beneath it, and at the\n\
                 workspace root it runs everywhere.\n\
                 \n\
                 Use -p/--project or --projects to target specific projects, -a/--all to run\n\
                 across the whole workspace, and --include-only/--exclude to filter by name.\n\
                 --git-only and --existing-only restrict the set further. --parallel runs the\n\
                 command concurrently and --include-main also runs it in the meta repo itself.\n\
                 \n\
                 Examples:\n  \
                   meta exec --all git status\n  \
                   meta exec -p doop npm install\n  \
                   meta exec --git-only --parallel git pull",
            ))
            .version(env!("CARGO_PKG_VERSION"))
            .allow_external_subcommands(true)
            // Keep `meta exec help` meaning "run `help` across repos" rather than
            // printing clap help.
            .disable_help_subcommand(true)
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
                clap::Arg::new("include-disabled")
                    .long("include-disabled")
                    .help("Also run in projects disabled in the .meta config")
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
