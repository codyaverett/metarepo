use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat,
    plugin, command, arg,
};
use super::{import_project, import_project_recursive, list_projects, remove_project, show_project_tree, update_projects};

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
                    .arg(
                        arg("path")
                            .help("Local directory name for the project")
                            .required(true)
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
            )
            .command(
                command("list")
                    .about("List all projects in the workspace")
                    .aliases(vec!["ls".to_string(), "l".to_string()])
                    .arg(
                        arg("tree")
                            .long("tree")
                            .short('t')
                            .help("Display projects in tree format showing nested structure")
                    )
            )
            .command(
                command("tree")
                    .about("Display project hierarchy as a tree")
            )
            .command(
                command("update")
                    .about("Update all projects (pull latest changes)")
                    .aliases(vec!["pull".to_string()])
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
                    .arg(
                        arg("name")
                            .help("Name of the project to remove")
                            .required(true)
                            .takes_value(true)
                    )
                    .arg(
                        arg("force")
                            .long("force")
                            .short('f')
                            .help("Force removal even with uncommitted changes, and delete directory")
                    )
            )
            .handler("add", handle_add)
            .handler("list", handle_list)
            .handler("tree", handle_tree)
            .handler("update", handle_update)
            .handler("remove", handle_remove)
            .build()
    }
}

/// Handler for the add command
fn handle_add(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let source = matches.get_one::<String>("source").map(|s| s.as_str());
    
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };
    
    // Check for recursive import flags
    let recursive = matches.get_flag("recursive");
    let no_recursive = matches.get_flag("no-recursive");
    let flatten = matches.get_flag("flatten");
    let max_depth = matches.get_one::<String>("max-depth")
        .and_then(|s| s.parse::<usize>().ok());
    
    // Determine if we should use recursive import
    let use_recursive = if no_recursive {
        false // Explicitly disabled
    } else if recursive || flatten || max_depth.is_some() {
        true // Explicitly enabled or has related flags
    } else {
        // Check configuration
        config.meta_config.nested.as_ref()
            .map(|n| n.recursive_import)
            .unwrap_or(false)
    };
    
    if use_recursive || flatten || max_depth.is_some() {
        import_project_recursive(path, source, &base_path, use_recursive, max_depth, flatten)?;
    } else {
        import_project(path, source, &base_path)?;
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
    
    // Check if --tree flag is set
    if matches.get_flag("tree") {
        show_project_tree(&base_path)?;
    } else {
        list_projects(&base_path)?;
    }
    Ok(())
}

/// Handler for the tree command
fn handle_tree(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };
    
    show_project_tree(&base_path)?;
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
    let depth = matches.get_one::<String>("depth")
        .and_then(|s| s.parse::<usize>().ok());
    
    update_projects(&base_path, recursive, depth)?;
    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    let force = matches.get_flag("force");
    
    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };
    
    remove_project(name, &base_path, force)?;
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