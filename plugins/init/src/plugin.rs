use crate::initialize_meta_repo;
use anyhow::Result;
use clap::{ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig, output_format_arg};

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
                .visible_alias("i")
                .about("Initialize a new meta repository")
                .long_about("Initialize the current directory as a meta repository by creating a .meta file with default configuration and updating .gitignore patterns.")
                .arg(output_format_arg())
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let output_format = self.get_output_format(matches);
        initialize_meta_repo(&config.working_dir, output_format)?;
        Ok(())
    }
    
    fn supports_output_format(&self) -> bool {
        true
    }
}

impl Default for InitPlugin {
    fn default() -> Self {
        Self::new()
    }
}