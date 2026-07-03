use super::{
    check_workspace, convert_to_bare, import_project_recursive_with_options,
    import_project_with_options, init_child_workspace, list_projects, list_projects_minimal,
    remove_project, rename_project, show_project_tree, update_project_gitignore, update_projects,
};
use crate::plugins::shared::parse_depth_arg;
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
            .help_description(
                "Manage the set of repositories tracked in a workspace's .meta file.\n\
                 \n\
                 A project is an entry under \"projects\" in .meta, mapping a local path\n\
                 to a git URL (or the literal \"local\" for an in-tree directory). These\n\
                 commands add, list, update, and remove those entries, keeping the .meta\n\
                 file and the working tree in sync.\n\
                 \n\
                 Common workflows:\n\
                 \n\
                   meta project add web https://github.com/acme/web.git   clone and track\n\
                   meta project list --flat                               show every project\n\
                   meta project update --recursive                        sync nested repos\n\
                 \n\
                 Directories named in the .meta \"ignore\" list (.git, target,\n\
                 node_modules, ...) are skipped during discovery.",
            )
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
                    .help_description(
                        "Track a repository under a workspace path in .meta.\n\
                         \n\
                         Adds an entry to the \"projects\" map in .meta and reconciles the\n\
                         working tree with it. The source argument decides the mode: a git\n\
                         URL is cloned into <path>; an external local path that is a git repo\n\
                         is symlinked in and its remote recorded as external:<url>; an\n\
                         existing in-tree directory is adopted as-is (recording its remote, or\n\
                         local: when it has none). With no source you are prompted in a TTY.\n\
                         \n\
                         By default clones use the bare-with-worktrees layout (disable per\n\
                         workspace via default_bare); pass --bare to force it. Use --depth to\n\
                         perform a shallow git clone (the depth is recorded so re-clones via\n\
                         meta git update stay shallow); this does not apply to recursive\n\
                         imports. Use --init-git to git init a plain directory before tracking\n\
                         it. Use --recursive\n\
                         (with --max-depth, --flatten) to import nested meta repositories, or\n\
                         --no-recursive to override a workspace that enables it by default. If\n\
                         the added repo declares itself a meta module, you are shown it and,\n\
                         in a TTY, offered to enable it.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project add web https://github.com/acme/web.git   clone and track\n\
                           meta project add libs ../shared-libs                   symlink an external repo\n\
                           meta project add docs                                  adopt an existing directory\n\
                           meta project add mono URL --recursive --flatten        import nested repos flat",
                    )
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
                    .arg(
                        arg("depth")
                            .long("depth")
                            .help("Git shallow clone depth (limits history fetched when cloning)")
                            .takes_value(true)
                    )
            )
            .command(
                command("list")
                    .about("List all projects in the workspace (tree view by default)")
                    .help_description(
                        "List the projects tracked in the current workspace.\n\
                         \n\
                         Reads the \"projects\" map from .meta and prints the entries that\n\
                         fall within the directory-aware scope (running inside a subtree\n\
                         narrows the listing). The default is a tree view; --flat prints a\n\
                         detailed list with each project's URL and on-disk status (present,\n\
                         missing, or symlink), and --minimal prints just the names, one per\n\
                         line, for scripting.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project list             tree view of the workspace\n\
                           meta project list --flat       names with URLs and status\n\
                           meta project list --minimal    bare names for scripts",
                    )
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
                    .about("Display the project hierarchy as a tree")
                    .help_description(
                        "Show the workspace projects as an indented hierarchy.\n\
                         \n\
                         Equivalent to \"meta project list\" in its default mode, rendering the\n\
                         scoped \"projects\" entries as a tree that reflects nested paths. The\n\
                         same overrides apply: --flat switches to a detailed list with URLs\n\
                         and on-disk status, and --minimal prints only project names.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project tree            hierarchy of tracked projects\n\
                           meta project tree --flat      flat list with details",
                    )
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
                    .about("Update all projects by pulling the latest changes")
                    .help_description(
                        "Pull the latest changes for every tracked project.\n\
                         \n\
                         Walks the \"projects\" map and runs a fetch-and-pull on each repo that\n\
                         exists on disk. Projects whose directory is missing or that are not\n\
                         git repositories are skipped with a note, and per-repo failures are\n\
                         reported without aborting the rest. A summary of updated and failed\n\
                         counts is printed at the end.\n\
                         \n\
                         With --recursive, any updated project that is itself a meta workspace\n\
                         has its nested projects updated too, down to --depth levels (default\n\
                         3). Aliased as \"pull\".\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project update                 pull every project\n\
                           meta project update --recursive      also update nested workspaces",
                    )
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
                    .help_description(
                        "Stop tracking a project and remove its .meta entry.\n\
                         \n\
                         Deletes the named project from the \"projects\" map in .meta. With no\n\
                         name you are prompted to pick one in a TTY. Before removing, the\n\
                         working tree is checked for uncommitted changes (across all worktrees\n\
                         for bare repos); if any are found the command refuses unless --force\n\
                         is given. Plain --remove only edits .meta and leaves files in place;\n\
                         --force additionally deletes the project directory from disk.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project remove web              untrack web (keep files)\n\
                           meta project remove web --force       untrack and delete the directory",
                    )
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
                    .about("Deprecated: promote a local project to its remote (use 'check --fix')")
                    .help_description(
                        "Deprecated. Record a newly added remote for a project and ignore it.\n\
                         \n\
                         This behavior is now part of 'meta project check': running check\n\
                         detects any local: project whose repo has gained a remote and, with\n\
                         --fix, promotes it (rewrites the .meta entry from local: to the remote\n\
                         URL and adds the directory to .gitignore) across the whole workspace.\n\
                         Prefer 'meta project check --fix'; this single-project command is kept\n\
                         for backwards compatibility.\n\
                         \n\
                         Use this after a project that was tracked as local: (no remote) has\n\
                         had a git remote added to it. The command reads the repo's origin\n\
                         URL, rewrites the project's .meta entry from local: to that URL, and\n\
                         adds the project directory to the workspace .gitignore so the now\n\
                         independently cloneable repo is no longer committed to the meta repo.\n\
                         \n\
                         If the project already has a remote URL it reports that and does\n\
                         nothing; if no remote is configured yet it tells you to add one\n\
                         first.\n\
                         \n\
                         Examples:\n\
                         \n\
                           git -C web remote add origin URL\n\
                           meta project check --fix             promote web (and any others)\n\
                           meta project update-gitignore web    deprecated single-project form",
                    )
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
                    .about("Rename a project and move its directory")
                    .help_description(
                        "Rename a tracked project and move its on-disk directory.\n\
                         \n\
                         Re-keys the project's entry in the \"projects\" map from <old_name> to\n\
                         <new_name> and renames its directory to match. Fails if the new name\n\
                         is already tracked or the target directory already exists. For real\n\
                         git repositories (not symlinks) the working tree is checked first and\n\
                         the rename is refused when there are uncommitted changes; commit or\n\
                         stash them before retrying. Aliased as \"mv\" and \"move\".\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project rename web frontend     rename web to frontend",
                    )
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
                    .about("Convert a normal repository to a bare repo with worktrees")
                    .help_description(
                        "Convert a tracked repository to the bare-with-worktrees layout.\n\
                         \n\
                         Rewrites a normal project clone into a bare repository whose checked\n\
                         out branches live as worktrees, the same layout meta uses for new\n\
                         clones by default. The project must exist on disk, be a git\n\
                         repository, and not already be bare. If it is already configured as\n\
                         bare the command reports that and exits without changes.\n\
                         \n\
                         Use this to migrate older flat clones so they work with meta's\n\
                         worktree commands.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project convert-to-bare web     migrate web to bare layout",
                    )
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .help("Name of the project to convert")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("init")
                    .about("Initialize a nested child workspace and register it in the parent")
                    .help_description(
                        "Create a child metarepo under the current workspace and register it.\n\
                         \n\
                         Makes a directory <name> inside the workspace root, gives it its own\n\
                         .meta via the standard init path, and adds it to the parent config as a\n\
                         tracked local project. The child config inherits shared defaults from\n\
                         the enclosing .meta chain and overrides only what it needs, so settings\n\
                         and global scripts set once at the top flow down to nested repos.\n\
                         \n\
                         The name must be a relative path inside the workspace (no absolute\n\
                         paths and no .. escapes). Refuses to clobber an existing child config.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project init services/api    scaffold a nested workspace",
                    )
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Relative path of the child workspace to create")
                            .required(true)
                            .takes_value(true),
                    ),
            )
            .command(
                command("check")
                    .about("Report (and optionally fix) workspace drift")
                    .aliases(vec!["ck".to_string()])
                    .help_description(
                        "Check the workspace for drift between the config and the working tree.\n\
                         \n\
                         Runs a set of hygiene checks and prints a report. By default it is a\n\
                         dry run and exits non-zero when any drift is found, so it works as a CI\n\
                         or pre-commit lint. Pass --fix to apply the fixable corrections.\n\
                         \n\
                         Checks:\n  \
                           - .gitignore missing an entry for a remote-backed project (fixable)\n  \
                           - a tracked project whose directory is missing on disk (report)\n  \
                           - a top-level git repo on disk not tracked in the config (report)\n\
                         \n\
                         Stale .gitignore lines are reported context permitting but never\n\
                         auto-removed, since a former project entry cannot be told apart from a\n\
                         hand-added ignore.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta project check           report drift, non-zero exit if any\n\
                           meta project check --fix      apply the fixable corrections",
                    )
                    .with_help_formatting()
                    .arg(
                        arg("fix")
                            .long("fix")
                            .help("Apply the fixable corrections instead of only reporting"),
                    ),
            )
            .handler("add", handle_add)
            .handler("list", handle_list)
            .handler("tree", handle_tree)
            .handler("update", handle_update)
            .handler("remove", handle_remove)
            .handler("update-gitignore", handle_update_gitignore)
            .handler("rename", handle_rename)
            .handler("convert-to-bare", handle_convert_to_bare)
            .handler("init", handle_init)
            .handler("check", handle_check)
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
    let clone_depth = parse_depth_arg(matches.get_one::<String>("depth"))?;

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
        if clone_depth.is_some() {
            eprintln!(
                "  {} --depth is ignored for recursive imports; nested projects are cloned in full",
                "⚠".yellow()
            );
        }
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
        import_project_with_options(&path, source, &base_path, init_git, use_bare, clone_depth)?;
    }

    // If the added repo declares itself a meta module, surface it (and, in a
    // TTY, offer to enable it). Activation is always explicit.
    let repo_root = base_path.join(&path);
    crate::plugins::module::offer_enable_after_add(&repo_root, config, non_interactive);

    Ok(())
}

/// Handler for the list command
fn handle_list(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config
        .meta_root()
        .unwrap_or_else(|| config.working_dir.clone());
    let scope = config.scoped_project_keys();

    // Check flags for output format
    if matches.get_flag("minimal") {
        // Minimal: just project names
        list_projects_minimal(&base_path, &scope)?;
    } else if matches.get_flag("flat") {
        // Flat: list with details
        list_projects(&base_path, &scope)?;
    } else {
        // Default: tree view
        show_project_tree(&base_path, &scope)?;
    }
    Ok(())
}

/// Handler for the tree command
fn handle_tree(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config
        .meta_root()
        .unwrap_or_else(|| config.working_dir.clone());
    let scope = config.scoped_project_keys();

    // Check flags for output format (same as list command)
    if matches.get_flag("minimal") {
        // Minimal: just project names
        list_projects_minimal(&base_path, &scope)?;
    } else if matches.get_flag("flat") {
        // Flat: list with details
        list_projects(&base_path, &scope)?;
    } else {
        // Default: tree view
        show_project_tree(&base_path, &scope)?;
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

/// Handler for the init command: scaffold a nested child workspace.
fn handle_init(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    init_child_workspace(name, &base_path)?;
    Ok(())
}

/// Handler for the check command: report or fix workspace drift.
fn handle_check(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let fix = matches.get_flag("fix");

    let base_path = if config.meta_root().is_some() {
        config.meta_root().unwrap()
    } else {
        config.working_dir.clone()
    };

    check_workspace(&base_path, fix)?;
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
