use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};
use super::{execute_in_all_projects, execute_in_specific_projects};

pub struct ExecPlugin;

impl ExecPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest exec")
            .about("Execute commands across multiple repositories")
            .arg(
                Arg::new("command")
                    .value_name("COMMAND")
                    .help("Command to execute in each project directory")
                    .required(true)
            )
            .arg(
                Arg::new("projects")
                    .long("projects")
                    .short('p')
                    .value_name("PROJECTS")
                    .help("Comma-separated list of specific projects to run command in")
                    .value_delimiter(',')
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for ExecPlugin {
    fn name(&self) -> &str {
        "exec"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("exec")
                .visible_aliases(["e", "x"])
                .about("Execute commands across multiple repositories")
                .disable_help_subcommand(true)
                .allow_external_subcommands(true)
                .arg(
                    Arg::new("projects")
                        .long("projects")
                        .short('p')
                        .value_name("PROJECTS")
                        .help("Comma-separated list of specific projects to run command in")
                        .value_delimiter(',')
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some((command, sub_matches)) => {
                // Parse remaining arguments - external subcommands store args differently
                let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                    Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                    None => Vec::new()
                };
                
                // Check if specific projects were specified
                if let Some(projects) = matches.get_many::<String>("projects") {
                    let project_list: Vec<&str> = projects.map(|s| s.as_str()).collect();
                    execute_in_specific_projects(command, &args, &project_list)?;
                } else {
                    execute_in_all_projects(command, &args)?;
                }
                
                Ok(())
            }
            None => self.show_help()
        }
    }
}

impl Default for ExecPlugin {
    fn default() -> Self {
        Self::new()
    }
}