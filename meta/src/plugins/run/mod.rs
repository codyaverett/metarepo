use crate::plugins::exec::ProjectIterator;
use crate::plugins::shared::{OutputManager, ProgressIndicator};
use anyhow::{Context, Result};
use colored::*;
use metarepo_core::{MetaConfig, ProjectEntry};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub use self::plugin::RunPlugin;

mod plugin;

/// Execute a script for selected projects
#[allow(clippy::too_many_arguments)]
pub fn run_script(
    script_name: &str,
    projects: &[String],
    base_path: &Path,
    current_project: Option<&str>,
    parallel: bool,
    existing_only: bool,
    git_only: bool,
    no_progress: bool,
    streaming: bool,
    env_vars: &HashMap<String, String>,
) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    // Determine which projects to operate on
    let mut selected_projects = if projects.is_empty() {
        // If no projects specified, check for current project context
        if let Some(current) = current_project {
            vec![current.to_string()]
        } else {
            // Run in all projects that have this script
            find_projects_with_script(&config, script_name)
        }
    } else if projects.len() == 1 && projects[0] == "--all" {
        // Run in all projects
        config.projects.keys().cloned().collect()
    } else {
        // Resolve project identifiers
        let mut selected = Vec::new();
        for project_id in projects {
            if let Some(project_name) = resolve_project_identifier(&config, project_id) {
                selected.push(project_name);
            } else {
                eprintln!("  {} Project '{}' not found", "⚠️".yellow(), project_id);
            }
        }
        selected
    };

    // Apply filters using ProjectIterator if needed
    if existing_only || git_only {
        let mut iterator = ProjectIterator::new(&config, base_path);

        if existing_only {
            iterator = iterator.filter_existing();
        }

        if git_only {
            iterator = iterator.filter_git_repos();
        }

        // Collect filtered project names
        let filtered_projects: Vec<String> =
            iterator.collect_all().into_iter().map(|p| p.name).collect();

        // Keep only selected projects that pass the filters
        selected_projects.retain(|p| filtered_projects.contains(p));
    }

    if selected_projects.is_empty() {
        println!(
            "  {} No projects selected or script not found",
            "ℹ".bright_black()
        );
        return Ok(());
    }

    println!(
        "\n  {} {}",
        "🚀".cyan(),
        format!(
            "Running '{}' in {} project(s)",
            script_name,
            selected_projects.len()
        )
        .bold()
    );
    println!("  {}", "═".repeat(60).bright_black());

    let mut success_count = 0;
    let mut failed = Vec::new();

    if parallel && selected_projects.len() > 1 && !streaming {
        // Use buffered output for parallel execution
        let output_manager = Arc::new(OutputManager::new(selected_projects.clone()));
        let mut progress_indicator =
            ProgressIndicator::new(Arc::clone(&output_manager), script_name.to_string());

        println!(
            "\n  {} {} [parallel mode]",
            "🚀".cyan(),
            format!(
                "Running '{}' in {} project(s)",
                script_name,
                selected_projects.len()
            )
            .bold()
        );

        if !no_progress {
            progress_indicator.start();
        }

        use std::thread;
        let mut handles = vec![];

        for project_name in selected_projects.clone() {
            let script_name = script_name.to_string();
            let base_path = base_path.to_path_buf();
            let config = config.clone();
            let env_vars = env_vars.clone();
            let project_name_clone = project_name.clone();
            let output_manager_clone = Arc::clone(&output_manager);

            let handle = thread::spawn(move || {
                output_manager_clone.start_project(&project_name_clone);

                match execute_script_in_project_buffered(
                    &script_name,
                    &project_name_clone,
                    &base_path,
                    &config,
                    &env_vars,
                ) {
                    Ok((exit_code, stdout, stderr, command)) => {
                        output_manager_clone.set_project_command(&project_name_clone, command);
                        output_manager_clone.complete_project(
                            &project_name_clone,
                            exit_code,
                            stdout,
                            stderr,
                        );
                    }
                    Err(e) => {
                        // Command execution failed (couldn't start process)
                        let error_msg = format!("Error: {}", e);
                        output_manager_clone.complete_project(
                            &project_name_clone,
                            -1,
                            Vec::new(),
                            error_msg.into_bytes(),
                        );
                    }
                }
            });
            handles.push((project_name, handle));
        }

        // Wait for all threads to complete
        for (project_name, handle) in handles {
            match handle.join() {
                Ok(()) => {
                    if let Some(output) = output_manager.get_project_output(&project_name) {
                        match output.status {
                            crate::plugins::shared::JobStatus::Completed => success_count += 1,
                            _ => failed.push(project_name),
                        }
                    }
                }
                Err(_) => failed.push(project_name),
            }
        }

        // Stop progress indicator and display results
        if !no_progress {
            progress_indicator.stop();
        } else {
            // Clear any partial output and show completion without progress
            print!("\r\x1b[K");
        }
        output_manager.display_final_results();

        return Ok(());
    } else {
        for project_name in &selected_projects {
            match execute_script_in_project(script_name, project_name, base_path, &config, env_vars)
            {
                Ok(_) => success_count += 1,
                Err(e) => {
                    eprintln!("     {} {}", "❌".red(), format!("Failed: {}", e).red());
                    failed.push(project_name.clone());
                }
            }
        }
    }

    println!("\n  {}", "─".repeat(60).bright_black());
    println!(
        "  {} {} scripts completed, {} failed",
        "Summary:".bright_black(),
        success_count.to_string().green(),
        if !failed.is_empty() {
            failed.len().to_string().red()
        } else {
            "0".bright_black()
        }
    );

    Ok(())
}

/// Execute a script in a specific project
fn execute_script_in_project(
    script_name: &str,
    project_name: &str,
    base_path: &Path,
    config: &MetaConfig,
    env_vars: &HashMap<String, String>,
) -> Result<()> {
    let project_path = base_path.join(project_name);

    if !project_path.exists() {
        return Err(anyhow::anyhow!(
            "Project directory '{}' not found",
            project_name
        ));
    }

    println!("\n  {} {}", "📦".blue(), project_name.bold());

    // Get the script command
    let scripts = config.get_all_scripts(Some(project_name));
    let script_cmd = scripts.get(script_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Script '{}' not found for project '{}'",
            script_name,
            project_name
        )
    })?;

    println!("     {} {}", "►".bright_black(), script_cmd.bright_white());

    // Parse the command (simple split by spaces - could be improved)
    let parts: Vec<&str> = script_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty script command"));
    }

    let mut cmd = Command::new(parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    cmd.current_dir(&project_path);

    // Add environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    // Add project-specific environment variables
    if let Some(ProjectEntry::Metadata(metadata)) = config.projects.get(project_name) {
        for (key, value) in &metadata.env {
            cmd.env(key, value);
        }
    }

    let output = cmd
        .output()
        .context(format!("Failed to execute script for {}", project_name))?;

    if output.status.success() {
        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        println!("     {} {}", "✅".green(), "Completed successfully".green());
    } else {
        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        return Err(anyhow::anyhow!(
            "Script failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}

/// Execute a script in a specific project with buffered output (for parallel execution)
fn execute_script_in_project_buffered(
    script_name: &str,
    project_name: &str,
    base_path: &Path,
    config: &MetaConfig,
    env_vars: &HashMap<String, String>,
) -> Result<(i32, Vec<u8>, Vec<u8>, String)> {
    let project_path = base_path.join(project_name);

    if !project_path.exists() {
        return Err(anyhow::anyhow!(
            "Project directory '{}' not found",
            project_name
        ));
    }

    // Get the script command
    let scripts = config.get_all_scripts(Some(project_name));
    let script_cmd = scripts.get(script_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Script '{}' not found for project '{}'",
            script_name,
            project_name
        )
    })?;

    // Parse the command (simple split by spaces - could be improved)
    let parts: Vec<&str> = script_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty script command"));
    }

    let mut cmd = Command::new(parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    cmd.current_dir(&project_path);

    // Add environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    // Add project-specific environment variables
    if let Some(ProjectEntry::Metadata(metadata)) = config.projects.get(project_name) {
        for (key, value) in &metadata.env {
            cmd.env(key, value);
        }
    }

    let output = cmd
        .output()
        .context(format!("Failed to execute script for {}", project_name))?;

    Ok((
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
        script_cmd.to_string(),
    ))
}

/// Find all projects that have a specific script defined
fn find_projects_with_script(config: &MetaConfig, script_name: &str) -> Vec<String> {
    let mut projects = Vec::new();

    // Check global scripts
    let has_global_script = config
        .scripts
        .as_ref()
        .map(|scripts| scripts.contains_key(script_name))
        .unwrap_or(false);

    for (project_name, entry) in &config.projects {
        // Check if project has this script or if there's a global script
        let has_script = match entry {
            ProjectEntry::Metadata(metadata) => metadata.scripts.contains_key(script_name),
            _ => false,
        } || has_global_script;

        if has_script {
            projects.push(project_name.clone());
        }
    }

    projects
}

/// Resolve a project identifier to its full name
fn resolve_project_identifier(config: &MetaConfig, identifier: &str) -> Option<String> {
    // First check if it's a full project name
    if config.project_exists(identifier) {
        return Some(identifier.to_string());
    }

    // Check global aliases
    if let Some(aliases) = &config.aliases {
        if let Some(project_path) = aliases.get(identifier) {
            return Some(project_path.clone());
        }
    }

    // Check project-specific aliases
    for (project_name, entry) in &config.projects {
        if let ProjectEntry::Metadata(metadata) = entry {
            if metadata.aliases.contains(&identifier.to_string()) {
                return Some(project_name.clone());
            }
        }
    }

    // Check if it's a basename match
    for project_name in config.projects.keys() {
        if let Some(basename) = std::path::Path::new(project_name).file_name() {
            if basename.to_string_lossy() == identifier {
                return Some(project_name.clone());
            }
        }
    }

    None
}

/// List all available scripts
pub fn list_scripts(base_path: &Path, project: Option<&str>) -> Result<()> {
    let meta_file_path = base_path.join(".meta");
    if !meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "No .meta file found. Run 'meta init' first."
        ));
    }

    let config = MetaConfig::load_from_file(&meta_file_path)?;

    println!("\n  {} {}", "📜".cyan(), "Available Scripts".bold());
    println!("  {}", "═".repeat(60).bright_black());

    // Show global scripts
    if let Some(global_scripts) = &config.scripts {
        if !global_scripts.is_empty() {
            println!("\n  {} {}", "🌍".blue(), "Global Scripts".bold());
            for (name, cmd) in global_scripts {
                println!(
                    "     {} {} {}",
                    name.bright_white(),
                    "→".bright_black(),
                    cmd.bright_black()
                );
            }
        }
    }

    // Show project-specific scripts
    if let Some(project_name) = project {
        if let Some(project_scripts) = config.get_project_scripts(project_name) {
            println!(
                "\n  {} {} {}",
                "📦".blue(),
                "Project Scripts".bold(),
                format!("({})", project_name).bright_black()
            );
            for (name, cmd) in project_scripts {
                println!(
                    "     {} {} {}",
                    name.bright_white(),
                    "→".bright_black(),
                    cmd.bright_black()
                );
            }
        }
    } else {
        // Show all project scripts
        for (project_name, entry) in &config.projects {
            if let ProjectEntry::Metadata(metadata) = entry {
                if !metadata.scripts.is_empty() {
                    println!(
                        "\n  {} {} {}",
                        "📦".blue(),
                        project_name.bold(),
                        "(project)".bright_black()
                    );
                    for (name, cmd) in &metadata.scripts {
                        println!(
                            "     {} {} {}",
                            name.bright_white(),
                            "→".bright_black(),
                            cmd.bright_black()
                        );
                    }
                }
            }
        }
    }

    println!();
    Ok(())
}
