use anyhow::Result;
use meta_core::MetaConfig;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub use crate::plugin::InitPlugin;

mod plugin;

fn create_default_config() -> MetaConfig {
    MetaConfig {
        ignore: vec![
            ".git".to_string(),
            ".vscode".to_string(),
            "node_modules".to_string(),
            "target".to_string(),
            ".DS_Store".to_string(),
        ],
        projects: HashMap::new(),
        plugins: None,
        nested: None,
    }
}

pub fn initialize_meta_repo<P: AsRef<Path>>(path: P) -> Result<()> {
    let meta_file_path = path.as_ref().join(".meta");

    // Check if .meta file already exists
    if meta_file_path.exists() {
        return Err(anyhow::anyhow!(
            "Meta repository already initialized (.meta file exists)"
        ));
    }

    // Create default configuration
    let config = create_default_config();

    // Write .meta file
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&meta_file_path, content)?;

    // Create or update .gitignore
    update_gitignore(&path)?;

    println!("Meta repository initialized successfully!");
    println!("Created .meta file with default configuration.");

    Ok(())
}

fn update_gitignore<P: AsRef<Path>>(path: P) -> Result<()> {
    let gitignore_path = path.as_ref().join(".gitignore");

    let mut existing_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Add meta-specific ignores if they don't exist
    let meta_ignores = vec![
        "# Meta repository ignores",
        ".DS_Store",
        "*.log",
        "node_modules/",
        "target/",
    ];

    let mut updated = false;
    for ignore_line in meta_ignores {
        if !existing_content.contains(ignore_line) {
            if !existing_content.ends_with('\n') && !existing_content.is_empty() {
                existing_content.push('\n');
            }
            existing_content.push_str(ignore_line);
            existing_content.push('\n');
            updated = true;
        }
    }

    if updated {
        fs::write(&gitignore_path, existing_content)?;
        println!("Updated .gitignore with meta repository patterns.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_initialize_meta_repo() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();

        // Initialize meta repo
        initialize_meta_repo(path).unwrap();

        // Check .meta file was created
        let meta_file = path.join(".meta");
        assert!(meta_file.exists());

        // Check .gitignore was created/updated
        let gitignore_file = path.join(".gitignore");
        assert!(gitignore_file.exists());

        // Verify .meta file content
        let content = fs::read_to_string(&meta_file).unwrap();
        let config: MetaConfig = serde_json::from_str(&content).unwrap();
        assert!(!config.ignore.is_empty());
        assert!(config.projects.is_empty());
    }

    #[test]
    fn test_initialize_existing_meta_repo() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();

        // Create existing .meta file
        let meta_file = path.join(".meta");
        fs::write(&meta_file, "{}").unwrap();

        // Try to initialize again
        let result = initialize_meta_repo(path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already initialized"));
    }

    #[test]
    fn test_update_gitignore() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path();

        // Create existing .gitignore with some content
        let gitignore_path = path.join(".gitignore");
        fs::write(&gitignore_path, "*.tmp\n").unwrap();

        // Update gitignore
        update_gitignore(path).unwrap();

        // Check content
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("*.tmp"));
        assert!(content.contains(".DS_Store"));
        assert!(content.contains("node_modules/"));
    }
}
