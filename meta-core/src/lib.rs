use anyhow::Result;
use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trait that all meta plugins must implement
pub trait MetaPlugin: Send + Sync {
    /// Returns the plugin name (used for command routing)
    fn name(&self) -> &str;
    
    /// Register CLI commands for this plugin
    fn register_commands(&self, app: Command) -> Command;
    
    /// Handle a command for this plugin
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()>;
}

/// Runtime configuration available to all plugins
#[derive(Debug)]
pub struct RuntimeConfig {
    pub meta_config: MetaConfig,
    pub working_dir: PathBuf,
    pub meta_file_path: Option<PathBuf>,
}

impl RuntimeConfig {
    pub fn has_meta_file(&self) -> bool {
        self.meta_file_path.is_some()
    }
    
    pub fn meta_root(&self) -> Option<PathBuf> {
        self.meta_file_path.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf()))
    }
}

/// The .meta file configuration format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub projects: HashMap<String, String>, // path -> repo_url
    #[serde(default)]
    pub plugins: Option<HashMap<String, String>>, // name -> version/path
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            ignore: vec![
                ".git".to_string(),
                ".vscode".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
            ],
            projects: HashMap::new(),
            plugins: None,
        }
    }
}

impl MetaConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: MetaConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    pub fn find_meta_file() -> Option<PathBuf> {
        let mut current = std::env::current_dir().ok()?;
        
        loop {
            let meta_file = current.join(".meta");
            if meta_file.exists() {
                return Some(meta_file);
            }
            
            if !current.pop() {
                break;
            }
        }
        
        None
    }
    
    pub fn load() -> Result<Self> {
        if let Some(meta_file) = Self::find_meta_file() {
            Self::load_from_file(meta_file)
        } else {
            Err(anyhow::anyhow!("No .meta file found"))
        }
    }
}