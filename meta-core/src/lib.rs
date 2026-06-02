use anyhow::Result;
use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// New plugin system modules
pub mod config_format;
pub mod config_setting;
pub mod interactive;
mod module_manifest;
mod plugin_base;
mod plugin_builder;
mod plugin_manifest;
pub mod protocol;
pub mod security;
pub mod tui;

pub use config_format::{ConfigFormat, CANONICAL_FILENAME, KNOWN_FILENAMES, LEGACY_FILENAME};
pub use config_setting::{ConfigSetting, ConfigValueType};
pub use interactive::{
    is_interactive, prompt_confirm, prompt_multiselect, prompt_select, prompt_text, prompt_url,
    NonInteractiveMode,
};
pub use module_manifest::{
    MetaModuleManifest, ModuleInfo, ModulePluginRef, ModuleSkillRef, MODULE_MANIFEST_FILENAMES,
};
pub use plugin_base::{
    ArgumentInfo, BasePlugin, CommandInfo, HelpFormat, HelpFormatter, JsonHelpFormatter,
    MarkdownHelpFormatter, PluginMetadata, TerminalHelpFormatter, YamlHelpFormatter,
};
pub use plugin_builder::{
    arg, command, plugin, with_standard_help, ArgBuilder, BuiltPlugin, CommandBuilder,
    PluginBuilder,
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

    /// Declare the configuration settings this plugin understands. The
    /// `meta config` command aggregates these across all plugins so users can
    /// list, get, and set them without hand-editing `.meta`. Each key is dotted
    /// and namespaced by the plugin (e.g. `skill.dest`). Default: none.
    fn settings(&self) -> Vec<ConfigSetting> {
        Vec::new()
    }

    /// The version the plugin reports about itself (protocol `Info` handshake or
    /// manifest `[plugin].version`), if any. Built-in plugins return `None`;
    /// external plugins override this so the loader can enforce the version
    /// declared in `.metarepo`. Default: `None`.
    fn reported_version(&self) -> Option<&str> {
        None
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
    /// When true, multi-project commands operate on every project regardless of
    /// the current directory (set by the global `--workspace`/`-w` flag).
    pub scope_workspace: bool,
    /// Aggregated configuration settings declared by all registered plugins
    /// (see [`MetaPlugin::settings`]). Populated by the host before dispatch so
    /// the `config` command can list/validate them. Empty by default.
    pub settings_catalog: Vec<ConfigSetting>,
}

impl RuntimeConfig {
    pub fn has_meta_file(&self) -> bool {
        self.meta_file_path.is_some()
    }

    pub fn meta_root(&self) -> Option<PathBuf> {
        meta_root_of(self.meta_file_path.as_deref())
    }

    pub fn is_experimental(&self) -> bool {
        self.experimental
    }

    /// Detect if we're currently inside a project directory and return its name
    pub fn current_project(&self) -> Option<String> {
        let meta_root = self.meta_root()?;
        current_project_of(&self.meta_config, &meta_root, &self.working_dir)
    }

    /// Resolve the set of project keys a directory-aware command should act on,
    /// honoring the `--workspace` flag. See [`scoped_keys`].
    pub fn scoped_project_keys(&self) -> Vec<String> {
        scoped_keys(
            &self.meta_config,
            &self.working_dir,
            self.meta_file_path.as_deref(),
            self.scope_workspace,
        )
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

// ============================================================================
// Directory-aware project scoping (shared by RuntimeConfig and the wire DTO)
// ============================================================================

/// The workspace root: the parent directory of the discovered meta file.
pub fn meta_root_of(meta_file_path: Option<&Path>) -> Option<PathBuf> {
    meta_file_path.and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// The project that `working_dir` is inside, if any. A project matches when the
/// working directory is at or below `meta_root/<project key>`.
pub fn current_project_of(
    meta_config: &MetaConfig,
    meta_root: &Path,
    working_dir: &Path,
) -> Option<String> {
    if !working_dir.starts_with(meta_root) {
        return None;
    }
    // Prefer the deepest (longest key) match so nested project keys win.
    let mut best: Option<&String> = None;
    for project_name in meta_config.projects.keys() {
        if working_dir.starts_with(meta_root.join(project_name))
            && best.is_none_or(|b| project_name.len() > b.len())
        {
            best = Some(project_name);
        }
    }
    best.cloned()
}

/// The directory-contextual 3-level project scope:
/// - inside a project (`current_project` is `Some`) → just that project
/// - at the workspace root, or `working_dir` outside it → all projects
/// - in a subdirectory of the root → the projects nested beneath it
pub fn projects_in_scope(
    meta_root: &Path,
    working_dir: &Path,
    project_keys: &[String],
    current_project: Option<String>,
) -> Vec<String> {
    if let Some(project) = current_project {
        return vec![project];
    }
    let Ok(rel) = working_dir.strip_prefix(meta_root) else {
        // cwd is outside the workspace root — operate on everything.
        return project_keys.to_vec();
    };
    if rel.as_os_str().is_empty() {
        // At the workspace root.
        return project_keys.to_vec();
    }
    // In a subdirectory: keep only projects whose key path is nested under it.
    project_keys
        .iter()
        .filter(|key| Path::new(key).starts_with(rel))
        .cloned()
        .collect()
}

/// Resolve the set of project keys a directory-aware command should operate on.
///
/// Keys are returned sorted for deterministic output. When `scope_workspace` is
/// true (the `--workspace`/`-w` flag), every project is returned; otherwise the
/// [`projects_in_scope`] 3-level rule is applied relative to the workspace root.
pub fn scoped_keys(
    meta_config: &MetaConfig,
    working_dir: &Path,
    meta_file_path: Option<&Path>,
    scope_workspace: bool,
) -> Vec<String> {
    let mut keys: Vec<String> = meta_config.projects.keys().cloned().collect();
    keys.sort();
    if scope_workspace {
        return keys;
    }
    let Some(meta_root) = meta_root_of(meta_file_path) else {
        // No workspace root known — fall back to all projects.
        return keys;
    };
    let current = current_project_of(meta_config, &meta_root, working_dir);
    projects_in_scope(&meta_root, working_dir, &keys, current)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<HashMap<String, String>>, // module name -> repo-relative path
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
    #[serde(rename = "plugins-integrity", default)]
    pub plugins_integrity: Option<String>, // "off" (default) | "required"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillSettings>, // `meta skill` configuration
}

/// Configuration for the `meta skill` commands (the `[skill]` block in `.meta`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillSettings {
    /// Default install dir for stolen skills (tilde-expanded). Overridden by `--dest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    /// AI command used by `--adapt` (default: `claude`).
    #[serde(
        rename = "adapt-command",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub adapt_command: Option<String>,
    /// Args template for the adapt command; `{prompt}` is replaced with the
    /// built prompt at run time.
    #[serde(
        rename = "adapt-args",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub adapt_args: Option<Vec<String>>,
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
            modules: None,
            nested: None,
            aliases: None,
            scripts: None,
            worktree_init: None,
            default_bare: None,
            plugins_integrity: None,
            skill: None,
        }
    }
}

/// A located metarepo config file along with its detected format. Returned by
/// [`MetaConfig::discover_from`] and consumable directly by [`MetaConfig::load_from_file`].
#[derive(Debug, Clone)]
pub struct DiscoveredConfig {
    pub path: PathBuf,
    pub format: ConfigFormat,
}

/// Errors surfaced by [`MetaConfig::discover_from`]. Separated out as its own enum
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
    /// Whether plugin checksum integrity is enforced for this workspace.
    /// Controlled by the `plugins-integrity` key (`"required"` turns it on;
    /// anything else, including absent, leaves it off).
    pub fn integrity_required(&self) -> bool {
        self.plugins_integrity
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("required"))
            .unwrap_or(false)
    }

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
                if candidate.is_file() {
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

    /// Like [`discover_from`](Self::discover_from), but keeps walking to the
    /// filesystem root and returns the **outermost** config found, so a metarepo
    /// nested inside another can be driven from the top (the `--root` flag).
    ///
    /// Each directory follows the same single-match rule as `discover_from`:
    /// two recognized files in one directory is still an error.
    pub fn discover_topmost_from(
        start: &Path,
    ) -> std::result::Result<Option<DiscoveredConfig>, ConfigDiscoveryError> {
        let mut current = start.to_path_buf();
        let mut outermost: Option<DiscoveredConfig> = None;
        loop {
            let mut found: Vec<PathBuf> = Vec::new();
            for name in KNOWN_FILENAMES {
                let candidate = current.join(name);
                if candidate.is_file() {
                    found.push(candidate);
                }
            }
            match found.len() {
                0 => {}
                1 => {
                    let path = found.into_iter().next().unwrap();
                    let format = ConfigFormat::from_path(&path).unwrap_or(ConfigFormat::Json);
                    // Keep the highest one seen so far; keep walking upward.
                    outermost = Some(DiscoveredConfig { path, format });
                }
                _ => {
                    return Err(ConfigDiscoveryError::Multiple {
                        dir: current,
                        files: found,
                    });
                }
            }
            if !current.pop() {
                return Ok(outermost);
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

    /// Locate the workspace config in (or above) `base_path`, honoring every
    /// supported filename/format rather than the legacy `.meta` name alone.
    /// Errors with a "run meta init" message when none is found, and surfaces
    /// the multi-file conflict from [`discover_from`](Self::discover_from).
    /// Command handlers that receive an already-resolved workspace root should
    /// use this instead of hand-rolling `base_path.join(".meta")`.
    pub fn locate_in(base_path: &Path) -> Result<DiscoveredConfig> {
        match Self::discover_from(base_path) {
            Ok(Some(found)) => Ok(found),
            Ok(None) => Err(anyhow::anyhow!(
                "No metarepo config file found. Run 'meta init' first."
            )),
            Err(e) => Err(anyhow::anyhow!("{}", e)),
        }
    }

    /// The recognized metarepo config file directly inside `dir`, if any. Looks
    /// only in `dir` itself (no walking up), so it answers "is this directory a
    /// meta repository?" regardless of which supported config filename it uses.
    pub fn config_in_dir(dir: &Path) -> Option<DiscoveredConfig> {
        for name in KNOWN_FILENAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let format = ConfigFormat::from_path(&candidate).unwrap_or(ConfigFormat::Json);
                return Some(DiscoveredConfig {
                    path: candidate,
                    format,
                });
            }
        }
        None
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

    /// Read a value at a dotted key path (e.g. `skill.dest`) from the config,
    /// navigating the serialized JSON representation. Returns `None` if any
    /// segment is missing or null.
    pub fn get_dotted(&self, key: &str) -> Option<serde_json::Value> {
        let json = serde_json::to_value(self).ok()?;
        let mut current = &json;
        for part in key.split('.') {
            match current.get(part) {
                Some(v) if !v.is_null() => current = v,
                _ => return None,
            }
        }
        Some(current.clone())
    }

    /// Return a copy of the config with `value` set at a dotted key path,
    /// creating intermediate objects as needed (so `skill.dest` works even when
    /// the `[skill]` block is absent). Fails only if the result no longer
    /// deserializes into a valid [`MetaConfig`].
    pub fn with_dotted_set(&self, key: &str, value: serde_json::Value) -> Result<MetaConfig> {
        let mut json = serde_json::to_value(self)?;
        let parts: Vec<&str> = key.split('.').collect();

        // Ensure the root is an object we can index into.
        if !json.is_object() {
            json = serde_json::Value::Object(serde_json::Map::new());
        }

        let mut current = &mut json;
        for part in &parts[..parts.len() - 1] {
            // Replace a missing or null intermediate with a fresh object.
            let slot = current
                .as_object_mut()
                .ok_or_else(|| {
                    anyhow::anyhow!("Cannot set '{}': '{}' is not an object", key, part)
                })?
                .entry(part.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if slot.is_null() {
                *slot = serde_json::Value::Object(serde_json::Map::new());
            }
            current = slot;
        }

        let last = parts[parts.len() - 1];
        current
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Cannot set '{}': parent is not an object", key))?
            .insert(last.to_string(), value);

        Ok(serde_json::from_value(json)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dotted_set_creates_missing_nested_block() {
        // `skill` block is absent on a default config.
        let cfg = MetaConfig::default();
        assert!(cfg.get_dotted("skill.dest").is_none());

        let updated = cfg
            .with_dotted_set("skill.dest", serde_json::json!("~/.claude/skills"))
            .unwrap();

        assert_eq!(
            updated.get_dotted("skill.dest"),
            Some(serde_json::json!("~/.claude/skills"))
        );
        assert_eq!(
            updated.skill.as_ref().and_then(|s| s.dest.as_deref()),
            Some("~/.claude/skills")
        );
    }

    #[test]
    fn dotted_get_returns_none_for_null_segment() {
        let cfg = MetaConfig::default();
        // `worktree_init` defaults to null → treated as unset.
        assert!(cfg.get_dotted("worktree_init").is_none());
    }

    #[test]
    fn dotted_set_rejects_invalid_shape() {
        let cfg = MetaConfig::default();
        // `default_bare` is a bool; setting a nested key under it can't
        // deserialize back into MetaConfig.
        let err = cfg.with_dotted_set("default_bare.x", serde_json::json!(1));
        assert!(err.is_err());
    }

    #[test]
    fn scope_inside_a_project_targets_only_that_project() {
        let scope = projects_in_scope(
            Path::new("/ws"),
            Path::new("/ws/app/src"),
            &keys(&["app", "api", "plugins/a"]),
            Some("app".to_string()),
        );
        assert_eq!(scope, vec!["app".to_string()]);
    }

    #[test]
    fn scope_at_workspace_root_targets_all_projects() {
        let all = keys(&["app", "api", "plugins/a"]);
        let scope = projects_in_scope(Path::new("/ws"), Path::new("/ws"), &all, None);
        assert_eq!(scope, all);
    }

    #[test]
    fn scope_in_a_subdirectory_targets_projects_beneath_it() {
        let scope = projects_in_scope(
            Path::new("/ws"),
            Path::new("/ws/plugins"),
            &keys(&["app", "plugins/a", "plugins/b", "tools/x"]),
            None,
        );
        assert_eq!(
            scope,
            vec!["plugins/a".to_string(), "plugins/b".to_string()]
        );
    }

    #[test]
    fn scope_in_an_empty_subdirectory_is_empty() {
        let scope = projects_in_scope(
            Path::new("/ws"),
            Path::new("/ws/docs"),
            &keys(&["app", "plugins/a"]),
            None,
        );
        assert!(scope.is_empty());
    }

    #[test]
    fn scope_outside_the_workspace_targets_all_projects() {
        let all = keys(&["app", "api"]);
        let scope = projects_in_scope(Path::new("/ws"), Path::new("/elsewhere"), &all, None);
        assert_eq!(scope, all);
    }

    #[test]
    fn current_project_of_picks_the_deepest_match() {
        let mut cfg = MetaConfig::default();
        cfg.projects
            .insert("plugins".to_string(), ProjectEntry::Url("u".to_string()));
        cfg.projects
            .insert("plugins/a".to_string(), ProjectEntry::Url("u".to_string()));
        let got = current_project_of(&cfg, Path::new("/ws"), Path::new("/ws/plugins/a/src"));
        assert_eq!(got, Some("plugins/a".to_string()));
    }

    #[test]
    fn scoped_keys_workspace_flag_returns_all_sorted() {
        let mut cfg = MetaConfig::default();
        for k in ["b", "a", "plugins/x"] {
            cfg.projects
                .insert(k.to_string(), ProjectEntry::Url("u".to_string()));
        }
        let keys = scoped_keys(
            &cfg,
            Path::new("/ws/plugins"), // would otherwise scope to subtree
            Some(Path::new("/ws/.meta")),
            true, // scope_workspace
        );
        assert_eq!(keys, keys_sorted(&["a", "b", "plugins/x"]));
    }

    fn keys_sorted(list: &[&str]) -> Vec<String> {
        let mut v = keys(list);
        v.sort();
        v
    }

    #[test]
    fn discover_topmost_returns_outermost_metarepo() {
        let tmp = tempdir().unwrap();
        let outer = tmp.path();
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(outer.join(".metarepo"), "{}").unwrap();
        fs::write(inner.join(".metarepo"), "{}").unwrap();

        let nearest = MetaConfig::discover_from(&inner).unwrap().unwrap();
        assert_eq!(nearest.path, inner.join(".metarepo"));

        let topmost = MetaConfig::discover_topmost_from(&inner).unwrap().unwrap();
        assert_eq!(topmost.path, outer.join(".metarepo"));
    }

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

    // Regression for #72: workspace-config resolution must honor `.metarepo`
    // (and other supported filenames), not just the legacy `.meta`.
    #[test]
    fn locate_in_finds_metarepo_config() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join(".metarepo");
        MetaConfig::default().save_to_file(&path).unwrap();

        let found = MetaConfig::locate_in(temp_dir.path()).unwrap();
        assert_eq!(found.path, path);
        assert_eq!(found.format, ConfigFormat::Json);
    }

    #[test]
    fn locate_in_errors_when_no_config_present() {
        let temp_dir = tempdir().unwrap();
        let err = MetaConfig::locate_in(temp_dir.path()).err().unwrap();
        assert!(err.to_string().contains("meta init"));
    }

    #[test]
    fn config_in_dir_detects_each_supported_filename() {
        for name in [".meta", ".metarepo", ".metarepo.yaml"] {
            let temp_dir = tempdir().unwrap();
            // `.metarepo.yaml` must be valid YAML; the extensionless names parse
            // as JSON. Empty objects are valid in both.
            let body = if name.ends_with(".yaml") {
                "{}\n"
            } else {
                "{}"
            };
            fs::write(temp_dir.path().join(name), body).unwrap();

            let found = MetaConfig::config_in_dir(temp_dir.path());
            assert!(found.is_some(), "should detect {name}");
            assert_eq!(found.unwrap().path, temp_dir.path().join(name));
        }
    }

    #[test]
    fn config_in_dir_does_not_walk_up() {
        let temp_dir = tempdir().unwrap();
        let child = temp_dir.path().join("child");
        fs::create_dir_all(&child).unwrap();
        MetaConfig::default()
            .save_to_file(temp_dir.path().join(".metarepo"))
            .unwrap();

        // The config lives in the parent; a dir-local check on the child must
        // not find it (unlike discover_from, which walks up).
        assert!(MetaConfig::config_in_dir(&child).is_none());
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
            scope_workspace: false,
            settings_catalog: Vec::new(),
        };

        let config_without_meta = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: temp_dir.path().to_path_buf(),
            meta_file_path: None,
            experimental: false,
            non_interactive: None,
            scope_workspace: false,
            settings_catalog: Vec::new(),
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
            scope_workspace: false,
            settings_catalog: Vec::new(),
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
    fn skill_block_roundtrips_and_is_optional() {
        // Absent block ⇒ None, and is not serialized.
        let tmp = tempdir().unwrap();
        let bare = tmp.path().join(".metarepo");
        MetaConfig::default()
            .save_to_file_with_format(&bare, ConfigFormat::Json)
            .unwrap();
        assert!(MetaConfig::load_from_file(&bare).unwrap().skill.is_none());
        assert!(!std::fs::read_to_string(&bare).unwrap().contains("skill"));

        // Present block round-trips across formats.
        for (filename, format) in [
            (".metarepo", ConfigFormat::Json),
            (".metarepo.yaml", ConfigFormat::Yaml),
            (".metarepo.toml", ConfigFormat::Toml),
        ] {
            let path = tmp.path().join(filename);
            let config = MetaConfig {
                skill: Some(SkillSettings {
                    dest: Some("~/.config/agent-skills".into()),
                    adapt_command: Some("codex".into()),
                    adapt_args: Some(vec!["exec".into(), "{prompt}".into()]),
                }),
                ..Default::default()
            };
            config.save_to_file_with_format(&path, format).unwrap();
            let loaded = MetaConfig::load_from_file(&path).unwrap();
            assert_eq!(loaded.skill, config.skill, "{filename} lost [skill]");
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
