use super::{
    add_worktrees, clean_worktrees, list_all_worktrees, prune_worktrees, remove_worktrees,
    repair_worktrees, CleanOptions,
};
use anyhow::Result;
use clap::ArgMatches;
use colored::Colorize;
use metarepo_core::{
    arg, command, is_interactive, plugin, prompt_multiselect, prompt_text, BasePlugin, MetaPlugin,
    NonInteractiveMode, RuntimeConfig,
};

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
                                 The command intelligently handles branches:\n\
                                   • If the branch exists locally, it checks it out\n\
                                   • If it exists remotely, it creates a local tracking branch\n\
                                   • If it doesn't exist, it prompts for a starting point or uses --from\n\n\
                                 Examples:\n\
                                   meta worktree add feature-123                           # Smart detection\n\
                                   meta worktree add feature-123 --from origin/main        # Create from specific branch\n\
                                   meta worktree add feature-123 --project containers      # Single project\n\
                                   meta worktree add feature-123 --all                     # All projects\n\
                                   meta worktree add -b feature-123                        # Force create new branch")
                    .aliases(vec!["create".to_string(), "new".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("branch")
                            .help("Branch name or commit to create worktree from")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("commit")
                            .help("Starting point (branch/tag/commit) for the worktree")
                            .required(false)
                            .takes_value(true)
                    )
                    .arg(
                        arg("from")
                            .long("from")
                            .short('f')
                            .help("Starting point to create the branch from (e.g., origin/main, HEAD)")
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
                    .arg(
                        arg("no-hooks")
                            .long("no-hooks")
                            .help("Skip running post-create worktree_init command")
                    )
                    .arg(
                        arg("allow-hooks")
                            .long("allow-hooks")
                            .help("Run worktree_init hooks without an interactive confirmation prompt (otherwise the hook command is displayed and confirmed before each run)")
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
                            .required(false)
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
            .command(
                command("repair")
                    .about("Repair worktree administrative paths after worktrees have been moved")
                    .long_about("Runs 'git worktree repair' for each project to update the\n\
                                 administrative links between a repository and its worktrees.\n\
                                 Useful when worktree directories were moved on disk and git\n\
                                 has lost track of their new locations.\n\n\
                                 Examples:\n\
                                   meta worktree repair                 # Repair the current project\n\
                                   meta worktree repair --global        # Repair every project in the workspace\n\
                                   meta worktree repair --project foo   # Repair a specific project\n\
                                   meta worktree repair --dry-run       # Show what would be repaired")
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Single project to repair worktrees for")
                            .takes_value(true)
                    )
                    .arg(
                        arg("dry-run")
                            .long("dry-run")
                            .short('n')
                            .help("Show which projects would be repaired without running git")
                    )
            )
            .command(
                command("clean")
                    .about("Remove worktrees whose branches are already merged")
                    .aliases(vec!["tidy".to_string()])
                    .long_about("Remove worktrees whose branches are already merged into (or contribute\n\
                                 no changes relative to) their project's base branch — for example old\n\
                                 feature branches that have already landed.\n\n\
                                 Safe by design: worktrees with uncommitted or untracked changes, locked\n\
                                 worktrees, detached HEADs, and each project's primary worktree are always\n\
                                 skipped, and you are shown the full list and asked to confirm before\n\
                                 anything is removed. Each removed worktree's local branch is deleted with\n\
                                 'git branch -d' (which refuses unmerged branches) unless --keep-branches.\n\n\
                                 Scope follows the current directory: inside a project it cleans that\n\
                                 project; inside a subdirectory it cleans the projects beneath it; at the\n\
                                 workspace root it cleans every project. Use --global to force all.\n\n\
                                 Examples:\n\
                                   meta worktree clean                  # Preview, then confirm\n\
                                   meta worktree clean --dry-run        # Show candidates only\n\
                                   meta worktree clean --yes            # Skip the confirmation prompt\n\
                                   meta worktree clean --keep-branches  # Remove worktrees, keep branches\n\
                                   meta worktree clean --global         # Across every project")
                    .with_help_formatting()
                    .arg(
                        arg("dry-run")
                            .long("dry-run")
                            .short('n')
                            .help("Show what would be removed without removing anything")
                    )
                    .arg(
                        arg("yes")
                            .long("yes")
                            .short('y')
                            .help("Skip the confirmation prompt and remove eligible worktrees")
                    )
                    .arg(
                        arg("keep-branches")
                            .long("keep-branches")
                            .help("Remove the worktrees but do not delete their local branches")
                    )
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Clean a single project (overrides directory context)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("projects")
                            .long("projects")
                            .help("Clean a comma-separated list of projects (overrides directory context)")
                            .takes_value(true)
                    )
            )
            .handler("add", handle_add)
            .handler("remove", handle_remove)
            .handler("list", handle_list)
            .handler("prune", handle_prune)
            .handler("repair", handle_repair)
            .handler("clean", handle_clean)
            .build()
    }
}

/// Handler for the add command
fn handle_add(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let non_interactive = config
        .non_interactive
        .unwrap_or(NonInteractiveMode::Defaults);

    // Get or prompt for branch name
    let branch = match matches.get_one::<String>("branch") {
        Some(b) => b.clone(),
        None => {
            if is_interactive() {
                println!("\n  🌳 {}", "Create a new worktree".cyan().bold());
                prompt_text("Branch name or commit", None, false, non_interactive)?
            } else {
                return Err(anyhow::anyhow!(
                    "Branch name is required. Use 'meta worktree add <branch>' or run interactively in a terminal"
                ));
            }
        }
    };

    let commit = matches.get_one::<String>("commit");
    let from_ref = matches.get_one::<String>("from");
    let create_branch = matches.get_flag("create-branch");
    let path_suffix = matches.get_one::<String>("path").map(|s| s.as_str());
    let no_hooks = matches.get_flag("no-hooks");
    let allow_hooks = matches.get_flag("allow-hooks");

    // Prefer --from over positional commit arg
    let starting_point = from_ref.or(commit).map(|s| s.as_str());

    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());

    // Get current project context, unless --workspace was passed to force
    // workspace-wide scope.
    let global = config.scope_workspace;
    let current_project = if global {
        None
    } else {
        config.current_project()
    };

    // Collect selected projects
    let mut projects = Vec::new();

    if matches.get_flag("all") || global {
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
    } else if is_interactive() && current_project.is_none() {
        // Prompt for project selection if none specified and no current project
        let project_names: Vec<String> = config.meta_config.projects.keys().cloned().collect();

        if !project_names.is_empty() {
            println!("\n  📋 {}", "Select projects for worktree".cyan().bold());
            let selected = prompt_multiselect("Projects", project_names, vec![], non_interactive)?;
            projects.extend(selected);
        }
    }
    // If no projects specified, will use current project or trigger interactive selection

    add_worktrees(
        &branch,
        &projects,
        &base_path,
        path_suffix,
        create_branch,
        starting_point,
        no_hooks,
        allow_hooks,
        current_project.as_deref(),
        &config.meta_config,
    )?;
    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let non_interactive = config
        .non_interactive
        .unwrap_or(NonInteractiveMode::Defaults);

    // Get or prompt for branch name
    let branch = match matches.get_one::<String>("branch") {
        Some(b) => b.clone(),
        None => {
            if is_interactive() {
                println!("\n  🌳 {}", "Remove a worktree".cyan().bold());
                prompt_text(
                    "Branch name or worktree directory",
                    None,
                    false,
                    non_interactive,
                )?
            } else {
                return Err(anyhow::anyhow!(
                    "Branch name is required. Use 'meta worktree remove <branch>' or run interactively in a terminal"
                ));
            }
        }
    };

    let force = matches.get_flag("force");
    let global = config.scope_workspace;

    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());

    // Directory-context-aware scope. When no explicit project is given,
    // remove_worktrees limits auto-detection (and any interactive selection) to
    // this set, so removal never reaches out-of-scope projects.
    let scope = config.scoped_project_keys();

    // Collect explicitly selected projects, if any.
    let mut projects = Vec::new();
    if matches.get_flag("all") || global {
        projects.push("--all".to_string());
    } else if let Some(project) = matches.get_one::<String>("project") {
        if let Some(resolved) = config.resolve_project(project) {
            projects.push(resolved);
        } else {
            projects.push(project.clone());
        }
    } else if let Some(project_list) = matches.get_one::<String>("projects") {
        for p in project_list.split(',') {
            let trimmed = p.trim();
            if let Some(resolved) = config.resolve_project(trimmed) {
                projects.push(resolved);
            } else {
                projects.push(trimmed.to_string());
            }
        }
    }
    // If no projects specified, remove_worktrees selects from `scope` (using an
    // interactive multiselect when several in-scope projects have the branch).

    remove_worktrees(&branch, &projects, &base_path, force, &scope)?;
    Ok(())
}

/// Handler for the list command
fn handle_list(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());
    let scope = config.scoped_project_keys();
    if scope.is_empty() {
        println!("\n{}", "No projects in this directory".dimmed());
        return Ok(());
    }
    list_all_worktrees(&base_path, &scope)?;
    Ok(())
}

/// Handler for the prune command
fn handle_prune(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let dry_run = matches.get_flag("dry-run");
    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());
    let scope = config.scoped_project_keys();
    if scope.is_empty() {
        println!("\n{}", "No projects in this directory".dimmed());
        return Ok(());
    }
    prune_worktrees(&base_path, dry_run, &scope)?;
    Ok(())
}

/// Handler for the repair command
fn handle_repair(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());
    let dry_run = matches.get_flag("dry-run");

    // Explicit --project wins; otherwise use the directory-context-aware scope
    // (with --global forcing all projects).
    let scope: Vec<String> = if let Some(project) = matches.get_one::<String>("project") {
        vec![config
            .resolve_project(project)
            .unwrap_or_else(|| project.clone())]
    } else {
        config.scoped_project_keys()
    };

    if scope.is_empty() {
        println!("\n{}", "No projects in this directory".dimmed());
        return Ok(());
    }

    repair_worktrees(&base_path, &scope, dry_run)?;
    Ok(())
}

/// Handler for the clean command.
fn handle_clean(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config.meta_root().unwrap_or(config.working_dir.clone());
    let opts = CleanOptions {
        dry_run: matches.get_flag("dry-run"),
        assume_yes: matches.get_flag("yes"),
        keep_branches: matches.get_flag("keep-branches"),
    };
    let non_interactive = config
        .non_interactive
        .unwrap_or(NonInteractiveMode::Defaults);

    // Scope resolution: explicit --project/--projects win, otherwise the
    // directory-context-aware scope (with --global forcing all projects).
    let scope: Vec<String> = if let Some(project) = matches.get_one::<String>("project") {
        vec![config
            .resolve_project(project)
            .unwrap_or_else(|| project.clone())]
    } else if let Some(list) = matches.get_one::<String>("projects") {
        list.split(',')
            .map(|p| {
                let trimmed = p.trim();
                config
                    .resolve_project(trimmed)
                    .unwrap_or_else(|| trimmed.to_string())
            })
            .collect()
    } else {
        config.scoped_project_keys()
    };

    if scope.is_empty() {
        println!("\n{}", "No projects in scope for cleanup".dimmed());
        return Ok(());
    }

    clean_worktrees(&base_path, &scope, opts, non_interactive)?;
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
