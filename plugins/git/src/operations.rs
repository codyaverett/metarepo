use anyhow::Result;
use git2::Repository;
use std::path::Path;

pub fn get_git_status(repo_path: &Path) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let statuses = repo.statuses(None)?;
    
    if statuses.is_empty() {
        Ok("Clean working directory".to_string())
    } else {
        let mut status_lines = Vec::new();
        
        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let status = entry.status();
                let mut status_str = String::new();
                
                if status.is_wt_new() { status_str.push('?'); } else { status_str.push(' '); }
                if status.is_wt_modified() { status_str.push('M'); } else { status_str.push(' '); }
                if status.is_wt_deleted() { status_str.push('D'); } else { status_str.push(' '); }
                if status.is_index_new() { status_str.push('A'); } else { status_str.push(' '); }
                if status.is_index_modified() { status_str.push('M'); } else { status_str.push(' '); }
                if status.is_index_deleted() { status_str.push('D'); } else { status_str.push(' '); }
                
                status_lines.push(format!("{} {}", status_str, path));
            }
        }
        
        Ok(status_lines.join("\n"))
    }
}