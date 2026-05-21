//! # metarepo-plugin-sdk
//!
//! Author a metarepo external plugin by implementing one trait and calling
//! [`serve`]. The SDK owns the v1 stdio wire protocol — request framing,
//! parsing, dispatch, error handling, and the protocol-version handshake — so
//! plugin code never touches stdin/stdout or JSON directly.
//!
//! ```no_run
//! use metarepo_plugin_sdk::{serve, CommandInfo, Plugin, RuntimeConfigDto};
//!
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn name(&self) -> &str { "hello" }
//!     fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
//!
//!     fn commands(&self) -> Vec<CommandInfo> {
//!         vec![CommandInfo::new("hello", "Print a greeting")]
//!     }
//!
//!     fn handle(
//!         &self,
//!         _command: &str,
//!         _args: &[String],
//!         _config: &RuntimeConfigDto,
//!     ) -> anyhow::Result<Option<String>> {
//!         Ok(Some("hello from a plugin".to_string()))
//!     }
//! }
//!
//! fn main() -> anyhow::Result<()> {
//!     serve(Hello)
//! }
//! ```

use std::io::{BufRead, Write};

// Re-export the wire types so plugin authors depend only on the SDK. These are
// also the names used internally by `serve_io`/`dispatch` below.
pub use metarepo_core::protocol::{
    ArgInfo, CommandInfo, PluginRequest, PluginResponse, RuntimeConfigDto, PLUGIN_PROTOCOL_VERSION,
};

/// A metarepo plugin. Implement this trait and pass an instance to [`serve`].
///
/// This is the subprocess-oriented analogue of the host's `MetaPlugin` trait:
/// commands are declared as data ([`CommandInfo`]) rather than built from clap,
/// and the runtime config arrives as a serialized [`RuntimeConfigDto`] snapshot
/// instead of a borrowed host value.
pub trait Plugin {
    /// The command name this plugin registers (e.g. `"hello"` for `meta hello`).
    fn name(&self) -> &str;

    /// The plugin's own version string (typically `env!("CARGO_PKG_VERSION")`).
    fn version(&self) -> &str;

    /// Whether this plugin is experimental. Defaults to `false`.
    fn is_experimental(&self) -> bool {
        false
    }

    /// The command tree this plugin exposes. The host rebuilds clap commands
    /// from this for `meta --help` and argument routing.
    fn commands(&self) -> Vec<CommandInfo>;

    /// Execute an invocation. `command` is the top-level command name and
    /// `args` are the positional arguments the host parsed. Return an optional
    /// message to print on success, or an error to report failure.
    fn handle(
        &self,
        command: &str,
        args: &[String],
        config: &RuntimeConfigDto,
    ) -> anyhow::Result<Option<String>>;
}

/// Run the plugin against the process stdin/stdout.
///
/// This blocks, serving requests until stdin reaches EOF (the host closes the
/// pipe). Call it from `main`.
pub fn serve<P: Plugin>(plugin: P) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve_io(&plugin, stdin.lock(), stdout.lock())
}

/// Run the request loop against arbitrary reader/writer streams.
///
/// Exposed for testing with in-memory buffers; [`serve`] wraps this with the
/// process std streams.
pub fn serve_io<P, R, W>(plugin: &P, reader: R, mut writer: W) -> anyhow::Result<()>
where
    P: Plugin,
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<PluginRequest>(&line) {
            Ok(request) => dispatch(plugin, request),
            Err(e) => PluginResponse::Error {
                message: format!("Failed to parse request: {e}"),
            },
        };

        writeln!(writer, "{}", serde_json::to_string(&response)?)?;
        writer.flush()?;
    }

    Ok(())
}

/// Map a single request to its response using the plugin's trait methods.
fn dispatch<P: Plugin>(plugin: &P, request: PluginRequest) -> PluginResponse {
    match request {
        PluginRequest::GetInfo => PluginResponse::Info {
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            experimental: plugin.is_experimental(),
            protocol_version: Some(PLUGIN_PROTOCOL_VERSION.to_string()),
        },
        PluginRequest::RegisterCommands => PluginResponse::Commands {
            commands: plugin.commands(),
        },
        PluginRequest::HandleCommand {
            command,
            args,
            config,
        } => match plugin.handle(&command, &args, &config) {
            Ok(message) => PluginResponse::Success { message },
            Err(e) => PluginResponse::Error {
                message: e.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn name(&self) -> &str {
            "test"
        }
        fn version(&self) -> &str {
            "9.9.9"
        }
        fn commands(&self) -> Vec<CommandInfo> {
            vec![CommandInfo::new("test", "A test command").arg(ArgInfo::new(
                "name",
                "Name to greet",
                true,
            ))]
        }
        fn handle(
            &self,
            command: &str,
            args: &[String],
            _config: &RuntimeConfigDto,
        ) -> anyhow::Result<Option<String>> {
            if command == "boom" {
                anyhow::bail!("explicit failure");
            }
            Ok(Some(format!("handled {command} with {args:?}")))
        }
    }

    /// Drive serve_io with a sequence of request lines, return the response lines.
    fn run(input: &str) -> Vec<String> {
        let mut out = Vec::new();
        serve_io(&TestPlugin, input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn get_info_reports_name_version_and_protocol() {
        let lines = run(r#"{"type":"GetInfo"}"#);
        assert_eq!(lines.len(), 1);
        let resp: PluginResponse = serde_json::from_str(&lines[0]).unwrap();
        match resp {
            PluginResponse::Info {
                name,
                version,
                experimental,
                protocol_version,
            } => {
                assert_eq!(name, "test");
                assert_eq!(version, "9.9.9");
                assert!(!experimental);
                assert_eq!(protocol_version.as_deref(), Some(PLUGIN_PROTOCOL_VERSION));
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn register_commands_returns_declared_tree() {
        let lines = run(r#"{"type":"RegisterCommands"}"#);
        let resp: PluginResponse = serde_json::from_str(&lines[0]).unwrap();
        match resp {
            PluginResponse::Commands { commands } => {
                assert_eq!(commands.len(), 1);
                assert_eq!(commands[0].name, "test");
                assert_eq!(commands[0].args.len(), 1);
                assert_eq!(commands[0].args[0].name, "name");
            }
            _ => panic!("expected Commands"),
        }
    }

    #[test]
    fn handle_command_success_carries_message() {
        let req = r#"{"type":"HandleCommand","command":"greet","args":["world"],"config":{"meta_config":{"projects":{}},"working_dir":"/tmp","meta_file_path":null,"experimental":false}}"#;
        let lines = run(req);
        let resp: PluginResponse = serde_json::from_str(&lines[0]).unwrap();
        match resp {
            PluginResponse::Success { message } => {
                assert!(message.unwrap().contains("handled greet"));
            }
            _ => panic!("expected Success"),
        }
    }

    #[test]
    fn handle_command_error_is_reported() {
        let req = r#"{"type":"HandleCommand","command":"boom","args":[],"config":{"meta_config":{"projects":{}},"working_dir":"/tmp","meta_file_path":null,"experimental":false}}"#;
        let lines = run(req);
        let resp: PluginResponse = serde_json::from_str(&lines[0]).unwrap();
        match resp {
            PluginResponse::Error { message } => assert!(message.contains("explicit failure")),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn malformed_request_yields_error_not_panic() {
        let lines = run("not json at all");
        let resp: PluginResponse = serde_json::from_str(&lines[0]).unwrap();
        match resp {
            PluginResponse::Error { message } => assert!(message.contains("Failed to parse")),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn blank_lines_are_skipped_and_multiple_requests_served() {
        let lines = run("\n{\"type\":\"GetInfo\"}\n\n{\"type\":\"GetInfo\"}\n");
        assert_eq!(lines.len(), 2);
    }
}
