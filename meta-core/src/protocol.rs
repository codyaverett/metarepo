//! Wire protocol (v1) for communication between the metarepo host and external
//! plugins running as subprocesses.
//!
//! The host writes a newline-delimited JSON [`PluginRequest`] to the plugin's
//! stdin and reads a single newline-delimited JSON [`PluginResponse`] back from
//! its stdout. These types are the canonical definition of that format; both the
//! host (`metarepo`) and the plugin-author SDK (`metarepo-plugin-sdk`) depend on
//! them so the wire format is defined exactly once.
//!
//! See `docs/PLUGIN_PROTOCOL_V1.md` for the full specification.

use crate::{MetaConfig, RuntimeConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Wire-format protocol version this build speaks. Plugins must report a
/// matching major version in their [`PluginResponse::Info`] or the host refuses
/// to load them.
pub const PLUGIN_PROTOCOL_VERSION: &str = "1.0";

/// A request sent from the host to a plugin subprocess.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginRequest {
    /// Ask the plugin to identify itself (name, version, protocol).
    GetInfo,
    /// Ask the plugin for its command tree.
    RegisterCommands,
    /// Ask the plugin to execute a command.
    HandleCommand {
        command: String,
        args: Vec<String>,
        config: Box<RuntimeConfigDto>,
    },
}

/// A response sent from a plugin subprocess back to the host.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    Info {
        name: String,
        version: String,
        experimental: bool,
        /// Wire-protocol version the plugin implements (e.g. "1.0"). Optional in
        /// the deserialized form so the host can detect legacy plugins that
        /// predate v1 and surface a useful error instead of a parse failure.
        #[serde(default)]
        protocol_version: Option<String>,
    },
    Commands {
        commands: Vec<CommandInfo>,
    },
    Success {
        message: Option<String>,
    },
    Error {
        message: String,
    },
}

/// Declarative description of a command (and its subcommands/args) that a plugin
/// exposes. The host rebuilds clap commands from this over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub about: String,
    pub subcommands: Vec<CommandInfo>,
    pub args: Vec<ArgInfo>,
}

impl CommandInfo {
    /// Create a leaf command with no args or subcommands.
    pub fn new(name: impl Into<String>, about: impl Into<String>) -> Self {
        CommandInfo {
            name: name.into(),
            about: about.into(),
            subcommands: Vec::new(),
            args: Vec::new(),
        }
    }

    /// Add a positional/required argument (builder style).
    pub fn arg(mut self, arg: ArgInfo) -> Self {
        self.args.push(arg);
        self
    }

    /// Add a nested subcommand (builder style).
    pub fn subcommand(mut self, sub: CommandInfo) -> Self {
        self.subcommands.push(sub);
        self
    }
}

/// Declarative description of a single command argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgInfo {
    pub name: String,
    pub help: String,
    pub required: bool,
}

impl ArgInfo {
    pub fn new(name: impl Into<String>, help: impl Into<String>, required: bool) -> Self {
        ArgInfo {
            name: name.into(),
            help: help.into(),
            required,
        }
    }
}

/// Serializable snapshot of [`RuntimeConfig`] passed to a plugin over the wire.
///
/// This intentionally omits host-only fields (e.g. `non_interactive`) that have
/// no meaning in a subprocess; they default when reconstructed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfigDto {
    pub meta_config: MetaConfig,
    pub working_dir: PathBuf,
    pub meta_file_path: Option<PathBuf>,
    pub experimental: bool,
    /// Whether the user requested whole-workspace scope (`--workspace`/`-w`).
    /// Defaults to `false` so older hosts/plugins remain compatible.
    #[serde(default)]
    pub scope_workspace: bool,
}

impl RuntimeConfigDto {
    /// Resolve the project keys an external plugin should operate on, applying
    /// the same directory-aware rules as the host. See [`crate::scoped_keys`].
    pub fn scoped_project_keys(&self) -> Vec<String> {
        crate::scoped_keys(
            &self.meta_config,
            &self.working_dir,
            self.meta_file_path.as_deref(),
            self.scope_workspace,
        )
    }
}

impl From<&RuntimeConfig> for RuntimeConfigDto {
    fn from(config: &RuntimeConfig) -> Self {
        RuntimeConfigDto {
            meta_config: config.meta_config.clone(),
            working_dir: config.working_dir.clone(),
            meta_file_path: config.meta_file_path.clone(),
            experimental: config.experimental,
            scope_workspace: config.scope_workspace,
        }
    }
}

impl From<RuntimeConfigDto> for RuntimeConfig {
    fn from(dto: RuntimeConfigDto) -> Self {
        RuntimeConfig {
            meta_config: dto.meta_config,
            working_dir: dto.working_dir,
            meta_file_path: dto.meta_file_path,
            experimental: dto.experimental,
            non_interactive: None,
            scope_workspace: dto.scope_workspace,
        }
    }
}

/// Verify that a plugin's reported `protocol_version` is compatible with this
/// build. Same major version = compatible (additive minor changes remain
/// backwards-compatible). Missing or mismatched major = rejected.
pub fn check_protocol_version(reported: Option<&str>) -> anyhow::Result<()> {
    let reported = reported.ok_or_else(|| {
        anyhow::anyhow!(
            "Plugin does not declare a protocol_version. This metarepo speaks v{}; rebuild the plugin against the latest metarepo-plugin-sdk.",
            PLUGIN_PROTOCOL_VERSION
        )
    })?;

    let (their_major, _) = split_major_minor(reported).map_err(|_| {
        anyhow::anyhow!(
            "Plugin reported an unparseable protocol_version '{}'. Expected something like '{}'.",
            reported,
            PLUGIN_PROTOCOL_VERSION
        )
    })?;
    let (our_major, _) = split_major_minor(PLUGIN_PROTOCOL_VERSION).unwrap();

    if their_major != our_major {
        return Err(anyhow::anyhow!(
            "Plugin reports protocol v{} but this metarepo supports v{}. Rebuild the plugin against a compatible metarepo-plugin-sdk.",
            reported,
            PLUGIN_PROTOCOL_VERSION
        ));
    }
    Ok(())
}

fn split_major_minor(s: &str) -> std::result::Result<(u32, u32), std::num::ParseIntError> {
    let mut parts = s.splitn(2, '.');
    let major: u32 = parts.next().unwrap_or("").parse()?;
    let minor: u32 = parts.next().unwrap_or("0").parse()?;
    Ok((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialization_roundtrips() {
        let request = PluginRequest::GetInfo;
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("GetInfo"));
    }

    #[test]
    fn response_deserialization_legacy_missing_protocol_version() {
        let json = r#"{"type":"Info","name":"test","version":"1.0.0","experimental":false}"#;
        let response: PluginResponse = serde_json::from_str(json).unwrap();
        match response {
            PluginResponse::Info {
                protocol_version, ..
            } => assert!(protocol_version.is_none()),
            _ => panic!("expected Info variant"),
        }
    }

    #[test]
    fn response_deserialization_with_protocol_version() {
        let json = r#"{"type":"Info","name":"test","version":"1.0.0","experimental":false,"protocol_version":"1.0"}"#;
        let response: PluginResponse = serde_json::from_str(json).unwrap();
        match response {
            PluginResponse::Info {
                protocol_version, ..
            } => assert_eq!(protocol_version.as_deref(), Some("1.0")),
            _ => panic!("expected Info variant"),
        }
    }

    #[test]
    fn check_protocol_version_accepts_same_major() {
        assert!(check_protocol_version(Some("1.0")).is_ok());
        assert!(check_protocol_version(Some("1.5")).is_ok());
    }

    #[test]
    fn check_protocol_version_rejects_missing() {
        let err = check_protocol_version(None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("does not declare"));
        assert!(msg.contains(PLUGIN_PROTOCOL_VERSION));
    }

    #[test]
    fn check_protocol_version_rejects_different_major() {
        let err = check_protocol_version(Some("2.0")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("v2.0"));
        assert!(msg.contains(PLUGIN_PROTOCOL_VERSION));
    }

    #[test]
    fn check_protocol_version_rejects_garbage() {
        let err = check_protocol_version(Some("not-a-version")).unwrap_err();
        assert!(err.to_string().contains("unparseable"));
    }

    #[test]
    fn runtime_config_dto_roundtrips() {
        let config = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: PathBuf::from("/tmp"),
            meta_file_path: None,
            experimental: false,
            non_interactive: None,
            scope_workspace: false,
        };
        let dto: RuntimeConfigDto = (&config).into();
        assert_eq!(dto.working_dir, config.working_dir);
        assert_eq!(dto.experimental, config.experimental);
        let back: RuntimeConfig = dto.into();
        assert_eq!(back.working_dir, config.working_dir);
    }
}
