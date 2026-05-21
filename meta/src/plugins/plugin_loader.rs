use anyhow::{Context, Result};
use clap::{ArgMatches, Command as ClapCommand};
use metarepo_core::protocol::{check_protocol_version, CommandInfo, PluginRequest, PluginResponse};
use metarepo_core::{MetaConfig, MetaPlugin, RuntimeConfig};
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

    fn build_command_from_info(info: &CommandInfo) -> ClapCommand {
        // Store command info as leaked static strings to satisfy clap's lifetime requirements
        let name: &'static str = Box::leak(info.name.clone().into_boxed_str());
        let about: &'static str = Box::leak(info.about.clone().into_boxed_str());

        let mut cmd = ClapCommand::new(name).about(about);

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
            cmd = cmd.subcommand(Self::build_command_from_info(subcmd));
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
            app.subcommand(Self::build_command_from_info(root_cmd))
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
    /// Load an external plugin from a file path
    pub fn load_from_path(path: &Path) -> Result<Box<dyn MetaPlugin>> {
        if !path.exists() {
            return Err(anyhow::anyhow!("Plugin path does not exist: {:?}", path));
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

    /// Load all plugins specified in the meta config
    pub fn load_from_config(config: &MetaConfig) -> Vec<Box<dyn MetaPlugin>> {
        let mut plugins = Vec::new();

        if let Some(plugin_specs) = &config.plugins {
            for (name, spec) in plugin_specs {
                match Self::load_plugin_spec(name, spec) {
                    Ok(plugin) => plugins.push(plugin),
                    Err(e) => eprintln!("Failed to load plugin '{}': {}", name, e),
                }
            }
        }

        plugins
    }

    fn load_plugin_spec(name: &str, spec: &str) -> Result<Box<dyn MetaPlugin>> {
        // Handle different spec formats
        if let Some(stripped) = spec.strip_prefix("file:") {
            // Local file path
            let path = PathBuf::from(stripped);
            Self::load_from_path(&path)
        } else if spec.starts_with("git+") {
            // Git repository - would need to clone and build
            Err(anyhow::anyhow!("Git plugin loading not yet implemented"))
        } else {
            // Assume it's a version spec from crates.io
            // Would need to check if installed via cargo install
            Self::load_from_installed(name)
        }
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

    /// Discover plugins in standard locations
    pub fn discover_plugins() -> Vec<Box<dyn MetaPlugin>> {
        let mut plugins = Vec::new();

        // Check user's plugin directory
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            let plugin_dir = PathBuf::from(home)
                .join(".config")
                .join("metarepo")
                .join("plugins");

            if plugin_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(plugin_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Ok(plugin) = Self::load_from_path(&path) {
                                plugins.push(plugin);
                            }
                        }
                    }
                }
            }
        }

        plugins
    }
}
