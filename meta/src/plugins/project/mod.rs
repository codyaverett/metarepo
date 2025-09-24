use anyhow::Result;
use colored::*;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository, Status, StatusOptions};
use metarepo_core::{MetaConfig, NestedConfig, ProjectEntry};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::io::{self, Write};

#[cfg(unix)]
use std::os::unix::fs;

#[cfg(windows)]
use std::os::windows::fs;

// Export the main plugin
pub use self::plugin::ProjectPlugin;

mod plugin;

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
                cycle_path.iter().enumerate()
                    .map(|(i, p)| format!("  {}. {}{}", 
                        i + 1, 
                        p,
                        if i == cycle_path.len() - 1 { " (CYCLE!)" } else { "" }
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
    import_project_with_options(project_path, source, base_path, false)
}

pub fn import_project_with_options(project_path: &str, source: Option<&str>, base_path: &Path, init_git: bool) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project already exists in config
    if config.projects.contains_key(project_path) {
        return Err(anyhow::anyhow!("Project '{}' already exists in .meta file", project_path));
    }
    
    let local_project_path = base_path.join(project_path);
    
    // Determine what the source is and how to handle it
    let (final_repo_url, is_external) = if let Some(src) = source {
        if !src.starts_with("http") && !src.starts_with("git@") && !src.starts_with("ssh://") {
            // This is a local path (relative or absolute)
            let external_path = if src.starts_with('/') {
                PathBuf::from(src)
            } else {
                // Resolve relative path from current working directory or base path
                let resolved = base_path.join(src).canonicalize()
                    .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(src).canonicalize()).unwrap_or(Ok(PathBuf::from(src))))
                    .unwrap_or_else(|_| PathBuf::from(src));
                resolved
            };
            
            // Check if this path is outside the workspace (external)
            let is_external_dir = !external_path.starts_with(base_path) || external_path == base_path.join(project_path);
            
            if external_path.exists() && external_path.join(".git").exists() {
                if is_external_dir {
                    // External directory exists and is a git repo - create symlink
                    let repo = Repository::open(&external_path)?;
                    let remote_url = get_remote_url(&repo)?;
                    
                    // Create symlink to external directory
                    if local_project_path.exists() {
                        return Err(anyhow::anyhow!("Directory '{}' already exists", project_path));
                    }
                    
                    println!("\n  {} {}", "🔗".cyan(), "Creating symlink...".bold());
                    println!("     {} {}", "From:".bright_black(), project_path.bright_white());
                    println!("     {} {}", "To:".bright_black(), external_path.display().to_string().bright_magenta());
                    create_symlink(&external_path, &local_project_path)?;
                    
                    let url = if let Some(detected_url) = remote_url {
                        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                        format!("external:{}", detected_url)
                    } else {
                        println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
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
                        println!("\n  {} {}", "📍".yellow(), "Using existing directory".bold());
                        println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                        format!("local:{}", project_path)
                    };
                    
                    (url, false)
                }
            } else if external_path.exists() {
                return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository", external_path.display()));
            } else {
                // Path doesn't exist - treat as URL for cloning
                (src.to_string(), false)
            }
        } else {
            // Regular git URL
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
                    println!("\n  {} {}", "📍".green(), "Using existing git repository".bold());
                    println!("     {} {}", "Remote:".bright_black(), detected_url.green());
                    detected_url
                } else {
                    println!("\n  {} {}", "📍".yellow(), "Using existing git repository".bold());
                    println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                    format!("local:{}", project_path)
                };
                
                (url, false)
            } else {
                // Directory exists but is not a git repository
                let should_init = if init_git {
                    true
                } else {
                    // Try to prompt user to initialize git repo
                    println!("\n  {} {}", "❓".yellow(), format!("Directory '{}' exists but is not a git repository", project_path).bold());
                    print!("     {} Initialize as git repository? [y/N]: ", "→".bright_black());
                    
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
                                    println!("     {} {}", "⚠️".yellow(), "Unable to read input from terminal".yellow());
                                    println!("     {} {}", "└".bright_black(), "Use --init-git flag to automatically initialize git".dimmed());
                                    return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                                }
                            }
                        }
                        Err(_) => {
                            println!();
                            println!("     {} {}", "⚠️".yellow(), "Terminal interaction not available".yellow());
                            return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                        }
                    }
                };
                
                if should_init {
                    println!("\n  {} {}", "🌱".green(), "Initializing git repository...".bold());
                    Repository::init(&local_project_path)?;
                    println!("     {} {}", "✅".green(), "Git repository initialized".green());
                    println!("     {} {}", "Type:".bright_black(), "Local project (no remote)".yellow());
                    (format!("local:{}", project_path), false)
                } else {
                    return Err(anyhow::anyhow!("Directory '{}' exists but is not a git repository.\n\nOptions:\n  1. Use --init-git flag: meta project add {} --init-git\n  2. Initialize manually: cd {} && git init", project_path, project_path, project_path));
                }
            }
        } else {
            return Err(anyhow::anyhow!("Directory '{}' doesn't exist and no repository URL provided", project_path));
        }
    };
    
    // If not external and directory doesn't exist, clone it
    if !is_external && !local_project_path.exists() {
        if !final_repo_url.starts_with("local:") && !final_repo_url.starts_with("external:") {
            println!("\n  {} {}", "🌱".green(), "Adding new project...".bold());
            println!("     {} {}", "Name:".bright_black(), project_path.bright_white());
            println!("     {} {}", "Source:".bright_black(), final_repo_url.bright_cyan());
            println!("     {} {}", "Status:".bright_black(), "Cloning repository...".yellow());
            clone_with_auth(&final_repo_url, &local_project_path)?;
        } else {
            return Err(anyhow::anyhow!("Cannot clone a local project URL"));
        }
    }
    
    // Add to .meta file
    config.projects.insert(project_path.to_string(), ProjectEntry::Url(final_repo_url.clone()));
    config.save_to_file(&meta_file_path)?;
    
    // Update .gitignore only if project has a remote URL (not local:)
    if !final_repo_url.starts_with("local:") {
        update_gitignore(base_path, project_path)?;
    }
    
    // Success message
    println!("\n  {} {}", "✅".green(), format!("Successfully added '{}'", project_path).bold().green());
    
    if is_external {
        println!("     {} {}", "└".bright_black(), "Created symlink to external directory".italic().bright_black());
    }
    
    if final_repo_url.starts_with("local:") {
        println!("     {} {}", "└".bright_black(), "Updated .meta file (not added to .gitignore)".italic().bright_black());
        println!("     {} {}", "ℹ".bright_black(), format!("Run 'meta project update-gitignore {}' after adding a remote", project_path).dimmed());
    } else {
        println!("     {} {}", "└".bright_black(), "Updated .meta file and .gitignore".italic().bright_black());
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
    import_project_recursive_with_options(project_path, source, base_path, recursive, max_depth, flatten, false)
}

pub fn import_project_recursive_with_options(
    project_path: &str, 
    source: Option<&str>, 
    base_path: &Path,
    recursive: bool,
    max_depth: Option<usize>,
    flatten: bool,
    init_git: bool,
) -> Result<()> {
    // Load the root meta config
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
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
    
    // Import the root project
    import_project_with_options(project_path, source, base_path, init_git)?;
    
    // If recursive import is enabled, process nested repositories
    if nested_config.recursive_import {
        let project_path_buf = base_path.join(project_path);
        if let Err(e) = process_nested_repositories(&project_path_buf, &mut context, &nested_config) {
            eprintln!("\n  {} {}", "⚠️".yellow(), "Warning: Failed to process nested repositories".yellow());
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
    // Check if this project has a .meta file (is a meta repository)
    let nested_meta_path = project_path.join(".meta");
    if !nested_meta_path.exists() {
        return Ok(()); // Not a meta repository, nothing to do
    }
    
    println!("\n  {} {}", "🔍".cyan(), format!("Found nested meta repository in '{}'", project_path.file_name().unwrap_or_default().to_string_lossy()).bold());
    
    // Load the nested meta configuration
    let nested_meta = MetaConfig::load_from_file(&nested_meta_path)?;
    
    // Check depth before processing
    if context.at_max_depth() {
        println!("     {} {}", "⚠️".yellow(), format!("Skipping nested imports (max depth {} reached)", context.max_depth).yellow());
        return Ok(());
    }
    
    // Process each project in the nested meta file
    let mut import_queue = VecDeque::new();
    for (name, _entry) in &nested_meta.projects {
        if context.should_ignore(name) {
            println!("     {} {}", "⏭".bright_black(), format!("Skipping ignored project '{}'", name).dimmed());
            continue;
        }
        let url = nested_meta.get_project_url(name).unwrap_or_else(|| format!("local:{}", name));
        import_queue.push_back((name.clone(), url));
    }
    
    println!("     {} {}", "📦".blue(), format!("Found {} nested projects to import", import_queue.len()).bright_white());
    
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
        println!("\n  {} {}", "📥".green(), format!("Importing nested project '{}'", name).bold());
        println!("     {} {}", "URL:".bright_black(), url.bright_cyan());
        println!("     {} {}", "Path:".bright_black(), import_path.display().to_string().bright_white());
        
        // Perform the actual import
        // For nested imports, we need to handle the base path differently
        let (import_name, import_base) = if context.should_flatten() {
            // For flattened imports, use root base path and full name
            (name.clone(), context.base_path.clone())
        } else {
            // For hierarchical imports, import into the parent project
            if let Some(parent) = import_path.parent() {
                if let Some(file_name) = import_path.file_name() {
                    (file_name.to_string_lossy().to_string(), parent.to_path_buf())
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
            println!("     {} {}", "⏭".yellow(), format!("Directory '{}' already exists, skipping", import_name).yellow());
            context.exit_import();
            continue;
        }
        
        // Clone the repository directly without going through import_project
        // to avoid .meta file conflicts
        // Handle special URL formats (external:, local:)
        let actual_url = if url.starts_with("external:local:") {
            // This is a local external project, skip it
            println!("     {} {}", "⏭".yellow(), format!("Skipping local external project '{}'", name).yellow());
            context.exit_import();
            continue;
        } else if url.starts_with("external:") {
            // Strip the "external:" prefix to get the actual URL
            url.strip_prefix("external:").unwrap_or(&url).to_string()
        } else if url.starts_with("local:") {
            // Local projects don't need cloning
            println!("     {} {}", "⏭".yellow(), format!("Skipping local project '{}'", name).yellow());
            context.exit_import();
            continue;
        } else {
            url.clone()
        };
        
        println!("     {} {}", "📦".blue(), format!("Cloning into '{}'", target_path.display()).bright_white());
        if let Err(e) = clone_with_auth(&actual_url, &target_path) {
            eprintln!("     {} {}", "❌".red(), format!("Failed to clone '{}': {}", name, e).red());
            context.exit_import();
            continue;
        }
        
        // Recursively process this nested repository if it's also a meta repo
        if nested_config.recursive_import && !context.at_max_depth() {
            if let Err(e) = process_nested_repositories(&import_path, context, nested_config) {
                eprintln!("     {} {}", "⚠️".yellow(), format!("Warning: Failed to process nested repos in '{}': {}", name, e).yellow());
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
        Err(anyhow::anyhow!("Symbolic links are not supported on this platform"))
    }
}

fn clone_with_auth(url: &str, path: &Path) -> Result<Repository> {
    // Check if this is an SSH URL
    if url.starts_with("git@") || url.starts_with("ssh://") {
        // Set up authentication callbacks for SSH
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            // Get the username (default to "git" for GitHub/GitLab/etc)
            let username = username_from_url.unwrap_or("git");
            
            // If SSH agent is requested, try it first
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                // Try to find SSH keys in standard locations
                if let Ok(home) = std::env::var("HOME") {
                    let ssh_dir = Path::new(&home).join(".ssh");
                    
                    // Try common SSH key names in order of preference
                    let key_names = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];
                    
                    for key_name in &key_names {
                        let private_key = ssh_dir.join(key_name);
                        if private_key.exists() {
                            // Check if there's a public key as well
                            let public_key = ssh_dir.join(format!("{}.pub", key_name));
                            let public_key_path = if public_key.exists() {
                                Some(public_key.as_path())
                            } else {
                                None
                            };
                            
                            if let Ok(cred) = Cred::ssh_key(
                                username,
                                public_key_path,
                                private_key.as_path(),
                                None, // No passphrase for now
                            ) {
                                return Ok(cred);
                            }
                        }
                    }
                }
                
                // Try SSH agent as fallback
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }
            }
            
            // If we couldn't authenticate, return an error
            Err(git2::Error::from_str("SSH authentication failed. Please ensure your SSH keys are set up correctly."))
        });
        
        // Configure fetch options with our callbacks
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);
        
        // Build the repository with authentication
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        
        // Clone the repository
        builder.clone(url, path).map_err(|e| {
            if e.to_string().contains("authentication") || e.to_string().contains("SSH") {
                anyhow::anyhow!("SSH authentication failed. Please ensure:\n  1. Your SSH key is set up correctly (~/.ssh/id_ed25519 or ~/.ssh/id_rsa)\n  2. The key is added to your GitHub/GitLab account\n  3. You have access to the repository\n\nOriginal error: {}", e)
            } else {
                anyhow::anyhow!("Failed to clone repository: {}", e)
            }
        })
    } else {
        // For HTTPS URLs, use standard clone without authentication callbacks
        Repository::clone(url, path).map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))
    }
}

fn get_remote_url(repo: &Repository) -> Result<Option<String>> {
    // Try to get the 'origin' remote first, then fallback to first available remote
    let remote_names = repo.remotes()?;
    
    // First try 'origin'
    if remote_names.iter().any(|n| n == Some("origin")) {
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }
    
    // Fallback to first available remote
    for name in remote_names.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                return Ok(Some(url.to_string()));
            }
        }
    }
    
    Ok(None)
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

pub fn list_projects(base_path: &Path) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    if config.projects.is_empty() {
        println!("\n  {} {}", "📦".bright_blue(), "No projects found in workspace".dimmed());
        println!("  {} {}", "".dimmed(), "Use 'meta project import' to add projects".dimmed());
        println!();
        return Ok(());
    }
    
    println!("\n  {} {}", "📦".bright_blue(), "Workspace Projects".bold());
    println!("  {}", "═".repeat(60).bright_black());
    
    for (name, entry) in &config.projects {
        let project_path = base_path.join(name);
        let url = config.get_project_url(name).unwrap_or_else(|| "unknown".to_string());
        
        // Check if it's a symlink
        let is_symlink = project_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        
        let (status_icon, status_text, status_color) = if project_path.exists() {
            if is_symlink {
                ("🔗", "External", "cyan")
            } else if project_path.join(".git").exists() {
                ("✅", "Active", "green")
            } else {
                ("⚠️ ", "No Git", "yellow")
            }
        } else {
            ("❌", "Missing", "red")
        };
        
        // Project name and status
        println!();
        print!("  {} {}", status_icon, name.bold());
        
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
            println!("  {}  {} {}", "│".bright_black(), "Type:".bright_black(), "Local (no remote)".italic());
            println!("  {}  {} {}", "│".bright_black(), "Path:".bright_black(), path.bright_white());
        } else if url.starts_with("external:") {
            let remote_url = url.strip_prefix("external:").unwrap();
            println!("  {}  {} {}", "│".bright_black(), "Type:".bright_black(), "External".cyan().italic());
            println!("  {}  {} {}", "│".bright_black(), "Remote:".bright_black(), remote_url.bright_white());
            if is_symlink {
                if let Ok(target) = std::fs::read_link(&project_path) {
                    println!("  {}  {} {}", "└".bright_black(), "Links to:".bright_black(), target.display().to_string().bright_magenta());
                }
            }
        } else if url.starts_with("local:") {
            println!("  {}  {} {}", "└".bright_black(), "Type:".bright_black(), "Local (no remote)".italic());
        } else {
            println!("  {}  {} {}", "└".bright_black(), "Remote:".bright_black(), url.bright_white());
        }
    }
    
    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {} projects total\n", config.projects.len().to_string().cyan().bold(), "workspace".dimmed());
    
    Ok(())
}

/// Display projects in a tree structure
pub fn show_project_tree(base_path: &Path) -> Result<()> {
    // Load the root meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    if config.projects.is_empty() {
        println!("\n  {} {}", "📦".bright_blue(), "No projects found in workspace".dimmed());
        println!("  {} {}", "".dimmed(), "Use 'meta project import' to add projects".dimmed());
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
        is_directory: bool,  // True for intermediate directories
        children: Vec<TreeNode>,
    }
    
    let mut root_nodes: Vec<TreeNode> = Vec::new();
    
    // Helper function to insert a path into the tree
    fn insert_path_into_tree(nodes: &mut Vec<TreeNode>, path: &str, is_meta: bool, base_path: &Path) {
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
                base_path.join(first).join(".meta").exists()
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
            insert_path_into_subtree(&mut node.children, &rest, is_meta, &child_full_path, base_path);
        }
        
        // If this node is a meta repo, load its nested projects
        if node.is_meta && node.children.is_empty() {
            let project_path = base_path.join(&node.name);
            if let Ok(nested_config) = MetaConfig::load_from_file(project_path.join(".meta")) {
                for (nested_name, _) in nested_config.projects.iter() {
                    insert_path_into_subtree(&mut node.children, nested_name, 
                        project_path.join(nested_name).join(".meta").exists(),
                        &format!("{}/{}", node.name, nested_name),
                        base_path);
                }
            }
        }
    }
    
    fn insert_path_into_subtree(nodes: &mut Vec<TreeNode>, path: &str, is_meta: bool, full_path: &str, base_path: &Path) {
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
                base_path.join(full_path.split('/').collect::<Vec<_>>()[0..full_path.split('/').count() - rest.split('/').count()].join("/"))
                    .join(first).join(".meta").exists()
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
            if let Ok(nested_config) = MetaConfig::load_from_file(project_path.join(".meta")) {
                for (nested_name, _) in nested_config.projects.iter() {
                    let nested_full_path = format!("{}/{}", full_path, nested_name);
                    insert_path_into_subtree(&mut node.children, nested_name,
                        base_path.join(&nested_full_path).join(".meta").exists(),
                        &nested_full_path,
                        base_path);
                }
            }
        }
    }
    
    // Sort and process all projects
    let mut sorted_projects: Vec<_> = config.projects.iter().collect();
    sorted_projects.sort_by_key(|(name, _)| name.as_str());
    
    for (name, _url) in sorted_projects {
        let project_path = base_path.join(name);
        let is_meta = project_path.join(".meta").exists();
        insert_path_into_tree(&mut root_nodes, name, is_meta, base_path);
    }
    
    // Display the tree
    fn print_tree(nodes: &[TreeNode], prefix: &str, _is_root: bool, base_path: &Path) {
        // Sort nodes: directories first, then meta repos, then regular files
        let mut sorted_nodes = nodes.to_vec();
        sorted_nodes.sort_by(|a, b| {
            match (a.is_directory, b.is_directory, a.is_meta, b.is_meta) {
                (true, false, _, _) => std::cmp::Ordering::Less,
                (false, true, _, _) => std::cmp::Ordering::Greater,
                (false, false, true, false) => std::cmp::Ordering::Less,
                (false, false, false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        for (i, node) in sorted_nodes.iter().enumerate() {
            let is_last = i == sorted_nodes.len() - 1;
            let connector = if is_last { "└──" } else { "├──" };
            
            // Check if this is a symlink
            let full_path = if node.full_path.is_empty() {
                base_path.join(&node.name)
            } else {
                base_path.join(&node.full_path)
            };
            let is_symlink = full_path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
            
            // Determine display based on node type
            let name_display = if is_symlink {
                format!("🔗 {}", node.name.bright_magenta())  // Symlinks keep icon inline
            } else if node.is_meta {
                format!("{}/", node.name.bold().white())       // Meta repos in bold white with trailing slash
            } else if node.is_directory {
                format!("{}/", node.name.bright_blue())        // Directories in bright blue with trailing slash
            } else {
                node.name.white().to_string()                  // Regular projects in white
            };
            
            // Print the node with consistent line formatting
            println!("{}{} {}", 
                prefix.dimmed(), 
                connector.dimmed(), 
                name_display
            );
            
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
    println!("  {} {}  {} {}  {} {}  {} {}", 
        "📦", format!("{}/", "Meta repository").bold().white(),
        "📁", format!("{}/", "Directory").bright_blue(),
        "📄", "Project".white(),
        "🔗", "Symlink".bright_magenta()
    );
    println!();
    
    Ok(())
}

/// Update all projects (pull latest changes)
pub fn update_projects(base_path: &Path, recursive: bool, depth: Option<usize>) -> Result<()> {
    // Load the meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    if config.projects.is_empty() {
        println!("\n  {} {}", "📦".bright_blue(), "No projects to update".dimmed());
        return Ok(());
    }
    
    println!("\n  {} {}", "🔄".cyan(), "Updating projects...".bold());
    println!("  {}", "═".repeat(60).bright_black());
    
    let mut updated = 0;
    let mut failed = 0;
    
    for (name, _entry) in &config.projects {
        let project_path = base_path.join(name);
        
        if !project_path.exists() {
            println!("\n  {} {} {}", "⏭".yellow(), name.bright_white(), "(missing)".yellow());
            continue;
        }
        
        if !project_path.join(".git").exists() {
            println!("\n  {} {} {}", "⏭".yellow(), name.bright_white(), "(not a git repo)".yellow());
            continue;
        }
        
        println!("\n  {} {}", "📥".green(), format!("Updating '{}'", name).bold());
        
        // Open the repository
        match Repository::open(&project_path) {
            Ok(repo) => {
                // Fetch and pull changes
                match pull_repository(&repo) {
                    Ok(_) => {
                        println!("     {} {}", "✅".green(), "Updated successfully".green());
                        updated += 1;
                        
                        // If recursive and this is a meta repo, update nested projects
                        if recursive && project_path.join(".meta").exists() {
                            let current_depth = depth.unwrap_or(3);
                            if current_depth > 0 {
                                println!("     {} {}", "🔍".cyan(), "Checking nested projects...".dimmed());
                                if let Err(e) = update_projects(&project_path, recursive, Some(current_depth - 1)) {
                                    eprintln!("     {} {}", "⚠️".yellow(), format!("Failed to update nested: {}", e).yellow());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("     {} {}", "❌".red(), format!("Failed to update: {}", e).red());
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("     {} {}", "❌".red(), format!("Failed to open repository: {}", e).red());
                failed += 1;
            }
        }
    }
    
    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {} projects updated, {} failed", 
        "Summary:".bright_black(), 
        updated.to_string().green(),
        if failed > 0 { failed.to_string().red() } else { failed.to_string().bright_black() }
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
                        if let Ok(cred) = Cred::ssh_key(
                            username,
                            None,
                            private_key.as_path(),
                            None,
                        ) {
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
        println!("     {} {}", "ℹ".bright_black(), "Already up to date".dimmed());
    } else if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", branch);
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        println!("     {} {}", "⬆".green(), "Fast-forwarded to latest".green());
    } else {
        return Err(anyhow::anyhow!("Cannot fast-forward, manual merge required"));
    }
    
    Ok(())
}

pub fn remove_project(project_name: &str, base_path: &Path, force: bool) -> Result<()> {
    // Find and load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project exists in config
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!("Project '{}' not found in .meta file", project_name));
    }
    
    let project_path = base_path.join(project_name);
    
    // Check for uncommitted changes if directory exists
    if project_path.exists() && project_path.join(".git").exists() && !force {
        let repo = Repository::open(&project_path)?;
        
        // Check for uncommitted changes
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        status_opts.include_ignored(false);
        
        let statuses = repo.statuses(Some(&mut status_opts))?;
        
        let has_changes = statuses.iter().any(|entry| {
            let status = entry.status();
            status.intersects(
                Status::INDEX_NEW | Status::INDEX_MODIFIED | Status::INDEX_DELETED |
                Status::INDEX_RENAMED | Status::INDEX_TYPECHANGE |
                Status::WT_NEW | Status::WT_MODIFIED | Status::WT_DELETED |
                Status::WT_TYPECHANGE | Status::WT_RENAMED
            )
        });
        
        if has_changes {
            eprintln!("\n  {} {}", "⚠️".yellow(), format!("Project '{}' has uncommitted changes!", project_name).bold().yellow());
            eprintln!("     {} {}", "│".bright_black(), "Use --force to remove anyway (changes will be lost)".bright_red());
            eprintln!("     {} {}", "└".bright_black(), "Or commit/stash your changes first".bright_white());
            eprintln!();
            return Err(anyhow::anyhow!("Uncommitted changes detected"));
        }
    }
    
    // Remove from .meta file
    config.projects.remove(project_name);
    config.save_to_file(&meta_file_path)?;
    
    // Remove from .gitignore
    remove_from_gitignore(base_path, project_name)?;
    
    println!("\n  {} {}", "🗑".red(), format!("Removed project '{}'", project_name).bold());
    println!("     {} {}", "└".bright_black(), "Removed from .meta file".italic().bright_black());
    
    // Optionally remove the directory
    if project_path.exists() {
        if force {
            std::fs::remove_dir_all(&project_path)?;
            println!("     {} {}", "└".bright_black(), format!("Deleted directory '{}'", project_name).italic().bright_red());
        } else {
            println!("     {} {}", "└".bright_black(), format!("Directory '{}' kept on disk", project_name).italic().bright_black());
            println!("     {} {}", " ".bright_black(), format!("To remove: rm -rf {}", project_name).dimmed());
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
    // Load the .meta file
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }
    
    let mut config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Check if project exists in config
    if !config.projects.contains_key(project_name) {
        return Err(anyhow::anyhow!("Project '{}' not found in .meta file", project_name));
    }
    
    let project_path = base_path.join(project_name);
    let current_url = config.get_project_url(project_name).unwrap_or_else(|| "".to_string());
    
    // Check if project is currently marked as local
    if !current_url.starts_with("local:") {
        println!("\n  {} {}", "ℹ".bright_black(), format!("Project '{}' already has a remote URL", project_name).dimmed());
        return Ok(());
    }
    
    // Check if directory exists and has git
    if !project_path.exists() || !project_path.join(".git").exists() {
        return Err(anyhow::anyhow!("Project '{}' directory doesn't exist or is not a git repository", project_name));
    }
    
    // Check for remote URL
    let repo = Repository::open(&project_path)?;
    let remote_url = get_remote_url(&repo)?;
    
    if let Some(detected_url) = remote_url {
        // Update the URL in config
        config.projects.insert(project_name.to_string(), ProjectEntry::Url(detected_url.clone()));
        config.save_to_file(&meta_file_path)?;
        
        // Add to gitignore
        update_gitignore(base_path, project_name)?;
        
        println!("\n  {} {}", "✅".green(), format!("Updated project '{}'", project_name).bold().green());
        println!("     {} {}", "Remote:".bright_black(), detected_url.green());
        println!("     {} {}", "└".bright_black(), "Added to .gitignore".italic().bright_black());
        println!();
    } else {
        println!("\n  {} {}", "⚠️".yellow(), format!("Project '{}' still has no remote", project_name).bold().yellow());
        println!("     {} {}", "└".bright_black(), "Add a remote with: git remote add origin <url>".dimmed());
        println!();
    }
    
    Ok(())
}