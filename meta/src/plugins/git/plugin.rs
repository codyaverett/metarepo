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
                            .help("Pull repositories in parallel (now the default)"),
                    )
                    .arg(
                        arg("sequential")
                            .long("sequential")
                            .help("Pull repositories one at a time instead of concurrently"),
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

    // After cloning, look for a workspace config and clone child repos
    if MetaConfig::config_in_dir(&target_path).is_some() {
        std::env::set_current_dir(&target_path)?;
        clone_missing_repos()?;
    }

    Ok(())
}

/// Handler for the status command
fn handle_status(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let scope = config.scoped_project_keys();
    if scope.is_empty() {
        println!("No projects in this directory.");
        return Ok(());
    }
    // Only show the workspace's main repository in the full-workspace view, not
    // when scoped to a project or subdirectory.
    let show_main = scope.len() == config.meta_config.projects.len();
    let base_path = config
        .meta_root()
        .unwrap_or_else(|| config.working_dir.clone());

    println!("Git status:");
    println!("===========");

    if show_main {
        println!("\nMain repository:");
        match get_git_status(&base_path) {
            Ok(status) => println!("{}", status),
            Err(e) => println!("Error: {}", e),
        }
    }

    for project_path in &scope {
        let full_path = base_path.join(project_path);
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
fn handle_pull(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let base_path = config
        .meta_root()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;

    // Directory-aware scope: only the in-scope projects are pulled.
    let scope = config.scoped_project_keys();
    if scope.is_empty() {
        println!("No projects in this directory.");
        return Ok(());
    }
    let full_scope = scope.len() == config.meta_config.projects.len();

    // Pulls are network-bound, so run them concurrently by default. `--sequential`
    // restores one-at-a-time behavior; `--parallel` is kept for back-compat.
    let parallel = !matches.get_flag("sequential");
    // Pull the main repo only in the full-workspace view (or when not skipped).
    let skip_main = matches.get_flag("skip-main") || !full_scope;

    // Build iterator scoped to the in-scope projects, filtered to existing repos.
    let mut iterator = ProjectIterator::new(&config.meta_config, &base_path)
        .with_scope(&scope)
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

    // Collect every candidate up front so the independent per-repo preflight
    // checks (bare detection, uncommitted-change and upstream probes, worktree
    // listing) can run concurrently rather than one repo at a time.
    let mut candidates: Vec<ProjectInfo> = iterator.collect();

    // Treat the main meta repository as just another candidate so it goes
    // through the same graceful skipping (uncommitted changes / no upstream)
    // instead of aborting the whole run, and so it is pulled alongside the rest.
    if !skip_main {
        let main_name = base_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| format!("{} (main)", n))
            .unwrap_or_else(|| "main repository".to_string());
        candidates.insert(
            0,
            ProjectInfo::new(main_name, base_path.to_path_buf(), "local".to_string()),
        );
    }

    // Expand each candidate into the directories that can actually be pulled.
    // Regular repos pull in place; bare repos (whose top-level git dir is bare)
    // pull in each managed worktree so we never hit
    // "fatal: this operation must be run in a work tree". Worktrees with
    // uncommitted changes are skipped to avoid conflicts.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let classifications = parallel_map(candidates, workers, classify_pull_target);

    let mut targets: Vec<ProjectInfo> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut no_upstream: Vec<String> = Vec::new();

    for classification in classifications {
        match classification {
            PullTarget::Pull(project) => targets.push(project),
            PullTarget::Skip(name) => skipped.push(name),
            PullTarget::NoUpstream(name) => no_upstream.push(name),
            PullTarget::Bare {
                targets: t,
                skipped: s,
                no_upstream: u,
            } => {
                targets.extend(t);
                skipped.extend(s);
                no_upstream.extend(u);
            }
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

    if !no_upstream.is_empty() {
        println!(
            "ℹ️  Skipping {} target(s) with no upstream tracking branch:",
            no_upstream.len()
        );
        for name in &no_upstream {
            println!("   - {}", name);
        }
        println!("   Set one with: git branch --set-upstream-to=origin/<branch>");
        println!();
    }

    // `include_main` is false here: the main repo, when not skipped, is already
    // part of `targets` so it is filtered and pulled like any other repository.
    execute_with_projects("git", &["pull"], targets, false, parallel, false, false)
}

/// Outcome of inspecting a single candidate before pulling.
enum PullTarget {
    /// A directory that can be pulled directly.
    Pull(ProjectInfo),
    /// Skipped because of uncommitted changes (carries the display name).
    Skip(String),
    /// Skipped because the current branch has no upstream (display name).
    NoUpstream(String),
    /// A bare repository expanded into its per-worktree results.
    Bare {
        targets: Vec<ProjectInfo>,
        skipped: Vec<String>,
        no_upstream: Vec<String>,
    },
}

/// Inspect one candidate and decide how (or whether) it should be pulled.
///
/// This is pure preflight: it only spawns short-lived, network-free git probes,
/// which makes it safe to run concurrently across many repositories.
fn classify_pull_target(project: ProjectInfo) -> PullTarget {
    if is_bare_repository(&project.path) {
        let mut targets = Vec::new();
        let mut skipped = Vec::new();
        let mut no_upstream = Vec::new();
        expand_bare_repo_targets(&project, &mut targets, &mut skipped, &mut no_upstream);
        PullTarget::Bare {
            targets,
            skipped,
            no_upstream,
        }
    } else if project.has_uncommitted_changes() {
        PullTarget::Skip(project.name)
    } else if !branch_has_upstream(&project.path) {
        PullTarget::NoUpstream(project.name)
    } else {
        PullTarget::Pull(project)
    }
}

/// Apply `f` to every item across a bounded pool of worker threads, preserving
/// input order in the returned vector.
///
/// Used to run the independent, per-repository preflight checks concurrently.
/// Falls back to a plain sequential map when there is nothing to gain.
fn parallel_map<T, R>(items: Vec<T>, workers: usize, f: impl Fn(T) -> R + Sync) -> Vec<R>
where
    T: Send,
    R: Send,
{
    let len = items.len();
    if len <= 1 || workers <= 1 {
        return items.into_iter().map(f).collect();
    }

    let workers = workers.min(len);
    let queue: std::sync::Mutex<std::collections::VecDeque<(usize, T)>> =
        std::sync::Mutex::new(items.into_iter().enumerate().collect());
    let slots: Vec<std::sync::Mutex<Option<R>>> =
        (0..len).map(|_| std::sync::Mutex::new(None)).collect();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let next = queue.lock().unwrap().pop_front();
                match next {
                    Some((index, item)) => {
                        let result = f(item);
                        *slots[index].lock().unwrap() = Some(result);
                    }
                    None => break,
                }
            });
        }
    });

    slots
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().expect("worker filled slot"))
        .collect()
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

/// Determine whether the current branch at `path` has an upstream tracking
/// branch configured.
///
/// `git pull` aborts with "There is no tracking information for the current
/// branch" when the checked-out branch has no upstream. Detecting that ahead of
/// time lets us skip the target with a helpful note instead of surfacing a
/// failure for what is an expected, benign state (e.g. a freshly created local
/// branch).
fn branch_has_upstream(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("--symbolic-full-name")
        .arg("@{upstream}")
        .output()
        .map(|output| output.status.success())
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
    no_upstream: &mut Vec<String>,
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
        } else if !branch_has_upstream(&info.path) {
            no_upstream.push(info.name.clone());
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
