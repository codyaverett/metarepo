use super::{clone_missing_repos, clone_repository, get_git_status};
use crate::plugins::exec::{execute_with_projects, ProjectInfo, ProjectIterator};
use crate::plugins::shared::{detect_default_branch, parse_depth_arg};
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
            .help_description(
                "Run git operations across every repository in the workspace.\n\
                 \n\
                 Metarepo treats the main repo and each project listed in .meta as a\n\
                 single fleet. These subcommands fan the same git action out across all\n\
                 of them at once, so you can clone, status, update, pull, push, fetch,\n\
                 and checkout the whole workspace with one command. Operations are\n\
                 scoped to your current directory: run them from a project subdirectory\n\
                 to act on just that project, or from the workspace root to act on\n\
                 everything.\n\
                 \n\
                 Examples:\n\
                 \n\
                   meta git status                    status for every repo\n\
                   meta git pull --skip-main          pull child repos only\n\
                   meta git push                      push every repo with an upstream\n\
                   meta git fetch                     fetch remotes in parallel\n\
                   meta git checkout feature/x        switch every repo to a branch\n\
                   meta git clone git@host:org/x.git  clone a workspace and its children",
            )
            .command(
                command("clone")
                    .about("Clone a meta repository and all of its child repositories")
                    .help_description(
                        "Clone a meta repository and then clone every project it tracks.\n\
                         \n\
                         The URL is cloned into a directory named after the repository in\n\
                         the current working directory. If the clone contains a workspace\n\
                         config (.meta), metarepo switches into it and clones each missing\n\
                         child project so the whole workspace is checked out in one step.\n\
                         Use --depth to perform a shallow clone; the depth is recorded so\n\
                         later re-clones (meta git update) stay shallow.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git clone git@github.com:org/workspace.git\n\
                           meta git c https://github.com/org/workspace.git\n\
                           meta git clone --depth 1 https://github.com/org/workspace.git",
                    )
                    .aliases(vec!["c".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("url")
                            .help("Repository URL to clone")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("depth")
                            .long("depth")
                            .help("Create a shallow clone with the given history depth")
                            .takes_value(true),
                    ),
            )
            .command(
                command("status")
                    .about("Show git status across all repositories")
                    .help_description(
                        "Show the working-tree status of every repository in scope.\n\
                         \n\
                         Prints a per-repository status (modified, added, deleted, and\n\
                         untracked files, or a clean marker) for the main repo and each\n\
                         tracked project. The main repository is only included in the\n\
                         full-workspace view; when you run this from inside a project or\n\
                         subdirectory, only the in-scope projects are reported. Projects\n\
                         listed in .meta that are not yet cloned are flagged as not cloned.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git status   status for the whole workspace\n\
                           meta git st       same, using an alias",
                    )
                    .aliases(vec!["st".to_string(), "s".to_string()])
                    .with_help_formatting(),
            )
            .command(
                command("update")
                    .about("Clone any repositories that are missing from the workspace")
                    .help_description(
                        "Clone every tracked project that is not yet checked out locally.\n\
                         \n\
                         Reads the workspace's .meta file, finds each project whose\n\
                         directory does not exist, and clones it from its configured URL\n\
                         (cloning bare repositories with a default worktree where the\n\
                         project is marked bare). Existing repositories are left untouched,\n\
                         so this is the command to run after pulling new entries into .meta.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git update   clone all missing projects\n\
                           meta git u        same, using an alias",
                    )
                    .aliases(vec!["up".to_string(), "u".to_string()])
                    .with_help_formatting(),
            )
            .command(
                command("pull")
                    .about("Pull latest changes for all repositories")
                    .help_description(
                        "Pull the latest changes into every repository in scope.\n\
                         \n\
                         Pulls run concurrently by default since they are network-bound;\n\
                         use --sequential to pull one repo at a time. Each repo is\n\
                         preflighted first: repositories with uncommitted changes or no\n\
                         upstream tracking branch are skipped with a note instead of\n\
                         failing the run. Bare repositories are expanded so each managed\n\
                         worktree is pulled in place. The main repo is pulled in the\n\
                         full-workspace view unless --skip-main is given.\n\
                         \n\
                         Use --include-only and --exclude with comma-separated patterns to\n\
                         narrow which projects are pulled.\n\
                         \n\
                         Shallow projects (cloned with --depth) accumulate history on a\n\
                         plain pull. Pass --shallow to re-truncate each project with a\n\
                         stored depth in .meta (git fetch --depth N) after pulling so\n\
                         history shrinks back to the configured depth.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git pull                       pull everything\n\
                           meta git pull --skip-main           pull child repos only\n\
                           meta git pull --exclude vendor,docs  pull all but matches\n\
                           meta git pull --shallow             re-truncate shallow repos",
                    )
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
                    )
                    .arg(arg("shallow").long("shallow").help(
                        "Re-truncate history after pulling for projects with a stored \
                         shallow clone depth in .meta (fetch --depth N), so shallow \
                         repos do not accumulate history over time",
                    )),
            )
            .command(
                command("push")
                    .about("Push commits for all repositories with an upstream")
                    .help_description(
                        "Push the current branch of every repository in scope to its\n\
                         upstream remote.\n\
                         \n\
                         Pushes run concurrently by default; use --sequential for one\n\
                         repo at a time. Repositories with no upstream tracking branch\n\
                         are skipped with a note. Bare repositories push from each\n\
                         managed worktree. Dirty working trees are still pushed (git\n\
                         allows this). The main repo is included in the full-workspace\n\
                         view unless --skip-main is given.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git push\n\
                           meta git push --skip-main\n\
                           meta git push --exclude vendor,docs",
                    )
                    .aliases(vec!["ps".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Push repositories in parallel (now the default)"),
                    )
                    .arg(
                        arg("sequential")
                            .long("sequential")
                            .help("Push repositories one at a time instead of concurrently"),
                    )
                    .arg(
                        arg("skip-main")
                            .long("skip-main")
                            .help("Skip pushing the main meta repository"),
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
            .command(
                command("fetch")
                    .about("Fetch remotes for all repositories")
                    .help_description(
                        "Fetch from the default remote for every repository in scope.\n\
                         \n\
                         Fetch is network-bound and runs concurrently by default; use\n\
                         --sequential to fetch one repo at a time. Bare repositories are\n\
                         fetched at the bare root (no worktree expansion). Dirty working\n\
                         trees are not skipped because fetch does not touch the work tree.\n\
                         The main repo is included in the full-workspace view unless\n\
                         --skip-main is given.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git fetch\n\
                           meta git fetch --skip-main\n\
                           meta git fetch --include-only frontend,backend",
                    )
                    .aliases(vec!["f".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Fetch repositories in parallel (now the default)"),
                    )
                    .arg(
                        arg("sequential")
                            .long("sequential")
                            .help("Fetch repositories one at a time instead of concurrently"),
                    )
                    .arg(
                        arg("skip-main")
                            .long("skip-main")
                            .help("Skip fetching the main meta repository"),
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
            .command(
                command("checkout")
                    .about("Check out a branch across all repositories")
                    .help_description(
                        "Switch every repository in scope to the given branch.\n\
                         \n\
                         Repositories with uncommitted changes are skipped so local work\n\
                         is not lost. Bare repositories check out inside each managed\n\
                         worktree. Pass --create (or -b) to create the branch when it\n\
                         does not already exist (equivalent to git checkout -b). The\n\
                         main repo is included in the full-workspace view unless\n\
                         --skip-main is given.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta git checkout main\n\
                           meta git checkout feature/auth\n\
                           meta git checkout -b feature/new\n\
                           meta git switch develop",
                    )
                    .aliases(vec![
                        "co".to_string(),
                        "switch".to_string(),
                        "sw".to_string(),
                    ])
                    .with_help_formatting()
                    .arg(
                        arg("branch")
                            .help("Branch name to check out")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        arg("create")
                            .short('b')
                            .long("create")
                            .help("Create the branch if it does not exist (git checkout -b)"),
                    )
                    .arg(
                        arg("parallel")
                            .long("parallel")
                            .help("Check out repositories in parallel (now the default)"),
                    )
                    .arg(
                        arg("sequential")
                            .long("sequential")
                            .help("Check out repositories one at a time instead of concurrently"),
                    )
                    .arg(
                        arg("skip-main")
                            .long("skip-main")
                            .help("Skip checking out the main meta repository"),
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
            .handler("push", handle_push)
            .handler("fetch", handle_fetch)
            .handler("checkout", handle_checkout)
            .build()
    }
}

/// Handler for the clone command
fn handle_clone(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let url = matches.get_one::<String>("url").unwrap();

    let depth = parse_depth_arg(matches.get_one::<String>("depth"))?;

    println!("Cloning meta repository from: {}", url);

    // Extract repo name from URL for directory name
    let repo_name = url
        .split('/')
        .next_back()
        .unwrap_or("meta-repo")
        .trim_end_matches(".git");

    let target_path = config.working_dir.join(repo_name);
    clone_repository(url, &target_path, false, depth)?;

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
    let shallow = matches.get_flag("shallow");
    let parallel = fanout_parallel(matches);
    let (targets, depths) = resolve_fanout_targets(
        matches,
        config,
        FanoutPolicy {
            expand_bare_worktrees: true,
            require_clean: true,
            require_upstream: true,
            track_depth: true,
        },
    )?;

    let workers = parallelism();
    let refetch_targets: Vec<(ProjectInfo, i32)> = targets
        .iter()
        .zip(depths.iter())
        .filter_map(|(p, d)| d.map(|d| (p.clone(), d)))
        .collect();

    execute_with_projects("git", &["pull"], targets, false, parallel, false, false)?;

    // With --shallow, re-truncate each depth-tracked repository after the
    // pull so its history shrinks back to the stored depth. This must run
    // after (not before) pulling: a `fetch --depth` that moves the shallow
    // boundary past the local HEAD leaves the local and remote branches with
    // no visible common ancestor, and a subsequent `git pull` then fails
    // with a divergent-branches error under default git configuration.
    if shallow {
        if refetch_targets.is_empty() {
            println!("\nℹ️  --shallow: no projects in scope have a stored clone depth in .meta");
        } else {
            println!(
                "\nRe-truncating {} shallow target(s) to their stored depth...",
                refetch_targets.len()
            );
            let results = parallel_map(refetch_targets, workers, |(project, depth)| {
                let result = crate::plugins::shared::refetch_shallow(&project.path, depth);
                (project.name, depth, result)
            });
            for (name, depth, result) in results {
                if let Err(e) = result {
                    eprintln!("⚠️  {} (depth {}): {}", name, depth, e);
                }
            }
        }
    }

    Ok(())
}

/// Handler for the push command
fn handle_push(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let parallel = fanout_parallel(matches);
    let (targets, _) = resolve_fanout_targets(
        matches,
        config,
        FanoutPolicy {
            expand_bare_worktrees: true,
            require_clean: false,
            require_upstream: true,
            track_depth: false,
        },
    )?;
    execute_with_projects("git", &["push"], targets, false, parallel, false, false)
}

/// Handler for the fetch command
fn handle_fetch(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let parallel = fanout_parallel(matches);
    // Fetch does not need a work tree: bare roots are fine, dirty trees ok.
    let (targets, _) = resolve_fanout_targets(
        matches,
        config,
        FanoutPolicy {
            expand_bare_worktrees: false,
            require_clean: false,
            require_upstream: false,
            track_depth: false,
        },
    )?;
    execute_with_projects("git", &["fetch"], targets, false, parallel, false, false)
}

/// Handler for the checkout / switch command
fn handle_checkout(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let branch = matches
        .get_one::<String>("branch")
        .ok_or_else(|| anyhow::anyhow!("branch is required"))?;
    let create = matches.get_flag("create");
    let parallel = fanout_parallel(matches);
    let (targets, _) = resolve_fanout_targets(
        matches,
        config,
        FanoutPolicy {
            expand_bare_worktrees: true,
            require_clean: true,
            require_upstream: false,
            track_depth: false,
        },
    )?;

    let args: Vec<&str> = if create {
        vec!["checkout", "-b", branch.as_str()]
    } else {
        vec!["checkout", branch.as_str()]
    };
    execute_with_projects("git", &args, targets, false, parallel, false, false)
}

/// Preflight policy shared by multi-repo git fan-out commands.
struct FanoutPolicy {
    /// Expand bare project roots into managed worktrees (pull/push/checkout).
    /// When false, operate on the bare root itself (fetch).
    expand_bare_worktrees: bool,
    /// Skip targets with uncommitted changes.
    require_clean: bool,
    /// Skip targets whose current branch has no upstream.
    require_upstream: bool,
    /// Attach each project's stored shallow depth (for pull --shallow).
    track_depth: bool,
}

fn fanout_parallel(matches: &ArgMatches) -> bool {
    // Network-bound ops run concurrently by default. `--sequential` restores
    // one-at-a-time; `--parallel` is kept for back-compat.
    !matches.get_flag("sequential")
}

fn parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Collect scoped project candidates, preflight them, print skip notes, and
/// return the executable targets (plus optional shallow depths when tracked).
fn resolve_fanout_targets(
    matches: &ArgMatches,
    config: &RuntimeConfig,
    policy: FanoutPolicy,
) -> Result<(Vec<ProjectInfo>, Vec<Option<i32>>)> {
    let base_path = config
        .meta_root()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;

    let scope = config.scoped_project_keys();
    if scope.is_empty() {
        println!("No projects in this directory.");
        return Ok((Vec::new(), Vec::new()));
    }
    let full_scope = scope.len() == config.meta_config.projects.len();
    let skip_main = matches.get_flag("skip-main") || !full_scope;

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

    let mut candidates: Vec<(ProjectInfo, Option<i32>)> = iterator
        .map(|p| {
            let depth = if policy.track_depth {
                config.meta_config.get_project_depth(&p.name)
            } else {
                None
            };
            (p, depth)
        })
        .collect();

    if !skip_main {
        let main_name = base_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| format!("{} (main)", n))
            .unwrap_or_else(|| "main repository".to_string());
        candidates.insert(
            0,
            (
                ProjectInfo::new(main_name, base_path.to_path_buf(), "local".to_string()),
                None,
            ),
        );
    }

    let workers = parallelism();
    let classifications = parallel_map(candidates, workers, |(project, depth)| {
        (
            classify_fanout_target(
                project,
                policy.expand_bare_worktrees,
                policy.require_clean,
                policy.require_upstream,
            ),
            depth,
        )
    });

    let mut targets: Vec<(ProjectInfo, Option<i32>)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut no_upstream: Vec<String> = Vec::new();

    for (classification, depth) in classifications {
        match classification {
            PullTarget::Pull(project) => targets.push((project, depth)),
            PullTarget::Skip(name) => skipped.push(name),
            PullTarget::NoUpstream(name) => no_upstream.push(name),
            PullTarget::Bare {
                targets: t,
                skipped: s,
                no_upstream: u,
            } => {
                targets.extend(t.into_iter().map(|p| (p, depth)));
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

    let depths: Vec<Option<i32>> = targets.iter().map(|(_, d)| *d).collect();
    let projects: Vec<ProjectInfo> = targets.into_iter().map(|(p, _)| p).collect();
    Ok((projects, depths))
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

/// Inspect one candidate and decide how (or whether) it should be operated on.
///
/// This is pure preflight: it only spawns short-lived, network-free git probes,
/// which makes it safe to run concurrently across many repositories.
fn classify_fanout_target(
    project: ProjectInfo,
    expand_bare_worktrees: bool,
    require_clean: bool,
    require_upstream: bool,
) -> PullTarget {
    if is_bare_repository(&project.path) {
        if expand_bare_worktrees {
            let mut targets = Vec::new();
            let mut skipped = Vec::new();
            let mut no_upstream = Vec::new();
            expand_bare_repo_targets(
                &project,
                require_clean,
                require_upstream,
                &mut targets,
                &mut skipped,
                &mut no_upstream,
            );
            PullTarget::Bare {
                targets,
                skipped,
                no_upstream,
            }
        } else {
            // Fetch-style: bare roots accept the command without a work tree.
            PullTarget::Pull(project)
        }
    } else if require_clean && project.has_uncommitted_changes() {
        PullTarget::Skip(project.name)
    } else if require_upstream && !branch_has_upstream(&project.path) {
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

/// Expand a bare repository into one target per checked-out worktree.
///
/// Every managed branch (worktree) is added so they all get the fan-out
/// command. The bare entry and detached worktrees are skipped because there
/// is no work tree to operate on. When `require_upstream` is set and no
/// worktree for the default branch exists, fall back to a bare-root fetch so
/// the default branch refs are still updated (pull-oriented behavior).
fn expand_bare_repo_targets(
    project: &ProjectInfo,
    require_clean: bool,
    require_upstream: bool,
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
        // Skip the bare entry and any detached HEADs: neither has a branch to act on.
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

        if require_clean && info.has_uncommitted_changes() {
            skipped.push(info.name.clone());
        } else if require_upstream && !branch_has_upstream(&info.path) {
            no_upstream.push(info.name.clone());
        } else {
            targets.push(info);
        }
    }

    // Pull-oriented fallback: if no worktree for the default branch exists,
    // fetch its refs at the bare root so the bare repo is not left untouched.
    if require_upstream && !added_default {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command as ClapCommand;

    #[test]
    fn git_plugin_registers_push_fetch_checkout() {
        let plugin = GitPlugin::create_plugin();
        let app = plugin.register_commands(ClapCommand::new("meta"));
        let git = app
            .get_subcommands()
            .find(|c| c.get_name() == "git")
            .expect("git subcommand");
        let names: Vec<&str> = git.get_subcommands().map(|c| c.get_name()).collect();
        for expected in [
            "push", "fetch", "checkout", "pull", "clone", "status", "update",
        ] {
            assert!(
                names.contains(&expected),
                "expected git subcommand '{}', got {:?}",
                expected,
                names
            );
        }
        // switch is an alias of checkout
        let checkout = git
            .get_subcommands()
            .find(|c| c.get_name() == "checkout")
            .expect("checkout");
        let aliases: Vec<&str> = checkout.get_all_aliases().collect();
        assert!(aliases.contains(&"switch"));
        assert!(aliases.contains(&"co"));
    }

    #[test]
    fn classify_fetch_keeps_bare_root() {
        // Synthetic bare detection is path-based; without a real repo the
        // non-bare path falls through to Pull when no filters apply.
        let project = ProjectInfo::new(
            "demo".into(),
            std::env::temp_dir().join("metarepo-git-classify-missing"),
            "local".into(),
        );
        match classify_fanout_target(project, false, false, false) {
            PullTarget::Pull(_) | PullTarget::Skip(_) | PullTarget::NoUpstream(_) => {}
            PullTarget::Bare { .. } => panic!("missing path should not expand as bare"),
        }
    }
}
