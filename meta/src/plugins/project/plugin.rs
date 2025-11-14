use super::{
    convert_to_bare, import_project_recursive_with_options, import_project_with_options,
    list_projects, list_projects_minimal, remove_project, rename_project, show_project_tree,
    update_project_gitignore, update_projects,
};
use anyhow::Result;
use clap::ArgMatches;
use colored::Colorize;
use metarepo_core::{
    arg, command, is_interactive, plugin, prompt_select, prompt_text, prompt_url, BasePlugin,
    MetaPlugin, NonInteractiveMode, RuntimeConfig,
};

/// ProjectPlugin using the new simplified plugin architecture
pub struct ProjectPlugin;

impl ProjectPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("project")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Project management operations")
            .author("Metarepo Contributors")
            .command(
                command("add")
                    .about("Add a project to the workspace")
                    .long_about("Add a project to the workspace.\n\n\
                                 This command can:\n\
                                 • Clone a new repository from a URL\n\
                                 • Import an existing local repository\n\
                                 • Create a symlink to an external directory\n\
                                 • Auto-detect repository URLs from existing directories\n\
                                 • Recursively import nested meta repositories\n\n\
                                 Examples:\n\
                                   meta project add myproject https://github.com/user/repo.git  # Clone new\n\
                                   meta project add myproject ../external-repo                   # Symlink\n\
                                   meta project add myproject                                    # Use existing")
                    .aliases(vec!["import".to_string(), "i".to_string(), "a".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("path")
                            .help("Local directory name for the project")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("source")
                            .help("Git URL or path to external directory (optional)")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("recursive")
                            .long("recursive")
                            .short('r')
                            .help("Recursively import nested meta repositories")
                    )
                    .arg(
                        arg("max-depth")
                            .long("max-depth")
                            .help("Maximum depth for recursive imports (default: 3)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("flatten")
                            .long("flatten")
                            .help("Import nested projects at root level instead of maintaining hierarchy")
                    )
                    .arg(
                        arg("no-recursive")
                            .long("no-recursive")
                            .help("Disable recursive import even if configured in .meta")
                    )
                    .arg(
                        arg("init-git")
                            .long("init-git")
                            .help("Automatically initialize git repository if directory is not a git repo")
                    )
                    .arg(
                        arg("bare")
                            .long("bare")
                            .help("Clone as bare repository with worktree structure")
                    )
            )
            .command(
                command("list")
                    .about("List all projects in the workspace (tree view by default)")
                    .with_help_formatting()
                    .aliases(vec!["ls".to_string(), "l".to_string()])
                    .arg(
                        arg("flat")
                            .long("flat")
                            .short('f')
                            .help("Display projects as a flat list with details")
                    )
                    .arg(
                        arg("minimal")
                            .long("minimal")
                            .short('m')
                            .help("Display only project names (minimal output)")
                    )
            )
            .command(
                command("tree")
                    .about("Display project hierarchy as a tree")
                    .with_help_formatting()
                    .arg(
                        arg("flat")
                            .long("flat")
                            .short('f')
                            .help("Display projects as a flat list with details")
                    )
                    .arg(
                        arg("minimal")
                            .long("minimal")
                            .short('m')
                            .help("Display only project names (minimal output)")
                    )
            )
            .command(
                command("update")
                    .about("Update all projects (pull latest changes)")
                    .aliases(vec!["pull".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("recursive")
                            .long("recursive")
                            .short('r')
                            .help("Also update nested repositories")
                    )
                    .arg(
                        arg("depth")
                            .long("depth")
                            .help("Maximum depth for recursive updates (default: 3)")
                            .takes_value(true)
                    )
            )
            .command(
                command("remove")
                    .about("Remove a project from the workspace")
                    .aliases(vec!["rm".to_string(), "r".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Name of the project to remove")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("force")
                            .long("force")
                            .short('f')
                            .help("Force removal even with uncommitted changes, and delete directory")
                    )
            )
            .command(
                command("update-gitignore")
                    .about("Update .gitignore for a project that now has a remote")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Name of the project to update")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("rename")
                    .about("Rename a project in the workspace")
                    .aliases(vec!["mv".to_string(), "move".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("old_name")
                            .help("Current name of the project")
                            .required(true)
                            .takes_value(true)
                    )
                    .arg(
                        arg("new_name")
                            .help("New name for the project")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("convert-to-bare")
                    .about("Convert a normal repository to bare repository with worktrees")
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .help("Name of the project to convert")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .handler("add", handle_add)
            .handler("list", handle_list)
            .handler("tree", handle_tree)
            .handler("update", handle_update)
            .handler("remove", handle_remove)
            .handler("update-gitignore", handle_update_gitignore)
            .handler("rename", handle_rename)
            .handler("convert-to-bare", handle_convert_to_bare)
            .build()
    }
}

/// Handler for the add command
fn handle_add(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let non_interactive = config
        .non_interactive
        .unwrap_or(NonInteractiveMode::Defaults);

    // Get or prompt for the project path
    let path = match matches.get_one::<String>("path") {
        Some(p) => p.clone(),
        None => {
            if is_interactive() {
                println!(
                    "\n  📋 {}",
                    "Add a new project to your workspace".cyan().bold()
                );
                prompt_text("Project name/path", None, false, non_interactive)?
            } else {
                return Err(anyhow::anyhow!(
                    "Project path is required. Use 'meta project add <path>' or run interactively in a terminal"
                ));
            }
        }
    };

    // Get or prompt for the source URL
    let source_opt = match matches.get_one::<String>("source") {
        Some(s) => Some(s.clone()),
        None => {
            if is_interactive() {
                prompt_url("Repository URL or path", None, false, non_interactive)?
            } else {
                None
            }
        }
    };
    let source = source_opt.as_deref();

    let init_git = matches.get_flag("init-git");
    let bare = matches.get_flag("bare");

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    // Check for recursive import flags
    let recursive = matches.get_flag("recursive");
    let no_recursive = matches.get_flag("no-recursive");
    let flatten = matches.get_flag("flatten");
    let max_depth = matches
        .get_one::<String>("max-depth")
        .and_then(|s| s.parse::<usize>().ok());

    // Determine if we should use recursive import
    let use_recursive = if no_recursive {
        false // Explicitly disabled
    } else if recursive || flatten || max_depth.is_some() {
        true // Explicitly enabled or has related flags
    } else {
        // Check configuration or global default
        config
            .meta_config
            .nested
            .as_ref()
            .map(|n| n.recursive_import)
            .unwrap_or(false)
    };

    // Determine if we should use bare repository
    let use_bare = if bare {
        true // Explicitly enabled via flag
    } else {
        // Check global default (defaults to true for bare repos)
        config.meta_config.default_bare.unwrap_or(true)
    };

    if use_recursive || flatten || max_depth.is_some() {
        import_project_recursive_with_options(
            &path,
            source,
            &base_path,
            use_recursive,
            max_depth,
            flatten,
            init_git,
            use_bare,
        )?;
    } else {
        import_project_with_options(&path, source, &base_path, init_git, use_bare)?;
    }
    Ok(())
}

/// Handler for the list command
fn handle_list(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    // Check flags for output format
    if matches.get_flag("minimal") {
        // Minimal: just project names
        list_projects_minimal(&base_path)?;
    } else if matches.get_flag("flat") {
        // Flat: list with details
        list_projects(&base_path)?;
    } else {
        // Default: tree view
        show_project_tree(&base_path)?;
    }
    Ok(())
}

/// Handler for the tree command
fn handle_tree(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    // Check flags for output format (same as list command)
    if matches.get_flag("minimal") {
        // Minimal: just project names
        list_projects_minimal(&base_path)?;
    } else if matches.get_flag("flat") {
        // Flat: list with details
        list_projects(&base_path)?;
    } else {
        // Default: tree view
        show_project_tree(&base_path)?;
    }
    Ok(())
}

/// Handler for the update command
fn handle_update(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    let recursive = matches.get_flag("recursive");
    let depth = matches
        .get_one::<String>("depth")
        .and_then(|s| s.parse::<usize>().ok());

    update_projects(&base_path, recursive, depth)?;
    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let non_interactive = config
        .non_interactive
        .unwrap_or(NonInteractiveMode::Defaults);

    // Get or prompt for project name
    let name = match matches.get_one::<String>("name") {
        Some(n) => n.clone(),
        None => {
            if is_interactive() {
                let project_names: Vec<String> =
                    config.meta_config.projects.keys().cloned().collect();

                if project_names.is_empty() {
                    return Err(anyhow::anyhow!("No projects found in workspace"));
                }

                println!(
                    "\n  🗑️  {}",
                    "Remove a project from workspace".cyan().bold()
                );
                prompt_select("Project to remove", project_names, None, non_interactive)?
            } else {
                return Err(anyhow::anyhow!(
                    "Project name is required. Use 'meta project remove <name>' or run interactively in a terminal"
                ));
            }
        }
    };

    let force = matches.get_flag("force");

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    remove_project(&name, &base_path, force)?;
    Ok(())
}

/// Handler for the update-gitignore command
fn handle_update_gitignore(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    update_project_gitignore(name, &base_path)?;
    Ok(())
}

/// Handler for the rename command
fn handle_rename(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let old_name = matches.get_one::<String>("old_name").unwrap();
    let new_name = matches.get_one::<String>("new_name").unwrap();

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    rename_project(old_name, new_name, &base_path)?;
    Ok(())
}

/// Handler for the convert-to-bare command
fn handle_convert_to_bare(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let project = matches.get_one::<String>("project").unwrap();

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    convert_to_bare(project, &base_path)?;
    Ok(())
}

// Traditional implementation for backward compatibility
impl MetaPlugin for ProjectPlugin {
    fn name(&self) -> &str {
        "project"
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.handle_command(matches, config)
    }
}

impl BasePlugin for ProjectPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Project management operations")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for ProjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}
