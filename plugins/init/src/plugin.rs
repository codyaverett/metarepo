use crate::initialize_meta_repo;
use anyhow::Result;
use clap::{ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};

pub struct InitPlugin;

impl InitPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl MetaPlugin for InitPlugin {
    fn name(&self) -> &str {
        "init"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("init")
                .about("Initialize a new meta repository")
                .long_about("Initialize the current directory as a meta repository by creating a .meta file with default configuration and updating .gitignore patterns.")
        )
    }
    
    fn handle_command(&self, _matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        initialize_meta_repo(&config.working_dir)?;
        Ok(())
    }
}

impl Default for InitPlugin {
    fn default() -> Self {
        Self::new()
    }
}