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
    pub is_locked: bool,
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
                is_locked: false,
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
        } else if line == "locked" || line.starts_with("locked ") {
            // Porcelain emits a bare `locked` line, or `locked <reason>`.
            if let Some(ref mut wt) = current_worktree {
                wt.is_locked = true;
            }
        }
    }

    if let Some(wt) = current_worktree {
        worktrees.push(wt);
    }

    Ok(worktrees)
}

/// Strip the `refs/heads/` prefix from a branch ref, if present.
fn short_branch_name(branch_ref: &str) -> &str {
    branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref)
}

/// Validate that every key in `scope_projects` is a workspace project, returning
/// them as the iteration set. Errors on an unknown project (e.g. a typo passed
/// via `--project`); directory-derived scopes are always valid and pass through.
fn validate_scope<'a>(
    config: &MetaConfig,
    scope_projects: &'a [String],
) -> Result<Vec<&'a String>> {
    for name in scope_projects {
        if !config.projects.contains_key(name) {
            return Err(anyhow::anyhow!(
                "Project '{}' is not in the workspace .meta file",
                name
            ));
        }
    }
    Ok(scope_projects.iter().collect())
}

/// True if `refname` resolves in the given repo.
fn git_ref_exists(project_path: &Path, refname: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg(refname)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve the ref that worktree branches should be compared against, given the
/// project's base branch name. Prefers the local base branch; otherwise falls
/// back to the remote-tracking ref, then the bare name.
fn resolve_base_ref(project_path: &Path, base_name: &str) -> String {
    if git_ref_exists(project_path, base_name) {
        return base_name.to_string();
    }
    let remote = format!("origin/{}", base_name);
    if git_ref_exists(project_path, &remote) {
        return remote;
    }
    base_name.to_string()
}

/// True if `branch_ref` is fully contained in `base_ref` (an ordinary merge):
/// the branch tip is an ancestor of base.
fn branch_is_merged(project_path: &Path, branch_ref: &str, base_ref: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("merge-base")
        .arg("--is-ancestor")
        .arg(branch_ref)
        .arg(base_ref)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// True if `branch_ref` introduces no changes relative to `base_ref`. Uses a
/// three-dot diff so it catches squash- and rebase-merged branches (whose tips
/// are not ancestors of base) as well as branches with no commits of their own.
fn branch_has_no_diff(project_path: &Path, branch_ref: &str, base_ref: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("diff")
        .arg("--quiet")
        .arg(format!("{}...{}", base_ref, branch_ref))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// True if the worktree at `wt_path` has uncommitted or untracked changes.
fn worktree_is_dirty(wt_path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(wt_path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Relative date of the last commit on `branch_ref` (e.g. "3 weeks ago").
fn last_commit_relative(project_path: &Path, branch_ref: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("log")
        .arg("-1")
        .arg("--format=%cr")
        .arg(branch_ref)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
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
    allow_hooks: bool,
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
        // Reject path-traversal in branch / path_suffix before joining.
        if let Err(e) = metarepo_core::validate_path_segment("worktree name", worktree_dir) {
            eprintln!("  {} {}", "✗".red(), e);
            failed.push(project_name.clone());
            continue;
        }
        let worktree_path = if is_bare {
            // For bare repos: <project>/<branch>/
            project_path.join(worktree_dir)
        } else {
            // For normal repos: <project>/.worktrees/<branch>/
            project_path.join(".worktrees").join(worktree_dir)
        };
        // Defense in depth: confirm the canonical path stays inside the project.
        if let Err(e) = metarepo_core::ensure_within_base(&project_path, &worktree_path) {
            eprintln!("  {} {}", "✗".red(), e);
            failed.push(project_name.clone());
            continue;
        }

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

            // Execute post-create command if configured and not skipped.
            // worktree_init is shell code from .meta — typically committed by
            // collaborators, so we surface the command and require explicit
            // opt-in (either --allow-hooks or an interactive confirmation).
            if !no_hooks {
                if let Some(worktree_init) = config.get_worktree_init(project_name) {
                    let proceed = if allow_hooks {
                        true
                    } else if metarepo_core::is_interactive() {
                        println!(
                            "  {} worktree_init hook for {}:",
                            "🪝".yellow(),
                            project_name.bold()
                        );
                        println!("     {}", worktree_init.bright_white());
                        match metarepo_core::prompt_confirm(
                            "  Run this hook?",
                            false,
                            metarepo_core::NonInteractiveMode::Defaults,
                        ) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("  {} Skipping hook: {}", "✗".yellow(), e);
                                false
                            }
                        }
                    } else {
                        eprintln!(
                            "  {} Skipping worktree_init hook (non-interactive; pass --allow-hooks to opt in)",
                            "⚠".yellow()
                        );
                        false
                    };

                    if !proceed {
                        continue;
                    }
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
    scope: &[String],
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
        // No explicit projects: limit auto-detection to the directory-derived
        // scope so we never touch (or prompt about) out-of-scope projects.
        let in_scope: Vec<String> = projects_with_worktree
            .iter()
            .filter(|p| scope.iter().any(|s| s == *p))
            .cloned()
            .collect();

        if in_scope.is_empty() {
            println!(
                "{} No project in scope has a worktree '{}'",
                "·".bright_black(),
                branch
            );
            return Ok(());
        } else if in_scope.len() == 1 {
            println!("Using project: {}", in_scope[0].bold());
            in_scope
        } else {
            // Multiple in-scope projects have it — let the user choose.
            select_projects_for_removal(&in_scope, branch)?
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

/// List worktrees across the workspace, optionally scoped to a single project.
///
/// `scope_projects` is the set of project keys to list (already resolved from
/// directory context by the caller) — running `meta worktree list` inside a
/// project shows just that project, inside a subdirectory the projects beneath
/// it, and at the workspace root every project.
pub fn list_all_worktrees(base_path: &Path, scope_projects: &[String]) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;
    let project_iter = validate_scope(&config, scope_projects)?;

    let header = if scope_projects.len() == 1 {
        format!("Worktrees for {}", scope_projects[0])
    } else {
        "Workspace Worktrees".to_string()
    };
    println!("\n{}\n", header.bold());

    let mut total_worktrees = 0;
    let mut projects_with_worktrees = 0;
    let mut worktree_map: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new();

    // Collect all worktrees grouped by branch name
    for project_name in project_iter {
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
        let mut missing_count = 0;
        for (branch, projects) in worktree_map.iter() {
            println!("{}", branch.bold().white());
            for (project, path) in projects {
                let status = if path.exists() {
                    "active".green()
                } else {
                    missing_count += 1;
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

        if missing_count > 0 {
            println!(
                "\n{} {} worktree path{} appear missing — they may have been moved.",
                "⚠".yellow(),
                missing_count.to_string().yellow(),
                if missing_count == 1 { "" } else { "s" }
            );
            println!(
                "  Try {} to update git's administrative paths.",
                "meta worktree repair".bright_white()
            );
        }
    }

    println!();
    Ok(())
}

/// Prune stale worktrees across the given set of projects.
pub fn prune_worktrees(base_path: &Path, dry_run: bool, scope_projects: &[String]) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;
    let project_iter = validate_scope(&config, scope_projects)?;

    if dry_run {
        println!("\nChecking for stale worktrees (dry run)\n");
    } else {
        println!("\nPruning stale worktrees\n");
    }

    let mut total_pruned = 0usize;
    let mut repos_with_stale = 0usize;

    for project_name in project_iter {
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

        // Capture rather than inherit: `git worktree prune --verbose` only emits
        // a "Removing ..." line per stale entry and is silent when there is
        // nothing to do. Parsing it lets us say what was (or would be) pruned
        // and report "nothing to prune" instead of leaving the user guessing.
        let output = cmd
            .output()
            .context(format!("Failed to prune worktrees for {}", project_name))?;

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let pruned_lines: Vec<&str> = combined
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with("Removing "))
            .collect();

        if pruned_lines.is_empty() {
            println!("  {} nothing to prune", "·".bright_black());
        } else {
            repos_with_stale += 1;
            for line in &pruned_lines {
                total_pruned += 1;
                let verb = if dry_run { "would remove" } else { "removed" };
                // Strip git's leading "Removing " so we can prepend our own verb.
                let detail = line.strip_prefix("Removing ").unwrap_or(line);
                println!("  {} {} {}", "✓".green(), verb, detail);
            }
        }
    }

    let entry_word = if total_pruned == 1 {
        "entry"
    } else {
        "entries"
    };
    let repo_word = if repos_with_stale == 1 {
        "repo"
    } else {
        "repos"
    };
    if dry_run {
        println!(
            "\n{}",
            format!(
                "Would prune {} stale {} across {} {}",
                total_pruned, entry_word, repos_with_stale, repo_word
            )
            .dimmed()
        );
        if total_pruned > 0 {
            println!("{}", "Run without --dry-run to remove them".dimmed());
        }
    } else {
        println!(
            "\n{} {} stale {} removed across {} {}",
            "Prune complete:".green(),
            total_pruned,
            entry_word,
            repos_with_stale,
            repo_word
        );
    }
    println!(
        "{}",
        "Prune only removes references to worktree directories that no longer exist; \
         it never deletes a worktree that still has files."
            .dimmed()
    );

    Ok(())
}

/// Options controlling [`clean_worktrees`].
#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub dry_run: bool,
    pub assume_yes: bool,
    pub keep_branches: bool,
}

/// A worktree eligible for cleanup, with the reason it qualified.
struct CleanCandidate {
    project: String,
    project_path: PathBuf,
    wt_path: PathBuf,
    branch_ref: String,
    reason: String,
    age: Option<String>,
}

/// A worktree that resembled a cleanup target but was skipped, with the reason.
struct CleanSkip {
    project: String,
    label: String,
    reason: String,
}

/// Remove worktrees whose branches are already merged into (or contribute no
/// changes relative to) their project's base branch. Destructive, but gated:
/// dirty, locked, detached, and primary worktrees are always skipped, and a
/// confirmation is required before anything is removed (unless `assume_yes`).
pub fn clean_worktrees(
    base_path: &Path,
    scope_projects: &[String],
    opts: CleanOptions,
    non_interactive: metarepo_core::NonInteractiveMode,
) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }
    let config = MetaConfig::load_from_file(&meta_file_path)?;

    let mut candidates: Vec<CleanCandidate> = Vec::new();
    let mut skipped: Vec<CleanSkip> = Vec::new();

    for project_name in scope_projects {
        if !config.projects.contains_key(project_name) {
            continue;
        }
        let project_path = base_path.join(project_name);
        if !project_path.exists() || !project_path.join(".git").exists() {
            continue;
        }
        let Ok(worktrees) = list_worktrees(&project_path) else {
            continue;
        };

        let base_name = crate::plugins::shared::detect_default_branch(&project_path)
            .unwrap_or_else(|_| "main".to_string());
        let base_ref = resolve_base_ref(&project_path, &base_name);

        for wt in worktrees {
            // Never touch the primary working tree or a bare entry.
            if wt.is_bare || wt.path == project_path {
                continue;
            }

            let label = if !wt.branch.is_empty() {
                short_branch_name(&wt.branch).to_string()
            } else {
                wt.path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            };

            if wt.is_locked {
                skipped.push(CleanSkip {
                    project: project_name.clone(),
                    label,
                    reason: "locked".to_string(),
                });
                continue;
            }
            if wt.is_detached || wt.branch.is_empty() {
                skipped.push(CleanSkip {
                    project: project_name.clone(),
                    label,
                    reason: "detached HEAD".to_string(),
                });
                continue;
            }
            // Don't evaluate a worktree that has the base branch checked out.
            if short_branch_name(&wt.branch) == base_name {
                continue;
            }
            if worktree_is_dirty(&wt.path) {
                skipped.push(CleanSkip {
                    project: project_name.clone(),
                    label,
                    reason: "uncommitted changes".to_string(),
                });
                continue;
            }

            let merged = branch_is_merged(&project_path, &wt.branch, &base_ref);
            let no_diff = !merged && branch_has_no_diff(&project_path, &wt.branch, &base_ref);
            if merged || no_diff {
                let reason = if merged {
                    format!("merged into {}", base_name)
                } else {
                    format!("no changes vs {}", base_name)
                };
                candidates.push(CleanCandidate {
                    age: last_commit_relative(&project_path, &wt.branch),
                    project: project_name.clone(),
                    project_path: project_path.clone(),
                    wt_path: wt.path.clone(),
                    branch_ref: wt.branch.clone(),
                    reason,
                });
            } else {
                skipped.push(CleanSkip {
                    project: project_name.clone(),
                    label,
                    reason: "not merged".to_string(),
                });
            }
        }
    }

    println!("\n{}\n", "Cleaning up merged worktrees".bold());

    if candidates.is_empty() {
        println!("{}", "Nothing to clean up".dimmed());
        print_clean_skips(&skipped);
        println!();
        return Ok(());
    }

    println!("{}", "Worktrees eligible for cleanup:".bold());
    for c in &candidates {
        let rel = c
            .wt_path
            .strip_prefix(base_path)
            .unwrap_or(&c.wt_path)
            .display();
        let location = match &c.age {
            Some(age) => format!("{} ({})", rel, age),
            None => format!("{}", rel),
        };
        println!(
            "  {} {}  {}  {}",
            c.project.bright_blue(),
            short_branch_name(&c.branch_ref).white(),
            c.reason.green(),
            location.dimmed()
        );
    }
    print_clean_skips(&skipped);

    if opts.dry_run {
        println!("\n{}", "Dry run — nothing removed".dimmed());
        println!(
            "{}",
            "Run without --dry-run to remove these worktrees".dimmed()
        );
        return Ok(());
    }

    if !opts.assume_yes {
        let proceed = metarepo_core::prompt_confirm(
            &format!(
                "Remove {} worktree{}?",
                candidates.len(),
                if candidates.len() == 1 { "" } else { "s" }
            ),
            false,
            non_interactive,
        )?;
        if !proceed {
            println!("{}", "Aborted — nothing removed".dimmed());
            return Ok(());
        }
    }

    println!();
    let mut removed = 0usize;
    let mut branches_deleted = 0usize;
    let mut failed = 0usize;

    for c in &candidates {
        let short = short_branch_name(&c.branch_ref);
        let status = Command::new("git")
            .arg("-C")
            .arg(&c.project_path)
            .arg("worktree")
            .arg("remove")
            .arg(&c.wt_path)
            .status();

        match status {
            Ok(s) if s.success() => {
                removed += 1;
                println!("  {} {} ({})", "✓".green(), short, c.project.bright_blue());

                if !opts.keep_branches {
                    let del = Command::new("git")
                        .arg("-C")
                        .arg(&c.project_path)
                        .arg("branch")
                        .arg("-d")
                        .arg(short)
                        .output();
                    match del {
                        Ok(o) if o.status.success() => branches_deleted += 1,
                        _ => println!(
                            "    {} kept branch {} (delete manually if intended)",
                            "·".bright_black(),
                            short
                        ),
                    }
                }
            }
            _ => {
                failed += 1;
                eprintln!("  {} {} ({})", "✗".red(), short, c.project.bright_blue());
            }
        }
    }

    println!(
        "\nSummary: {} removed, {} branch{} deleted, {} skipped{}",
        removed.to_string().green(),
        branches_deleted.to_string().green(),
        if branches_deleted == 1 { "" } else { "es" },
        skipped.len().to_string().bright_black(),
        if failed > 0 {
            format!(", {} failed", failed.to_string().red())
        } else {
            String::new()
        }
    );

    Ok(())
}

/// Print the dimmed "Skipped" section listing worktrees that were not eligible.
fn print_clean_skips(skipped: &[CleanSkip]) {
    if skipped.is_empty() {
        return;
    }
    println!("\n{}", "Skipped (not eligible):".dimmed());
    for s in skipped {
        println!(
            "  {} {} ({}) — {}",
            "·".bright_black(),
            s.label,
            s.project.bright_blue(),
            s.reason.dimmed()
        );
    }
}

/// Repair worktree administrative paths after worktrees have been moved on
/// disk. Wraps `git worktree repair` for each project in the (optionally
/// scoped) workspace, mirroring [`prune_worktrees`] in shape so the two can be
/// composed into a future maintenance command.
pub fn repair_worktrees(base_path: &Path, scope_projects: &[String], dry_run: bool) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;
    let project_iter = validate_scope(&config, scope_projects)?;

    if dry_run {
        println!("\nChecking worktree repair (dry run)\n");
    } else {
        println!("\nRepairing worktrees\n");
    }

    let mut repaired = 0;
    let mut healthy = 0;
    let mut skipped = 0;
    let mut failed: Vec<String> = Vec::new();

    for project_name in project_iter {
        let project_path = base_path.join(project_name);

        if !project_path.exists() || !project_path.join(".git").exists() {
            skipped += 1;
            continue;
        }

        println!("{}", project_name.bold());

        if dry_run {
            println!(
                "  {} Would run: git -C {} worktree repair",
                "→".bright_black(),
                project_path.display()
            );
            continue;
        }

        // Capture output: `git worktree repair` exits 0 even when nothing was
        // wrong, and only writes to stdout/stderr when it actually rewrites
        // administrative paths. Use that signal to label "Repaired" vs
        // "Nothing to repair" — otherwise every run looks like a fix.
        let output = Command::new("git")
            .arg("-C")
            .arg(&project_path)
            .arg("worktree")
            .arg("repair")
            .output()
            .context(format!(
                "Failed to run git worktree repair for {}",
                project_name
            ));

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let made_changes = !stdout.trim().is_empty() || !stderr.trim().is_empty();
                if made_changes {
                    for line in stdout.lines().chain(stderr.lines()) {
                        if !line.trim().is_empty() {
                            println!("  {}", line);
                        }
                    }
                    println!("  {} Repaired", "✓".green());
                    repaired += 1;
                } else {
                    println!("  {} Nothing to repair", "·".bright_black());
                    healthy += 1;
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!(
                    "  {} Repair failed (exit {}): {}",
                    "✗".red(),
                    out.status.code().unwrap_or(-1),
                    stderr.trim()
                );
                failed.push(project_name.clone());
            }
            Err(e) => {
                eprintln!("  {} {}", "✗".red(), e);
                failed.push(project_name.clone());
            }
        }
    }

    println!(
        "\nSummary: {} repaired, {} healthy, {} skipped, {} failed",
        repaired.to_string().green(),
        healthy.to_string().bright_black(),
        skipped.to_string().bright_black(),
        if failed.is_empty() {
            "0".bright_black()
        } else {
            failed.len().to_string().red()
        }
    );

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
