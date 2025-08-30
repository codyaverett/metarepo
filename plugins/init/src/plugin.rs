use crate::initialize_meta_repo_formatted;
use anyhow::Result;
use clap::{ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig, FormattedPlugin, OutputContext, output_format_arg};

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
        <Self as FormattedPlugin>::handle_command(self, matches, config)
    }
}

impl FormattedPlugin for InitPlugin {
    fn formatted_commands(&self) -> Vec<&str> {
        vec!["init"]
    }
    
    fn handle_formatted_command(
        &self,
        _command: &str,
        _matches: &ArgMatches,
        config: &RuntimeConfig,
        output: &mut dyn OutputContext,
    ) -> Result<()> {
        initialize_meta_repo_formatted(&config.working_dir, output)?;
        Ok(())
    }
}

impl Default for InitPlugin {
    fn default() -> Self {
        Self::new()
    }
}