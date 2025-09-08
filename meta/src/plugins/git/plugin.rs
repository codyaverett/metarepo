use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat,
    plugin, command, arg,
};
use super::{clone_repository, get_git_status, clone_missing_repos};

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
                    .arg(
                        arg("url")
                            .help("Repository URL to clone")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("status")
                    .about("Show git status across all repositories")
                    .aliases(vec!["st".to_string(), "s".to_string()])
            )
            .command(
                command("update")
                    .about("Clone missing repositories")
                    .aliases(vec!["up".to_string(), "u".to_string()])
            )
            .handler("clone", handle_clone)
            .handler("status", handle_status)
            .handler("update", handle_update)
            .build()
    }
}

/// Handler for the clone command
fn handle_clone(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let url = matches.get_one::<String>("url").unwrap();
    println!("Cloning meta repository from: {}", url);
    
    // Extract repo name from URL for directory name
    let repo_name = url.split('/').last()
        .unwrap_or("meta-repo")
        .trim_end_matches(".git");
    
    let target_path = config.working_dir.join(repo_name);
    clone_repository(url, &target_path)?;
    
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
    for (project_path, _repo_url) in &config.meta_config.projects {
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
        // Check for output format flag
        if let Some(format_str) = matches.get_one::<String>("output-format") {
            if let Some(format) = HelpFormat::from_str(format_str) {
                return self.show_help(format);
            }
        }
        
        // Check for AI help flag
        if matches.get_flag("ai") {
            return self.show_ai_help();
        }
        
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