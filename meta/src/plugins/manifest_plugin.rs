//! Manifest-based external plugins.
//!
//! Unlike protocol plugins (which speak JSON over stdio via the SDK), a
//! manifest plugin ships a `plugin.manifest.{toml,yaml,json}` describing its
//! commands in a clap-shaped schema plus a path to an executable. metarepo
//! registers the declared commands without spawning the binary, and on
//! invocation execs the binary with the resolved subcommand and parsed
//! arguments as argv (plus `METAREPO_*` context and `METAREPO_ARG_*` env vars).
//!
//! This lets a shell or Python script be a `meta` subcommand without
//! implementing the protocol.

use anyhow::{anyhow, bail, Result};
use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};
use metarepo_core::{ManifestCommand, MetaPlugin, PluginManifest, RuntimeConfig};
use std::path::PathBuf;
use std::process::Command;

pub struct ManifestPlugin {
    manifest: PluginManifest,
    binary: PathBuf,
}

impl ManifestPlugin {
    pub fn new(manifest: PluginManifest, binary: PathBuf) -> Self {
        Self { manifest, binary }
    }
}

/// Whether a manifest arg is a positional (no `--long`/`-short` flag).
fn is_positional(arg: &metarepo_core::ManifestArg) -> bool {
    arg.long.is_none() && arg.short.is_none()
}

/// Build a clap command for one manifest command (recursively).
fn build_command(cmd: &ManifestCommand, version: &'static str) -> ClapCommand {
    let name: &'static str = Box::leak(cmd.name.clone().into_boxed_str());
    let about: &'static str = Box::leak(cmd.description.clone().into_boxed_str());
    // A version is required on every command because the host injects a global
    // `--version` (ArgAction::Version) that propagates into subcommands.
    let mut c = ClapCommand::new(name).about(about).version(version);

    if !cmd.aliases.is_empty() {
        let aliases: Vec<&'static str> = cmd
            .aliases
            .iter()
            .map(|a| Box::leak(a.clone().into_boxed_str()) as &'static str)
            .collect();
        c = c.visible_aliases(aliases);
    }

    for a in &cmd.args {
        let arg_name: &'static str = Box::leak(a.name.clone().into_boxed_str());
        let help: &'static str = Box::leak(a.help.clone().into_boxed_str());
        let mut arg = Arg::new(arg_name).help(help);
        if let Some(long) = &a.long {
            arg = arg.long(Box::leak(long.clone().into_boxed_str()) as &'static str);
        }
        if let Some(short) = a.short {
            arg = arg.short(short);
        }
        if is_positional(a) || a.takes_value {
            arg = arg.action(ArgAction::Set);
        } else {
            arg = arg.action(ArgAction::SetTrue);
        }
        if a.required {
            arg = arg.required(true);
        }
        c = c.arg(arg);
    }

    for sub in &cmd.subcommands {
        c = c.subcommand(build_command(sub, version));
    }
    c
}

/// Build the top-level plugin command (`meta <plugin-name>`).
fn build_top_command(manifest: &PluginManifest) -> ClapCommand {
    let name: &'static str = Box::leak(manifest.plugin.name.clone().into_boxed_str());
    let about: &'static str = Box::leak(manifest.plugin.description.clone().into_boxed_str());
    let version: &'static str = Box::leak(manifest.plugin.version.clone().into_boxed_str());
    let mut top = ClapCommand::new(name).about(about).version(version);
    for cmd in &manifest.commands {
        top = top.subcommand(build_command(cmd, version));
    }
    top
}

fn env_key(arg_name: &str) -> String {
    let sanitized: String = arg_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("METAREPO_ARG_{sanitized}")
}

/// Reconstruct the argv tail and `METAREPO_ARG_*` env from a matched command.
/// Returns (argv, env) for `cmd` and any matched nested subcommand.
fn reconstruct(
    cmd: &ManifestCommand,
    matches: &ArgMatches,
) -> (Vec<String>, Vec<(String, String)>) {
    let mut argv = Vec::new();
    let mut envs = Vec::new();

    for a in &cmd.args {
        if is_positional(a) || a.takes_value {
            if let Some(value) = matches.get_one::<String>(&a.name) {
                if is_positional(a) {
                    argv.push(value.clone());
                } else if let Some(long) = &a.long {
                    argv.push(format!("--{long}"));
                    argv.push(value.clone());
                } else if let Some(short) = a.short {
                    argv.push(format!("-{short}"));
                    argv.push(value.clone());
                }
                envs.push((env_key(&a.name), value.clone()));
            }
        } else {
            // boolean flag
            if matches.get_flag(&a.name) {
                if let Some(long) = &a.long {
                    argv.push(format!("--{long}"));
                } else if let Some(short) = a.short {
                    argv.push(format!("-{short}"));
                }
                envs.push((env_key(&a.name), "1".to_string()));
            }
        }
    }

    if let Some((sub_name, sub_matches)) = matches.subcommand() {
        if let Some(sub) = cmd.subcommands.iter().find(|c| c.name == sub_name) {
            argv.push(sub_name.to_string());
            let (mut sub_argv, mut sub_env) = reconstruct(sub, sub_matches);
            argv.append(&mut sub_argv);
            envs.append(&mut sub_env);
        }
    }

    (argv, envs)
}

/// Build the full argv (top subcommand chain + args) for an invocation of the
/// plugin's top-level command.
fn build_invocation(
    manifest: &PluginManifest,
    matches: &ArgMatches,
) -> (Vec<String>, Vec<(String, String)>) {
    if let Some((sub_name, sub_matches)) = matches.subcommand() {
        if let Some(cmd) = manifest.commands.iter().find(|c| c.name == sub_name) {
            let mut argv = vec![sub_name.to_string()];
            let (mut rest, envs) = reconstruct(cmd, sub_matches);
            argv.append(&mut rest);
            return (argv, envs);
        }
    }
    (Vec::new(), Vec::new())
}

impl MetaPlugin for ManifestPlugin {
    fn name(&self) -> &str {
        &self.manifest.plugin.name
    }

    fn register_commands(&self, app: ClapCommand) -> ClapCommand {
        app.subcommand(build_top_command(&self.manifest))
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if !self.binary.exists() {
            bail!(
                "Plugin '{}' binary not found at {}",
                self.manifest.plugin.name,
                self.binary.display()
            );
        }

        let (argv, arg_envs) = build_invocation(&self.manifest, matches);

        let mut cmd = Command::new(&self.binary);
        cmd.args(&argv);

        // Workspace context.
        if let Some(root) = config.meta_root() {
            cmd.env("METAREPO_ROOT", root);
        }
        if let Some(cfg) = &config.meta_file_path {
            cmd.env("METAREPO_CONFIG_PATH", cfg);
        }
        if let Some(project) = config.current_project() {
            cmd.env("METAREPO_PROJECT", project);
        }
        for (k, v) in arg_envs {
            cmd.env(k, v);
        }

        let status = cmd.status().map_err(|e| {
            anyhow!(
                "Failed to execute plugin '{}': {}",
                self.manifest.plugin.name,
                e
            )
        })?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Plugin '{}' exited with {}",
                self.manifest.plugin.name,
                status
                    .code()
                    .map(|c| format!("code {c}"))
                    .unwrap_or_else(|| "a signal".to_string())
            ))
        }
    }

    fn is_experimental(&self) -> bool {
        self.manifest.plugin.experimental
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        let toml = r#"
[plugin]
name = "foo"
version = "0.1.0"
description = "foo plugin"

[[commands]]
name = "greet"
description = "greet"

[[commands.args]]
name = "name"
help = "who"
required = true
takes_value = true

[[commands.args]]
name = "loud"
long = "loud"
help = "shout"

[config.execution]
binary = "./foo.sh"
"#;
        PluginManifest::from_toml_str(toml).unwrap()
    }

    #[test]
    fn registers_top_command_with_subcommands() {
        let m = sample_manifest();
        let app = ClapCommand::new("meta").subcommand(build_top_command(&m));
        let foo = app.find_subcommand("foo").unwrap();
        assert!(foo.find_subcommand("greet").is_some());
    }

    #[test]
    fn reconstructs_positional_and_flag_argv() {
        let m = sample_manifest();
        let app = ClapCommand::new("meta").subcommand(build_top_command(&m));
        let matches = app.get_matches_from(["meta", "foo", "greet", "Ada", "--loud"]);
        let (_, foo_matches) = matches.subcommand().unwrap();
        let (argv, envs) = build_invocation(&m, foo_matches);
        assert_eq!(argv, vec!["greet", "Ada", "--loud"]);
        assert!(envs.contains(&("METAREPO_ARG_NAME".to_string(), "Ada".to_string())));
        assert!(envs.contains(&("METAREPO_ARG_LOUD".to_string(), "1".to_string())));
    }

    #[test]
    fn flag_omitted_when_not_set() {
        let m = sample_manifest();
        let app = ClapCommand::new("meta").subcommand(build_top_command(&m));
        let matches = app.get_matches_from(["meta", "foo", "greet", "Bob"]);
        let (_, foo_matches) = matches.subcommand().unwrap();
        let (argv, _) = build_invocation(&m, foo_matches);
        assert_eq!(argv, vec!["greet", "Bob"]);
    }
}
