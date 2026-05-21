use super::{
    bundled_version, install, installed_version, is_installed, remove, update, SkillAction,
};
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use colored::Colorize;
use metarepo_core::{BasePlugin, MetaPlugin, RuntimeConfig};

/// Manages the bundled meta-tool Claude Code skill (install/update/status/remove).
pub struct SkillPlugin;

impl SkillPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SkillPlugin {
    fn default() -> Self {
        Self::new()
    }
}

fn print_action(action: &SkillAction) {
    match action {
        SkillAction::Installed => println!(
            "  {} Installed Claude Code skill at .claude/skills/meta-tool/",
            "✓".green()
        ),
        SkillAction::Updated { from, to } => println!(
            "  {} Updated Claude Code skill ({} → {})",
            "✓".green(),
            from.as_deref().unwrap_or("unknown"),
            to.as_deref().unwrap_or("unknown"),
        ),
        SkillAction::AlreadyCurrent => println!(
            "  {} Claude Code skill already up to date",
            "·".bright_black()
        ),
    }
}

fn handle_status(config: &RuntimeConfig) -> Result<()> {
    let bundled = bundled_version();
    if !is_installed(&config.working_dir) {
        println!(
            "  {} Skill not installed (bundled version {}). Run 'meta skill install'.",
            "·".bright_black(),
            bundled.as_deref().unwrap_or("unknown"),
        );
        return Ok(());
    }
    let installed = installed_version(&config.working_dir);
    let installed_label = installed.as_deref().unwrap_or("unknown");
    let bundled_label = bundled.as_deref().unwrap_or("unknown");
    if installed.is_some() && installed == bundled {
        println!(
            "  {} Skill up to date (version {})",
            "✓".green(),
            installed_label
        );
    } else {
        println!(
            "  {} Update available ({} → {}). Run 'meta skill update'.",
            "⚠".yellow(),
            installed_label,
            bundled_label,
        );
    }
    Ok(())
}

impl MetaPlugin for SkillPlugin {
    fn name(&self) -> &str {
        "skill"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("skill")
                .about("Manage the bundled Claude Code meta-tool skill")
                .version(env!("CARGO_PKG_VERSION"))
                .long_about(
                    "Install and maintain the bundled meta-tool Claude Code skill under\n\
                     .claude/skills/meta-tool/.\n\n\
                     Examples:\n  \
                       meta skill              Show installed vs bundled version\n  \
                       meta skill install      Install the skill (no-op if present)\n  \
                       meta skill install -f   Reinstall, overwriting the current copy\n  \
                       meta skill update       Refresh when the bundled version is newer\n  \
                       meta skill remove       Delete the installed skill",
                )
                .subcommand_required(false)
                .subcommand(
                    Command::new("install")
                        .about("Install the skill into .claude/skills/meta-tool/")
                        .version(env!("CARGO_PKG_VERSION"))
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .short('f')
                                .action(ArgAction::SetTrue)
                                .help("Overwrite the skill even if already installed"),
                        ),
                )
                .subcommand(
                    Command::new("update")
                        .about("Refresh the skill if the bundled version is newer")
                        .version(env!("CARGO_PKG_VERSION")),
                )
                .subcommand(
                    Command::new("status")
                        .about("Show installed vs bundled skill version")
                        .version(env!("CARGO_PKG_VERSION")),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Delete the installed skill")
                        .version(env!("CARGO_PKG_VERSION")),
                ),
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("install", m)) => {
                let action = install(&config.working_dir, m.get_flag("force"))?;
                print_action(&action);
                Ok(())
            }
            Some(("update", _)) => {
                let action = update(&config.working_dir)?;
                print_action(&action);
                Ok(())
            }
            Some(("remove", _)) => {
                if remove(&config.working_dir)? {
                    println!("  {} Removed Claude Code skill", "✓".yellow());
                } else {
                    println!("  {} No installed skill to remove", "·".bright_black());
                }
                Ok(())
            }
            _ => handle_status(config),
        }
    }
}

impl BasePlugin for SkillPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Manage the bundled Claude Code meta-tool skill")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}
