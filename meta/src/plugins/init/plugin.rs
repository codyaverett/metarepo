use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat,
    plugin,
};
use super::initialize_meta_repo;

/// InitPlugin using the new simplified plugin architecture
pub struct InitPlugin;

impl InitPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("init")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Initialize a new meta repository")
            .author("Metarepo Contributors")
            .build()
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for InitPlugin {
    fn name(&self) -> &str {
        "init"
    }
    
    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Register the init command directly at the top level
        app.subcommand(
            clap::Command::new("init")
                .about("Initialize a new meta repository")
                .long_about("Initialize the current directory as a meta repository by creating a .meta file with default configuration and updating .gitignore patterns.")
                .visible_aliases(vec!["i"])
                .version(env!("CARGO_PKG_VERSION"))
        )
    }
    
    fn handle_command(&self, _matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Check for output format flag
        if let Some(format_str) = _matches.get_one::<String>("output-format") {
            if let Some(format) = HelpFormat::from_str(format_str) {
                return self.show_help(format);
            }
        }
        
        // Check for AI help flag
        if _matches.get_flag("ai") {
            return self.show_ai_help();
        }
        
        // Directly initialize the meta repository
        initialize_meta_repo(&config.working_dir)?;
        Ok(())
    }
}

impl BasePlugin for InitPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }
    
    fn description(&self) -> Option<&str> {
        Some("Initialize a new meta repository")
    }
    
    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for InitPlugin {
    fn default() -> Self {
        Self::new()
    }
}