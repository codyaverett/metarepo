use anyhow::{Context, Result};
use colored::*;
use metarepo_core::MetaConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use self::plugin::WorktreePlugin;

mod plugin;

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub branch: String,
    pub path: PathBuf,
    pub is_bare: bool,
    pub is_detached: bool,
    pub head: Option<String>,
}

#[derive(Debug)]
pub enum BranchStatus {
    Local,
    Remote(String), // Contains the remote ref (e.g., "origin/feature-123")
    NotFound,
}

/// Check if a branch exists locally or remotely
fn check_branch_exists(repo_path: &Path, branch: &str) -> Result<BranchStatus> {
    // Check local branches first
    let local_output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("branch")
        .arg("--list")
        .arg(branch)
        .output()
        .context("Failed to check local branches")?;

    if local_output.status.success() {
        let stdout = String::from_utf8_lossy(&local_output.stdout);
        if stdout.trim().contains(branch) {
            return Ok(BranchStatus::Local);
        }
    }

    // Check remote branches
    let remote_output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("branch")
        .arg("-r")
        .arg("--list")
        .arg(format!("*/{}", branch))
        .output()
        .context("Failed to check remote branches")?;

    if remote_output.status.success() {
        let stdout = String::from_utf8_lossy(&remote_output.stdout);
        // Look for pattern like "  origin/feature-123"
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with(branch) {
                return Ok(BranchStatus::Remote(trimmed.to_string()));
            }
        }
    }

    Ok(BranchStatus::NotFound)
}

#[derive(Debug)]
pub struct ProjectWorktrees {
    pub project_name: String,
    pub project_path: PathBuf,
    pub worktrees: Vec<WorktreeInfo>,
}

/// Resolve a project identifier to its full name
fn resolve_project_identifier(config: &MetaConfig, identifier: &str) -> Option<String> {
    // First check if it's a full project name
    if config.projects.contains_key(identifier) {
        return Some(identifier.to_string());
    }

    // Check if it's a basename match
    for project_name in config.projects.keys() {
        if let Some(basename) = std::path::Path::new(project_name).file_name() {
            if basename.to_string_lossy() == identifier {
                return Some(project_name.clone());
            }
        }
    }

    // TODO: Check custom aliases when implemented
    None
}

/// List all worktrees for a given repository
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
        .context("Failed to list worktrees")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_worktree: Option<WorktreeInfo> = None;

    for line in stdout.lines() {
        if line.starts_with("worktree ") {
            if let Some(wt) = current_worktree.take() {
                worktrees.push(wt);
            }
            let path = line.strip_prefix("worktree ").unwrap_or("");
            current_worktree = Some(WorktreeInfo {
                branch: String::new(),
                path: PathBuf::from(path),
                is_bare: false,
                is_detached: false,
                head: None,
            });
        } else if line.starts_with("HEAD ") {
            if let Some(ref mut wt) = current_worktree {
                wt.head = Some(line.strip_prefix("HEAD ").unwrap_or("").to_string());
            }
        } else if line.starts_with("branch ") {
            if let Some(ref mut wt) = current_worktree {
                wt.branch = line.strip_prefix("branch ").unwrap_or("").to_string();
            }
        } else if line == "bare" {
            if let Some(ref mut wt) = current_worktree {
                wt.is_bare = true;
            }
        } else if line == "detached" {
            if let Some(ref mut wt) = current_worktree {
                wt.is_detached = true;
            }
        }
    }

    if let Some(wt) = current_worktree {
        worktrees.push(wt);
    }

    Ok(worktrees)
}

/// Add a worktree for selected projects
#[allow(clippy::too_many_arguments)]
pub fn add_worktrees(
    branch: &str,
    projects: &[String],
    base_path: &Path,
    path_suffix: Option<&str>,
    create_branch: bool,
    starting_point: Option<&str>,
    no_hooks: bool,
    current_project: Option<&str>,
    config: &MetaConfig,
) -> Result<()> {
    // Determine which projects to operate on
    let selected_projects = if projects.is_empty() {
        // If no projects specified, check for current project context
        if let Some(current) = current_project {
            println!("Using current project: {}", current.bold());
            vec![current.to_string()]
        } else {
            // Interactive selection
            select_projects_interactive(config)?
        }
    } else if projects.len() == 1 && projects[0] == "--all" {
        config.projects.keys().cloned().collect()
    } else {
        // Resolve project identifiers (could be aliases or basenames)
        let mut selected = Vec::new();
        for project_id in projects {
            // Try to find the project by full name, basename, or alias
            let resolved = resolve_project_identifier(config, project_id);
            if let Some(project_name) = resolved {
                selected.push(project_name);
            } else {
                eprintln!(
                    "{} Project '{}' not found in workspace",
                    "✗".yellow(),
                    project_id
                );
            }
        }
        selected
    };

    if selected_projects.is_empty() {
        println!("No projects selected");
        return Ok(());
    }

    println!(
        "\nCreating worktree '{}' for {} project{}\n",
        branch.bright_white(),
        selected_projects.len(),
        if selected_projects.len() == 1 {
            ""
        } else {
            "s"
        }
    );

    let mut success_count = 0;
    let mut failed = Vec::new();

    for project_name in &selected_projects {
        let project_path = base_path.join(project_name);

        if !project_path.exists() {
            eprintln!("{} {} (missing)", "✗".yellow(), project_name.bright_white());
            failed.push(project_name.clone());
            continue;
        }

        if !project_path.join(".git").exists() {
            eprintln!(
                "{} {} (not a git repo)",
                "✗".yellow(),
                project_name.bright_white()
            );
            failed.push(project_name.clone());
            continue;
        }

        println!("{}", project_name.bold());

        // Determine worktree path based on whether this is a bare repo
        let is_bare = config.is_bare_repo(project_name);
        let worktree_dir = path_suffix.unwrap_or(branch);
        let worktree_path = if is_bare {
            // For bare repos: <project>/<branch>/
            project_path.join(worktree_dir)
        } else {
            // For normal repos: <project>/.worktrees/<branch>/
            project_path.join(".worktrees").join(worktree_dir)
        };

        // Check if worktree already exists
        if worktree_path.exists() {
            println!("  {} Already exists", "✗".yellow());
            continue;
        }

        // Determine git directory
        let git_dir = if is_bare {
            // For bare repos, the git directory is at <project>/.git/
            project_path.join(".git")
        } else {
            // For normal repos, the git directory is at <project>/
            project_path.clone()
        };

        // Smart branch detection and worktree creation
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&git_dir)
            .arg("worktree")
            .arg("add")
            .arg("-q"); // Quiet mode - suppress text output

        // Determine the strategy based on flags and branch existence
        if create_branch {
            // Explicit branch creation requested with -b flag
            cmd.arg("-b").arg(branch);
            cmd.arg(&worktree_path);
            if let Some(start) = starting_point {
                cmd.arg(start);
            }
        } else {
            // Smart detection: check if branch exists
            match check_branch_exists(&git_dir, branch) {
                Ok(BranchStatus::Local) => {
                    // Branch exists locally, checkout that branch
                    cmd.arg(&worktree_path);
                    cmd.arg(branch);
                }
                Ok(BranchStatus::Remote(remote_ref)) => {
                    // Branch exists remotely, create local tracking branch
                    println!(
                        "  {} Found remote branch: {}",
                        "ℹ".cyan(),
                        remote_ref.bright_white()
                    );
                    cmd.arg("-b").arg(branch);
                    cmd.arg(&worktree_path);
                    cmd.arg(&remote_ref);
                }
                Ok(BranchStatus::NotFound) => {
                    // Branch doesn't exist - need to create it
                    let start_point = if let Some(start) = starting_point {
                        start.to_string()
                    } else {
                        // Prompt user for starting point
                        println!(
                            "  {} Branch '{}' not found",
                            "⚠".yellow(),
                            branch.bright_white()
                        );
                        prompt_for_starting_point()?
                    };

                    println!(
                        "  {} Creating new branch from {}",
                        "✓".green(),
                        start_point.bright_white()
                    );
                    cmd.arg("-b").arg(branch);
                    cmd.arg(&worktree_path);
                    cmd.arg(&start_point);
                }
                Err(e) => {
                    eprintln!("  {} Failed to check branch status: {}", "✗".red(), e);
                    failed.push(project_name.clone());
                    continue;
                }
            }
        }

        // Stream git output in real-time
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

        let status = cmd
            .status()
            .context(format!("Failed to create worktree for {}", project_name))?;

        if status.success() {
            println!("  {} Complete", "✓".green());
            success_count += 1;

            // Execute post-create command if configured and not skipped
            if !no_hooks {
                if let Some(worktree_init) = config.get_worktree_init(project_name) {
                    println!("  Running worktree_init...");

                    let mut cmd = Command::new("sh");
                    cmd.arg("-c")
                        .arg(&worktree_init)
                        .current_dir(&worktree_path);

                    // Add project environment variables if configured
                    if let Some(metarepo_core::ProjectEntry::Metadata(metadata)) =
                        config.projects.get(project_name)
                    {
                        for (key, value) in &metadata.env {
                            cmd.env(key, value);
                        }
                    }

                    match cmd.output() {
                        Ok(hook_output) => {
                            if hook_output.status.success() {
                                println!("  {} Hook complete", "✓".green());
                            } else {
                                let stderr = String::from_utf8_lossy(&hook_output.stderr);
                                eprintln!("  {} Hook failed: {}", "✗".yellow(), stderr.trim());
                            }
                        }
                        Err(e) => {
                            eprintln!("  {} Failed to run hook: {}", "✗".yellow(), e);
                        }
                    }
                }
            }
        } else {
            eprintln!("  {} Failed", "✗".red());
            failed.push(project_name.clone());
        }
    }

    println!(
        "\nSummary: {} created, {} failed",
        success_count.to_string().green(),
        if !failed.is_empty() {
            failed.len().to_string().red()
        } else {
            "0".bright_black()
        }
    );

    Ok(())
}

/// Remove worktrees for selected projects
pub fn remove_worktrees(
    branch: &str,
    projects: &[String],
    base_path: &Path,
    force: bool,
    current_project: Option<&str>,
) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    // Find projects that have this worktree
    let mut projects_with_worktree = Vec::new();
    for project_name in config.projects.keys() {
        let project_path = base_path.join(project_name);
        if let Ok(worktrees) = list_worktrees(&project_path) {
            for wt in worktrees {
                if wt.path.file_name().map(|n| n.to_string_lossy().to_string())
                    == Some(branch.to_string())
                    || wt.branch == branch
                {
                    projects_with_worktree.push(project_name.clone());
                    break;
                }
            }
        }
    }

    let selected_projects = if projects.is_empty() {
        // If no projects specified, check for current project context
        if let Some(current) = current_project {
            // Check if current project has this worktree
            if projects_with_worktree.contains(&current.to_string()) {
                println!("Using current project: {}", current.bold());
                vec![current.to_string()]
            } else {
                println!(
                    "{} Current project '{}' doesn't have worktree '{}'",
                    "✗".yellow(),
                    current,
                    branch
                );
                return Ok(());
            }
        } else if !projects_with_worktree.is_empty() {
            // Interactive selection from projects that have this worktree
            select_projects_for_removal(&projects_with_worktree, branch)?
        } else {
            Vec::new()
        }
    } else if projects.len() == 1 && projects[0] == "--all" {
        projects_with_worktree
    } else {
        // Resolve project identifiers
        let mut selected = Vec::new();
        for project_id in projects {
            let resolved = resolve_project_identifier(&config, project_id);
            if let Some(project_name) = resolved {
                selected.push(project_name);
            } else {
                eprintln!("{} Project '{}' not found", "✗".yellow(), project_id);
            }
        }
        selected
    };

    if selected_projects.is_empty() {
        println!("No projects selected");
        return Ok(());
    }

    println!(
        "\nRemoving worktree '{}' from {} project{}\n",
        branch.bright_white(),
        selected_projects.len(),
        if selected_projects.len() == 1 {
            ""
        } else {
            "s"
        }
    );

    let mut success_count = 0;

    for project_name in &selected_projects {
        let project_path = base_path.join(project_name);

        println!("{}", project_name.bold());

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("remove")
            .arg("-q"); // Quiet mode

        if force {
            cmd.arg("--force");
        }

        // Try to find the worktree path
        if let Ok(worktrees) = list_worktrees(&project_path) {
            let matching_wt = worktrees.iter().find(|wt| {
                wt.path.file_name().map(|n| n.to_string_lossy().to_string())
                    == Some(branch.to_string())
                    || wt.branch == branch
            });

            if let Some(wt) = matching_wt {
                cmd.arg(&wt.path);

                // Stream git output in real-time
                cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

                let status = cmd
                    .status()
                    .context(format!("Failed to remove worktree for {}", project_name))?;

                if status.success() {
                    println!("  {} Complete", "✓".green());
                    success_count += 1;
                } else {
                    eprintln!("  {} Failed", "✗".red());
                }
            } else {
                println!("  {} Not found", "✗".yellow());
            }
        }
    }

    println!("\nSummary: {} removed", success_count.to_string().green());

    Ok(())
}

/// List all worktrees across the workspace
pub fn list_all_worktrees(base_path: &Path) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    println!("\n{}\n", "Workspace Worktrees".bold());

    let mut total_worktrees = 0;
    let mut projects_with_worktrees = 0;
    let mut worktree_map: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new();

    // Collect all worktrees grouped by branch name
    for project_name in config.projects.keys() {
        let project_path = base_path.join(project_name);

        if !project_path.exists() || !project_path.join(".git").exists() {
            continue;
        }

        if let Ok(worktrees) = list_worktrees(&project_path) {
            let non_main_worktrees: Vec<_> = worktrees
                .into_iter()
                .filter(|wt| !wt.path.starts_with(&project_path) || wt.path != project_path)
                .collect();

            if !non_main_worktrees.is_empty() {
                projects_with_worktrees += 1;

                for wt in non_main_worktrees {
                    total_worktrees += 1;
                    let branch_name = if !wt.branch.is_empty() {
                        wt.branch.clone()
                    } else if let Some(name) = wt.path.file_name() {
                        name.to_string_lossy().to_string()
                    } else {
                        "unknown".to_string()
                    };

                    worktree_map
                        .entry(branch_name)
                        .or_default()
                        .push((project_name.clone(), wt.path));
                }
            }
        }
    }

    if worktree_map.is_empty() {
        println!("{}", "No worktrees found in workspace".dimmed());
        println!("{}", "Use 'meta worktree add' to create worktrees".dimmed());
    } else {
        // Display worktrees grouped by branch
        for (branch, projects) in worktree_map.iter() {
            println!("{}", branch.bold().white());
            for (project, path) in projects {
                let status = if path.exists() {
                    "active".green()
                } else {
                    "missing".red()
                };

                // Show relative path from project root
                let relative_path = path.strip_prefix(base_path).unwrap_or(path).display();

                println!(
                    "  {}: {} ({})",
                    project.bright_blue(),
                    relative_path,
                    status
                );
            }
            println!();
        }

        println!(
            "Total: {} worktrees across {} projects",
            total_worktrees.to_string().cyan(),
            projects_with_worktrees.to_string().cyan()
        );
    }

    println!();
    Ok(())
}

/// Prune worktrees for all projects
pub fn prune_worktrees(base_path: &Path, dry_run: bool) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    if dry_run {
        println!("\nChecking for stale worktrees (dry run)\n");
    } else {
        println!("\nPruning stale worktrees\n");
    }

    for project_name in config.projects.keys() {
        let project_path = base_path.join(project_name);

        if !project_path.exists() || !project_path.join(".git").exists() {
            continue;
        }

        println!("{}", project_name.bold());

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("prune");

        if dry_run {
            cmd.arg("--dry-run");
        }

        cmd.arg("--verbose");

        // Stream git output in real-time
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

        let _status = cmd
            .status()
            .context(format!("Failed to prune worktrees for {}", project_name))?;
    }

    if dry_run {
        println!("\n{}", "Check complete".dimmed());
        println!(
            "{}",
            "Run without --dry-run to remove stale worktrees".dimmed()
        );
    } else {
        println!("\n{}", "Prune complete".green());
    }

    Ok(())
}

/// Interactive project selection
fn select_projects_interactive(config: &MetaConfig) -> Result<Vec<String>> {
    use std::io::{self, Write};

    println!(
        "\n  {} {}",
        "📋".cyan(),
        "Select projects for worktree (space to toggle, enter to confirm):".bold()
    );
    println!("  {}", "─".repeat(60).bright_black());

    let projects: Vec<String> = config.projects.keys().cloned().collect();
    // Removed unused selected variable

    // Simple text-based selection
    for (i, project) in projects.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).bright_black(), project);
    }

    print!(
        "\n  {} Enter project numbers (comma-separated) or 'all': ",
        "→".bright_black()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    let selected_projects = if input.eq_ignore_ascii_case("all") {
        projects
    } else if input.is_empty() {
        Vec::new()
    } else {
        let mut result = Vec::new();
        for part in input.split(',') {
            if let Ok(num) = part.trim().parse::<usize>() {
                if num > 0 && num <= projects.len() {
                    result.push(projects[num - 1].clone());
                }
            }
        }
        result
    };

    Ok(selected_projects)
}

/// Interactive selection for removal
fn select_projects_for_removal(available: &[String], branch: &str) -> Result<Vec<String>> {
    use std::io::{self, Write};

    println!(
        "\n  {} {}",
        "📋".cyan(),
        format!("Select projects to remove worktree '{}' from:", branch).bold()
    );
    println!("  {}", "─".repeat(60).bright_black());

    for (i, project) in available.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).bright_black(), project);
    }

    print!(
        "\n  {} Enter project numbers (comma-separated) or 'all': ",
        "→".bright_black()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    let selected_projects = if input.eq_ignore_ascii_case("all") {
        available.to_vec()
    } else if input.is_empty() {
        Vec::new()
    } else {
        let mut result = Vec::new();
        for part in input.split(',') {
            if let Ok(num) = part.trim().parse::<usize>() {
                if num > 0 && num <= available.len() {
                    result.push(available[num - 1].clone());
                }
            }
        }
        result
    };

    Ok(selected_projects)
}

/// Prompt user for starting point when creating a new branch
fn prompt_for_starting_point() -> Result<String> {
    use std::io::{self, Write};

    println!(
        "\n  {} {}",
        "🌿".cyan(),
        "Branch doesn't exist. Create it from:".bold()
    );
    println!("  {}", "─".repeat(60).bright_black());
    println!("  {} HEAD (current commit)", "[1]".bright_black());
    println!("  {} origin/main", "[2]".bright_black());
    println!("  {} origin/develop", "[3]".bright_black());
    println!("  {} Custom ref", "[4]".bright_black());

    print!("\n  {} Select option [1-4]: ", "→".bright_black());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    match choice {
        "1" | "" => Ok("HEAD".to_string()),
        "2" => Ok("origin/main".to_string()),
        "3" => Ok("origin/develop".to_string()),
        "4" => {
            print!(
                "  {} Enter custom ref (branch/tag/commit): ",
                "→".bright_black()
            );
            io::stdout().flush()?;
            let mut custom = String::new();
            io::stdin().read_line(&mut custom)?;
            Ok(custom.trim().to_string())
        }
        _ => {
            println!("  {} Invalid choice, using HEAD", "⚠".yellow());
            Ok("HEAD".to_string())
        }
    }
}
