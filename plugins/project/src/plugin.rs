use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::{create_project, import_project};

pub struct ProjectPlugin;

impl ProjectPlugin {
    pub fn new() -> Self {
        Self
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
            _ => {
                println!("Unknown project subcommand. Use --help to see available commands.");
                Ok(())
            }
        }
    }
}

impl Default for ProjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}