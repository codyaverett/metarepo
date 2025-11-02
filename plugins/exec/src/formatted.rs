use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{
    FormattedPlugin, MetaPlugin, RuntimeConfig, OutputContext, Status,
    TableOutput, output_format_arg
};
use serde::Serialize;

/// ExecPlugin implementation using FormattedPlugin trait
pub struct FormattedExecPlugin;

impl FormattedExecPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest exec")
            .about("Execute commands across multiple repositories");
        app.print_help()?;
        println!();
        Ok(())
    }
}

#[derive(Serialize)]
struct ExecutionResult {
    project: String,
    command: String,
    success: bool,
    output: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ExecutionSummary {
    command: String,
    total_projects: usize,
    successful: usize,
    failed: usize,
    results: Vec<ExecutionResult>,
}

impl MetaPlugin for FormattedExecPlugin {
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
                .arg(output_format_arg())
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Since exec uses external subcommands, we need custom handling
        let output_format = self.get_output_format(matches);
        let mut context = meta_core::OutputContextImpl::new(output_format);
        
        // For exec, the command is the external subcommand
        self.handle_formatted_command("", matches, config, &mut context)?;
        context.flush();
        Ok(())
    }
}

impl FormattedPlugin for FormattedExecPlugin {
    fn formatted_commands(&self) -> Vec<&str> {
        // Empty because exec uses external subcommands
        vec![]
    }
    
    fn handle_formatted_command(
        &self,
        _command: &str,
        matches: &ArgMatches,
        config: &RuntimeConfig,
        output: &mut dyn OutputContext,
    ) -> Result<()> {
        // Get the actual command from subcommand matches
        match matches.subcommand() {
            Some((command, sub_matches)) => {
                // Parse remaining arguments
                let args: Vec<&str> = match sub_matches.get_many::<std::ffi::OsString>("") {
                    Some(os_args) => os_args.map(|s| s.to_str().unwrap_or("")).collect(),
                    None => Vec::new()
                };
                
                let full_command = if args.is_empty() {
                    command.to_string()
                } else {
                    format!("{} {}", command, args.join(" "))
                };
                
                output.print_header(&format!("Executing: {}", full_command));
                
                // Track results for summary
                let mut results = Vec::new();
                let mut success_count = 0;
                let mut fail_count = 0;
                
                // Check if specific projects were specified
                let project_list: Vec<String> = if let Some(projects) = matches.get_many::<String>("projects") {
                    projects.map(|s| s.to_string()).collect()
                } else {
                    // Get all projects from config
                    config.meta_config.projects.keys().cloned().collect()
                };
                
                let total_projects = project_list.len();
                
                // Build a table for results
                let mut table = TableOutput::new(vec![
                    "Project".to_string(),
                    "Status".to_string(),
                    "Output".to_string(),
                ]);
                
                for project in &project_list {
                    output.print_section(&format!("Project: {}", project));
                    
                    // Here we would actually execute the command
                    // For now, this is a mock implementation
                    let (success, output_text) = execute_command_in_project(project, command, &args)?;
                    
                    if success {
                        output.print_status(Status::Success, &format!("{}: Command completed successfully", project));
                        success_count += 1;
                        
                        table.add_row(vec![
                            project.clone(),
                            "✓ Success".to_string(),
                            output_text.clone().unwrap_or_default(),
                        ]);
                        
                        results.push(ExecutionResult {
                            project: project.clone(),
                            command: full_command.clone(),
                            success: true,
                            output: Some(output_text.unwrap_or_default()),
                            error: None,
                        });
                    } else {
                        output.print_status(Status::Error, &format!("{}: Command failed", project));
                        fail_count += 1;
                        
                        table.add_row(vec![
                            project.clone(),
                            "✗ Failed".to_string(),
                            output_text.clone().unwrap_or_else(|| "Error".to_string()),
                        ]);
                        
                        results.push(ExecutionResult {
                            project: project.clone(),
                            command: full_command.clone(),
                            success: false,
                            output: None,
                            error: output_text,
                        });
                    }
                }
                
                // Print summary
                output.print_section("Summary");
                output.print_table(table);
                
                let summary_msg = format!(
                    "Executed on {} projects: {} successful, {} failed",
                    total_projects, success_count, fail_count
                );
                
                if fail_count == 0 {
                    output.print_status(Status::Success, &summary_msg);
                } else if success_count == 0 {
                    output.print_status(Status::Error, &summary_msg);
                } else {
                    output.print_status(Status::Warning, &summary_msg);
                }
                
                // Add structured data for JSON output
                let summary = ExecutionSummary {
                    command: full_command,
                    total_projects,
                    successful: success_count,
                    failed: fail_count,
                    results,
                };
                
                output.add_data("summary", serde_json::to_value(summary)?);
                
                Ok(())
            }
            None => self.show_help()
        }
    }
    
    fn handle_unformatted_command(
        &self,
        _command: &str,
        _matches: &ArgMatches,
        _config: &RuntimeConfig,
    ) -> Result<()> {
        // For exec plugin, all commands support formatting
        Ok(())
    }
}

// Mock implementation - replace with actual execution logic
fn execute_command_in_project(project: &str, command: &str, args: &[&str]) -> Result<(bool, Option<String>)> {
    // This would actually run the command in the project directory
    // For now, return mock results
    
    // Simulate some successes and failures
    if project.contains("test") {
        Ok((false, Some("Mock error: command not found".to_string())))
    } else {
        Ok((true, Some(format!("Mock output from {} {}", command, args.join(" ")))))
    }
}

impl Default for FormattedExecPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_formatted_commands() {
        let plugin = FormattedExecPlugin::new();
        // Empty because exec uses external subcommands
        assert_eq!(plugin.formatted_commands(), Vec::<&str>::new());
        assert!(<FormattedExecPlugin as FormattedPlugin>::supports_output_format(&plugin));
    }
    
    #[test]
    fn test_execution_summary_serialization() {
        let summary = ExecutionSummary {
            command: "test command".to_string(),
            total_projects: 2,
            successful: 1,
            failed: 1,
            results: vec![
                ExecutionResult {
                    project: "project1".to_string(),
                    command: "test".to_string(),
                    success: true,
                    output: Some("output".to_string()),
                    error: None,
                },
                ExecutionResult {
                    project: "project2".to_string(),
                    command: "test".to_string(),
                    success: false,
                    output: None,
                    error: Some("error".to_string()),
                },
            ],
        };
        
        let json = serde_json::to_value(summary).unwrap();
        assert!(json.is_object());
        assert_eq!(json["total_projects"], 2);
    }
}