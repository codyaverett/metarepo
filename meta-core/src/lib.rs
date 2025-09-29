use anyhow::Result;
use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// New plugin system modules
mod plugin_base;
mod plugin_builder;
mod plugin_manifest;

pub use plugin_base::{
    BasePlugin, PluginMetadata, HelpFormat, HelpFormatter,
    TerminalHelpFormatter, JsonHelpFormatter, YamlHelpFormatter, MarkdownHelpFormatter,
    CommandInfo, ArgumentInfo,
};
pub use plugin_builder::{
    PluginBuilder, BuiltPlugin, CommandBuilder, ArgBuilder,
    plugin, command, arg,
};
pub use plugin_manifest::{
    PluginManifest, PluginInfo, ManifestCommand, ManifestArg,
    ArgValueType, Example, PluginConfig, ExecutionConfig, Dependency,
};

/// Trait that all meta plugins must implement
pub trait MetaPlugin: Send + Sync {
    /// Returns the plugin name (used for command routing)
    fn name(&self) -> &str;
    
    /// Register CLI commands for this plugin
    fn register_commands(&self, app: Command) -> Command;
    
    /// Handle a command for this plugin
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()>;
    
    /// Returns true if this plugin is experimental (default: false)
    fn is_experimental(&self) -> bool {
        false
    }
}

/// Runtime configuration available to all plugins
#[derive(Debug)]
pub struct RuntimeConfig {
    pub meta_config: MetaConfig,
    pub working_dir: PathBuf,
    pub meta_file_path: Option<PathBuf>,
    pub experimental: bool,
}

impl RuntimeConfig {
    pub fn has_meta_file(&self) -> bool {
        self.meta_file_path.is_some()
    }
    
    pub fn meta_root(&self) -> Option<PathBuf> {
        self.meta_file_path.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf()))
    }
    
    pub fn is_experimental(&self) -> bool {
        self.experimental
    }
    
    /// Detect if we're currently inside a project directory and return its name
    pub fn current_project(&self) -> Option<String> {
        let meta_root = self.meta_root()?;
        let cwd = &self.working_dir;
        
        // Check if we're inside the meta root
        if !cwd.starts_with(&meta_root) {
            return None;
        }
        
        // Get relative path from meta root
        let _relative = cwd.strip_prefix(&meta_root).ok()?;
        
        // Check each project to see if we're inside it
        for (project_name, _) in &self.meta_config.projects {
            let project_path = meta_root.join(project_name);
            if cwd.starts_with(&project_path) {
                return Some(project_name.clone());
            }
        }
        
        None
    }
    
    /// Resolve a project identifier (could be full name, basename, or alias)
    pub fn resolve_project(&self, identifier: &str) -> Option<String> {
        // First, check if it's a full project name
        if self.meta_config.projects.contains_key(identifier) {
            return Some(identifier.to_string());
        }
        
        // Check global aliases
        if let Some(aliases) = &self.meta_config.aliases {
            if let Some(project_path) = aliases.get(identifier) {
                return Some(project_path.clone());
            }
        }
        
        // Check project-specific aliases
        for (project_name, entry) in &self.meta_config.projects {
            if let ProjectEntry::Metadata(metadata) = entry {
                if metadata.aliases.contains(&identifier.to_string()) {
                    return Some(project_name.clone());
                }
            }
        }
        
        // Check if it's a basename match
        for project_name in self.meta_config.projects.keys() {
            if let Some(basename) = std::path::Path::new(project_name).file_name() {
                if basename.to_string_lossy() == identifier {
                    return Some(project_name.clone());
                }
            }
        }
        
        None
    }
    
    /// Get all valid identifiers for a project (full name, basename, aliases)
    pub fn project_identifiers(&self, project_name: &str) -> Vec<String> {
        let mut identifiers = vec![project_name.to_string()];
        
        // Add basename if different from full name
        if let Some(basename) = std::path::Path::new(project_name).file_name() {
            let basename_str = basename.to_string_lossy().to_string();
            if basename_str != project_name {
                identifiers.push(basename_str);
            }
        }
        
        // TODO: Add custom aliases when implemented
        
        identifiers
    }
}

/// Configuration for nested repository handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedConfig {
    #[serde(default = "default_recursive_import")]
    pub recursive_import: bool,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub flatten: bool,
    #[serde(default = "default_cycle_detection")]
    pub cycle_detection: bool,
    #[serde(default)]
    pub ignore_nested: Vec<String>,
    #[serde(default)]
    pub namespace_separator: Option<String>,
    #[serde(default)]
    pub preserve_structure: bool,
}

fn default_recursive_import() -> bool { false }
fn default_max_depth() -> usize { 3 }
fn default_cycle_detection() -> bool { true }

impl Default for NestedConfig {
    fn default() -> Self {
        Self {
            recursive_import: default_recursive_import(),
            max_depth: default_max_depth(),
            flatten: false,
            cycle_detection: default_cycle_detection(),
            ignore_nested: Vec::new(),
            namespace_separator: None,
            preserve_structure: false,
        }
    }
}

/// Project metadata including scripts and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectEntry {
    /// Simple string format (backwards compatible)
    Url(String),
    /// Full metadata format with scripts
    Metadata(ProjectMetadata),
}

/// Detailed project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub url: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// The .meta file configuration format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectEntry>, // Now supports both String and ProjectMetadata
    #[serde(default)]
    pub plugins: Option<HashMap<String, String>>, // name -> version/path
    #[serde(default)]
    pub nested: Option<NestedConfig>, // nested repository configuration
    #[serde(default)]
    pub aliases: Option<HashMap<String, String>>, // Global aliases: alias -> project_path
    #[serde(default)]
    pub scripts: Option<HashMap<String, String>>, // Global scripts
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
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
            aliases: None,
            scripts: None,
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
    
    /// Get the URL for a project (handles both string and metadata formats)
    pub fn get_project_url(&self, project_name: &str) -> Option<String> {
        self.projects.get(project_name).map(|entry| match entry {
            ProjectEntry::Url(url) => url.clone(),
            ProjectEntry::Metadata(metadata) => metadata.url.clone(),
        })
    }
    
    /// Get scripts for a specific project
    pub fn get_project_scripts(&self, project_name: &str) -> Option<HashMap<String, String>> {
        self.projects.get(project_name).and_then(|entry| match entry {
            ProjectEntry::Url(_) => None,
            ProjectEntry::Metadata(metadata) => {
                if metadata.scripts.is_empty() {
                    None
                } else {
                    Some(metadata.scripts.clone())
                }
            }
        })
    }
    
    /// Get all available scripts (project-specific and global)
    pub fn get_all_scripts(&self, project_name: Option<&str>) -> HashMap<String, String> {
        let mut scripts = HashMap::new();
        
        // Add global scripts first
        if let Some(global_scripts) = &self.scripts {
            scripts.extend(global_scripts.clone());
        }
        
        // Add project-specific scripts (overrides global)
        if let Some(project) = project_name {
            if let Some(project_scripts) = self.get_project_scripts(project) {
                scripts.extend(project_scripts);
            }
        }
        
        scripts
    }
    
    /// Check if a project exists (for backwards compatibility)
    pub fn project_exists(&self, project_name: &str) -> bool {
        self.projects.contains_key(project_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_meta_config_default() {
        let config = MetaConfig::default();
        assert_eq!(config.ignore.len(), 4);
        assert!(config.ignore.contains(&".git".to_string()));
        assert!(config.ignore.contains(&".vscode".to_string()));
        assert!(config.ignore.contains(&"node_modules".to_string()));
        assert!(config.ignore.contains(&"target".to_string()));
        assert!(config.projects.is_empty());
        assert!(config.plugins.is_none());
        assert!(config.nested.is_none());
    }
    
    #[test]
    fn test_meta_config_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join(".meta");
        
        // Create a config with some data
        let mut config = MetaConfig::default();
        config.projects.insert("project1".to_string(), ProjectEntry::Url("https://github.com/user/repo.git".to_string()));
        config.projects.insert("project2".to_string(), ProjectEntry::Url("https://github.com/user/repo2.git".to_string()));
        
        // Save the config
        config.save_to_file(&meta_file).unwrap();
        
        // Load the config back
        let loaded_config = MetaConfig::load_from_file(&meta_file).unwrap();
        
        // Verify the loaded config matches
        assert_eq!(loaded_config.projects.len(), 2);
        assert_eq!(loaded_config.projects.get("project1"), Some(&ProjectEntry::Url("https://github.com/user/repo.git".to_string())));
        assert_eq!(loaded_config.projects.get("project2"), Some(&ProjectEntry::Url("https://github.com/user/repo2.git".to_string())));
        assert_eq!(loaded_config.ignore, config.ignore);
    }
    
    #[test]
    fn test_meta_config_with_nested() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join(".meta");
        
        // Create a config with nested configuration
        let mut config = MetaConfig::default();
        config.nested = Some(NestedConfig {
            recursive_import: true,
            max_depth: 5,
            flatten: true,
            cycle_detection: false,
            ignore_nested: vec!["ignored-project".to_string()],
            namespace_separator: Some("::".to_string()),
            preserve_structure: true,
        });
        
        // Save and load
        config.save_to_file(&meta_file).unwrap();
        let loaded_config = MetaConfig::load_from_file(&meta_file).unwrap();
        
        // Verify nested configuration
        assert!(loaded_config.nested.is_some());
        let nested = loaded_config.nested.unwrap();
        assert_eq!(nested.recursive_import, true);
        assert_eq!(nested.max_depth, 5);
        assert_eq!(nested.flatten, true);
        assert_eq!(nested.cycle_detection, false);
        assert_eq!(nested.ignore_nested, vec!["ignored-project".to_string()]);
        assert_eq!(nested.namespace_separator, Some("::".to_string()));
        assert_eq!(nested.preserve_structure, true);
    }
    
    #[test]
    fn test_find_meta_file() {
        let temp_dir = tempdir().unwrap();
        let nested_dir = temp_dir.path().join("nested").join("deep");
        fs::create_dir_all(&nested_dir).unwrap();
        
        // Create .meta file in temp_dir
        let meta_file = temp_dir.path().join(".meta");
        let config = MetaConfig::default();
        config.save_to_file(&meta_file).unwrap();
        
        // Change to nested directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested_dir).unwrap();
        
        // Find meta file should traverse up
        let found_file = MetaConfig::find_meta_file();
        assert!(found_file.is_some());
        // Compare canonical paths to handle symlinks like /private/var vs /var on macOS
        assert_eq!(found_file.unwrap().canonicalize().unwrap(), meta_file.canonicalize().unwrap());
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }
    
    #[test]
    fn test_nested_config_default() {
        let nested = NestedConfig::default();
        assert_eq!(nested.recursive_import, false);
        assert_eq!(nested.max_depth, 3);
        assert_eq!(nested.flatten, false);
        assert_eq!(nested.cycle_detection, true);
        assert!(nested.ignore_nested.is_empty());
        assert!(nested.namespace_separator.is_none());
        assert_eq!(nested.preserve_structure, false);
    }
    
    #[test]
    fn test_runtime_config_has_meta_file() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join(".meta");
        
        let config_with_meta = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: temp_dir.path().to_path_buf(),
            meta_file_path: Some(meta_file.clone()),
            experimental: false,
        };
        
        let config_without_meta = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: temp_dir.path().to_path_buf(),
            meta_file_path: None,
            experimental: false,
        };
        
        assert!(config_with_meta.has_meta_file());
        assert!(!config_without_meta.has_meta_file());
    }
    
    #[test]
    fn test_runtime_config_meta_root() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join("subdir").join(".meta");
        fs::create_dir_all(meta_file.parent().unwrap()).unwrap();
        
        let config = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: temp_dir.path().to_path_buf(),
            meta_file_path: Some(meta_file.clone()),
            experimental: false,
        };
        
        assert_eq!(config.meta_root(), Some(temp_dir.path().join("subdir")));
    }
    
    #[test]
    fn test_load_invalid_json() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join(".meta");
        
        // Write invalid JSON
        fs::write(&meta_file, "{ invalid json }").unwrap();
        
        // Should return an error
        let result = MetaConfig::load_from_file(&meta_file);
        assert!(result.is_err());
    }
}