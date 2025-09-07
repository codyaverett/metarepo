use anyhow::Result;
use metarepo_core::MetaConfig;
use std::path::Path;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

pub mod iterator;
pub mod plugin;

pub use plugin::ExecPlugin;
pub use iterator::{ProjectIterator, ProjectInfo};

pub fn execute_command_in_directory<P: AsRef<Path>>(
    command: &str, 
    args: &[&str], 
    directory: P
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
        return Err(anyhow::anyhow!("Command failed with exit code: {}", 
            status.code().unwrap_or(-1)));
    }
    
    Ok(())
}

pub fn execute_with_iterator(
    command: &str, 
    args: &[&str], 
    iterator: ProjectIterator,
    include_main: bool,
    parallel: bool,
) -> Result<()> {
    let projects: Vec<_> = iterator.collect();
    
    if projects.is_empty() && !include_main {
        println!("No projects matched the criteria");
        return Ok(());
    }
    
    let total = projects.len() + if include_main { 1 } else { 0 };
    println!("Executing command in {} project(s)", total);
    println!("Command: {} {}", command, args.join(" "));
    if parallel {
        println!("Mode: Parallel execution");
    }
    println!();
    
    // Execute in main repository if requested
    if include_main {
        let meta_file = MetaConfig::find_meta_file()
            .ok_or_else(|| anyhow::anyhow!("No .meta file found"))?;
        let base_path = meta_file.parent().unwrap();
        
        println!("=== Main Repository ===");
        if let Err(e) = execute_command_in_directory(command, args, base_path) {
            eprintln!("Failed in main repository: {}", e);
        }
    }
    
    // Execute in projects
    if parallel {
        use std::thread;
        let mut handles = vec![];
        
        for project in projects {
            let cmd = command.to_string();
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            
            let handle = thread::spawn(move || {
                println!("[{}] Starting...", project.name);
                if !project.exists {
                    println!("[{}] ⚠️  Directory does not exist, skipping", project.name);
                    return;
                }
                
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                if let Err(e) = execute_command_in_directory(&cmd, &args_refs, &project.path) {
                    eprintln!("[{}] Failed: {}", project.name, e);
                } else {
                    println!("[{}] ✅ Complete", project.name);
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
    } else {
        for (idx, project) in projects.iter().enumerate() {
            println!("[{}/{}] {}", idx + 1, projects.len(), project.name);
            
            if !project.exists {
                println!("  ⚠️  Directory does not exist, skipping");
                continue;
            }
            
            if let Err(e) = execute_command_in_directory(command, args, &project.path) {
                eprintln!("  ❌ Failed: {}", e);
            } else {
                println!("  ✅ Success");
            }
        }
    }
    
    println!("\n=== Execution Complete ===");
    Ok(())
}

pub fn execute_in_all_projects(command: &str, args: &[&str]) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
    
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    let iterator = ProjectIterator::new(&config, base_path);
    execute_with_iterator(command, args, iterator, true, false)
}

pub fn execute_in_specific_projects(command: &str, args: &[&str], projects: &[&str]) -> Result<()> {
    let meta_file = MetaConfig::find_meta_file()
        .ok_or_else(|| anyhow::anyhow!("No .meta file found. Run 'meta init' first."))?;
    
    let config = MetaConfig::load_from_file(&meta_file)?;
    let base_path = meta_file.parent().unwrap();
    
    println!("Executing '{}' in specified projects", 
        format!("{} {}", command, args.join(" ")));
    
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
            eprintln!("Project '{}' not found in .meta configuration", project_name);
        }
    }
    
    println!("\n=== Execution Complete ===");
    Ok(())
}