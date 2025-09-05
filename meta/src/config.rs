use anyhow::Result;
use metarepo_core::{MetaConfig, RuntimeConfig};

pub fn create_runtime_config(experimental: bool) -> Result<RuntimeConfig> {
    let working_dir = std::env::current_dir()?;
    let meta_file_path = MetaConfig::find_meta_file();
    
    let meta_config = if meta_file_path.is_some() {
        MetaConfig::load()?
    } else {
        MetaConfig::default()
    };
    
    Ok(RuntimeConfig {
        meta_config,
        working_dir,
        meta_file_path,
        experimental,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_default_config() {
        let config = MetaConfig::default();
        assert!(!config.ignore.is_empty());
        assert!(config.projects.is_empty());
        assert!(config.plugins.is_none());
    }
    
    #[test]
    fn test_config_serialization() {
        let mut config = MetaConfig::default();
        config.projects.insert("app1".to_string(), "https://github.com/user/app1.git".to_string());
        
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: MetaConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(config.projects, deserialized.projects);
    }
    
    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".meta");
        
        let mut config = MetaConfig::default();
        config.projects.insert("test".to_string(), "https://github.com/test/repo.git".to_string());
        
        // Save config
        config.save_to_file(&config_path).unwrap();
        assert!(config_path.exists());
        
        // Load config
        let loaded = MetaConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.projects, loaded.projects);
    }
}