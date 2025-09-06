use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};
use crate::{create_project, import_project, import_project_recursive, list_projects, remove_project, show_project_tree, update_projects};

pub struct ProjectPlugin;

impl ProjectPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("meta project")
            .about("Project management operations")
            .subcommand(
                Command::new("create")
                    .about("Clone a new project into the workspace (directory must not exist)")
                    .long_about("Clone a new project into the workspace.\n\n\
                                 This command will:\n\
                                 • Clone the repository into a new directory\n\
                                 • Add the project to the .meta file\n\
                                 • Update .gitignore to exclude the project\n\n\
                                 Fails if the directory already exists.")
                    .arg(
                        Arg::new("path")
                            .value_name("PATH")
                            .help("Local directory name for the project (must not exist)")
                            .required(true)
                    )
                    .arg(
                        Arg::new("repo-url")
                            .value_name("REPO_URL")
                            .help("Git repository URL to clone from")
                            .required(true)
                    )
            )
            .subcommand(
                Command::new("import")
                    .about("Import a project into the workspace")
                    .long_about("Import a project into the workspace.\n\n\
                                 This command will:\n\
                                 • Create a symlink if SOURCE is an external directory\n\
                                 • Clone if SOURCE is a git URL\n\
                                 • Auto-detect git remote if SOURCE is omitted\n\
                                 • Add the project to the .meta file\n\
                                 • Update .gitignore to exclude the project\n\
                                 • Optionally import nested meta repositories (with --recursive)\n\n\
                                 PATH: Where to place the project in the workspace\n\
                                 SOURCE: Git URL or path to external directory (optional)")
                    .arg(
                        Arg::new("path")
                            .value_name("PATH")
                            .help("Where to place the project in the workspace")
                            .required(true)
                    )
                    .arg(
                        Arg::new("source")
                            .value_name("SOURCE")
                            .help("Git URL or path to external directory (optional)")
                            .required(false)
                    )
                    .arg(
                        Arg::new("recursive")
                            .long("recursive")
                            .short('r')
                            .help("Recursively import nested meta repositories")
                            .action(clap::ArgAction::SetTrue)
                    )
                    .arg(
                        Arg::new("max-depth")
                            .long("max-depth")
                            .value_name("DEPTH")
                            .help("Maximum depth for recursive imports (default: 3)")
                            .value_parser(clap::value_parser!(usize))
                    )
                    .arg(
                        Arg::new("flatten")
                            .long("flatten")
                            .help("Import nested projects at root level instead of maintaining hierarchy")
                            .action(clap::ArgAction::SetTrue)
                    )
                    .arg(
                        Arg::new("no-recursive")
                            .long("no-recursive")
                            .help("Disable recursive import even if configured in .meta")
                            .action(clap::ArgAction::SetTrue)
                            .conflicts_with("recursive")
                    )
            )
            .subcommand(
                Command::new("list")
                    .about("List all projects in the workspace")
                    .long_about("List all projects in the workspace.\n\n\
                                 Shows each project with its status:\n\
                                 • ✓ Present - Directory exists with git repository\n\
                                 • ⚠ Present (not a git repo) - Directory exists but not a git repo\n\
                                 • ✗ Missing - Listed in .meta but directory doesn't exist")
                    .arg(
                        Arg::new("tree")
                            .long("tree")
                            .short('t')
                            .help("Display projects in tree format showing nested structure")
                            .action(clap::ArgAction::SetTrue)
                    )
            )
            .subcommand(
                Command::new("tree")
                    .about("Display project hierarchy as a tree")
                    .long_about("Display the project hierarchy as a tree.\n\n\
                                 Shows nested meta repositories and their structure.\n\
                                 • 📦 = Meta repository (contains .meta file)\n\
                                 • 📄 = Regular project")
            )
            .subcommand(
                Command::new("update")
                    .about("Update all projects (pull latest changes)")
                    .long_about("Update all projects by pulling latest changes.\n\n\
                                 This command will:\n\
                                 • Fetch and pull changes for each project\n\
                                 • Optionally update nested repositories (with --recursive)\n\
                                 • Skip missing or non-git directories\n\
                                 • Report success/failure for each project")
                    .arg(
                        Arg::new("recursive")
                            .long("recursive")
                            .short('r')
                            .help("Also update nested repositories")
                            .action(clap::ArgAction::SetTrue)
                    )
                    .arg(
                        Arg::new("depth")
                            .long("depth")
                            .value_name("DEPTH")
                            .help("Maximum depth for recursive updates (default: 3)")
                            .value_parser(clap::value_parser!(usize))
                    )
            )
            .subcommand(
                Command::new("remove")
                    .about("Remove a project from the workspace")
                    .long_about("Remove a project from the workspace.\n\n\
                                 This command will:\n\
                                 • Check for uncommitted changes (unless --force is used)\n\
                                 • Remove the project from the .meta file\n\
                                 • Remove the project from .gitignore\n\
                                 • Optionally delete the project directory (with --force)\n\n\
                                 By default, the directory is kept on disk to prevent data loss.")
                    .arg(
                        Arg::new("name")
                            .value_name("NAME")
                            .help("Name of the project to remove")
                            .required(true)
                    )
                    .arg(
                        Arg::new("force")
                            .long("force")
                            .short('f')
                            .help("Force removal even with uncommitted changes, and delete directory")
                            .action(clap::ArgAction::SetTrue)
                    )
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for ProjectPlugin {
    fn name(&self) -> &str {
        "project"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("project")
                .visible_alias("p")
                .about("Project management operations")
                .disable_help_subcommand(true)
                .allow_external_subcommands(true) // This allows unknown subcommands to pass through
                .subcommand(
                    Command::new("create")
                        .visible_alias("c")
                        .about("Clone a new project into the workspace (directory must not exist)")
                        .long_about("Clone a new project into the workspace.\n\n\
                                     This command will:\n\
                                     • Clone the repository into a new directory\n\
                                     • Add the project to the .meta file\n\
                                     • Update .gitignore to exclude the project\n\n\
                                     Fails if the directory already exists.")
                        .arg(
                            Arg::new("path")
                                .value_name("PATH")
                                .help("Local directory name for the project (must not exist)")
                                .required(true)
                        )
                        .arg(
                            Arg::new("repo-url")
                                .value_name("REPO_URL")
                                .help("Git repository URL to clone from")
                                .required(true)
                        )
                )
                .subcommand(
                    Command::new("import")
                        .visible_alias("i")
                        .about("Import a project into the workspace")
                        .long_about("Import a project into the workspace.\n\n\
                                     This command will:\n\
                                     • Create a symlink if SOURCE is an external directory\n\
                                     • Clone if SOURCE is a git URL\n\
                                     • Auto-detect git remote if SOURCE is omitted\n\
                                     • Add the project to the .meta file\n\
                                     • Update .gitignore to exclude the project\n\
                                     • Optionally import nested meta repositories (with --recursive)\n\n\
                                     PATH: Where to place the project in the workspace\n\
                                     SOURCE: Git URL or path to external directory (optional)")
                        .arg(
                            Arg::new("path")
                                .value_name("PATH")
                                .help("Where to place the project in the workspace")
                                .required(true)
                        )
                        .arg(
                            Arg::new("source")
                                .value_name("SOURCE")
                                .help("Git URL or path to external directory (optional)")
                                .required(false)
                        )
                        .arg(
                            Arg::new("recursive")
                                .long("recursive")
                                .short('r')
                                .help("Recursively import nested meta repositories")
                                .action(clap::ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("max-depth")
                                .long("max-depth")
                                .value_name("DEPTH")
                                .help("Maximum depth for recursive imports (default: 3)")
                                .value_parser(clap::value_parser!(usize))
                        )
                        .arg(
                            Arg::new("flatten")
                                .long("flatten")
                                .help("Import nested projects at root level instead of maintaining hierarchy")
                                .action(clap::ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("no-recursive")
                                .long("no-recursive")
                                .help("Disable recursive import even if configured in .meta")
                                .action(clap::ArgAction::SetTrue)
                                .conflicts_with("recursive")
                        )
                )
                .subcommand(
                    Command::new("list")
                        .visible_aliases(["ls", "l"])
                        .about("List all projects in the workspace")
                        .long_about("List all projects in the workspace.\n\n\
                                     Shows each project with its status:\n\
                                     • ✓ Present - Directory exists with git repository\n\
                                     • ⚠ Present (not a git repo) - Directory exists but not a git repo\n\
                                     • ✗ Missing - Listed in .meta but directory doesn't exist")
                        .arg(
                            Arg::new("tree")
                                .long("tree")
                                .short('t')
                                .help("Display projects in tree format showing nested structure")
                                .action(clap::ArgAction::SetTrue)
                        )
                )
                .subcommand(
                    Command::new("tree")
                        .about("Display project hierarchy as a tree")
                        .long_about("Display the project hierarchy as a tree.\n\n\
                                     Shows nested meta repositories and their structure.\n\
                                     • 📦 = Meta repository (contains .meta file)\n\
                                     • 📄 = Regular project")
                )
                .subcommand(
                    Command::new("update")
                        .visible_alias("pull")
                        .about("Update all projects (pull latest changes)")
                        .long_about("Update all projects by pulling latest changes.\n\n\
                                     This command will:\n\
                                     • Fetch and pull changes for each project\n\
                                     • Optionally update nested repositories (with --recursive)\n\
                                     • Skip missing or non-git directories\n\
                                     • Report success/failure for each project")
                        .arg(
                            Arg::new("recursive")
                                .long("recursive")
                                .short('r')
                                .help("Also update nested repositories")
                                .action(clap::ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("depth")
                                .long("depth")
                                .value_name("DEPTH")
                                .help("Maximum depth for recursive updates (default: 3)")
                                .value_parser(clap::value_parser!(usize))
                        )
                )
                .subcommand(
                    Command::new("remove")
                        .visible_aliases(["rm", "r"])
                        .about("Remove a project from the workspace")
                        .long_about("Remove a project from the workspace.\n\n\
                                     This command will:\n\
                                     • Check for uncommitted changes (unless --force is used)\n\
                                     • Remove the project from the .meta file\n\
                                     • Remove the project from .gitignore\n\
                                     • Optionally delete the project directory (with --force)\n\n\
                                     By default, the directory is kept on disk to prevent data loss.")
                        .arg(
                            Arg::new("name")
                                .value_name("NAME")
                                .help("Name of the project to remove")
                                .required(true)
                        )
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .short('f')
                                .help("Force removal even with uncommitted changes, and delete directory")
                                .action(clap::ArgAction::SetTrue)
                        )
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // If no subcommand is provided, show help
        if matches.subcommand().is_none() {
            return self.show_help();
        }
        
        match matches.subcommand() {
            Some(("create", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                let repo_url = sub_matches.get_one::<String>("repo-url").unwrap();
                
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                create_project(path, repo_url, &base_path)?;
                Ok(())
            }
            Some(("import", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                let source = sub_matches.get_one::<String>("source").map(|s| s.as_str());
                
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                // Check for recursive import flags
                let recursive = sub_matches.get_flag("recursive");
                let no_recursive = sub_matches.get_flag("no-recursive");
                let flatten = sub_matches.get_flag("flatten");
                let max_depth = sub_matches.get_one::<usize>("max-depth").copied();
                
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
            Some(("list", sub_matches)) => {
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                // Check if --tree flag is set
                if sub_matches.get_flag("tree") {
                    show_project_tree(&base_path)?;
                } else {
                    list_projects(&base_path)?;
                }
                Ok(())
            }
            Some(("tree", _)) => {
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                show_project_tree(&base_path)?;
                Ok(())
            }
            Some(("update", sub_matches)) => {
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                let recursive = sub_matches.get_flag("recursive");
                let depth = sub_matches.get_one::<usize>("depth").copied();
                
                update_projects(&base_path, recursive, depth)?;
                Ok(())
            }
            Some(("remove", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name").unwrap();
                let force = sub_matches.get_flag("force");
                
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                remove_project(name, &base_path, force)?;
                Ok(())
            }
            Some((external_cmd, _args)) => {
                // Handle unknown/external subcommands by showing help
                println!("Unknown project subcommand: '{}'", external_cmd);
                println!();
                self.show_help()
            }
            None => {
                // This case is already handled above, but keeping for completeness
                self.show_help()
            }
        }
    }
}

impl Default for ProjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}