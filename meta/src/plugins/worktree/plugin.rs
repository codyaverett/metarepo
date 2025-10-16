use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig,
    plugin, command, arg,
};
use super::{add_worktrees, remove_worktrees, list_all_worktrees, prune_worktrees};

/// WorktreePlugin using the simplified plugin architecture
pub struct WorktreePlugin;

impl WorktreePlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("worktree")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Git worktree management across workspace projects")
            .author("Metarepo Contributors")
            .command(
                command("add")
                    .about("Create worktrees for selected projects")
                    .long_about("Create git worktrees for selected projects in the workspace.\n\n\
                                 Worktrees allow you to have multiple working trees attached to the same repository,\n\
                                 enabling parallel development on different branches without stashing or switching.\n\n\
                                 Examples:\n\
                                   meta worktree add feature-123                           # Interactive project selection\n\
                                   meta worktree add feature-123 --project containers      # Single project\n\
                                   meta worktree add feature-123 --projects foo,bar        # Multiple projects\n\
                                   meta worktree add feature-123 --all                     # All projects\n\
                                   meta worktree add -b feature-123 origin/main           # Create new branch")
                    .aliases(vec!["create".to_string(), "new".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("branch")
                            .help("Branch name or commit to create worktree from")
                            .required(true)
                            .takes_value(true)
                    )
                    .arg(
                        arg("commit")
                            .help("Starting point (branch/tag/commit) for the worktree")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Single project to create worktree for")
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
                            .help("Create worktrees for all projects")
                    )
                    .arg(
                        arg("create-branch")
                            .long("create-branch")
                            .short('b')
                            .help("Create a new branch for the worktree")
                    )
                    .arg(
                        arg("path")
                            .long("path")
                            .help("Custom path suffix for worktree directory (default: branch name)")
                            .takes_value(true)
                    )
            )
            .command(
                command("remove")
                    .about("Remove worktrees from selected projects")
                    .aliases(vec!["rm".to_string(), "delete".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("branch")
                            .help("Branch name or worktree directory name to remove")
                            .required(true)
                            .takes_value(true)
                    )
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Single project to remove worktree from")
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
                            .help("Remove worktrees from all projects that have it")
                    )
                    .arg(
                        arg("force")
                            .long("force")
                            .short('f')
                            .help("Force removal even if worktree has uncommitted changes")
                    )
            )
            .command(
                command("list")
                    .about("List all worktrees across the workspace")
                    .aliases(vec!["ls".to_string(), "l".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("verbose")
                            .long("verbose")
                            .help("Show detailed information about each worktree")
                    )
            )
            .command(
                command("prune")
                    .about("Remove stale worktrees that no longer exist")
                    .with_help_formatting()
                    .arg(
                        arg("dry-run")
                            .long("dry-run")
                            .short('n')
                            .help("Show what would be pruned without actually removing")
                    )
            )
            .handler("add", handle_add)
            .handler("remove", handle_remove)
            .handler("list", handle_list)
            .handler("prune", handle_prune)
            .build()
    }
}

/// Handler for the add command
fn handle_add(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let branch = matches.get_one::<String>("branch")
        .ok_or_else(|| anyhow::anyhow!("Branch name is required"))?;
    let _commit = matches.get_one::<String>("commit");
    let create_branch = matches.get_flag("create-branch");
    let path_suffix = matches.get_one::<String>("path").map(|s| s.as_str());
    
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    // Get current project context
    let current_project = config.current_project();
    
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
    // If no projects specified, will use current project or trigger interactive selection
    
    add_worktrees(branch, &projects, &base_path, path_suffix, create_branch, current_project.as_deref())?;
    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let branch = matches.get_one::<String>("branch")
        .ok_or_else(|| anyhow::anyhow!("Branch name is required"))?;
    let force = matches.get_flag("force");
    
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    // Get current project context
    let current_project = config.current_project();
    
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
    // If no projects specified, will use current project or trigger interactive selection
    
    remove_worktrees(branch, &projects, &base_path, force, current_project.as_deref())?;
    Ok(())
}

/// Handler for the list command
fn handle_list(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    list_all_worktrees(&base_path)?;
    Ok(())
}

/// Handler for the prune command
fn handle_prune(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let dry_run = matches.get_flag("dry-run");
    
    let base_path = config.meta_root()
        .unwrap_or(config.working_dir.clone());
    
    prune_worktrees(&base_path, dry_run)?;
    Ok(())
}

// Traditional implementation for backward compatibility
impl MetaPlugin for WorktreePlugin {
    fn name(&self) -> &str {
        "worktree"
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

impl BasePlugin for WorktreePlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }
    
    fn description(&self) -> Option<&str> {
        Some("Git worktree management across workspace projects")
    }
    
    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for WorktreePlugin {
    fn default() -> Self {
        Self::new()
    }
}