use anyhow::Result;
use git2::Repository;
use meta_core::{OutputFormat, TableOutput, OutputFormatter};
use serde_json;
use std::path::Path;

pub fn get_git_status(repo_path: &Path, output_format: OutputFormat) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let statuses = repo.statuses(None)?;
    
    if statuses.is_empty() {
        match output_format {
            OutputFormat::Human => Ok("Clean working directory".to_string()),
            OutputFormat::Ai => Ok("✓ **Status**: Clean working directory".to_string()),
            OutputFormat::Json => Ok(serde_json::json!({
                "status": "clean",
                "files": []
            }).to_string()),
        }
    } else {
        match output_format {
            OutputFormat::Human => {
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
            },
            OutputFormat::Ai => {
                let mut table = TableOutput::new(vec!["Status".to_string(), "File".to_string()]);
                
                for entry in statuses.iter() {
                    if let Some(path) = entry.path() {
                        let status = entry.status();
                        let status_desc = if status.is_wt_new() {
                            "Untracked"
                        } else if status.is_wt_modified() || status.is_index_modified() {
                            "Modified"
                        } else if status.is_wt_deleted() || status.is_index_deleted() {
                            "Deleted"
                        } else if status.is_index_new() {
                            "Added"
                        } else {
                            "Unknown"
                        };
                        
                        table.add_row(vec![status_desc.to_string(), path.to_string()]);
                    }
                }
                
                Ok(table.format(output_format))
            },
            OutputFormat::Json => {
                let mut files = Vec::new();
                
                for entry in statuses.iter() {
                    if let Some(path) = entry.path() {
                        let status = entry.status();
                        let status_desc = if status.is_wt_new() {
                            "untracked"
                        } else if status.is_wt_modified() || status.is_index_modified() {
                            "modified"
                        } else if status.is_wt_deleted() || status.is_index_deleted() {
                            "deleted"
                        } else if status.is_index_new() {
                            "added"
                        } else {
                            "unknown"
                        };
                        
                        files.push(serde_json::json!({
                            "path": path,
                            "status": status_desc
                        }));
                    }
                }
                
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "status": "dirty",
                    "files": files
                }))?)
            }
        }
    }
}