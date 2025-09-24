use anyhow::{Context, Result};
use colored::*;
use metarepo_core::MetaConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

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
pub fn add_worktrees(
    branch: &str,
    projects: &[String],
    base_path: &Path,
    path_suffix: Option<&str>,
    create_branch: bool,
    current_project: Option<&str>,
) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Determine which projects to operate on
    let selected_projects = if projects.is_empty() {
        // If no projects specified, check for current project context
        if let Some(current) = current_project {
            println!("  {} Using current project: {}", "📍".cyan(), current.bold());
            vec![current.to_string()]
        } else {
            // Interactive selection
            select_projects_interactive(&config)?
        }
    } else if projects.len() == 1 && projects[0] == "--all" {
        config.projects.keys().cloned().collect()
    } else {
        // Resolve project identifiers (could be aliases or basenames)
        let mut selected = Vec::new();
        for project_id in projects {
            // Try to find the project by full name, basename, or alias
            let resolved = resolve_project_identifier(&config, project_id);
            if let Some(project_name) = resolved {
                selected.push(project_name);
            } else {
                eprintln!("  {} Project '{}' not found in workspace", "⚠️".yellow(), project_id);
            }
        }
        selected
    };

    if selected_projects.is_empty() {
        println!("  {} No projects selected", "ℹ".bright_black());
        return Ok(());
    }

    println!("\n  {} {}", "🌿".green(), format!("Creating worktree '{}' for {} project(s)", branch, selected_projects.len()).bold());
    println!("  {}", "═".repeat(60).bright_black());

    let mut success_count = 0;
    let mut failed = Vec::new();

    for project_name in &selected_projects {
        let project_path = base_path.join(project_name);
        
        if !project_path.exists() {
            eprintln!("\n  {} {} {}", "⏭".yellow(), project_name.bright_white(), "(missing)".yellow());
            failed.push(project_name.clone());
            continue;
        }

        if !project_path.join(".git").exists() {
            eprintln!("\n  {} {} {}", "⏭".yellow(), project_name.bright_white(), "(not a git repo)".yellow());
            failed.push(project_name.clone());
            continue;
        }

        println!("\n  {} {}", "📦".blue(), project_name.bold());

        // Determine worktree path
        let worktree_dir = path_suffix.unwrap_or(branch);
        let worktree_path = project_path.join(".worktrees").join(worktree_dir);

        // Check if worktree already exists
        if worktree_path.exists() {
            println!("     {} {}", "⚠️".yellow(), "Worktree already exists".yellow());
            continue;
        }

        // Create the worktree
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("add");

        if create_branch {
            cmd.arg("-b").arg(branch);
        }

        cmd.arg(&worktree_path);
        
        if !create_branch {
            cmd.arg(branch);
        }

        let output = cmd.output()
            .context(format!("Failed to create worktree for {}", project_name))?;

        if output.status.success() {
            println!("     {} {}", "✅".green(), format!("Created at {}", worktree_path.display()).green());
            success_count += 1;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("     {} {}", "❌".red(), format!("Failed: {}", stderr.trim()).red());
            failed.push(project_name.clone());
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {} worktrees created, {} failed", 
        "Summary:".bright_black(),
        success_count.to_string().green(),
        if !failed.is_empty() { failed.len().to_string().red() } else { "0".bright_black() }
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
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;
    
    // Find projects that have this worktree
    let mut projects_with_worktree = Vec::new();
    for (project_name, _) in &config.projects {
        let project_path = base_path.join(project_name);
        if let Ok(worktrees) = list_worktrees(&project_path) {
            for wt in worktrees {
                if wt.path.file_name().map(|n| n.to_string_lossy().to_string()) == Some(branch.to_string()) 
                    || wt.branch == branch {
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
                println!("  {} Using current project: {}", "📍".cyan(), current.bold());
                vec![current.to_string()]
            } else {
                println!("  {} Current project '{}' doesn't have worktree '{}'", "⚠️".yellow(), current, branch);
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
                eprintln!("  {} Project '{}' not found", "⚠️".yellow(), project_id);
            }
        }
        selected
    };

    if selected_projects.is_empty() {
        println!("  {} No projects selected", "ℹ".bright_black());
        return Ok(());
    }

    println!("\n  {} {}", "🗑".red(), format!("Removing worktree '{}' from {} project(s)", branch, selected_projects.len()).bold());
    println!("  {}", "═".repeat(60).bright_black());

    let mut success_count = 0;

    for project_name in &selected_projects {
        let project_path = base_path.join(project_name);
        
        println!("\n  {} {}", "📦".blue(), project_name.bold());

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("remove");

        if force {
            cmd.arg("--force");
        }

        // Try to find the worktree path
        if let Ok(worktrees) = list_worktrees(&project_path) {
            let matching_wt = worktrees.iter().find(|wt| {
                wt.path.file_name().map(|n| n.to_string_lossy().to_string()) == Some(branch.to_string())
                    || wt.branch == branch
            });

            if let Some(wt) = matching_wt {
                cmd.arg(&wt.path);
                
                let output = cmd.output()
                    .context(format!("Failed to remove worktree for {}", project_name))?;

                if output.status.success() {
                    println!("     {} {}", "✅".green(), "Removed successfully".green());
                    success_count += 1;
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("     {} {}", "❌".red(), format!("Failed: {}", stderr.trim()).red());
                }
            } else {
                println!("     {} {}", "⏭".yellow(), "Worktree not found".yellow());
            }
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    println!("  {} {} worktrees removed", 
        "Summary:".bright_black(),
        success_count.to_string().green()
    );

    Ok(())
}

/// List all worktrees across the workspace
pub fn list_all_worktrees(base_path: &Path) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    println!("\n  {} {}", "🌿".green(), "Workspace Worktrees".bold());
    println!("  {}", "═".repeat(60).bright_black());

    let mut total_worktrees = 0;
    let mut projects_with_worktrees = 0;
    let mut worktree_map: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new();

    // Collect all worktrees grouped by branch name
    for (project_name, _) in &config.projects {
        let project_path = base_path.join(project_name);
        
        if !project_path.exists() || !project_path.join(".git").exists() {
            continue;
        }

        if let Ok(worktrees) = list_worktrees(&project_path) {
            let non_main_worktrees: Vec<_> = worktrees.into_iter()
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

                    worktree_map.entry(branch_name)
                        .or_insert_with(Vec::new)
                        .push((project_name.clone(), wt.path));
                }
            }
        }
    }

    if worktree_map.is_empty() {
        println!("\n  {} {}", "📦".bright_blue(), "No worktrees found in workspace".dimmed());
        println!("  {} {}", "".dimmed(), "Use 'meta worktree add' to create worktrees".dimmed());
    } else {
        // Display worktrees grouped by branch
        for (branch, projects) in worktree_map.iter() {
            println!("\n  {} {}", "🌿".green(), branch.bold().white());
            for (project, path) in projects {
                let status = if path.exists() {
                    "[active]".green()
                } else {
                    "[missing]".red()
                };
                
                // Show relative path from project root
                let relative_path = path.strip_prefix(base_path)
                    .unwrap_or(path)
                    .display();
                    
                println!("  {} {} {}", 
                    "├──".bright_black(),
                    format!("{}: {}", project.bright_blue(), relative_path).white(),
                    status
                );
            }
        }

        println!("\n  {}", "─".repeat(60).bright_black());
        println!("  {} {} worktrees across {} projects", 
            total_worktrees.to_string().cyan().bold(),
            "total".dimmed(),
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
        return Err(anyhow::anyhow!("No .meta file found. Run 'meta init' first."));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    println!("\n  {} {}", "🧹".yellow(), if dry_run { "Checking for stale worktrees (dry run)" } else { "Pruning stale worktrees" }.bold());
    println!("  {}", "═".repeat(60).bright_black());

    let mut total_pruned = 0;

    for (project_name, _) in &config.projects {
        let project_path = base_path.join(project_name);
        
        if !project_path.exists() || !project_path.join(".git").exists() {
            continue;
        }

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("prune");

        if dry_run {
            cmd.arg("--dry-run");
        }

        cmd.arg("--verbose");

        let output = cmd.output()
            .context(format!("Failed to prune worktrees for {}", project_name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        let output_text = format!("{}{}", stdout, stderr);
        
        if !output_text.trim().is_empty() {
            println!("\n  {} {}", "📦".blue(), project_name.bold());
            for line in output_text.lines() {
                if !line.trim().is_empty() {
                    println!("     {} {}", "│".bright_black(), line.trim());
                    if line.contains("Removing") {
                        total_pruned += 1;
                    }
                }
            }
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    if dry_run {
        println!("  {} {} stale worktrees found", 
            "Summary:".bright_black(),
            if total_pruned > 0 { total_pruned.to_string().yellow() } else { "0".green() }
        );
        if total_pruned > 0 {
            println!("  {} {}", "ℹ".bright_black(), "Run without --dry-run to remove them".dimmed());
        }
    } else {
        println!("  {} {} worktrees pruned", 
            "Summary:".bright_black(),
            total_pruned.to_string().green()
        );
    }

    Ok(())
}

/// Interactive project selection
fn select_projects_interactive(config: &MetaConfig) -> Result<Vec<String>> {
    use std::io::{self, Write};

    println!("\n  {} {}", "📋".cyan(), "Select projects for worktree (space to toggle, enter to confirm):".bold());
    println!("  {}", "─".repeat(60).bright_black());

    let projects: Vec<String> = config.projects.keys().cloned().collect();
    // Removed unused selected variable

    // Simple text-based selection
    for (i, project) in projects.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).bright_black(), project);
    }

    print!("\n  {} Enter project numbers (comma-separated) or 'all': ", "→".bright_black());
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

    println!("\n  {} {}", "📋".cyan(), format!("Select projects to remove worktree '{}' from:", branch).bold());
    println!("  {}", "─".repeat(60).bright_black());

    for (i, project) in available.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).bright_black(), project);
    }

    print!("\n  {} Enter project numbers (comma-separated) or 'all': ", "→".bright_black());
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