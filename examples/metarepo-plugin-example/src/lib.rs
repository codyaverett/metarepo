//! Example external plugin for metarepo, built on `metarepo-plugin-sdk`.
//!
//! The entire wire protocol (stdin/stdout framing, JSON, the version handshake)
//! is handled by the SDK. A plugin author only implements the [`Plugin`] trait
//! and calls `metarepo_plugin_sdk::serve` from `main` (see `src/main.rs`).

use metarepo_plugin_sdk::{ArgInfo, CommandInfo, Plugin, RuntimeConfigDto};

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
            CommandInfo::new("hello", "Print a greeting message")
                .arg(ArgInfo::new("name", "Name to greet", true)),
        )
        .subcommand(CommandInfo::new(
            "info",
            "Display information about the current meta repository",
        ))
        .subcommand(CommandInfo::new(
            "count",
            "Count the number of projects in the meta repository",
        ))]
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

        match sub.as_str() {
            "hello" => {
                let name = rest.first().map(String::as_str).unwrap_or("world");
                Ok(Some(format!(
                    "Hello, {name}! This is the example plugin.\nWorking from: {}",
                    config.working_dir.display()
                )))
            }
            "info" => Ok(Some(render_info(config))),
            "count" => {
                let msg = if config.meta_file_path.is_some() {
                    match config.meta_config.projects.len() {
                        0 => "No projects in this meta repository.".to_string(),
                        1 => "1 project in this meta repository.".to_string(),
                        n => format!("{n} projects in this meta repository."),
                    }
                } else {
                    "Not in a meta repository. Run 'meta init' first.".to_string()
                };
                Ok(Some(msg))
            }
            other => Ok(Some(format!(
                "Unknown subcommand '{other}'. Use 'meta example --help'."
            ))),
        }
    }
}

fn render_info(config: &RuntimeConfigDto) -> String {
    let mut out = String::new();
    out.push_str("Meta Repository Information:\n");
    out.push_str("============================\n");
    out.push_str(&format!("Working directory: {}\n", config.working_dir.display()));

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
        }
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
        let subs: Vec<&str> = cmds[0].subcommands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(subs, ["hello", "info", "count"]);
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
}
