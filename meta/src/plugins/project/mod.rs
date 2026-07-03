use anyhow::{Context, Result};
use colored::*;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository, Status, StatusOptions};
use metarepo_core::{MetaConfig, NestedConfig, ProjectEntry};
use std::collections::{HashSet, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// Import shared git operations
use crate::plugins::shared::{clone_with_auth, create_default_worktree};

#[cfg(unix)]
use std::os::unix::fs;

#[cfg(windows)]
use std::os::windows::fs;

// Export the main plugin
pub use self::convert::convert_to_bare;
pub use self::plugin::ProjectPlugin;

mod convert;
mod plugin;

/// Locate the workspace config file inside `base_path`, honoring every
/// supported filename/format (`.meta`, `.metarepo`, `.metarepo.yaml`, ...)
/// rather than the legacy `.meta` name alone. Returns the discovered path;
/// `MetaConfig::load_from_file` / `save_to_file` then detect the format from
/// that filename, so existing load/save calls keep working unchanged.
fn locate_workspace_config(base_path: &Path) -> Result<PathBuf> {
    Ok(MetaConfig::locate_in(base_path)?.path)
}

/// Initialize a child metarepo under `base_path/name` and register it in the
/// enclosing (parent) config, so nested config inheritance and recursive_import
/// see it. Mirrors [`convert_to_bare`] in shape: operate on a directory under
/// the workspace root and update the parent `.meta`.
///
/// The child gets its own `.meta` via the standard init path, so its config
/// inherits shared defaults from the parent chain and overrides only what it
/// needs. `name` must be a simple relative path inside the workspace (no
/// absolute paths and no `..` escapes).
pub fn init_child_workspace(name: &str, base_path: &Path) -> Result<()> {
    // Reject absolute paths and parent-directory escapes: a child workspace must
    // live inside the enclosing metarepo for the cascade to make sense.
    let rel = Path::new(name);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(anyhow::anyhow!(
            "Child workspace name must be a relative path inside the workspace (got '{}')",
            name
        ));
    }

    let child_path = base_path.join(name);
    std::fs::create_dir_all(&child_path).with_context(|| {
        format!(
            "Failed to create child workspace directory '{}'",
            child_path.display()
        )
    })?;

    // Bail out if the child already has a config rather than overwriting it.
    if dir_has_meta_config(&child_path) {
        return Err(anyhow::anyhow!(
            "'{}' already contains a metarepo config; nothing to initialize",
            child_path.display()
        ));
    }

    // Create the child .meta using the standard init path (default options).
    let report = crate::plugins::init::initialize_meta_repo_with_options(
        &child_path,
        crate::plugins::init::InitOptions::default(),
    )?;

    // Register the child in the parent config so it is a tracked project and
    // recursive_import can pull its projects up. Local (non-remote) projects use
    // the `local:` URL convention already used by `meta project add`.
    let parent_cfg_path = locate_workspace_config(base_path)?;
    let mut parent = MetaConfig::load_from_file(&parent_cfg_path)?;
    if parent.projects.contains_key(name) {
        println!(
            "  {} '{}' is already registered in the parent config; left unchanged.",
            "·".bright_black(),
            name
        );
    } else {
        parent.projects.insert(
            name.to_string(),
            ProjectEntry::Url(format!("local:{}", name)),
        );
        parent.save_to_file(&parent_cfg_path)?;
    }

    let child_cfg = report
        .config_path
        .unwrap_or_else(|| child_path.join(".metarepo"));
    println!(
        "\n  {} {}",
        "✅".green(),
        format!("Initialized child workspace '{}'", name)
            .bold()
            .green()
    );
    println!(
        "     {} {}",
        "└".bright_black(),
        format!("Created {}", child_cfg.display())
            .italic()
            .bright_black()
    );
    println!(
        "     {} {}",
        "└".bright_black(),
        "Registered in the parent config".italic().bright_black()
    );
    println!();

    Ok(())
}

/// Whether `dir` directly contains any recognized metarepo config file. Used to
/// detect whether a child project is itself a meta repository, regardless of
/// which supported config filename it uses.
fn dir_has_meta_config(dir: &Path) -> bool {
    MetaConfig::config_in_dir(dir).is_some()
}

/// Load the metarepo config directly inside `dir`, if `dir` is itself a meta
/// repository. Returns `None` when no recognized config is present or it fails
/// to parse (best-effort, for tree display of nested repos).
fn load_dir_meta_config(dir: &Path) -> Option<MetaConfig> {
    MetaConfig::config_in_dir(dir).and_then(|found| MetaConfig::load_from_file(found.path).ok())
}

/// Context for tracking nested repository imports
pub struct ImportContext {
    /// Set of repository URLs that have been visited
    visited: HashSet<String>,
    /// Current import chain for cycle detection
    import_chain: Vec<String>,
    /// Current depth in the import tree
    current_depth: usize,
    /// Maximum allowed depth
    max_depth: usize,
    /// Whether cycle detection is enabled
    cycle_detection: bool,
    /// Projects to ignore during nested import
    ignore_nested: HashSet<String>,
    /// Whether to flatten the structure
    flatten: bool,
    /// Base path for all imports
    base_path: PathBuf,
}

impl ImportContext {
    pub fn new(base_path: &Path, nested_config: Option<&NestedConfig>) -> Self {
        let default_config = NestedConfig::default();
        let config = nested_config.unwrap_or(&default_config);
        Self {
            visited: HashSet::new(),
            import_chain: Vec::new(),
            current_depth: 0,
            max_depth: config.max_depth,
            cycle_detection: config.cycle_detection,
            ignore_nested: config.ignore_nested.iter().cloned().collect(),
            flatten: config.flatten,
            base_path: base_path.to_path_buf(),
        }
    }

    /// Check if importing this URL would create a cycle
    pub fn would_create_cycle(&self, url: &str) -> Option<Vec<String>> {
        if !self.cycle_detection {
            return None;
        }

        if self.import_chain.contains(&url.to_string()) {
            let mut cycle_path = self.import_chain.clone();
            cycle_path.push(url.to_string());
            return Some(cycle_path);
        }

        None
    }

    /// Check if we've reached the maximum depth
    pub fn at_max_depth(&self) -> bool {
        self.current_depth >= self.max_depth
    }

    /// Enter a new import level
    pub fn enter_import(&mut self, url: &str) -> Result<()> {
        if let Some(cycle_path) = self.would_create_cycle(url) {
            return Err(anyhow::anyhow!(
                "Circular dependency detected!\n  {}\n\nCycle path:\n{}",
                cycle_path.join(" → "),
                cycle_path
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!(
                        "  {}. {}{}",
                        i + 1,
                        p,
                        if i == cycle_path.len() - 1 {
                            " (CYCLE!)"
                        } else {
                            ""
                        }
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        if self.at_max_depth() {
            return Err(anyhow::anyhow!(
                "Maximum recursion depth ({}) exceeded. Use --max-depth to increase the limit.",
                self.max_depth
            ));
        }

        self.import_chain.push(url.to_string());
        self.current_depth += 1;
        self.visited.insert(url.to_string());

        Ok(())
    }

    /// Exit the current import level
    pub fn exit_import(&mut self) {
        self.import_chain.pop();
        if self.current_depth > 0 {
            self.current_depth -= 1;
        }
    }

    /// Check if a project should be ignored
    pub fn should_ignore(&self, project_name: &str) -> bool {
        self.ignore_nested.contains(project_name)
    }

    /// Check if we should flatten the import structure
    pub fn should_flatten(&self) -> bool {
        self.flatten
    }
}

pub fn import_project(project_path: &str, source: Option<&str>, base_path: &Path) -> Result<()> {
    import_project_with_options(project_path, source, base_path, false, false, None)
}

#[allow(clippy::too_many_arguments)]
pub fn import_project_with_options(
    project_path: &str,
    source: Option<&str>,
    base_path: &Path,
    init_git: bool,
    bare: bool,
    clone_depth: Option<i32>,
) -> Result<()> {
    // Reject path-traversal / absolute / null-byte project names before they
    // flow into base_path.join(...) or filesystem operations below.
    metarepo_core::validate_path_segment("project name", project_path)?;
    if let Some(src) = source {
        metarepo_core::validate_project_url(src).ok(); // tolerate local paths
    }

    // Find and load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Check if project already exists in config
    if config.projects.contains_key(project_path) {
        return Err(anyhow::anyhow!(
            "Project '{}' already exists in workspace config",
            project_path
        ));
    }

    let local_project_path = base_path.join(project_path);
    // Defense in depth: even though we validated project_path above, confirm
    // the canonical join stays inside base_path.
    metarepo_core::ensure_within_base(base_path, &local_project_path)?;

    // Determine what the source is and how to handle it
    let (final_repo_url, is_external) = if let Some(src) = source {
        if !metarepo_core::is_supported_git_url(src) {
            // This is a local path (relative or absolute)
            let external_path = if src.starts_with('/') {
                PathBuf::from(src)
            } else {
                // Resolve relative path from current working directory or base path
                base_path
                    .join(src)
                    .canonicalize()
                    .or_else(|_| {
                        std::env::current_dir()
                            .map(|cwd| cwd.join(src).canonicalize())
                            .unwrap_or(Ok(PathBuf::from(src)))
                    })
                    .unwrap_or_else(|_| PathBuf::from(src))
            };

            // Check if this path is outside the workspace (external)
            let is_external_dir = !external_path.starts_with(base_path)
                || external_path == base_path.join(project_path);

            if external_path.exists() && external_path.join(".git").exists() {
                if is_external_dir {
                    // External directory exists and is a git repo - create symlink
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;

                    // Create symlink to external directory
                    if local_project_path.exists() {
                        return Err(anyhow::anyhow!(
                            "Directory '{}' already exists",
                            project_path
                        ));
                    }

                    println!("\n  {} {}", "🔗".cyan(), "Creating symlink...".bold());
                    println!(
                        "     {} {}",
                        "From:".bright_black(),
                        project_path.bright_white()
                    );
                    println!(
                        "     {} {}",
                        "To:".bright_black(),
                        external_path.display().to_string().bright_magenta()
                    );
                    create_symlink(&external_path, &local_project_path)?;

                    let url = if let Some(detected_url) = remote_url {
                        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                        format!("external:{}", detected_url)
                    } else {
                        println!(
                            "     {} {}",
                            "Type:".bright_black(),
                            "Local project (no remote)".yellow()
                        );
                        format!("external:local:{}", external_path.display())
                    };

                    (url, true)
                } else {
                    // Internal directory - just use it as is
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;

                    let url = if let Some(detected_url) = remote_url {
                        println!("\n  {} {}", "📍".green(), "Using existing directory".bold());
                        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                        detected_url
                    } else {
                        println!(
                            "\n  {} {}",
                            "📍".yellow(),
                            "Using existing directory".bold()
                        );
                        println!(
                            "     {} {}",
                            "Type:".bright_black(),
                            "Local project (no remote)".yellow()
                        );
                        format!("local:{}", project_path)
                    };

                    (url, false)
                }
            } else if external_path.exists() {
                return Err(anyhow::anyhow!(
                    "Directory '{}' exists but is not a git repository",
                    external_path.display()
                ));
            } else {
                // Path doesn't exist - treat as URL for cloning
                (src.to_string(), false)
            }
        } else {
            // Regular git URL
            if metarepo_core::is_unencrypted_git_scheme(src) {
                eprintln!(
                    "  {} Source uses an unencrypted scheme ({}): traffic is unauthenticated",
                    "⚠".yellow(),
                    src.split("://").next().unwrap_or(src)
                );
            }
            (src.to_string(), false)
        }
    } else {
        // No URL provided, check if directory exists locally
        if local_project_path.exists() {
            if local_project_path.join(".git").exists() {
                // Directory is already a git repository
                let repo = Repository::open(&local_project_path)?;
                let remote_url = get_remote_url(&repo)?;

                let url = if let Some(detected_url) = remote_url {
                    println!(
                        "\n  {} {}",
                        "📍".green(),
                        "Using existing git repository".bold()
                    );
                    println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                    detected_url
                } else {
                    println!(
                        "\n  {} {}",
                        "📍".yellow(),
                        "Using existing git repository".bold()
                    );
                    println!(
                        "     {} {}",
                        "Type:".bright_black(),
                        "Local project (no remote)".yellow()
                    );
                    format!("local:{}", project_path)
                };

                (url, false)
            } else {
                // Directory exists but is not a git repository
                let should_init = if init_git {
                    true
                } else {
                    // Try to prompt user to initialize git repo
                    println!(
                        "\n  {} {}",
                        "❓".yellow(),
                        format!(
                            "Directory '{}' exists but is not a git repository",
                            project_path
                        )
                        .bold()
                    );
                    print!(
                        "     {} Initialize as git repository? [y/N]: ",
                        "→".bright_black()
                    );

                    // Try to flush stdout and read input
                    match io::stdout().flush() {
                        Ok(_) => {
                            let mut input = String::new();
                            match io::stdin().read_line(&mut input) {
                                Ok(_) => {
                                    let response = input.trim().to_lowercase();
                                    response == "y" || response == "yes"
                                }
                                Err(_) => {
                                    // If we can't read input, provide helpful error
                                    println!();
                                    println!(
                                        "     {} {}",
                                        "⚠️".yellow(),
                                        "Unable to read input from terminal".yellow()
                                    );
                                    println!(
                                        "     {} {}",
                                        "└".bright_black(),
                                        "Use --init-git flag to automatically initialize git"
                                            .dimmed()
                                    );
                                    return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                                }
                            }
                        }
                        Err(_) => {
                            println!();
                            println!(
                                "     {} {}",
                                "⚠️".yellow(),
                                "Terminal interaction not available".yellow()
                            );
                            return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                        }
                    }
                };

                if should_init {
                    println!(
                        "\n  {} {}",
                        "🌱".green(),
                        "Initializing git repository...".bold()
                    );
                    Repository::init(&local_project_path)?;
                    println!(
                        "     {} {}",
                        "✅".green(),
                        "Git repository initialized".green()
                    );
                    println!(
                        "     {} {}",
                        "Type:".bright_black(),
                        "Local project (no remote)".yellow()
                    );
                    (format!("local:{}", project_path), false)
                } else {
                    return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                }
            }
        } else {
            return Err(anyhow::anyhow!(
                "Directory '{}' doesn't exist and no repository URL provided",
                project_path
            ));
        }
    };

    // If not external and directory doesn't exist, clone it
    if !is_external && !local_project_path.exists() {
        if !final_repo_url.starts_with("local:") && !final_repo_url.starts_with("external:") {
            println!("\n  {} {}", "🌱".green(), "Adding new project...".bold());
            println!(
                "     {} {}",
                "Name:".bright_black(),
                project_path.bright_white()
            );
            println!(
                "     {} {}",
                "Source:".bright_black(),
                final_repo_url.bright_cyan()
            );

            if bare {
                println!(
                    "     {} {}",
                    "Type:".bright_black(),
                    "Bare repository".bright_magenta()
                );
                println!(
                    "     {} {}",
                    "Status:".bright_black(),
                    "Cloning bare repository...".yellow()
                );

                // Clone as bare repo to <project>/.git/
                let bare_path = local_project_path.join(".git");
                clone_with_auth(&final_repo_url, &bare_path, true, clone_depth)?;

                // Create the project directory
                std::fs::create_dir_all(&local_project_path)?;

                // Create default worktree at <project>/<default-branch>/
                println!(
                    "     {} {}",
                    "Status:".bright_black(),
                    "Creating default worktree...".yellow()
                );
                create_default_worktree(&bare_path, &local_project_path)?;

                println!(
                    "     {} {}",
                    "✅".green(),
                    "Bare repository and default worktree created".green()
                );
            } else {
                println!(
                    "     {} {}",
                    "Status:".bright_black(),
                    "Cloning repository...".yellow()
                );
                clone_with_auth(&final_repo_url, &local_project_path, false, clone_depth)?;
            }
        } else {
            return Err(anyhow::anyhow!("Cannot clone a local project URL"));
        }
    }

    // Add to .meta file
    if bare || clone_depth.is_some() {
        // Use ProjectMetadata format to store the bare flag and/or clone depth
        use metarepo_core::ProjectMetadata;
        config.projects.insert(
            project_path.to_string(),
            ProjectEntry::Metadata(ProjectMetadata {
                url: final_repo_url.clone(),
                aliases: Vec::new(),
                scripts: std::collections::HashMap::new(),
                env: std::collections::HashMap::new(),
                worktree_init: None,
                bare: if bare { Some(true) } else { None },
                enabled: None,
                depth: clone_depth,
            }),
        );
    } else {
        config.projects.insert(
            project_path.to_string(),
            ProjectEntry::Url(final_repo_url.clone()),
        );
    }
    config.save_to_file(&meta_file_path)?;

    // Update .gitignore only if project has a remote URL (not local:)
    if !final_repo_url.starts_with("local:") {
        update_gitignore(base_path, project_path)?;
    }

    // Success message
    println!(
        "\n  {} {}",
        "✅".green(),
        format!("Successfully added '{}'", project_path)
            .bold()
            .green()
    );

    if is_external {
        println!(
            "     {} {}",
            "└".bright_black(),
            "Created symlink to external directory"
                .italic()
                .bright_black()
        );
    }

    if final_repo_url.starts_with("local:") {
        println!(
            "     {} {}",
            "└".bright_black(),
            "Updated workspace config (not added to .gitignore)"
                .italic()
                .bright_black()
        );
        println!(
            "     {} {}",
            "ℹ".bright_black(),
            format!(
                "Run 'meta project update-gitignore {}' after adding a remote",
                project_path
            )
            .dimmed()
        );
    } else {
        println!(
            "     {} {}",
            "└".bright_black(),
            "Updated workspace config and .gitignore"
                .italic()
                .bright_black()
        );
    }
    println!();

    Ok(())
}

/// Import a project with recursive nested repository support
pub fn import_project_recursive(
    project_path: &str,
    source: Option<&str>,
    base_path: &Path,
    recursive: bool,
    max_depth: Option<usize>,
    flatten: bool,
) -> Result<()> {
    import_project_recursive_with_options(
        project_path,
        source,
        base_path,
        recursive,
        max_depth,
        flatten,
        false,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn import_project_recursive_with_options(
    project_path: &str,
    source: Option<&str>,
    base_path: &Path,
    recursive: bool,
    max_depth: Option<usize>,
    flatten: bool,
    init_git: bool,
    bare: bool,
) -> Result<()> {
    // Load the root meta config
    let meta_file_path = locate_workspace_config(base_path)?;

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    // Create import context with configuration
    let mut nested_config = config.nested.clone().unwrap_or_default();
    if let Some(depth) = max_depth {
        nested_config.max_depth = depth;
    }
    if recursive {
        nested_config.recursive_import = true;
    }
    nested_config.flatten = flatten;

    let mut context = ImportContext::new(base_path, Some(&nested_config));

    // Import the root project. Shallow-clone depth is not threaded through the
    // recursive import path; nested clones always use a full clone.
    import_project_with_options(project_path, source, base_path, init_git, bare, None)?;

    // If recursive import is enabled, process nested repositories
    if nested_config.recursive_import {
        let project_path_buf = base_path.join(project_path);
        if let Err(e) = process_nested_repositories(&project_path_buf, &mut context, &nested_config)
        {
            eprintln!(
                "\n  {} {}",
                "⚠️".yellow(),
                "Warning: Failed to process nested repositories".yellow()
            );
            eprintln!("     {} {}", "└".bright_black(), e.to_string().bright_red());
        }
    }

    Ok(())
}

/// Process nested repositories in a project
fn process_nested_repositories(
    project_path: &Path,
    context: &mut ImportContext,
    nested_config: &NestedConfig,
) -> Result<()> {
    // Check if this project has a config file (is a meta repository)
    let Some(nested) = MetaConfig::config_in_dir(project_path) else {
        return Ok(()); // Not a meta repository, nothing to do
    };
    let nested_meta_path = nested.path;

    println!(
        "\n  {} {}",
        "🔍".cyan(),
        format!(
            "Found nested meta repository in '{}'",
            project_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
        .bold()
    );

    // Load the nested meta configuration
    let nested_meta = MetaConfig::load_from_file(&nested_meta_path)?;

    // Check depth before processing
    if context.at_max_depth() {
        println!(
            "     {} {}",
            "⚠️".yellow(),
            format!(
                "Skipping nested imports (max depth {} reached)",
                context.max_depth
            )
            .yellow()
        );
        return Ok(());
    }

    // Process each project in the nested meta file
    let mut import_queue = VecDeque::new();
    for name in nested_meta.projects.keys() {
        if context.should_ignore(name) {
            println!(
                "     {} {}",
                "⏭".bright_black(),
                format!("Skipping ignored project '{}'", name).dimmed()
            );
            continue;
        }
        let url = nested_meta
            .get_project_url(name)
            .unwrap_or_else(|| format!("local:{}", name));
        import_queue.push_back((name.clone(), url));
    }

    println!(
        "     {} {}",
        "📦".blue(),
        format!("Found {} nested projects to import", import_queue.len()).bright_white()
    );

    while let Some((name, url)) = import_queue.pop_front() {
        // Determine the import path based on flatten setting
        let import_path = if context.should_flatten() {
            // Import at root level
            context.base_path.join(&name)
        } else {
            // Maintain hierarchy
            project_path.join(&name)
        };

        // Check for cycles
        if let Err(e) = context.enter_import(&url) {
            eprintln!("\n  {} {}", "❌".red(), e.to_string().red());
            continue; // Skip this import but continue with others
        }

        // Import the nested project
        println!(
            "\n  {} {}",
            "📥".green(),
            format!("Importing nested project '{}'", name).bold()
        );
        println!("     {} {}", "URL:".bright_black(), url.bright_cyan());
        println!(
            "     {} {}",
            "Path:".bright_black(),
            import_path.display().to_string().bright_white()
        );

        // Perform the actual import
        // For nested imports, we need to handle the base path differently
        let (import_name, import_base) = if context.should_flatten() {
            // For flattened imports, use root base path and full name
            (name.clone(), context.base_path.clone())
        } else {
            // For hierarchical imports, import into the parent project
            if let Some(parent) = import_path.parent() {
                if let Some(file_name) = import_path.file_name() {
                    (
                        file_name.to_string_lossy().to_string(),
                        parent.to_path_buf(),
                    )
                } else {
                    (name.clone(), parent.to_path_buf())
                }
            } else {
                (name.clone(), project_path.to_path_buf())
            }
        };

        // Skip if directory already exists (might be from parent import)
        let target_path = import_base.join(&import_name);
        if target_path.exists() {
            println!(
                "     {} {}",
                "⏭".yellow(),
                format!("Directory '{}' already exists, skipping", import_name).yellow()
            );
            context.exit_import();
            continue;
        }

        // Clone the repository directly without going through import_project
        // to avoid .meta file conflicts
        // Handle special URL formats (external:, local:)
        let actual_url = if url.starts_with("external:local:") {
            // This is a local external project, skip it
            println!(
                "     {} {}",
                "⏭".yellow(),
                format!("Skipping local external project '{}'", name).yellow()
            );
            context.exit_import();
            continue;
        } else if url.starts_with("external:") {
            // Strip the "external:" prefix to get the actual URL
            url.strip_prefix("external:").unwrap_or(&url).to_string()
        } else if url.starts_with("local:") {
            // Local projects don't need cloning
            println!(
                "     {} {}",
                "⏭".yellow(),
                format!("Skipping local project '{}'", name).yellow()
            );
            context.exit_import();
            continue;
        } else {
            url.clone()
        };

        println!(
            "     {} {}",
            "📦".blue(),
            format!("Cloning into '{}'", target_path.display()).bright_white()
        );
        // Nested imports don't support bare repositories or shallow clones for now
        if let Err(e) = clone_with_auth(&actual_url, &target_path, false, None) {
            eprintln!(
                "     {} {}",
                "❌".red(),
                format!("Failed to clone '{}': {}", name, e).red()
            );
            context.exit_import();
            continue;
        }

        // Recursively process this nested repository if it's also a meta repo
        if nested_config.recursive_import && !context.at_max_depth() {
            if let Err(e) = process_nested_repositories(&import_path, context, nested_config) {
                eprintln!(
                    "     {} {}",
                    "⚠️".yellow(),
                    format!(
                        "Warning: Failed to process nested repos in '{}': {}",
                        name, e
                    )
                    .yellow()
                );
            }
        }

        context.exit_import();
    }

    Ok(())
}

fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::symlink(target, link)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        // On Windows, try to create a directory symlink
        // This requires admin privileges or developer mode
        if target.is_dir() {
            fs::symlink_dir(target, link)?;
        } else {
            fs::symlink_file(target, link)?;
        }
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err(anyhow::anyhow!(
            "Symbolic links are not supported on this platform"
        ))
    }
}

fn get_remote_url(repo: &Repository) -> Result<Option<String>> {
    // Try to get the 'origin' remote first, then fallback to first available remote
    let remote_names = repo.remotes()?;

    // First try 'origin'
    if remote_names.iter().any(|n| n == Ok(Some("origin"))) {
        if let Ok(remote) = repo.find_remote("origin") {
            if let Ok(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }

    // Fallback to first available remote
    for name in remote_names.iter().flatten().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Ok(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }

    Ok(None)
}

/// Report (and optionally fix) drift between the workspace config and the tree.
///
/// Runs a set of hygiene checks and prints a report. By default it is a dry run
/// and returns an error when any drift is found, so it is usable as a CI or
/// pre-commit lint. With `fix`, the auto-fixable checks are applied; any
/// remaining report-only drift still yields a non-zero exit.
///
/// Checks:
/// - **Missing .gitignore entry** (auto-fixable): a project backed by a real
///   remote (URL not `local:`) that is not listed in `.gitignore`. Such repos
///   are independently cloneable and should not be committed into the metarepo.
/// - **Missing directory** (report only): a project in the config whose
///   on-disk directory does not exist. Left to the user (clone or remove).
/// - **Untracked repository** (report only): a top-level git repository on disk
///   that is not tracked in the config. Suggests `meta project add`.
///
/// Stale `.gitignore` lines are intentionally not auto-removed: without
/// provenance we cannot tell a former project entry from a hand-added ignore.
/// If `name` is currently tracked as a `local:` project but its on-disk
/// directory is a git repository with a configured remote, return that remote
/// URL. This is the signal that a local project is ready to be promoted to an
/// independently cloneable repo (folded from the old `update-gitignore`).
fn detect_promotable_remote(base_path: &Path, name: &str) -> Option<String> {
    let project_path = base_path.join(name);
    if !project_path.join(".git").exists() {
        return None;
    }
    let repo = Repository::open(&project_path).ok()?;
    get_remote_url(&repo).ok().flatten()
}

pub fn check_workspace(base_path: &Path, fix: bool) -> Result<()> {
    let meta_file_path = locate_workspace_config(base_path)?;
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Current .gitignore lines, trimmed, for membership checks.
    let gitignore_path = base_path.join(".gitignore");
    let ignored: HashSet<String> = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        HashSet::new()
    };

    // Check A: remote-backed projects missing from .gitignore (auto-fixable).
    let mut missing_ignore: Vec<String> = Vec::new();
    // Check B: projects whose on-disk directory is missing (report only).
    let mut missing_dirs: Vec<String> = Vec::new();
    // Check D: local: projects whose repo now has a remote (auto-fixable) —
    // (name, detected_url). Folds in the old update-gitignore behavior.
    let mut promotable: Vec<(String, String)> = Vec::new();
    for name in config.projects.keys() {
        let url = config.get_project_url(name).unwrap_or_default();
        let is_local = url.is_empty() || url.starts_with("local:");
        let should_ignore = !is_local;
        if should_ignore && !ignored.contains(name) {
            missing_ignore.push(name.clone());
        }
        if !base_path.join(name).exists() {
            missing_dirs.push(name.clone());
        } else if is_local {
            if let Some(detected) = detect_promotable_remote(base_path, name) {
                promotable.push((name.clone(), detected));
            }
        }
    }

    // Check C: top-level git repos on disk not tracked in the config (report).
    // Compare against the first path segment of each project key so nested
    // project layouts (e.g. "services/api") don't flag their parent dir.
    let tracked_roots: HashSet<String> = config
        .projects
        .keys()
        .filter_map(|k| {
            Path::new(k)
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
        })
        .collect();
    let ignore_names: HashSet<&str> = config.ignore.iter().map(|s| s.as_str()).collect();
    let mut untracked: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || ignore_names.contains(name.as_str()) {
                continue;
            }
            if tracked_roots.contains(&name) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() && (path.join(".git").exists()) {
                untracked.push(name);
            }
        }
    }

    missing_ignore.sort();
    missing_dirs.sort();
    untracked.sort();
    promotable.sort();

    let total = missing_ignore.len() + missing_dirs.len() + untracked.len() + promotable.len();
    if total == 0 {
        println!("  {} Workspace is in sync.", "✓".green());
        return Ok(());
    }

    println!("{}", "Workspace check:".bold());

    // Missing .gitignore entries — fixable.
    let mut fixed = 0usize;
    for name in &missing_ignore {
        if fix {
            update_gitignore(base_path, name)?;
            println!("  {} Added {} to .gitignore", "✓".green(), name.cyan());
            fixed += 1;
        } else {
            println!(
                "  {} .gitignore missing entry: {}",
                "!".yellow(),
                name.cyan()
            );
        }
    }

    // Promotable local projects — fixable: rewrite the config entry from local:
    // to the detected remote and ignore the directory. Save the config once
    // after applying all promotions.
    let mut promoted_any = false;
    for (name, url) in &promotable {
        if fix {
            config
                .projects
                .insert(name.clone(), ProjectEntry::Url(url.clone()));
            update_gitignore(base_path, name)?;
            promoted_any = true;
            println!(
                "  {} Promoted {} to {} and added to .gitignore",
                "✓".green(),
                name.cyan(),
                url.green()
            );
            fixed += 1;
        } else {
            println!(
                "  {} local project has a remote now: {} -> {} (promote with --fix)",
                "!".yellow(),
                name.cyan(),
                url.green()
            );
        }
    }
    if promoted_any {
        config.save_to_file(&meta_file_path)?;
    }

    // Report-only checks.
    for name in &missing_dirs {
        println!(
            "  {} project directory missing on disk: {} (clone or remove from config)",
            "!".yellow(),
            name.cyan()
        );
    }
    for name in &untracked {
        println!(
            "  {} untracked git repository: {} (run 'meta project add {}')",
            "!".yellow(),
            name.cyan(),
            name
        );
    }

    let remaining = total - fixed;
    if remaining == 0 {
        println!(
            "\n  {} Fixed {} issue(s); workspace is now in sync.",
            "✓".green(),
            fixed
        );
        return Ok(());
    }

    if fix {
        println!(
            "\n  {} Fixed {} issue(s); {} remaining need manual attention.",
            "·".bright_black(),
            fixed,
            remaining
        );
    } else {
        println!(
            "\n  {} {} issue(s) found. Run with --fix to apply the fixable ones.",
            "·".bright_black(),
            total
        );
    }

    Err(anyhow::anyhow!(
        "workspace check found {} issue(s)",
        remaining
    ))
}

fn update_gitignore(base_path: &Path, project_path: &str) -> Result<()> {
    let gitignore_path = base_path.join(".gitignore");

    let mut content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Check if project path is already ignored
    if !content.lines().any(|line| line.trim() == project_path) {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(project_path);
        content.push('\n');

        std::fs::write(&gitignore_path, content)?;
        // Silent - shown in summary
    }

    Ok(())
}

pub fn list_projects(base_path: &Path, scope: &[String]) -> Result<()> {
    // Find and load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    // Restrict to the directory-aware scope resolved by the caller.
    config.projects.retain(|k, _| scope.iter().any(|s| s == k));

    if config.projects.is_empty() {
        println!(
            "\n  {} {}",
            "📦".bright_blue(),
            "No projects found in workspace".dimmed()
        );
        println!(
            "  {} {}",
            "".dimmed(),
            "Use 'meta project import' to add projects".dimmed()
        );
        println!();
        return Ok(());
    }

    println!("\n  {} {}", "📦".bright_blue(), "Workspace Projects".bold());
    println!("  {}", "═".repeat(60).bright_black());

    for name in config.projects.keys() {
        let project_path = base_path.join(name);
        let url = config
            .get_project_url(name)
            .unwrap_or_else(|| "unknown".to_string());

        // Check if it's a symlink
        let is_symlink = project_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        let (status_text, status_color) = if project_path.exists() {
            if is_symlink {
                ("External", "cyan")
            } else if project_path.join(".git").exists() {
                ("Active", "green")
            } else {
                ("No Git", "yellow")
            }
        } else {
            ("Missing", "red")
        };

        // Project name and status
        println!();
        print!("  {}", name.bold());

        match status_color {
            "green" => println!(" {}", format!("[{}]", status_text).green()),
            "cyan" => println!(" {}", format!("[{}]", status_text).cyan()),
            "yellow" => println!(" {}", format!("[{}]", status_text).yellow()),
            "red" => println!(" {}", format!("[{}]", status_text).red()),
            _ => println!(" [{}]", status_text),
        }

        // Project details with proper indentation and styling
        if url.starts_with("external:local:") {
            let path = url.strip_prefix("external:local:").unwrap();
            println!(
                "  {}  {} {}",
                "│".bright_black(),
                "Type:".bright_black(),
                "Local (no remote)".italic()
            );
            println!(
                "  {}  {} {}",
                "│".bright_black(),
                "Path:".bright_black(),
                path.bright_white()
            );
        } else if url.starts_with("external:") {
            let remote_url = url.strip_prefix("external:").unwrap();
            println!(
                "  {}  {} {}",
                "│".bright_black(),
                "Type:".bright_black(),
                "External".cyan().italic()
            );
            println!(
                "  {}  {} {}",
                "│".bright_black(),
                "Remote:".bright_black(),
                remote_url.bright_white()
            );
            if is_symlink {
                if let Ok(target) = std::fs::read_link(&project_path) {
                    println!(
                        "  {}  {} {}",
                        "└".bright_black(),
                        "Links to:".bright_black(),
                        target.display().to_string().bright_magenta()
                    );
                }
            }
        } else if url.starts_with("local:") {
            println!(
                "  {}  {} {}",
                "└".bright_black(),
                "Type:".bright_black(),
                "Local (no remote)".italic()
            );
        } else {
            println!(
                "  {}  {} {}",
                "└".bright_black(),
                "Remote:".bright_black(),
                url.bright_white()
            );
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    println!(
        "  {} {} projects total\n",
        config.projects.len().to_string().cyan().bold(),
        "workspace".dimmed()
    );

    Ok(())
}

/// List projects in minimal format (just names)
pub fn list_projects_minimal(base_path: &Path, scope: &[String]) -> Result<()> {
    // Find and load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    config.projects.retain(|k, _| scope.iter().any(|s| s == k));

    if config.projects.is_empty() {
        return Ok(());
    }

    // Sort and print project names only
    let mut sorted_projects: Vec<_> = config.projects.keys().collect();
    sorted_projects.sort();

    for name in sorted_projects {
        println!("{}", name);
    }

    Ok(())
}

/// Display projects in a tree structure
pub fn show_project_tree(base_path: &Path, scope: &[String]) -> Result<()> {
    // Load the root meta file
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    config.projects.retain(|k, _| scope.iter().any(|s| s == k));

    if config.projects.is_empty() {
        println!(
            "\n  {} {}",
            "📦".bright_blue(),
            "No projects found in workspace".dimmed()
        );
        println!(
            "  {} {}",
            "".dimmed(),
            "Use 'meta project import' to add projects".dimmed()
        );
        println!();
        return Ok(());
    }

    println!("\n  {} {}", "🌳".green(), "Project Tree".bold());
    println!("  {}", "═".repeat(60).bright_black());
    println!();

    // Display the root workspace with consistent formatting
    let root_name = base_path.file_name().unwrap_or_default().to_string_lossy();
    if meta_file_path.exists() {
        println!("  {}/", root_name.bold().white()); // Meta repo in bold white with slash
    } else {
        println!("  {}/", root_name.bright_blue()); // Directory in bright blue with slash
    }

    // Build a tree structure from the flat project list
    #[derive(Debug, Clone)]
    struct TreeNode {
        name: String,
        full_path: String,
        is_meta: bool,
        is_directory: bool, // True for intermediate directories
        children: Vec<TreeNode>,
    }

    let mut root_nodes: Vec<TreeNode> = Vec::new();

    // Helper function to insert a path into the tree
    fn insert_path_into_tree(
        nodes: &mut Vec<TreeNode>,
        path: &str,
        is_meta: bool,
        base_path: &Path,
    ) {
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() {
            return;
        }

        let first = parts[0];
        let rest = parts[1..].join("/");

        // Find or create the node for the first part
        let node = if let Some(existing) = nodes.iter_mut().find(|n| n.name == first) {
            existing
        } else {
            // Create new node
            let is_this_meta = if rest.is_empty() {
                // This is the final part, use the provided is_meta
                is_meta
            } else {
                // This is an intermediate, check if it's a meta repo itself
                dir_has_meta_config(&base_path.join(first))
            };

            let is_dir = !rest.is_empty() && !is_this_meta;

            nodes.push(TreeNode {
                name: first.to_string(),
                full_path: first.to_string(),
                is_meta: is_this_meta,
                is_directory: is_dir,
                children: Vec::new(),
            });
            nodes.last_mut().unwrap()
        };

        // If there are more parts, recurse
        if !rest.is_empty() {
            let child_full_path = format!("{}/{}", first, rest);
            insert_path_into_subtree(
                &mut node.children,
                &rest,
                is_meta,
                &child_full_path,
                base_path,
            );
        }

        // If this node is a meta repo, load its nested projects
        if node.is_meta && node.children.is_empty() {
            let project_path = base_path.join(&node.name);
            if let Some(nested_config) = load_dir_meta_config(&project_path) {
                for nested_name in nested_config.projects.keys() {
                    insert_path_into_subtree(
                        &mut node.children,
                        nested_name,
                        dir_has_meta_config(&project_path.join(nested_name)),
                        &format!("{}/{}", node.name, nested_name),
                        base_path,
                    );
                }
            }
        }
    }

    fn insert_path_into_subtree(
        nodes: &mut Vec<TreeNode>,
        path: &str,
        is_meta: bool,
        full_path: &str,
        base_path: &Path,
    ) {
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() {
            return;
        }

        let first = parts[0];
        let rest = parts[1..].join("/");

        // Find or create the node
        let node = if let Some(existing) = nodes.iter_mut().find(|n| n.name == first) {
            existing
        } else {
            let is_this_meta = if rest.is_empty() {
                is_meta
            } else {
                let intermediate = base_path
                    .join(
                        full_path.split('/').collect::<Vec<_>>()
                            [0..full_path.split('/').count() - rest.split('/').count()]
                            .join("/"),
                    )
                    .join(first);
                dir_has_meta_config(&intermediate)
            };

            let is_dir = !rest.is_empty() && !is_this_meta;

            nodes.push(TreeNode {
                name: first.to_string(),
                full_path: full_path.to_string(),
                is_meta: is_this_meta,
                is_directory: is_dir,
                children: Vec::new(),
            });
            nodes.last_mut().unwrap()
        };

        // If there are more parts, recurse
        if !rest.is_empty() {
            insert_path_into_subtree(&mut node.children, &rest, is_meta, full_path, base_path);
        }

        // If this node is a meta repo and we haven't loaded its children yet
        if node.is_meta && node.children.is_empty() {
            let project_path = base_path.join(full_path);
            if let Some(nested_config) = load_dir_meta_config(&project_path) {
                for nested_name in nested_config.projects.keys() {
                    let nested_full_path = format!("{}/{}", full_path, nested_name);
                    insert_path_into_subtree(
                        &mut node.children,
                        nested_name,
                        dir_has_meta_config(&base_path.join(&nested_full_path)),
                        &nested_full_path,
                        base_path,
                    );
                }
            }
        }
    }

    // Sort and process all projects
    let mut sorted_projects: Vec<_> = config.projects.iter().collect();
    sorted_projects.sort_by_key(|(name, _)| name.as_str());

    for (name, _url) in sorted_projects {
        let project_path = base_path.join(name);
        let is_meta = dir_has_meta_config(&project_path);
        insert_path_into_tree(&mut root_nodes, name, is_meta, base_path);
    }

    // Display the tree
    fn print_tree(nodes: &[TreeNode], prefix: &str, _is_root: bool, base_path: &Path) {
        // Sort nodes: directories first, then meta repos, then regular files
        let mut sorted_nodes = nodes.to_vec();
        sorted_nodes.sort_by(
            |a, b| match (a.is_directory, b.is_directory, a.is_meta, b.is_meta) {
                (true, false, _, _) => std::cmp::Ordering::Less,
                (false, true, _, _) => std::cmp::Ordering::Greater,
                (false, false, true, false) => std::cmp::Ordering::Less,
                (false, false, false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            },
        );

        for (i, node) in sorted_nodes.iter().enumerate() {
            let is_last = i == sorted_nodes.len() - 1;
            let connector = if is_last { "└──" } else { "├──" };

            // Check if this is a symlink
            let full_path = if node.full_path.is_empty() {
                base_path.join(&node.name)
            } else {
                base_path.join(&node.full_path)
            };
            let is_symlink = full_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);

            // Determine display based on node type
            let name_display = if is_symlink {
                format!("{}", node.name.bright_magenta()) // Symlinks in magenta
            } else if node.is_meta {
                format!("{}/", node.name.bold().white()) // Meta repos in bold white with trailing slash
            } else if node.is_directory {
                format!("{}/", node.name.bright_blue()) // Directories in bright blue with trailing slash
            } else {
                node.name.white().to_string() // Regular projects in white
            };

            // Print the node with consistent line formatting
            println!("{}{} {}", prefix.dimmed(), connector.dimmed(), name_display);

            if !node.children.is_empty() {
                // Prepare child prefix
                let child_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}│   ", prefix)
                };

                print_tree(&node.children, &child_prefix, false, base_path);
            }
        }
    }

    print_tree(&root_nodes, "  ", true, base_path);

    println!();
    println!("  {}", "─".repeat(60).bright_black());
    println!(
        "  {}  {}  {}  {}",
        format!("{}/", "Meta repository").bold().white(),
        format!("{}/", "Directory").bright_blue(),
        "Project".white(),
        "Symlink".bright_magenta()
    );
    println!();

    Ok(())
}

/// Update all projects (pull latest changes)
pub fn update_projects(base_path: &Path, recursive: bool, depth: Option<usize>) -> Result<()> {
    // Load the meta file
    let meta_file_path = locate_workspace_config(base_path)?;

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    if config.projects.is_empty() {
        println!(
            "\n  {} {}",
            "📦".bright_blue(),
            "No projects to update".dimmed()
        );
        return Ok(());
    }

    println!("\n  {} {}", "🔄".cyan(), "Updating projects...".bold());
    println!("  {}", "═".repeat(60).bright_black());

    let mut updated = 0;
    let mut failed = 0;

    for name in config.projects.keys() {
        let project_path = base_path.join(name);

        if !project_path.exists() {
            println!(
                "\n  {} {} {}",
                "⏭".yellow(),
                name.bright_white(),
                "(missing)".yellow()
            );
            continue;
        }

        if !project_path.join(".git").exists() {
            println!(
                "\n  {} {} {}",
                "⏭".yellow(),
                name.bright_white(),
                "(not a git repo)".yellow()
            );
            continue;
        }

        println!(
            "\n  {} {}",
            "📥".green(),
            format!("Updating '{}'", name).bold()
        );

        // Open the repository
        match Repository::open(&project_path) {
            Ok(repo) => {
                // Fetch and pull changes
                match pull_repository(&repo) {
                    Ok(_) => {
                        println!("     {} {}", "✅".green(), "Updated successfully".green());
                        updated += 1;

                        // If recursive and this is a meta repo, update nested projects
                        if recursive && dir_has_meta_config(&project_path) {
                            let current_depth = depth.unwrap_or(3);
                            if current_depth > 0 {
                                println!(
                                    "     {} {}",
                                    "🔍".cyan(),
                                    "Checking nested projects...".dimmed()
                                );
                                if let Err(e) = update_projects(
                                    &project_path,
                                    recursive,
                                    Some(current_depth - 1),
                                ) {
                                    eprintln!(
                                        "     {} {}",
                                        "⚠️".yellow(),
                                        format!("Failed to update nested: {}", e).yellow()
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "     {} {}",
                            "❌".red(),
                            format!("Failed to update: {}", e).red()
                        );
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "     {} {}",
                    "❌".red(),
                    format!("Failed to open repository: {}", e).red()
                );
                failed += 1;
            }
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    println!(
        "  {} {} projects updated, {} failed",
        "Summary:".bright_black(),
        updated.to_string().green(),
        if failed > 0 {
            failed.to_string().red()
        } else {
            failed.to_string().bright_black()
        }
    );
    println!();

    Ok(())
}

/// Pull latest changes from a repository
fn pull_repository(repo: &Repository) -> Result<()> {
    // Get the current branch
    let head = repo.head()?;
    let branch = head.shorthand().unwrap_or("main");

    // Set up fetch options with authentication
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            if let Ok(home) = std::env::var("HOME") {
                let ssh_dir = Path::new(&home).join(".ssh");
                let key_names = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];

                for key_name in &key_names {
                    let private_key = ssh_dir.join(key_name);
                    if private_key.exists() {
                        if let Ok(cred) = Cred::ssh_key(username, None, private_key.as_path(), None)
                        {
                            return Ok(cred);
                        }
                    }
                }
            }

            if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
        }

        Err(git2::Error::from_str("Authentication failed"))
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    // Fetch from origin
    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&[branch], Some(&mut fetch_options), None)?;

    // Fast-forward merge
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.0.is_up_to_date() {
        println!(
            "     {} {}",
            "ℹ".bright_black(),
            "Already up to date".dimmed()
        );
    } else if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", branch);
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        println!(
            "     {} {}",
            "⬆".green(),
            "Fast-forwarded to latest".green()
        );
    } else {
        return Err(anyhow::anyhow!(
            "Cannot fast-forward, manual merge required"
        ));
    }

    Ok(())
}

pub fn remove_project(project_name: &str, base_path: &Path, force: bool) -> Result<()> {
    // Find and load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Check if project exists in config
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!(
            "Project '{}' not found in workspace config",
            project_name
        ));
    }

    let project_path = base_path.join(project_name);
    let is_bare = config.is_bare_repo(project_name);

    // Check for uncommitted changes if directory exists
    if project_path.exists() && !force {
        if is_bare {
            // For bare repos, check all worktrees for uncommitted changes
            let bare_repo_path = project_path.join(".git");
            if bare_repo_path.exists() {
                // List all worktrees
                let output = Command::new("git")
                    .arg("-C")
                    .arg(&bare_repo_path)
                    .arg("worktree")
                    .arg("list")
                    .arg("--porcelain")
                    .output()
                    .context("Failed to list worktrees")?;

                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut worktree_paths = Vec::new();

                    for line in stdout.lines() {
                        if line.starts_with("worktree ") {
                            if let Some(path_str) = line.strip_prefix("worktree ") {
                                worktree_paths.push(PathBuf::from(path_str));
                            }
                        }
                    }

                    // Check each worktree for uncommitted changes
                    for worktree_path in worktree_paths {
                        if worktree_path.exists() {
                            if let Ok(repo) = Repository::open(&worktree_path) {
                                let mut status_opts = StatusOptions::new();
                                status_opts.include_untracked(true);
                                status_opts.include_ignored(false);

                                let statuses = repo.statuses(Some(&mut status_opts))?;

                                let has_changes = statuses.iter().any(|entry| {
                                    let status = entry.status();
                                    status.intersects(
                                        Status::INDEX_NEW
                                            | Status::INDEX_MODIFIED
                                            | Status::INDEX_DELETED
                                            | Status::INDEX_RENAMED
                                            | Status::INDEX_TYPECHANGE
                                            | Status::WT_NEW
                                            | Status::WT_MODIFIED
                                            | Status::WT_DELETED
                                            | Status::WT_TYPECHANGE
                                            | Status::WT_RENAMED,
                                    )
                                });

                                if has_changes {
                                    let worktree_name = worktree_path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown");
                                    eprintln!("\n{} Project '{}' has uncommitted changes in worktree '{}'!",
                                        "✗".yellow(),
                                        project_name.bold(),
                                        worktree_name.bold()
                                    );
                                    eprintln!(
                                        "  Use --force to remove anyway (changes will be lost)"
                                    );
                                    eprintln!("  Or commit/stash your changes first");
                                    eprintln!();
                                    return Err(anyhow::anyhow!("Uncommitted changes detected"));
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // For regular repos, use existing logic
            if project_path.join(".git").exists() {
                let repo = Repository::open(&project_path)?;

                // Check for uncommitted changes
                let mut status_opts = StatusOptions::new();
                status_opts.include_untracked(true);
                status_opts.include_ignored(false);

                let statuses = repo.statuses(Some(&mut status_opts))?;

                let has_changes = statuses.iter().any(|entry| {
                    let status = entry.status();
                    status.intersects(
                        Status::INDEX_NEW
                            | Status::INDEX_MODIFIED
                            | Status::INDEX_DELETED
                            | Status::INDEX_RENAMED
                            | Status::INDEX_TYPECHANGE
                            | Status::WT_NEW
                            | Status::WT_MODIFIED
                            | Status::WT_DELETED
                            | Status::WT_TYPECHANGE
                            | Status::WT_RENAMED,
                    )
                });

                if has_changes {
                    eprintln!(
                        "\n{} Project '{}' has uncommitted changes!",
                        "✗".yellow(),
                        project_name.bold()
                    );
                    eprintln!("  Use --force to remove anyway (changes will be lost)");
                    eprintln!("  Or commit/stash your changes first");
                    eprintln!();
                    return Err(anyhow::anyhow!("Uncommitted changes detected"));
                }
            }
        }
    }

    // Remove from .meta file
    config.projects.remove(project_name);
    config.save_to_file(&meta_file_path)?;

    // Remove from .gitignore
    remove_from_gitignore(base_path, project_name)?;

    println!(
        "\n  {} {}",
        "🗑".red(),
        format!("Removed project '{}'", project_name).bold()
    );
    println!(
        "     {} {}",
        "└".bright_black(),
        "Removed from workspace config".italic().bright_black()
    );

    // Optionally remove the directory
    if project_path.exists() {
        if force {
            std::fs::remove_dir_all(&project_path)?;
            println!(
                "     {} {}",
                "└".bright_black(),
                format!("Deleted directory '{}'", project_name)
                    .italic()
                    .bright_red()
            );
        } else {
            println!(
                "     {} {}",
                "└".bright_black(),
                format!("Directory '{}' kept on disk", project_name)
                    .italic()
                    .bright_black()
            );
            println!(
                "     {} {}",
                " ".bright_black(),
                format!("To remove: rm -rf {}", project_name).dimmed()
            );
        }
    }

    Ok(())
}

fn remove_from_gitignore(base_path: &Path, project_name: &str) -> Result<()> {
    let gitignore_path = base_path.join(".gitignore");

    if !gitignore_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&gitignore_path)?;
    let new_content: Vec<&str> = content
        .lines()
        .filter(|line| line.trim() != project_name)
        .collect();

    std::fs::write(&gitignore_path, new_content.join("\n") + "\n")?;
    // Silent - shown in summary

    Ok(())
}

/// Update gitignore for a project that now has a remote
pub fn update_project_gitignore(project_name: &str, base_path: &Path) -> Result<()> {
    // Load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Check if project exists in config
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!(
            "Project '{}' not found in workspace config",
            project_name
        ));
    }

    let project_path = base_path.join(project_name);
    let current_url = config
        .get_project_url(project_name)
        .unwrap_or_else(|| "".to_string());

    // Check if project is currently marked as local
    if !current_url.starts_with("local:") {
        println!(
            "\n  {} {}",
            "ℹ".bright_black(),
            format!("Project '{}' already has a remote URL", project_name).dimmed()
        );
        return Ok(());
    }

    // Check if directory exists and has git
    if !project_path.exists() || !project_path.join(".git").exists() {
        return Err(anyhow::anyhow!(
            "Project '{}' directory doesn't exist or is not a git repository",
            project_name
        ));
    }

    // Check for remote URL
    let repo = Repository::open(&project_path)?;
    let remote_url = get_remote_url(&repo)?;

    if let Some(detected_url) = remote_url {
        // Update the URL in config
        config.projects.insert(
            project_name.to_string(),
            ProjectEntry::Url(detected_url.clone()),
        );
        config.save_to_file(&meta_file_path)?;

        // Add to gitignore
        update_gitignore(base_path, project_name)?;

        println!(
            "\n  {} {}",
            "✅".green(),
            format!("Updated project '{}'", project_name).bold().green()
        );
        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
        println!(
            "     {} {}",
            "└".bright_black(),
            "Added to .gitignore".italic().bright_black()
        );
        println!();
    } else {
        println!(
            "\n  {} {}",
            "⚠️".yellow(),
            format!("Project '{}' still has no remote", project_name)
                .bold()
                .yellow()
        );
        println!(
            "     {} {}",
            "└".bright_black(),
            "Add a remote with: git remote add origin <url>".dimmed()
        );
        println!();
    }

    Ok(())
}

/// Rename a project in the workspace
pub fn rename_project(old_name: &str, new_name: &str, base_path: &Path) -> Result<()> {
    // Load the workspace config
    let meta_file_path = locate_workspace_config(base_path)?;

    let mut config = MetaConfig::load_from_file(&meta_file_path)?;

    // Check if old project exists
    if !config.projects.contains_key(old_name) {
        return Err(anyhow::anyhow!(
            "Project '{}' not found in workspace config",
            old_name
        ));
    }

    // Check if new name is already taken
    if config.projects.contains_key(new_name) {
        return Err(anyhow::anyhow!(
            "Project '{}' already exists in workspace config",
            new_name
        ));
    }

    let old_path = base_path.join(old_name);
    let new_path = base_path.join(new_name);

    // Check if new path already exists on disk
    if new_path.exists() {
        return Err(anyhow::anyhow!("Directory '{}' already exists", new_name));
    }

    // Check if old path exists
    let is_symlink = old_path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    // Get the project entry before removing it
    let project_entry = config.projects.get(old_name).unwrap().clone();
    let project_url = config
        .get_project_url(old_name)
        .unwrap_or_else(|| "".to_string());

    // Check for uncommitted changes if it's a git repository (not for symlinks)
    if !is_symlink && old_path.exists() && old_path.join(".git").exists() {
        let repo = Repository::open(&old_path)?;

        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        status_opts.include_ignored(false);

        let statuses = repo.statuses(Some(&mut status_opts))?;

        let has_changes = statuses.iter().any(|entry| {
            let status = entry.status();
            status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE
                    | Status::WT_NEW
                    | Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_TYPECHANGE
                    | Status::WT_RENAMED,
            )
        });

        if has_changes {
            return Err(anyhow::anyhow!(
                "Project '{}' has uncommitted changes. Please commit or stash them first.",
                old_name
            ));
        }
    }

    println!(
        "\n  {} {}",
        "🔄".cyan(),
        format!("Renaming project '{}' to '{}'", old_name, new_name).bold()
    );

    // Update the .meta file first
    config.projects.remove(old_name);
    config.projects.insert(new_name.to_string(), project_entry);
    config.save_to_file(&meta_file_path)?;
    println!(
        "     {} {}",
        "✅".green(),
        "Updated workspace config".green()
    );

    // Rename the directory if it exists
    if old_path.exists() {
        std::fs::rename(&old_path, &new_path)?;
        if is_symlink {
            println!("     {} {}", "✅".green(), "Renamed symlink".green());
        } else {
            println!("     {} {}", "✅".green(), "Renamed directory".green());
        }
    }

    // Update .gitignore if the project has a remote URL (not local:)
    if !project_url.starts_with("local:") {
        // Remove old entry from .gitignore
        remove_from_gitignore(base_path, old_name)?;
        // Add new entry to .gitignore
        update_gitignore(base_path, new_name)?;
        println!("     {} {}", "✅".green(), "Updated .gitignore".green());
    }

    println!(
        "\n  {} {}",
        "✅".green(),
        format!("Successfully renamed '{}' to '{}'", old_name, new_name)
            .bold()
            .green()
    );
    println!();

    Ok(())
}

#[cfg(test)]
mod project_ops_tests {
    use super::*;
    use tempfile::tempdir;

    fn write_parent(root: &Path) {
        std::fs::write(root.join(".metarepo"), r#"{"projects":{}}"#).unwrap();
    }

    #[test]
    fn init_child_creates_config_and_registers_in_parent() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_parent(root);

        init_child_workspace("services/api", root).unwrap();

        // Child got its own config.
        assert!(dir_has_meta_config(&root.join("services/api")));

        // Parent now tracks the child as a local project.
        let parent = MetaConfig::load_from_file(root.join(".metarepo")).unwrap();
        assert_eq!(
            parent.projects.get("services/api"),
            Some(&ProjectEntry::Url("local:services/api".to_string()))
        );
    }

    #[test]
    fn init_child_rejects_parent_escape() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_parent(root);

        let err = init_child_workspace("../escape", root).unwrap_err();
        assert!(err
            .to_string()
            .contains("relative path inside the workspace"));
    }

    #[test]
    fn init_child_refuses_existing_child_config() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_parent(root);

        init_child_workspace("child", root).unwrap();
        // Second attempt must not clobber the existing child config.
        let err = init_child_workspace("child", root).unwrap_err();
        assert!(err
            .to_string()
            .contains("already contains a metarepo config"));
    }

    #[test]
    fn check_workspace_clean_is_ok() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        // A local: project needs no gitignore entry and its dir exists.
        std::fs::write(
            root.join(".metarepo"),
            r#"{"projects":{"docs":"local:docs"}}"#,
        )
        .unwrap();

        assert!(check_workspace(root, false).is_ok());
    }

    #[test]
    fn check_workspace_fix_adds_missing_gitignore_entry() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("web")).unwrap();
        // A remote-backed project present on disk but absent from .gitignore.
        std::fs::write(
            root.join(".metarepo"),
            r#"{"projects":{"web":"https://example.com/web.git"}}"#,
        )
        .unwrap();

        // Dry run reports drift (non-zero -> Err) and leaves .gitignore untouched.
        assert!(check_workspace(root, false).is_err());
        assert!(!root.join(".gitignore").exists());

        // --fix adds the entry and clears the only drift, so it now succeeds.
        check_workspace(root, true).unwrap();
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == "web"));
        assert!(check_workspace(root, false).is_ok());
    }

    #[test]
    fn check_workspace_reports_missing_directory() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // Local project whose directory does not exist -> report-only drift.
        std::fs::write(
            root.join(".metarepo"),
            r#"{"projects":{"gone":"local:gone"}}"#,
        )
        .unwrap();

        let err = check_workspace(root, true).unwrap_err();
        // Even with --fix, an unfixable missing dir keeps the exit non-zero.
        assert!(err.to_string().contains("workspace check found"));
    }

    #[test]
    fn check_workspace_fix_promotes_local_project_with_remote() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // A local: project whose on-disk repo has gained a remote.
        let repo = git2::Repository::init(root.join("web")).unwrap();
        repo.remote("origin", "https://example.com/web.git")
            .unwrap();
        std::fs::write(
            root.join(".metarepo"),
            r#"{"projects":{"web":"local:web"}}"#,
        )
        .unwrap();

        // Dry run reports the promotable project as drift.
        assert!(check_workspace(root, false).is_err());

        // --fix rewrites the config entry to the detected remote and ignores it.
        check_workspace(root, true).unwrap();
        let config = MetaConfig::load_from_file(root.join(".metarepo")).unwrap();
        assert_eq!(
            config.get_project_url("web").as_deref(),
            Some("https://example.com/web.git")
        );
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.lines().any(|l| l.trim() == "web"));
        assert!(check_workspace(root, false).is_ok());
    }
}
