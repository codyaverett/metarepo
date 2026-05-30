use super::{clone_missing_repos, clone_repository, get_git_status};
use crate::plugins::exec::{execute_with_projects, ProjectInfo, ProjectIterator};
use crate::plugins::shared::detect_default_branch;
use crate::plugins::worktree::list_worktrees;
use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{arg, command, plugin, BasePlugin, MetaConfig, MetaPlugin, RuntimeConfig};
use std::path::Path;
use std::process::Command;

/// GitPlugin using the new simplified plugin architecture
pub struct GitPlugin;

impl GitPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("git")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Git operations across multiple repositories")
            .author("Metarepo Contributors")
            .command(
                command("clone")
                    .about("Clone meta repository and all child repositories")
                    .aliases(vec!["c".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("url")
                            .help("Repository URL to clone")
                            .required(true)
                            .takes_value(true),
                    ),
            )
            .command(
                command("status")
                    .about("Show git status across all repositories")
                    .aliases(vec!["st".to_string(), "s".to_string()])
                    .with_help_formatting(),
            )
            .command(
                command("update")
                    .about("Clone missing repositories")
                    .aliases(vec!["up".to_string(), "u".to_string()])
                    .with_help_formatting(),
            )
            .command(
                command("pull")
                    .about("Pull latest changes for all repositories")
                    .aliases(vec!["p".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Pull repositories in parallel"),
                    )
                    .arg(
                        arg("skip-main")
                            .long("skip-main")
                            .help("Skip pulling the main meta repository"),
                    )
                    .arg(
                        arg("include-only")
                            .long("include-only")
                            .help("Only include projects matching patterns (comma-separated)")
                            .takes_value(true),
                    )
                    .arg(
                        arg("exclude")
                            .long("exclude")
                            .help("Exclude projects matching patterns (comma-separated)")
                            .takes_value(true),
                    ),
            )
            .handler("clone", handle_clone)
            .handler("status", handle_status)
            .handler("update", handle_update)
            .handler("pull", handle_pull)
            .build()
    }
}

/// Handler for the clone command
fn handle_clone(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let url = matches.get_one::<String>("url").unwrap();
    println!("Cloning meta repository from: {}", url);

    // Extract repo name from URL for directory name
    let repo_name = url
        .split('/')
        .next_back()
        .unwrap_or("meta-repo")
        .trim_end_matches(".git");

    let target_path = config.working_dir.join(repo_name);
    clone_repository(url, &target_path, false)?;

    // After cloning, look for .meta file and clone child repos
    let meta_file = target_path.join(".meta");
    if meta_file.exists() {
        std::env::set_current_dir(&target_path)?;
        clone_missing_repos()?;
    }

    Ok(())
}

/// Handler for the status command
fn handle_status(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    println!("Git status across all repositories:");
    println!("================================");

    // Show status for main repo
    println!("\nMain repository:");
    match get_git_status(&config.working_dir) {
        Ok(status) => println!("{}", status),
        Err(e) => println!("Error: {}", e),
    }

    // Show status for each project
    for project_path in config.meta_config.projects.keys() {
        let full_path = if config.meta_root().is_some() {
            config.meta_root().unwrap().join(project_path)
        } else {
            config.working_dir.join(project_path)
        };

        if full_path.exists() {
            println!("\n{}:", project_path);
            match get_git_status(&full_path) {
                Ok(status) => println!("{}", status),
                Err(e) => println!("Error: {}", e),
            }
        } else {
            println!("\n{}: (not cloned)", project_path);
        }
    }

    Ok(())
}

/// Handler for the update command
fn handle_update(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    println!("Cloning missing repositories...");
    clone_missing_repos()?;
    Ok(())
}

/// Handler for the pull command
fn handle_pull(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    let parallel = matches.get_flag("parallel");
    let skip_main = matches.get_flag("skip-main");
    let include_main = !skip_main;

    // Build iterator filtered to existing git repos
    let mut iterator = ProjectIterator::new(&config, base_path)
        .filter_existing()
        .filter_git_repos();

    if let Some(patterns_str) = matches.get_one::<String>("include-only") {
        let pattern_vec: Vec<String> = patterns_str.split(',').map(|s| s.to_string()).collect();
        iterator = iterator.with_include_patterns(pattern_vec);
    }

    if let Some(patterns_str) = matches.get_one::<String>("exclude") {
        let pattern_vec: Vec<String> = patterns_str.split(',').map(|s| s.to_string()).collect();
        iterator = iterator.with_exclude_patterns(pattern_vec);
    }

    // Expand each project into the directories that can actually be pulled.
    // Regular repos pull in place; bare repos (whose top-level git dir is bare)
    // pull in each managed worktree so we never hit
    // "fatal: this operation must be run in a work tree". Worktrees with
    // uncommitted changes are skipped to avoid conflicts.
    let mut targets: Vec<ProjectInfo> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for project in iterator {
        if is_bare_repository(&project.path) {
            expand_bare_repo_targets(&project, &mut targets, &mut skipped);
        } else if project.has_uncommitted_changes() {
            skipped.push(project.name.clone());
        } else {
            targets.push(project);
        }
    }

    if !skipped.is_empty() {
        println!(
            "⚠️  Skipping {} target(s) with uncommitted changes:",
            skipped.len()
        );
        for name in &skipped {
            println!("   - {}", name);
        }
        println!();
    }

    execute_with_projects(
        "git",
        &["pull"],
        targets,
        include_main,
        parallel,
        false,
        false,
    )
}

/// Determine whether the git repository discovered at `path` is bare.
///
/// Metarepo clones bare repositories into `<project>/.git` and checks branches
/// out into `<project>/<branch>` worktrees, so running `git pull` in the
/// project root itself fails because there is no work tree there.
fn is_bare_repository(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--is-bare-repository")
        .output()
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        })
        .unwrap_or(false)
}

/// Expand a bare repository into one pull target per checked-out worktree.
///
/// Every managed branch (worktree) is added so they all get updated, and the
/// default branch is verified to be present. The bare entry and detached
/// worktrees are skipped because there is nothing to pull into them.
fn expand_bare_repo_targets(
    project: &ProjectInfo,
    targets: &mut Vec<ProjectInfo>,
    skipped: &mut Vec<String>,
) {
    let worktrees = match list_worktrees(&project.path) {
        Ok(worktrees) => worktrees,
        Err(e) => {
            eprintln!("⚠️  Could not list worktrees for {}: {}", project.name, e);
            return;
        }
    };

    let default_branch = detect_default_branch(&project.path).ok();
    let mut added_default = false;

    for wt in &worktrees {
        // Skip the bare entry and any detached HEADs: neither can be pulled.
        if wt.is_bare || wt.is_detached {
            continue;
        }

        let branch = wt.branch.strip_prefix("refs/heads/").unwrap_or(&wt.branch);
        if branch.is_empty() {
            continue;
        }

        if default_branch.as_deref() == Some(branch) {
            added_default = true;
        }

        let info = ProjectInfo::new(
            format!("{} [{}]", project.name, branch),
            wt.path.clone(),
            project.repo_url.clone(),
        );

        if info.has_uncommitted_changes() {
            skipped.push(info.name.clone());
        } else {
            targets.push(info);
        }
    }

    // "Always use the default branch at least": if no worktree for the default
    // branch exists, fall back to fetching so its refs are still updated rather
    // than leaving the bare repo untouched.
    if !added_default {
        if let Some(branch) = &default_branch {
            println!(
                "ℹ️  {}: no worktree for default branch '{}', fetching instead",
                project.name, branch
            );
            let status = Command::new("git")
                .arg("-C")
                .arg(&project.path)
                .arg("fetch")
                .arg("origin")
                .arg(branch)
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(_) | Err(_) => {
                    eprintln!("⚠️  {}: fetch of '{}' failed", project.name, branch);
                }
            }
        }
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for GitPlugin {
    fn name(&self) -> &str {
        "git"
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

impl BasePlugin for GitPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Git operations across multiple repositories")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for GitPlugin {
    fn default() -> Self {
        Self::new()
    }
}
