use anyhow::Result;
use meta_core::MetaConfig;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub mod plugin;

pub use plugin::ExecPlugin;

pub fn execute_command_in_directory<P: AsRef<Path>>(
    command: &str,
    args: &[&str],
    directory: P,
) -> Result<()> {
    let dir = directory.as_ref();
    println!("\n=== Executing in {} ===", dir.display());
    println!("Command: {} {}", command, args.join(" "));

    let mut cmd = Command::new(command);
    cmd.args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    // Read stdout in real-time
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            println!("{}", line?);
        }
    }

    // Wait for the process to complete
    let status = child.wait()?;

    if !status.success() {
        // Read stderr if command failed
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                eprintln!("ERROR: {}", line?);
            }
        }
        return Err(anyhow::anyhow!(
            "Command failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}

pub fn execute_in_all_projects(command: &str, args: &[&str]) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'gest init' first."))?;

    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    println!(
        "Executing '{}' across all repositories",
        format!("{} {}", command, args.join(" "))
    );

    // Execute in main repository first
    println!("\n=== Main Repository ===");
    if let Err(e) = execute_command_in_directory(command, args, base_path) {
        eprintln!("Failed in main repository: {}", e);
    }

    // Execute in each project
    for (project_path, _repo_url) in &config.projects {
        let full_path = base_path.join(project_path);

        if full_path.exists() {
            if let Err(e) = execute_command_in_directory(command, args, &full_path) {
                eprintln!("Failed in {}: {}", project_path, e);
                // Continue with other projects even if one fails
            }
        } else {
            println!("\n=== {} ===", project_path);
            println!("Project directory not found, skipping");
        }
    }

    println!("\n=== Execution Complete ===");
    Ok(())
}

pub fn execute_in_specific_projects(command: &str, args: &[&str], projects: &[&str]) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'gest init' first."))?;

    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();

    println!(
        "Executing '{}' in specified projects",
        format!("{} {}", command, args.join(" "))
    );

    for project_name in projects {
        if let Some(_repo_url) = config.projects.get(*project_name) {
            let full_path = base_path.join(project_name);

            if full_path.exists() {
                if let Err(e) = execute_command_in_directory(command, args, &full_path) {
                    eprintln!("Failed in {}: {}", project_name, e);
                }
            } else {
                println!("\n=== {} ===", project_name);
                println!("Project directory not found, skipping");
            }
        } else {
            eprintln!(
                "Project '{}' not found in .meta configuration",
                project_name
            );
        }
    }

    println!("\n=== Execution Complete ===");
    Ok(())
}
