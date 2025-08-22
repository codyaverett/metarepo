pub mod plugin;
pub mod config;
pub mod cli;

pub use plugin::PluginRegistry;
pub use config::create_runtime_config;
pub use cli::MetaCli;
pub use meta_core::{MetaPlugin, MetaConfig, RuntimeConfig};

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    #[error("Git operation failed: {0}")]
    Git(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_meta_config_creation() {
        let config = MetaConfig::default();
        assert!(!config.ignore.is_empty());
        assert!(config.projects.is_empty());
    }
    
    #[test] 
    fn test_meta_config_file_operations() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join(".meta");
        
        let config = MetaConfig::default();
        config.save_to_file(&config_path).unwrap();
        
        assert!(config_path.exists());
        
        let loaded = MetaConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.ignore, loaded.ignore);
    }
    
    #[test]
    fn test_runtime_config_creation() {
        let config = create_runtime_config().unwrap();
        assert!(config.working_dir.exists());
    }
    
    #[test]
    fn test_plugin_registry() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.list_plugins().len(), 0);
    }
    
    #[test]
    fn test_cli_creation() {
        let cli = MetaCli::new();
        let app = cli.build_app();
        assert_eq!(app.get_name(), "meta");
    }
}