use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::{create_project, import_project};

pub struct ProjectPlugin;

impl ProjectPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("meta project")
            .about("Project management operations")
            .subcommand(
                Command::new("create")
                    .about("Create a new project")
                    .arg(
                        Arg::new("path")
                            .value_name("PATH")
                            .help("Project path/name")
                            .required(true)
                    )
                    .arg(
                        Arg::new("repo-url")
                            .value_name("REPO_URL")
                            .help("Repository URL")
                            .required(true)
                    )
            )
            .subcommand(
                Command::new("import")
                    .about("Import existing project")
                    .arg(
                        Arg::new("path")
                            .value_name("PATH")
                            .help("Project path/name")
                            .required(true)
                    )
                    .arg(
                        Arg::new("repo-url")
                            .value_name("REPO_URL")
                            .help("Repository URL")
                            .required(true)
                    )
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for ProjectPlugin {
    fn name(&self) -> &str {
        "project"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("project")
                .about("Project management operations")
                .allow_external_subcommands(true) // This allows unknown subcommands to pass through
                .subcommand(
                    Command::new("create")
                        .about("Create a new project")
                        .arg(
                            Arg::new("path")
                                .value_name("PATH")
                                .help("Project path/name")
                                .required(true)
                        )
                        .arg(
                            Arg::new("repo-url")
                                .value_name("REPO_URL")
                                .help("Repository URL")
                                .required(true)
                        )
                )
                .subcommand(
                    Command::new("import")
                        .about("Import existing project")
                        .arg(
                            Arg::new("path")
                                .value_name("PATH")
                                .help("Project path/name")
                                .required(true)
                        )
                        .arg(
                            Arg::new("repo-url")
                                .value_name("REPO_URL")
                                .help("Repository URL")
                                .required(true)
                        )
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // If no subcommand is provided, show help
        if matches.subcommand().is_none() {
            return self.show_help();
        }
        
        match matches.subcommand() {
            Some(("create", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                let repo_url = sub_matches.get_one::<String>("repo-url").unwrap();
                
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                create_project(path, repo_url, &base_path)?;
                Ok(())
            }
            Some(("import", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                let repo_url = sub_matches.get_one::<String>("repo-url").unwrap();
                
                let base_path = if config.meta_root().is_some() {
                    config.meta_root().unwrap()
                } else {
                    config.working_dir.clone()
                };
                
                import_project(path, repo_url, &base_path)?;
                Ok(())
            }
            Some((external_cmd, _args)) => {
                // Handle unknown/external subcommands by showing help
                println!("Unknown project subcommand: '{}'", external_cmd);
                println!();
                self.show_help()
            }
            None => {
                // This case is already handled above, but keeping for completeness
                self.show_help()
            }
        }
    }
}

impl Default for ProjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}