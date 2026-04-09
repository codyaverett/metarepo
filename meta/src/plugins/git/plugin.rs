use super::{clone_missing_repos, clone_repository, get_git_status};
use crate::plugins::exec::{execute_with_iterator, ProjectIterator};
use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{arg, command, plugin, BasePlugin, MetaConfig, MetaPlugin, RuntimeConfig};

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

    // Filter out repos with uncommitted changes to avoid conflicts
    let (iterator, skipped) = iterator.filter_clean_repos();

    if !skipped.is_empty() {
        println!(
            "⚠️  Skipping {} repo(s) with uncommitted changes:",
            skipped.len()
        );
        for name in &skipped {
            println!("   - {}", name);
        }
        println!();
    }

    execute_with_iterator(
        "git",
        &["pull"],
        iterator,
        include_main,
        parallel,
        false,
        false,
    )
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
