use anyhow::Result;
use metarepo_core::{ConfigFormat, MetaConfig, NonInteractiveMode, RuntimeConfig};
use std::path::PathBuf;

pub fn create_runtime_config(experimental: bool) -> Result<RuntimeConfig> {
    create_runtime_config_with_flags(experimental, None)
}

pub fn create_runtime_config_with_flags(
    experimental: bool,
    non_interactive: Option<NonInteractiveMode>,
) -> Result<RuntimeConfig> {
    create_runtime_config_full(experimental, non_interactive, None, false, false)
}

/// Build the runtime config, allowing the caller to override config discovery
/// with an explicit file path (typically from `--config` or `METAREPO_CONFIG`).
#[allow(clippy::fn_params_excessive_bools)]
pub fn create_runtime_config_full(
    experimental: bool,
    non_interactive: Option<NonInteractiveMode>,
    config_override: Option<PathBuf>,
    scope_workspace: bool,
    discover_root: bool,
) -> Result<RuntimeConfig> {
    let working_dir = std::env::current_dir()?;

    let (meta_config, meta_file_path) = if let Some(path) = config_override {
        // Explicit override: load from this path verbatim. Format detection is
        // best-effort; an unrecognized extension falls back to JSON.
        let format = ConfigFormat::from_path(&path).unwrap_or(ConfigFormat::Json);
        let config = MetaConfig::load_from_file_with_format(&path, format)?;
        (config, Some(path))
    } else {
        // `--root` resolves the outermost enclosing metarepo; otherwise the
        // nearest one wins.
        let discovered = if discover_root {
            MetaConfig::discover_topmost_from(&working_dir)
        } else {
            MetaConfig::discover_from(&working_dir)
        };
        match discovered {
            Ok(Some(found)) => {
                let config = MetaConfig::load_from_file_with_format(&found.path, found.format)?;
                (config, Some(found.path))
            }
            Ok(None) => (MetaConfig::default(), None),
            Err(e) => {
                // Surface the structured error verbatim — its Display impl
                // already prints the list of conflicting files and the fix.
                return Err(anyhow::anyhow!("{}", e));
            }
        }
    };

    Ok(RuntimeConfig {
        meta_config,
        working_dir,
        meta_file_path,
        experimental,
        non_interactive,
        scope_workspace,
        // Populated by the CLI after the plugin registry is available.
        settings_catalog: Vec::new(),
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
        use metarepo_core::ProjectEntry;
        let mut config = MetaConfig::default();
        config.projects.insert(
            "app1".to_string(),
            ProjectEntry::Url("https://github.com/user/app1.git".to_string()),
        );

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: MetaConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.projects, deserialized.projects);
    }

    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".meta");

        use metarepo_core::ProjectEntry;
        let mut config = MetaConfig::default();
        config.projects.insert(
            "test".to_string(),
            ProjectEntry::Url("https://github.com/test/repo.git".to_string()),
        );

        // Save config
        config.save_to_file(&config_path).unwrap();
        assert!(config_path.exists());

        // Load config
        let loaded = MetaConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.projects, loaded.projects);
    }
}
