use anyhow::{Context, Result};
use clap::{ArgMatches, Command as ClapCommand};
use metarepo_core::protocol::{check_protocol_version, CommandInfo, PluginRequest, PluginResponse};
use metarepo_core::{MetaConfig, MetaPlugin, PluginManifest, RuntimeConfig};

use crate::plugins::manifest_plugin::ManifestPlugin;
use crate::plugins::plugin_manager::lockfile::Lockfile;
use crate::plugins::plugin_manager::spec::PluginSpec;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

// External plugin that runs as a subprocess
pub struct ExternalPlugin {
    path: PathBuf,
    name: String,
    version: String,
    experimental: bool,
    commands: Vec<CommandInfo>,
    process: Arc<Mutex<Option<Child>>>,
}

impl ExternalPlugin {
    /// Get the plugin version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Reject plugin paths that we wouldn't normally trust to spawn:
    /// - paths containing `..` components (traversal)
    /// - paths not located inside one of the allowed plugin directories
    ///
    /// Allowed roots:
    ///   - `$HOME/.config/metarepo/plugins`
    ///   - `$HOME/.cargo/bin` (where `cargo install metarepo-plugin-*` lands)
    ///   - `<workspace>/.metarepo/plugins` (per-repo plugins, if used)
    ///   - `<workspace>/.meta-modules` (plugins staged from enabled meta modules)
    ///
    /// The `METAREPO_PLUGIN_ALLOW_ANY_PATH=1` env var lets developers opt out
    /// of the restriction for local plugin development.
    pub fn validate_plugin_path(path: &Path) -> Result<()> {
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(anyhow::anyhow!(
                "Plugin path must not contain '..' segments: {:?}",
                path
            ));
        }

        if std::env::var_os("METAREPO_PLUGIN_ALLOW_ANY_PATH").is_some() {
            return Ok(());
        }

        let canon = path.canonicalize().context(format!(
            "Plugin path does not exist or is not accessible: {:?}",
            path
        ))?;

        let mut allowed: Vec<PathBuf> = Vec::new();
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            allowed.push(PathBuf::from(&home).join(".config/metarepo/plugins"));
            allowed.push(PathBuf::from(&home).join(".cargo/bin"));
        }
        if let Ok(cwd) = std::env::current_dir() {
            allowed.push(cwd.join(".metarepo/plugins"));
            allowed.push(cwd.join(".meta-modules"));
        }

        for root in &allowed {
            if let Ok(canon_root) = root.canonicalize() {
                if canon.starts_with(&canon_root) {
                    return Ok(());
                }
            }
        }

        Err(anyhow::anyhow!(
            "Plugin path {:?} is not in an allowed plugins directory. Allowed roots: {:?}. Set METAREPO_PLUGIN_ALLOW_ANY_PATH=1 to override.",
            path,
            allowed
        ))
    }
    /// Lightweight identity probe: spawn the plugin, send `GetInfo`, and return
    /// its reported `(name, version)`. Used by `meta plugin list` to report
    /// installed-vs-declared status without registering the plugin's commands.
    /// Applies the same path policy as `load`.
    pub fn probe(path: &Path) -> Result<(String, String)> {
        Self::validate_plugin_path(path)?;
        let mut child = Command::new(path)
            .env("METAREPO_PLUGIN_MODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start plugin process")?;

        let response = Self::send_request(&mut child, PluginRequest::GetInfo);
        let _ = child.kill();

        match response? {
            PluginResponse::Info { name, version, .. } => Ok((name, version)),
            PluginResponse::Error { message } => {
                Err(anyhow::anyhow!("Plugin returned error: {}", message))
            }
            _ => Err(anyhow::anyhow!("Unexpected response from plugin")),
        }
    }

    pub fn load(path: &Path) -> Result<Box<dyn MetaPlugin>> {
        Self::validate_plugin_path(path)?;
        // Start the plugin process
        let mut child = Command::new(path)
            .env("METAREPO_PLUGIN_MODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start plugin process")?;

        // Get plugin info
        let info = Self::send_request(&mut child, PluginRequest::GetInfo)?;

        let (name, version, experimental) = match info {
            PluginResponse::Info {
                name,
                version,
                experimental,
                protocol_version,
            } => {
                // Enforce protocol compatibility before doing anything else.
                check_protocol_version(protocol_version.as_deref()).map_err(|e| {
                    anyhow::anyhow!("Plugin {:?} failed protocol check: {}", path, e)
                })?;
                (name, version, experimental)
            }
            PluginResponse::Error { message } => {
                return Err(anyhow::anyhow!("Plugin returned error: {}", message));
            }
            _ => {
                return Err(anyhow::anyhow!("Unexpected response from plugin"));
            }
        };

        // Get command structure
        let commands = Self::send_request(&mut child, PluginRequest::RegisterCommands)?;

        let commands = match commands {
            PluginResponse::Commands { commands } => commands,
            PluginResponse::Error { message } => {
                return Err(anyhow::anyhow!("Plugin returned error: {}", message));
            }
            _ => {
                return Err(anyhow::anyhow!("Unexpected response from plugin"));
            }
        };

        // Log plugin information only in verbose mode
        // eprintln!("Loaded plugin '{}' v{} from {:?}", name, version, path);
        tracing::debug!("Loaded plugin '{}' v{} from {:?}", name, version, path);

        Ok(Box::new(ExternalPlugin {
            path: path.to_path_buf(),
            name,
            version,
            experimental,
            commands,
            process: Arc::new(Mutex::new(Some(child))),
        }))
    }

    fn send_request(child: &mut Child, request: PluginRequest) -> Result<PluginResponse> {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Failed to get plugin stdin"))?;

        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Failed to get plugin stdout"))?;

        // Send request
        let request_json = serde_json::to_string(&request)?;
        writeln!(stdin, "{}", request_json)?;
        stdin.flush()?;

        // Read response
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        // Parse response (trim to remove newline)
        let response: PluginResponse = serde_json::from_str(response_line.trim())
            .context("Failed to parse plugin response")?;

        Ok(response)
    }

    fn build_command_from_info(info: &CommandInfo, version: &'static str) -> ClapCommand {
        // Store command info as leaked static strings to satisfy clap's lifetime requirements
        let name: &'static str = Box::leak(info.name.clone().into_boxed_str());
        let about: &'static str = Box::leak(info.about.clone().into_boxed_str());

        // Set a version on every command: the host injects a global `--version`
        // (ArgAction::Version) that propagates into subcommands, and clap
        // requires any command carrying that action to declare a version.
        let mut cmd = ClapCommand::new(name).about(about).version(version);

        // Add arguments
        for arg in &info.args {
            let arg_name: &'static str = Box::leak(arg.name.clone().into_boxed_str());
            let arg_help: &'static str = Box::leak(arg.help.clone().into_boxed_str());

            let mut clap_arg = clap::Arg::new(arg_name).help(arg_help);

            if arg.required {
                clap_arg = clap_arg.required(true).index(1);
            }

            cmd = cmd.arg(clap_arg);
        }

        // Add subcommands recursively
        for subcmd in &info.subcommands {
            cmd = cmd.subcommand(Self::build_command_from_info(subcmd, version));
        }

        cmd
    }
}

impl MetaPlugin for ExternalPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn register_commands(&self, app: ClapCommand) -> ClapCommand {
        // Build commands from stored command info
        if let Some(root_cmd) = self.commands.first() {
            let version: &'static str = Box::leak(self.version.clone().into_boxed_str());
            app.subcommand(Self::build_command_from_info(root_cmd, version))
        } else {
            app
        }
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Extract command and arguments from matches
        let mut args = Vec::new();

        // Get subcommand and its arguments
        if let Some((subcmd, sub_matches)) = matches.subcommand() {
            args.push(subcmd.to_string());

            // Get all argument values from subcommand matches
            for arg_id in sub_matches.ids() {
                // Skip built-in arguments
                if arg_id == "verbose" || arg_id == "quiet" || arg_id == "experimental" {
                    continue;
                }

                // Try to get as string values
                if let Ok(Some(values)) = sub_matches.try_get_many::<String>(arg_id.as_str()) {
                    for value in values {
                        args.push(value.to_string());
                    }
                }
            }
        }

        // Start a new process for handling the command
        let mut child = Command::new(&self.path)
            .env("METAREPO_PLUGIN_MODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start plugin process")?;

        let request = PluginRequest::HandleCommand {
            command: self.name.clone(),
            args,
            config: Box::new(config.into()),
        };

        let response = Self::send_request(&mut child, request)?;

        // Terminate the process
        let _ = child.kill();

        match response {
            PluginResponse::Success { message } => {
                if let Some(msg) = message {
                    println!("{}", msg);
                }
                Ok(())
            }
            PluginResponse::Error { message } => Err(anyhow::anyhow!("Plugin error: {}", message)),
            _ => Err(anyhow::anyhow!("Unexpected response from plugin")),
        }
    }

    fn is_experimental(&self) -> bool {
        self.experimental
    }

    fn reported_version(&self) -> Option<&str> {
        Some(&self.version)
    }
}

impl Drop for ExternalPlugin {
    fn drop(&mut self) {
        // Clean up the process when the plugin is dropped
        if let Ok(mut guard) = self.process.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }
    }
}

// Plugin loader utilities
pub struct PluginLoader;

impl PluginLoader {
    /// Load an external plugin from a file path. A `plugin.manifest.*` file
    /// loads as a manifest plugin (argv dispatch); any other path is treated as
    /// a protocol plugin (JSON over stdio).
    pub fn load_from_path(path: &Path) -> Result<Box<dyn MetaPlugin>> {
        if !path.exists() {
            return Err(anyhow::anyhow!("Plugin path does not exist: {:?}", path));
        }

        if PluginManifest::is_manifest_path(path) {
            return Self::load_manifest_plugin(path);
        }

        // Check if it's an executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = path.metadata()?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(anyhow::anyhow!("Plugin is not executable: {:?}", path));
            }
        }

        ExternalPlugin::load(path)
    }

    /// Load a manifest plugin from its `plugin.manifest.*` file. The referenced
    /// binary must satisfy the same path policy as a protocol plugin.
    fn load_manifest_plugin(manifest_path: &Path) -> Result<Box<dyn MetaPlugin>> {
        let manifest = PluginManifest::from_file_auto(manifest_path)?;
        let binary = manifest.resolve_binary(manifest_path)?;
        ExternalPlugin::validate_plugin_path(&binary)
            .context("manifest plugin binary failed the path policy check")?;
        if !binary.exists() {
            return Err(anyhow::anyhow!(
                "manifest plugin binary not found: {:?}",
                binary
            ));
        }
        tracing::debug!(
            "Loaded manifest plugin '{}' from {:?}",
            manifest.plugin.name,
            manifest_path
        );
        Ok(Box::new(ManifestPlugin::new(manifest, binary)))
    }

    /// Load all plugins specified in the meta config
    pub fn load_from_config(config: &MetaConfig) -> Vec<Box<dyn MetaPlugin>> {
        let mut plugins = Vec::new();

        let Some(plugin_specs) = &config.plugins else {
            return plugins;
        };

        // Integrity (checksum) enforcement is per-workspace opt-in; version
        // enforcement is always on. Load the lockfile once when required.
        let integrity = config.integrity_required();
        let allow_mismatch = version_mismatch_allowed();
        let lockfile = if integrity {
            let path = Lockfile::locate();
            match path {
                Some(p) => Lockfile::load(&p).unwrap_or_else(|e| {
                    eprintln!("Failed to read plugin lockfile: {}", e);
                    Lockfile::default()
                }),
                None => Lockfile::default(),
            }
        } else {
            Lockfile::default()
        };

        for (name, spec) in plugin_specs {
            match Self::load_plugin_spec(name, spec, integrity, allow_mismatch, &lockfile) {
                Ok(plugin) => plugins.push(plugin),
                Err(e) => eprintln!("Failed to load plugin '{}': {}", name, e),
            }
        }

        plugins
    }

    fn load_plugin_spec(
        name: &str,
        spec: &str,
        integrity: bool,
        allow_mismatch: bool,
        lockfile: &Lockfile,
    ) -> Result<Box<dyn MetaPlugin>> {
        // Verify the on-disk bytes BEFORE spawning, so a tampered binary is never
        // executed when integrity is enforced.
        if integrity {
            let load_path = Self::resolve_load_path(name, spec)?;
            Self::verify_checksum(name, &load_path, lockfile)?;
        }

        // Handle different spec formats
        let plugin = if let Some(stripped) = spec.strip_prefix("file:") {
            // Local file path (may point at a binary or a plugin.manifest.*).
            let path = expand_tilde(stripped);
            Self::load_from_path(&path)?
        } else if spec.starts_with("git+") {
            // git+ plugins are built and copied into the plugin dir at install
            // time under the conventional name; load that binary.
            let binary = Self::plugin_dir()?.join(format!("metarepo-plugin-{}", name));
            Self::load_from_path(&binary)?
        } else {
            // Assume it's a crates.io plugin installed via cargo install.
            Self::load_from_installed(name)?
        };

        // Enforce the declared version against what the plugin reports.
        Self::enforce_version(name, spec, plugin.as_ref(), allow_mismatch)?;
        Ok(plugin)
    }

    /// Resolve the on-disk path a spec loads from (mirrors the dispatch in
    /// [`Self::load_plugin_spec`]) for integrity hashing.
    fn resolve_load_path(name: &str, spec: &str) -> Result<PathBuf> {
        if let Some(stripped) = spec.strip_prefix("file:") {
            Ok(expand_tilde(stripped))
        } else if spec.starts_with("git+") {
            Ok(Self::plugin_dir()?.join(format!("metarepo-plugin-{}", name)))
        } else {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .context("Could not determine home directory")?;
            Ok(PathBuf::from(home)
                .join(".cargo")
                .join("bin")
                .join(format!("metarepo-plugin-{}", name)))
        }
    }

    /// Refuse to load a plugin whose binary does not match the digest recorded
    /// in `.metarepo.lock`. Only called when `plugins-integrity = "required"`.
    fn verify_checksum(name: &str, load_path: &Path, lockfile: &Lockfile) -> Result<()> {
        let entry = lockfile.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "plugins-integrity is 'required' but '{name}' has no entry in .metarepo.lock; \
                 reinstall it (meta plugin install {name} ...) to record its checksum"
            )
        })?;
        let target = crate::plugins::plugin_manager::verify::integrity_target(load_path)?;
        let actual = crate::plugins::plugin_manager::verify::sha256_file(&target)?;
        if actual != entry.sha256 {
            return Err(anyhow::anyhow!(
                "checksum mismatch for plugin '{name}': {} does not match the digest in \
                 .metarepo.lock — refusing to load. Reinstall from a trusted source if this \
                 change is expected.",
                target.display()
            ));
        }
        Ok(())
    }

    /// Compare a plugin's self-reported version against the version pinned by its
    /// spec. No-op when the spec declares no version (e.g. unpinned crates,
    /// `file:`/`git+`) or the plugin reports none.
    fn enforce_version(
        name: &str,
        spec: &str,
        plugin: &dyn MetaPlugin,
        allow_mismatch: bool,
    ) -> Result<()> {
        let Ok(parsed) = PluginSpec::parse(name, spec) else {
            return Ok(());
        };
        let (Some(declared), Some(reported)) =
            (parsed.declared_version(), plugin.reported_version())
        else {
            return Ok(());
        };
        if crate::plugins::plugin_manager::verify::version_satisfies(declared, reported) {
            return Ok(());
        }
        if allow_mismatch {
            eprintln!(
                "warning: plugin '{name}' reports version {reported}, which does not satisfy the \
                 pinned '{declared}' (loaded anyway: --allow-version-mismatch)"
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "version mismatch: plugin '{name}' reports {reported} but .metarepo pins \
                 '{declared}'. Update the pin, reinstall the matching version, or pass \
                 --allow-version-mismatch"
            ))
        }
    }

    /// `~/.config/metarepo/plugins`.
    fn plugin_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("metarepo")
            .join("plugins"))
    }

    fn load_from_installed(name: &str) -> Result<Box<dyn MetaPlugin>> {
        // Check common installation locations
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;

        let cargo_bin = PathBuf::from(home).join(".cargo").join("bin");
        let plugin_binary = cargo_bin.join(format!("metarepo-plugin-{}", name));

        if plugin_binary.exists() {
            Self::load_from_path(&plugin_binary)
        } else {
            Err(anyhow::anyhow!(
                "Plugin '{}' not found. Install with: cargo install metarepo-plugin-{}",
                name,
                name
            ))
        }
    }

    /// Discover ambient plugins in the user's plugin directory: ones dropped in
    /// but not declared in `.metarepo`. Top-level executables load as protocol
    /// plugins; a `plugin.manifest.*` (top-level or inside a per-plugin
    /// subdirectory, how `meta plugin install` lays out manifest plugins) loads
    /// as a manifest plugin.
    ///
    /// `skip` holds the install names already declared in config. Those are
    /// loaded (and integrity/version enforced) by `load_from_config`, so
    /// discovery must not load them again: doing so would bypass enforcement (a
    /// tampered binary refused there could slip in here) and needlessly spawn
    /// the binary.
    pub fn discover_plugins(skip: &std::collections::HashSet<String>) -> Vec<Box<dyn MetaPlugin>> {
        let mut plugins = Vec::new();

        let Ok(plugin_dir) = Self::plugin_dir() else {
            return plugins;
        };
        if !plugin_dir.exists() {
            return plugins;
        }
        let Ok(entries) = std::fs::read_dir(&plugin_dir) else {
            return plugins;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Per-plugin subdirectory: its name is the install name.
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| skip.contains(n))
                {
                    continue;
                }
                if let Some(manifest) = PluginManifest::find_in_dir(&path) {
                    if let Ok(plugin) = Self::load_from_path(&manifest) {
                        if !skip.contains(plugin.name()) {
                            plugins.push(plugin);
                        }
                    }
                }
            } else if path.is_file() && !PluginManifest::is_manifest_path(&path) {
                // Loose executable: `metarepo-plugin-<name>` encodes the name.
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_prefix("metarepo-plugin-"))
                    .is_some_and(|n| skip.contains(n))
                {
                    continue;
                }
                if let Ok(plugin) = Self::load_from_path(&path) {
                    if !skip.contains(plugin.name()) {
                        plugins.push(plugin);
                    }
                }
            } else if PluginManifest::is_manifest_path(&path) {
                // A manifest sitting directly in the plugins dir.
                if let Ok(plugin) = Self::load_from_path(&path) {
                    if !skip.contains(plugin.name()) {
                        plugins.push(plugin);
                    }
                }
            }
        }

        plugins
    }
}

/// Whether a pinned-version mismatch should be downgraded to a warning instead
/// of refusing to load. Plugins are loaded before the CLI parses arguments, so
/// the override is detected from the raw args and the environment.
fn version_mismatch_allowed() -> bool {
    let env_set = std::env::var("METAREPO_ALLOW_VERSION_MISMATCH")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    env_set || std::env::args().any(|a| a == "--allow-version-mismatch")
}

/// Expand a leading `~/` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
