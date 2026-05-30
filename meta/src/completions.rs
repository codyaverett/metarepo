//! Shell completion generation.
//!
//! Exposes the native `meta completions <shell>` command, which prints a
//! completion script for the requested shell to stdout. The script is generated
//! from the fully-assembled clap [`Command`] tree, so it covers every built-in
//! and (when available) externally loaded plugin subcommand.

use clap::{Arg, Command};
use clap_complete::Shell;
use std::io;

/// Name of the native completions subcommand.
pub const COMMAND_NAME: &str = "completions";

/// Build the `completions` subcommand definition.
pub fn command() -> Command {
    Command::new(COMMAND_NAME)
        .about("Generate shell completion scripts")
        .version(env!("CARGO_PKG_VERSION"))
        .long_about(
            "Generate a shell completion script for meta and print it to stdout.\n\n\
             Examples:\n  \
             meta completions bash > /etc/bash_completion.d/meta\n  \
             meta completions zsh  > ~/.zsh/completion/_meta\n  \
             meta completions fish > ~/.config/fish/completions/meta.fish\n  \
             meta completions powershell >> $PROFILE",
        )
        .arg(
            Arg::new("shell")
                .value_name("SHELL")
                .required(true)
                .value_parser(clap::value_parser!(Shell))
                .help("Shell to generate completions for (bash, zsh, fish, powershell, elvish)"),
        )
}

/// Write a completion script for `shell` to stdout, generated from `app`.
///
/// `clap_complete` fully builds the whole command tree (running clap's debug
/// assertions) before generating. The CLI exposes `--version` as a global
/// `Version`-action arg that propagates onto every subcommand, but not every
/// plugin subcommand sets a version string — which trips the build assertion.
/// Propagating the root version to all subcommands satisfies that invariant for
/// the throwaway app we generate from.
pub fn print(shell: Shell, app: Command) {
    let mut app = app.propagate_version(true);
    let bin_name = app.get_name().to_string();
    clap_complete::generate(shell, &mut app, bin_name, &mut io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_requires_shell_argument() {
        let cmd = command();
        let arg = cmd
            .get_arguments()
            .find(|a| a.get_id() == "shell")
            .expect("shell argument should exist");
        assert!(arg.is_required_set());
    }

    #[test]
    fn generates_non_empty_script_for_each_shell() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let mut app =
                Command::new("meta").subcommand(Command::new("project").about("manage projects"));
            let mut buf: Vec<u8> = Vec::new();
            clap_complete::generate(shell, &mut app, "meta", &mut buf);
            assert!(
                !buf.is_empty(),
                "expected non-empty completion script for {shell:?}"
            );
        }
    }
}
