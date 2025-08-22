use anyhow::Result;
use clap::{ArgMatches, Command};

pub use crate::plugin::ExecPlugin;

mod plugin;

pub async fn execute_command_in_projects(
    command: &str,
    projects: Vec<std::path::PathBuf>,
    parallel: bool,
) -> Result<()> {
    if parallel {
        execute_parallel(command, projects).await
    } else {
        execute_sequential(command, projects).await
    }
}

async fn execute_sequential(command: &str, projects: Vec<std::path::PathBuf>) -> Result<()> {
    for project_path in projects {
        println!("Executing '{}' in {:?}", command, project_path);
        // TODO: Implement actual command execution
    }
    Ok(())
}

async fn execute_parallel(command: &str, projects: Vec<std::path::PathBuf>) -> Result<()> {
    let tasks: Vec<_> = projects
        .into_iter()
        .map(|project_path| {
            let cmd = command.to_string();
            tokio::spawn(async move {
                println!("Executing '{}' in {:?}", cmd, project_path);
                // TODO: Implement actual command execution
                Ok::<(), anyhow::Error>(())
            })
        })
        .collect();

    for task in tasks {
        task.await??;
    }
    
    Ok(())
}