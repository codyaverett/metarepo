//! Shell completion generation and installation.
//!
//! Completions are not exposed as a standalone CLI command; they are installed
//! as part of `meta init` (see `plugins::init`). This module owns the reusable
//! logic: generating a completion script from the assembled clap command tree,
//! detecting the user's shell, resolving the per-shell install location, and
//! writing the script with any cache refresh the shell needs.

use anyhow::{Context, Result};
use clap::Command;
use clap_complete::Shell;
use std::path::{Path, PathBuf};

/// Generate a completion script for `shell` from `app` into a byte buffer.
///
/// `clap_complete` fully builds the whole command tree (running clap's debug
/// assertions) before generating. The CLI exposes `--version` as a global
/// `Version`-action arg that propagates onto every subcommand, but not every
/// plugin subcommand sets a version string — which trips the build assertion.
/// Propagating the root version to all subcommands satisfies that invariant for
/// the throwaway app we generate from.
fn generate(shell: Shell, app: Command) -> Vec<u8> {
    let mut app = app.propagate_version(true);
    let bin_name = app.get_name().to_string();
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut app, bin_name, &mut buf);
    buf
}

/// Detect the user's shell from the basename of `$SHELL`.
pub fn detect_shell() -> Option<Shell> {
    let shell_path = std::env::var("SHELL").ok()?;
    shell_from_path(&shell_path)
}

/// Map a shell executable path to a [`Shell`], by its file name.
fn shell_from_path(path: &str) -> Option<Shell> {
    let name = Path::new(path).file_name().and_then(|n| n.to_str())?;
    match name {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        "elvish" => Some(Shell::Elvish),
        "pwsh" | "powershell" => Some(Shell::PowerShell),
        _ => None,
    }
}

/// What cache refresh, if any, is needed after writing a completion file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refresh {
    None,
    /// Remove `~/.zcompdump*` so zsh rebuilds its completion cache.
    ZshCompdump,
}

/// Result of installing a completion script.
pub struct InstallOutcome {
    pub shell: Shell,
    pub path: PathBuf,
    /// Whether a shell completion cache was refreshed as part of the install.
    pub refreshed: bool,
    /// A follow-up step the user must take manually (e.g. add a dir to `$fpath`).
    pub manual_note: Option<String>,
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("Could not determine home directory (HOME/USERPROFILE unset)")
}

/// Generate and install a completion script for `shell`, refreshing caches as
/// needed. Returns where the script was written and any manual follow-up step.
pub fn install(shell: Shell) -> Result<InstallOutcome> {
    let home = home_dir()?;
    let omz = oh_my_zsh_dir(&home);
    let (path, refresh, manual_note) = resolve_target(shell, &home, omz.as_deref())?;

    let script = generate(shell, crate::cli::MetarepoCli::new().build_app());

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, &script)
        .with_context(|| format!("Failed to write completion script to {}", path.display()))?;

    let refreshed = match refresh {
        Refresh::None => false,
        Refresh::ZshCompdump => {
            remove_zcompdump(&home);
            true
        }
    };

    Ok(InstallOutcome {
        shell,
        path,
        refreshed,
        manual_note,
    })
}

/// Locate an oh-my-zsh installation directory, if one exists, preferring `$ZSH`.
fn oh_my_zsh_dir(home: &Path) -> Option<PathBuf> {
    let candidate = std::env::var_os("ZSH")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".oh-my-zsh"));
    candidate.is_dir().then_some(candidate)
}

/// Resolve the target path, cache-refresh action, and any manual follow-up note
/// for a shell. Pure: all environment/filesystem inputs are passed in, so this
/// can be unit-tested without touching `$HOME` or env vars.
fn resolve_target(
    shell: Shell,
    home: &Path,
    omz_dir: Option<&Path>,
) -> Result<(PathBuf, Refresh, Option<String>)> {
    match shell {
        Shell::Zsh => {
            if let Some(omz) = omz_dir {
                // oh-my-zsh adds $ZSH/completions to $fpath before compinit.
                Ok((omz.join("completions/_meta"), Refresh::ZshCompdump, None))
            } else {
                let note = "Ensure the install dir is on your $fpath before compinit. \
                            Add to ~/.zshrc:\n      fpath=(~/.zsh/completions $fpath)\n      \
                            autoload -U compinit && compinit"
                    .to_string();
                Ok((
                    home.join(".zsh/completions/_meta"),
                    Refresh::ZshCompdump,
                    Some(note),
                ))
            }
        }
        Shell::Fish => Ok((
            home.join(".config/fish/completions/meta.fish"),
            Refresh::None,
            None,
        )),
        Shell::Bash => Ok((
            home.join(".local/share/bash-completion/completions/meta"),
            Refresh::None,
            Some("Requires the bash-completion package (v2) to be installed.".to_string()),
        )),
        Shell::Elvish => Ok((
            home.join(".config/elvish/lib/meta.elv"),
            Refresh::None,
            Some("Add `use meta` to your ~/.config/elvish/rc.elv.".to_string()),
        )),
        other => Err(anyhow::anyhow!(
            "Automatic completion install is not supported for {other}; \
             supported shells are bash, zsh, fish, and elvish"
        )),
    }
}

/// Remove cached zsh completion dumps (`~/.zcompdump*`) so the next interactive
/// shell rebuilds them and picks up the freshly installed script.
fn remove_zcompdump(home: &Path) {
    let Ok(entries) = std::fs::read_dir(home) else {
        return;
    };
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with(".zcompdump") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_from_path_maps_known_shells() {
        assert_eq!(shell_from_path("/bin/bash"), Some(Shell::Bash));
        assert_eq!(shell_from_path("/usr/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(shell_from_path("/opt/homebrew/bin/fish"), Some(Shell::Fish));
        assert_eq!(
            shell_from_path("/usr/local/bin/elvish"),
            Some(Shell::Elvish)
        );
        assert_eq!(shell_from_path("/usr/bin/pwsh"), Some(Shell::PowerShell));
        assert_eq!(shell_from_path("/bin/tcsh"), None);
        assert_eq!(shell_from_path(""), None);
    }

    #[test]
    fn resolve_target_zsh_prefers_oh_my_zsh() {
        let home = Path::new("/home/u");
        let omz = Path::new("/home/u/.oh-my-zsh");
        let (path, refresh, note) = resolve_target(Shell::Zsh, home, Some(omz)).unwrap();
        assert_eq!(path, Path::new("/home/u/.oh-my-zsh/completions/_meta"));
        assert_eq!(refresh, Refresh::ZshCompdump);
        assert!(note.is_none(), "oh-my-zsh dir is already on $fpath");
    }

    #[test]
    fn resolve_target_zsh_falls_back_with_fpath_note() {
        let home = Path::new("/home/u");
        let (path, refresh, note) = resolve_target(Shell::Zsh, home, None).unwrap();
        assert_eq!(path, Path::new("/home/u/.zsh/completions/_meta"));
        assert_eq!(refresh, Refresh::ZshCompdump);
        assert!(note.unwrap().contains("fpath"));
    }

    #[test]
    fn resolve_target_fish_and_bash_and_elvish() {
        let home = Path::new("/home/u");

        let (fish, refresh, note) = resolve_target(Shell::Fish, home, None).unwrap();
        assert_eq!(
            fish,
            Path::new("/home/u/.config/fish/completions/meta.fish")
        );
        assert_eq!(refresh, Refresh::None);
        assert!(note.is_none());

        let (bash, _, note) = resolve_target(Shell::Bash, home, None).unwrap();
        assert_eq!(
            bash,
            Path::new("/home/u/.local/share/bash-completion/completions/meta")
        );
        assert!(note.unwrap().contains("bash-completion"));

        let (elv, _, note) = resolve_target(Shell::Elvish, home, None).unwrap();
        assert_eq!(elv, Path::new("/home/u/.config/elvish/lib/meta.elv"));
        assert!(note.unwrap().contains("use meta"));
    }

    #[test]
    fn resolve_target_rejects_powershell() {
        assert!(resolve_target(Shell::PowerShell, Path::new("/home/u"), None).is_err());
    }

    #[test]
    fn generate_produces_non_empty_script() {
        // Root must carry a version for `propagate_version(true)`; the real app
        // sets one via `.version(env!("CARGO_PKG_VERSION"))`.
        let app = Command::new("meta")
            .version("0.0.0")
            .subcommand(Command::new("project").about("manage"));
        let script = generate(Shell::Zsh, app);
        assert!(!script.is_empty());
        assert!(String::from_utf8_lossy(&script).contains("project"));
    }
}
