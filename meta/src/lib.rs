pub mod cli;
pub mod config;
pub mod plugin;
pub mod plugins;

pub use cli::MetarepoCli;
pub use config::{create_runtime_config, create_runtime_config_with_flags};
pub use metarepo_core::{MetaConfig, MetaPlugin, NonInteractiveMode, RuntimeConfig};
pub use plugin::PluginRegistry;

#[derive(Debug, thiserror::Error)]
pub enum MetarepoError {
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
        let config = create_runtime_config(false).unwrap();
        assert!(config.working_dir.exists());
        assert!(!config.experimental);

        let experimental_config = create_runtime_config(true).unwrap();
        assert!(experimental_config.experimental);
    }

    #[test]
    fn test_plugin_registry() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.list_plugins().len(), 0);
    }

    #[test]
    fn test_cli_creation() {
        let cli = MetarepoCli::new();
        let app = cli.build_app();
        assert_eq!(app.get_name(), "meta");
    }
}
