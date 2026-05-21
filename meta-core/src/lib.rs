use anyhow::Result;
use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// New plugin system modules
pub mod config_format;
pub mod interactive;
mod plugin_base;
mod plugin_builder;
mod plugin_manifest;
pub mod protocol;
pub mod security;
pub mod tui;

pub use config_format::{ConfigFormat, CANONICAL_FILENAME, KNOWN_FILENAMES, LEGACY_FILENAME};
pub use interactive::{
    is_interactive, prompt_confirm, prompt_multiselect, prompt_select, prompt_text, prompt_url,
    NonInteractiveMode,
};
pub use plugin_base::{
    ArgumentInfo, BasePlugin, CommandInfo, HelpFormat, HelpFormatter, JsonHelpFormatter,
    MarkdownHelpFormatter, PluginMetadata, TerminalHelpFormatter, YamlHelpFormatter,
};
pub use plugin_builder::{
    arg, command, plugin, ArgBuilder, BuiltPlugin, CommandBuilder, PluginBuilder,
};
pub use plugin_manifest::{
    ArgValueType, Dependency, Example, ExecutionConfig, ManifestArg, ManifestCommand, PluginConfig,
    PluginInfo, PluginManifest, MANIFEST_FILENAMES,
};
pub use security::{
    canonicalize_creatable, ensure_within_base, is_dangerous_env_var, is_supported_git_url,
    is_unencrypted_git_scheme, validate_path_segment, validate_project_url, DANGEROUS_ENV_VARS,
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
    pub non_interactive: Option<NonInteractiveMode>,
}

impl RuntimeConfig {
    pub fn has_meta_file(&self) -> bool {
        self.meta_file_path.is_some()
    }

    pub fn meta_root(&self) -> Option<PathBuf> {
        self.meta_file_path
            .as_ref()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
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
        for project_name in self.meta_config.projects.keys() {
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

fn default_recursive_import() -> bool {
    false
}
fn default_max_depth() -> usize {
    3
}
fn default_cycle_detection() -> bool {
    true
}

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectEntry {
    /// Simple string format (backwards compatible)
    Url(String),
    /// Full metadata format with scripts
    Metadata(ProjectMetadata),
}

/// Detailed project metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub url: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub scripts: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub worktree_init: Option<String>,
    #[serde(default)]
    pub bare: Option<bool>,
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
    #[serde(default)]
    pub worktree_init: Option<String>, // Global worktree post-create command
    #[serde(default)]
    pub default_bare: Option<bool>, // Global default for bare repository clones
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
            worktree_init: None,
            default_bare: None,
        }
    }
}

/// A located metarepo config file along with its detected format. Returned by
/// [`MetaConfig::discover`] and consumable directly by [`MetaConfig::load_from_file`].
#[derive(Debug, Clone)]
pub struct DiscoveredConfig {
    pub path: PathBuf,
    pub format: ConfigFormat,
}

/// Errors surfaced by [`MetaConfig::discover`]. Separated out as its own enum
/// so the CLI can render a tailored message for the multi-file case.
#[derive(Debug)]
pub enum ConfigDiscoveryError {
    /// Two or more recognized config files coexist in the same directory.
    /// Returned with the directory and the conflicting paths so the caller
    /// can print all of them.
    Multiple { dir: PathBuf, files: Vec<PathBuf> },
    /// Anything else that went wrong while walking up the tree.
    Io(std::io::Error),
}

impl std::fmt::Display for ConfigDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigDiscoveryError::Multiple { dir, files } => {
                writeln!(
                    f,
                    "multiple metarepo config files found in {}:",
                    dir.display()
                )?;
                for p in files {
                    writeln!(f, "  - {}", p.display())?;
                }
                write!(
                    f,
                    "Pick one of: pass --config <path>, run `meta config migrate` to consolidate, or remove the unwanted file(s)."
                )
            }
            ConfigDiscoveryError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ConfigDiscoveryError {}

impl From<std::io::Error> for ConfigDiscoveryError {
    fn from(e: std::io::Error) -> Self {
        ConfigDiscoveryError::Io(e)
    }
}

impl MetaConfig {
    /// Read a config file from disk. Format is detected from the path's
    /// filename/extension; unrecognized names are rejected so callers don't
    /// accidentally try to parse, say, `package.json` as a metarepo config.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path).ok_or_else(|| {
            anyhow::anyhow!(
                "Unrecognized config filename: {}. Expected one of: {}",
                path.display(),
                KNOWN_FILENAMES.join(", ")
            )
        })?;
        Self::load_from_file_with_format(path, format)
    }

    /// Read a config file when the caller already knows the format (e.g., the
    /// path was supplied via `--config` and is non-standard).
    pub fn load_from_file_with_format<P: AsRef<Path>>(
        path: P,
        format: ConfigFormat,
    ) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let mut config: MetaConfig = config_format::deserialize_from_str(&content, format)?;
        config.sanitize_after_load();
        Ok(config)
    }

    /// Apply security-driven sanitization after deserialization:
    /// - drop project entries whose key contains path traversal / null bytes / absolute paths
    /// - drop dangerous env vars from each ProjectMetadata
    ///
    /// Emits warnings to stderr for any sanitized entries so committers see them.
    fn sanitize_after_load(&mut self) {
        let bad_keys: Vec<String> = self
            .projects
            .keys()
            .filter(|k| security::validate_path_segment("project key", k).is_err())
            .cloned()
            .collect();
        for k in bad_keys {
            eprintln!(
                "warning: dropping project '{}' from config (invalid path segment: traversal, null, or absolute path)",
                k
            );
            self.projects.remove(&k);
        }

        for (project, entry) in self.projects.iter_mut() {
            if let ProjectEntry::Metadata(metadata) = entry {
                let dangerous: Vec<String> = metadata
                    .env
                    .keys()
                    .filter(|k| security::is_dangerous_env_var(k))
                    .cloned()
                    .collect();
                for k in dangerous {
                    eprintln!(
                        "warning: ignoring env var '{}' for project '{}' (known to subvert subprocesses)",
                        k, project
                    );
                    metadata.env.remove(&k);
                }
            }
        }
    }

    /// Write the config to disk, choosing the on-wire format from the path's
    /// filename/extension. Unrecognized paths default to JSON so that legacy
    /// callers that pass arbitrary paths still get a sensible serialization.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path).unwrap_or(ConfigFormat::Json);
        self.save_to_file_with_format(path, format)
    }

    /// Write the config to disk in an explicit format.
    pub fn save_to_file_with_format<P: AsRef<Path>>(
        &self,
        path: P,
        format: ConfigFormat,
    ) -> Result<()> {
        let content = config_format::serialize_to_string(self, format)?;
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }

    /// Walk up from `start` looking for any recognized metarepo config file.
    ///
    /// Returns:
    /// - `Ok(Some(_))` when exactly one is found at the closest level.
    /// - `Ok(None)` when no config exists in any ancestor.
    /// - `Err(ConfigDiscoveryError::Multiple { .. })` when two or more
    ///   recognized files coexist in the same directory — we never silently
    ///   pick one. The CLI surfaces this with a tailored message and points
    ///   the user at `--config` or `meta config migrate`.
    pub fn discover_from(
        start: &Path,
    ) -> std::result::Result<Option<DiscoveredConfig>, ConfigDiscoveryError> {
        let mut current = start.to_path_buf();
        loop {
            let mut found: Vec<PathBuf> = Vec::new();
            for name in KNOWN_FILENAMES {
                let candidate = current.join(name);
                if candidate.exists() {
                    found.push(candidate);
                }
            }
            match found.len() {
                0 => {
                    if !current.pop() {
                        return Ok(None);
                    }
                }
                1 => {
                    let path = found.into_iter().next().unwrap();
                    let format = ConfigFormat::from_path(&path).unwrap_or(ConfigFormat::Json);
                    return Ok(Some(DiscoveredConfig { path, format }));
                }
                _ => {
                    return Err(ConfigDiscoveryError::Multiple {
                        dir: current,
                        files: found,
                    });
                }
            }
        }
    }

    /// Convenience wrapper around `discover_from` that starts at the current
    /// working directory. Returns just the path to keep older call sites that
    /// only need the location backwards-compatible.
    pub fn find_meta_file() -> Option<PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        // Multi-file errors are flattened to None here — callers that need
        // structured handling should call discover_from directly.
        Self::discover_from(&cwd).ok().flatten().map(|d| d.path)
    }

    pub fn load() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        match Self::discover_from(&cwd) {
            Ok(Some(found)) => Self::load_from_file_with_format(&found.path, found.format),
            Ok(None) => Err(anyhow::anyhow!(
                "No metarepo config found (looked for: {})",
                KNOWN_FILENAMES.join(", ")
            )),
            Err(e) => Err(anyhow::anyhow!("{}", e)),
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
        self.projects
            .get(project_name)
            .and_then(|entry| match entry {
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

    /// Get the worktree_init command for a project (project-level overrides global)
    pub fn get_worktree_init(&self, project_name: &str) -> Option<String> {
        // Check project-level first
        if let Some(ProjectEntry::Metadata(metadata)) = self.projects.get(project_name) {
            if let Some(worktree_init) = &metadata.worktree_init {
                return Some(worktree_init.clone());
            }
        }

        // Fall back to global
        self.worktree_init.clone()
    }

    /// Get whether a project should use bare repository
    pub fn is_bare_repo(&self, project_name: &str) -> bool {
        if let Some(ProjectEntry::Metadata(metadata)) = self.projects.get(project_name) {
            return metadata.bare.unwrap_or(false);
        }
        false
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
        assert_eq!(config.ignore.len(), 5);
        assert!(config.ignore.contains(&".git".to_string()));
        assert!(config.ignore.contains(&".vscode".to_string()));
        assert!(config.ignore.contains(&"node_modules".to_string()));
        assert!(config.ignore.contains(&"target".to_string()));
        assert!(config.ignore.contains(&".DS_Store".to_string()));
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
        config.projects.insert(
            "project1".to_string(),
            ProjectEntry::Url("https://github.com/user/repo.git".to_string()),
        );
        config.projects.insert(
            "project2".to_string(),
            ProjectEntry::Url("https://github.com/user/repo2.git".to_string()),
        );

        // Save the config
        config.save_to_file(&meta_file).unwrap();

        // Load the config back
        let loaded_config = MetaConfig::load_from_file(&meta_file).unwrap();

        // Verify the loaded config matches
        assert_eq!(loaded_config.projects.len(), 2);
        assert_eq!(
            loaded_config.projects.get("project1"),
            Some(&ProjectEntry::Url(
                "https://github.com/user/repo.git".to_string()
            ))
        );
        assert_eq!(
            loaded_config.projects.get("project2"),
            Some(&ProjectEntry::Url(
                "https://github.com/user/repo2.git".to_string()
            ))
        );
        assert_eq!(loaded_config.ignore, config.ignore);
    }

    #[test]
    fn test_meta_config_with_nested() {
        let temp_dir = tempdir().unwrap();
        let meta_file = temp_dir.path().join(".meta");

        // Create a config with nested configuration
        let config = MetaConfig {
            nested: Some(NestedConfig {
                recursive_import: true,
                max_depth: 5,
                flatten: true,
                cycle_detection: false,
                ignore_nested: vec!["ignored-project".to_string()],
                namespace_separator: Some("::".to_string()),
                preserve_structure: true,
            }),
            ..Default::default()
        };

        // Save and load
        config.save_to_file(&meta_file).unwrap();
        let loaded_config = MetaConfig::load_from_file(&meta_file).unwrap();

        // Verify nested configuration
        assert!(loaded_config.nested.is_some());
        let nested = loaded_config.nested.unwrap();
        assert!(nested.recursive_import);
        assert_eq!(nested.max_depth, 5);
        assert!(nested.flatten);
        assert!(!nested.cycle_detection);
        assert_eq!(nested.ignore_nested, vec!["ignored-project".to_string()]);
        assert_eq!(nested.namespace_separator, Some("::".to_string()));
        assert!(nested.preserve_structure);
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
        assert_eq!(
            found_file.unwrap().canonicalize().unwrap(),
            meta_file.canonicalize().unwrap()
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_nested_config_default() {
        let nested = NestedConfig::default();
        assert!(!nested.recursive_import);
        assert_eq!(nested.max_depth, 3);
        assert!(!nested.flatten);
        assert!(nested.cycle_detection);
        assert!(nested.ignore_nested.is_empty());
        assert!(nested.namespace_separator.is_none());
        assert!(!nested.preserve_structure);
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
            non_interactive: None,
        };

        let config_without_meta = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: temp_dir.path().to_path_buf(),
            meta_file_path: None,
            experimental: false,
            non_interactive: None,
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
            non_interactive: None,
        };

        assert_eq!(config.meta_root(), Some(temp_dir.path().join("subdir")));
    }

    #[test]
    fn roundtrip_each_format_preserves_projects() {
        for (filename, format) in [
            (".metarepo", ConfigFormat::Json),
            (".metarepo.yaml", ConfigFormat::Yaml),
            (".metarepo.toml", ConfigFormat::Toml),
        ] {
            let tmp = tempdir().unwrap();
            let path = tmp.path().join(filename);

            let mut config = MetaConfig::default();
            config.projects.insert(
                "alpha".to_string(),
                ProjectEntry::Url("https://example.com/alpha.git".to_string()),
            );

            // save_to_file dispatches by extension/filename; we also verify
            // the explicit-format API matches.
            config
                .save_to_file_with_format(&path, format)
                .unwrap_or_else(|e| panic!("save {:?} failed: {}", filename, e));

            let loaded = MetaConfig::load_from_file(&path).unwrap();
            assert!(
                loaded.projects.contains_key("alpha"),
                "{} roundtrip lost projects",
                filename
            );
        }
    }

    #[test]
    fn discover_finds_canonical_in_cwd() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join(".metarepo"), "{}").unwrap();
        let found = MetaConfig::discover_from(tmp.path()).unwrap().unwrap();
        assert_eq!(found.format, ConfigFormat::Json);
        assert_eq!(found.path.file_name().unwrap(), ".metarepo");
    }

    #[test]
    fn discover_walks_up_ancestors() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        std::fs::write(tmp.path().join(".metarepo.yaml"), "ignore: []\n").unwrap();
        let found = MetaConfig::discover_from(&nested).unwrap().unwrap();
        assert_eq!(found.format, ConfigFormat::Yaml);
    }

    #[test]
    fn discover_errors_on_multi_file_conflict() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join(".meta"), "{}").unwrap();
        std::fs::write(tmp.path().join(".metarepo.yaml"), "ignore: []\n").unwrap();
        let err = MetaConfig::discover_from(tmp.path()).expect_err("should error on conflict");
        match err {
            ConfigDiscoveryError::Multiple { ref files, .. } => {
                assert_eq!(files.len(), 2);
            }
            other => panic!("expected Multiple variant, got {:?}", other),
        }
        // Display impl must enumerate the conflicting files so users know
        // which ones to clean up.
        let msg = err.to_string();
        assert!(msg.contains(".meta"));
        assert!(msg.contains(".metarepo.yaml"));
        assert!(msg.contains("--config") || msg.contains("config migrate"));
    }

    #[test]
    fn discover_returns_none_when_no_config_anywhere() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("deep").join("nested");
        fs::create_dir_all(&nested).unwrap();
        assert!(MetaConfig::discover_from(&nested).unwrap().is_none());
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
