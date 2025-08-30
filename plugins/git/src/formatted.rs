use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{
    FormattedPlugin, MetaPlugin, RuntimeConfig, OutputContext, Status,
    TableOutput, output_format_arg
};
use serde_json;
use crate::{clone_repository_formatted, get_git_status_formatted, clone_missing_repos_formatted};

pub struct FormattedGitPlugin;

impl FormattedGitPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest git")
            .about("Git operations across multiple repositories");
        app.print_help()?;
        println!();
        Ok(())
    }
}

impl MetaPlugin for FormattedGitPlugin {
    fn name(&self) -> &str {
        "git"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("git")
                .visible_alias("g")
                .about("Git operations across multiple repositories")
                .disable_help_subcommand(true)
                .allow_external_subcommands(true)
                .subcommand(
                    Command::new("clone")
                        .visible_alias("c")
                        .about("Clone meta repository and all child repositories")
                        .arg(
                            Arg::new("url")
                                .value_name("REPO_URL")
                                .help("Repository URL to clone")
                                .required(true)
                        )
                        .arg(output_format_arg())
                )
                .subcommand(
                    Command::new("status")
                        .visible_aliases(["st", "s"])
                        .about("Show git status across all repositories")
                        .arg(output_format_arg())
                )
                .subcommand(
                    Command::new("update")
                        .visible_aliases(["up", "u"])
                        .about("Clone missing repositories")
                        .arg(output_format_arg())
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        <Self as FormattedPlugin>::handle_command(self, matches, config)
    }
}

impl FormattedPlugin for FormattedGitPlugin {
    fn formatted_commands(&self) -> Vec<&str> {
        vec!["clone", "status", "update"]
    }
    
    fn handle_formatted_command(
        &self,
        command: &str,
        matches: &ArgMatches,
        config: &RuntimeConfig,
        output: &mut dyn OutputContext,
    ) -> Result<()> {
        match command {
            "clone" => {
                let url = matches.get_one::<String>("url").unwrap();
                
                output.print_header("Cloning Meta Repository");
                output.print_status(Status::Info, &format!("Source: {}", url));
                
                // Extract repo name from URL for directory name
                let repo_name = url.split('/').last()
                    .unwrap_or("meta-repo")
                    .trim_end_matches(".git");
                
                let target_path = config.working_dir.join(repo_name);
                clone_repository_formatted(url, &target_path, output)?;
                
                // After cloning, look for .meta file and clone child repos
                let meta_file = target_path.join(".meta");
                if meta_file.exists() {
                    std::env::set_current_dir(&target_path)?;
                    output.print_section("Cloning Child Repositories");
                    clone_missing_repos_formatted(output)?;
                }
                
                Ok(())
            }
            "status" => {
                output.print_header("Git Status Across All Repositories");
                
                let mut table = TableOutput::new(vec![
                    "Repository".to_string(),
                    "Status".to_string(),
                    "Details".to_string(),
                ]);
                
                let mut all_statuses = Vec::new();
                
                // Show status for main repo
                let main_status = get_git_status_formatted(&config.working_dir)?;
                table.add_row(vec![
                    "Main".to_string(),
                    if main_status.is_clean { "Clean" } else { "Modified" }.to_string(),
                    main_status.summary.clone(),
                ]);
                
                all_statuses.push(serde_json::json!({
                    "project": ".",
                    "status": main_status
                }));
                
                // Show status for each project
                for (project_path, _repo_url) in &config.meta_config.projects {
                    let full_path = if config.meta_root().is_some() {
                        config.meta_root().unwrap().join(project_path)
                    } else {
                        config.working_dir.join(project_path)
                    };
                    
                    if full_path.exists() {
                        match get_git_status_formatted(&full_path) {
                            Ok(status) => {
                                table.add_row(vec![
                                    project_path.clone(),
                                    if status.is_clean { "Clean" } else { "Modified" }.to_string(),
                                    status.summary.clone(),
                                ]);
                                
                                all_statuses.push(serde_json::json!({
                                    "project": project_path,
                                    "status": status
                                }));
                            }
                            Err(e) => {
                                table.add_row(vec![
                                    project_path.clone(),
                                    "Error".to_string(),
                                    e.to_string(),
                                ]);
                                
                                all_statuses.push(serde_json::json!({
                                    "project": project_path,
                                    "error": e.to_string()
                                }));
                            }
                        }
                    } else {
                        table.add_row(vec![
                            project_path.clone(),
                            "Missing".to_string(),
                            "Directory not found".to_string(),
                        ]);
                        
                        all_statuses.push(serde_json::json!({
                            "project": project_path,
                            "status": "not_cloned"
                        }));
                    }
                }
                
                output.print_table(table);
                output.add_data("repositories", serde_json::Value::Array(all_statuses));
                
                Ok(())
            }
            "update" => {
                output.print_header("Cloning Missing Repositories");
                clone_missing_repos_formatted(output)?;
                Ok(())
            }
            _ => {
                // Handle unknown/external subcommands
                output.print_status(Status::Error, &format!("Unknown git subcommand: '{}'", command));
                self.show_help()
            }
        }
    }
    
    fn handle_unformatted_command(
        &self,
        command: &str,
        _matches: &ArgMatches,
        _config: &RuntimeConfig,
    ) -> Result<()> {
        // Handle external git commands that don't support formatting
        println!("Unknown git subcommand: '{}'", command);
        self.show_help()
    }
}

impl Default for FormattedGitPlugin {
    fn default() -> Self {
        Self::new()
    }
}