//! Example external plugin for metarepo, built on `metarepo-plugin-sdk`.
//!
//! The entire wire protocol (stdin/stdout framing, JSON, the version handshake)
//! is handled by the SDK. A plugin author only implements the [`Plugin`] trait
//! and calls `metarepo_plugin_sdk::serve` from `main` (see `src/main.rs`).

use metarepo_plugin_sdk::{
    ArgInfo, CommandInfo, ConfigSetting, ConfigValueType, Plugin, RuntimeConfigDto,
};
use serde::Deserialize;

/// This plugin's own `[example]` block in `.meta`, mirrored as a typed struct.
///
/// Every field is optional so a workspace with no `[example]` block at all still
/// deserializes; `resolved_*` below supply the defaults. Field names that are
/// not valid Rust identifiers are mapped with `#[serde(rename)]` — the wire form
/// is kebab-case, matching the `ConfigSetting` keys declared in
/// [`ExamplePlugin::settings`].
#[derive(Debug, Default, Deserialize)]
struct ExampleSettings {
    /// Who `meta example hello` greets when no name argument is given.
    greeting: Option<String>,
    /// How many projects `meta example count` will list before truncating.
    #[serde(rename = "max-projects")]
    max_projects: Option<usize>,
}

/// Default greeting when neither the argument nor `[example] greeting` is set.
const DEFAULT_GREETING: &str = "world";
/// Default project cap for `count`.
const DEFAULT_MAX_PROJECTS: usize = 10;

pub struct ExamplePlugin;

impl ExamplePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExamplePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExamplePlugin {
    fn name(&self) -> &str {
        "example"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn commands(&self) -> Vec<CommandInfo> {
        vec![CommandInfo::new(
            "example",
            "Example plugin demonstrating external plugin development",
        )
        .subcommand(
            // Optional on purpose: omitting it exercises the [example] greeting
            // fallback, which a required arg would make unreachable.
            CommandInfo::new("hello", "Print a greeting message").arg(ArgInfo::new(
                "name",
                "Name to greet (defaults to the [example] greeting)",
                false,
            )),
        )
        .subcommand(CommandInfo::new(
            "info",
            "Display information about the current meta repository",
        ))
        .subcommand(CommandInfo::new(
            "count",
            "Count the number of projects in the meta repository",
        ))
        .subcommand(CommandInfo::new(
            "config",
            "Show this plugin's resolved [example] settings",
        ))]
    }

    /// Declare the settings this plugin understands. The host requests these at
    /// load time (`GetSettings`, protocol 1.1+) and folds them into the
    /// `meta config` catalog, so `meta config list`, `get`, and `set` cover this
    /// plugin exactly like a built-in one.
    ///
    /// Declaring a setting does not read it — see `handle`'s `config` arm for
    /// the read side. The two are kept in sync by hand: a key declared here must
    /// match a field on [`ExampleSettings`].
    fn settings(&self) -> Vec<ConfigSetting> {
        vec![
            ConfigSetting::new(
                "example.greeting",
                "Who 'meta example hello' greets when no name is given",
                ConfigValueType::String,
            )
            .with_default(DEFAULT_GREETING),
            ConfigSetting::new(
                "example.max-projects",
                "How many projects 'meta example count' lists before truncating",
                ConfigValueType::Integer,
            )
            .with_default(DEFAULT_MAX_PROJECTS.to_string()),
        ]
    }

    fn handle(
        &self,
        _command: &str,
        args: &[String],
        config: &RuntimeConfigDto,
    ) -> anyhow::Result<Option<String>> {
        // The host passes the nested subcommand name as the first arg, followed
        // by that subcommand's positional values.
        let (sub, rest) = match args.split_first() {
            Some(parts) => parts,
            None => {
                return Ok(Some(
                    "Example plugin - use 'meta example --help' for available commands".into(),
                ))
            }
        };

        // Read this plugin's own config block once, up front. Absent or
        // unparseable, this is `None` and every resolver falls back to a
        // built-in default, so the plugin still works with no `.meta` at all.
        let settings: ExampleSettings = config.plugin_config("example").unwrap_or_default();

        match sub.as_str() {
            "hello" => {
                // Precedence: argument > [example] greeting > built-in default.
                let name = rest
                    .first()
                    .map(String::as_str)
                    .unwrap_or_else(|| resolved_greeting(&settings));
                Ok(Some(format!(
                    "Hello, {name}! This is the example plugin.\nWorking from: {}",
                    config.working_dir.display()
                )))
            }
            "info" => Ok(Some(render_info(config))),
            "count" => {
                let msg = if config.meta_file_path.is_some() {
                    let total = config.meta_config.projects.len();
                    let max = resolved_max_projects(&settings);
                    match total {
                        0 => "No projects in this meta repository.".to_string(),
                        1 => "1 project in this meta repository.".to_string(),
                        n if n > max => format!(
                            "{n} projects in this meta repository (listing capped at {max} by [example] max-projects)."
                        ),
                        n => format!("{n} projects in this meta repository."),
                    }
                } else {
                    "Not in a meta repository. Run 'meta init' first.".to_string()
                };
                Ok(Some(msg))
            }
            "config" => Ok(Some(render_config(&settings))),
            other => Ok(Some(format!(
                "Unknown subcommand '{other}'. Use 'meta example --help'."
            ))),
        }
    }
}

/// The greeting to use, falling back to the built-in default. A blank string in
/// `.meta` counts as unset rather than as an empty greeting.
fn resolved_greeting(settings: &ExampleSettings) -> &str {
    settings
        .greeting
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_GREETING)
}

/// The project cap, falling back to the built-in default.
fn resolved_max_projects(settings: &ExampleSettings) -> usize {
    settings.max_projects.unwrap_or(DEFAULT_MAX_PROJECTS)
}

/// Report each setting's resolved value and where it came from — the read-side
/// counterpart to `ExamplePlugin::settings`.
fn render_config(settings: &ExampleSettings) -> String {
    let source = |is_set: bool| if is_set { "[example]" } else { "default" };
    format!(
        "Resolved [example] settings:\n  {:<14} {:<20} ({})\n  {:<14} {:<20} ({})",
        "greeting",
        resolved_greeting(settings),
        source(settings.greeting.is_some()),
        "max-projects",
        resolved_max_projects(settings),
        source(settings.max_projects.is_some()),
    )
}

fn render_info(config: &RuntimeConfigDto) -> String {
    let mut out = String::new();
    out.push_str("Meta Repository Information:\n");
    out.push_str("============================\n");
    out.push_str(&format!(
        "Working directory: {}\n",
        config.working_dir.display()
    ));

    let Some(meta_file) = &config.meta_file_path else {
        out.push_str("No meta repository found in the current directory tree.\n");
        out.push_str("Run 'meta init' to create one.");
        return out;
    };

    out.push_str(&format!("Meta file found: {}\n", meta_file.display()));

    if config.meta_config.projects.is_empty() {
        out.push_str("\nNo projects configured yet.");
    } else {
        out.push_str("\nProjects:\n");
        let mut names: Vec<&String> = config.meta_config.projects.keys().collect();
        names.sort();
        for name in names {
            out.push_str(&format!("  - {name}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use metarepo_plugin_sdk::RuntimeConfigDto;
    use std::path::PathBuf;

    fn dto(meta_file: Option<&str>) -> RuntimeConfigDto {
        RuntimeConfigDto {
            meta_config: Default::default(),
            working_dir: PathBuf::from("/tmp"),
            meta_file_path: meta_file.map(PathBuf::from),
            experimental: false,
            scope_workspace: false,
        }
    }

    /// A DTO whose `.meta` carries an `[example]` block with `key = value`.
    fn dto_with(key: &str, value: serde_json::Value) -> RuntimeConfigDto {
        let mut d = dto(Some("/tmp/.meta"));
        d.meta_config = d
            .meta_config
            .with_dotted_set(&format!("example.{key}"), value)
            .expect("setting an [example] key");
        d
    }

    #[test]
    fn name_and_version() {
        let p = ExamplePlugin::new();
        assert_eq!(p.name(), "example");
        assert_eq!(p.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn commands_tree_is_declared() {
        let cmds = ExamplePlugin::new().commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "example");
        let subs: Vec<&str> = cmds[0]
            .subcommands
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(subs, ["hello", "info", "count", "config"]);
    }

    #[test]
    fn hello_greets_named_arg() {
        let p = ExamplePlugin::new();
        let out = p
            .handle("example", &["hello".into(), "Ada".into()], &dto(None))
            .unwrap()
            .unwrap();
        assert!(out.contains("Hello, Ada!"));
    }

    #[test]
    fn count_without_meta_file() {
        let p = ExamplePlugin::new();
        let out = p
            .handle("example", &["count".into()], &dto(None))
            .unwrap()
            .unwrap();
        assert!(out.contains("Not in a meta repository"));
    }

    #[test]
    fn info_without_meta_file() {
        let p = ExamplePlugin::new();
        let out = p
            .handle("example", &["info".into()], &dto(None))
            .unwrap()
            .unwrap();
        assert!(out.contains("No meta repository found"));
    }

    #[test]
    fn settings_are_declared_for_the_config_catalog() {
        let keys: Vec<String> = ExamplePlugin::new()
            .settings()
            .into_iter()
            .map(|s| s.key)
            .collect();
        assert_eq!(keys, ["example.greeting", "example.max-projects"]);
    }

    #[test]
    fn hello_falls_back_to_the_configured_greeting() {
        let p = ExamplePlugin::new();
        let cfg = dto_with("greeting", serde_json::json!("metarepo"));
        let out = p
            .handle("example", &["hello".into()], &cfg)
            .unwrap()
            .unwrap();
        assert!(out.contains("Hello, metarepo!"), "got: {out}");
    }

    #[test]
    fn an_explicit_argument_beats_the_configured_greeting() {
        let p = ExamplePlugin::new();
        let cfg = dto_with("greeting", serde_json::json!("metarepo"));
        let out = p
            .handle("example", &["hello".into(), "Ada".into()], &cfg)
            .unwrap()
            .unwrap();
        assert!(out.contains("Hello, Ada!"), "got: {out}");
    }

    #[test]
    fn hello_falls_back_to_the_default_without_config() {
        let p = ExamplePlugin::new();
        let out = p
            .handle("example", &["hello".into()], &dto(None))
            .unwrap()
            .unwrap();
        assert!(out.contains("Hello, world!"), "got: {out}");
    }

    #[test]
    fn config_reports_resolved_values_and_their_source() {
        let p = ExamplePlugin::new();

        let out = p
            .handle("example", &["config".into()], &dto(None))
            .unwrap()
            .unwrap();
        assert!(out.contains("world"), "got: {out}");
        assert!(out.contains("(default)"), "got: {out}");

        let cfg = dto_with("max-projects", serde_json::json!(3));
        let out = p
            .handle("example", &["config".into()], &cfg)
            .unwrap()
            .unwrap();
        // The configured key reports its value and a [example] provenance tag;
        // the unset one still reports its default.
        assert!(out.contains("3"), "got: {out}");
        assert!(
            out.contains("3                    ([example])"),
            "got: {out}"
        );
        assert!(out.contains("world"), "got: {out}");
    }

    #[test]
    fn a_blank_configured_greeting_is_treated_as_unset() {
        let settings = ExampleSettings {
            greeting: Some("   ".into()),
            max_projects: None,
        };
        assert_eq!(resolved_greeting(&settings), DEFAULT_GREETING);
    }
}
