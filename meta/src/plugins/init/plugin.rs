use super::{initialize_meta_repo_with_options, InitOptions};
use anyhow::Result;
use clap::ArgMatches;
use colored::Colorize;
use metarepo_core::{plugin, BasePlugin, ConfigFormat, MetaPlugin, RuntimeConfig};

/// InitPlugin using the new simplified plugin architecture
pub struct InitPlugin;

impl InitPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("init")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Initialize a new meta repository")
            .author("Metarepo Contributors")
            .build()
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for InitPlugin {
    fn name(&self) -> &str {
        "init"
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        app.subcommand(
            clap::Command::new("init")
                .about("Initialize a new meta repository")
                .long_about(
                    "Initialize the current directory as a meta repository.\n\n\
                     Idempotent by default: if .meta already exists it is left untouched and only\n\
                     missing artifacts (e.g., .gitignore patterns, optional Claude Code skill)\n\
                     are added.\n\n\
                     Examples:\n  \
                       meta init                  Idempotent init with defaults\n  \
                       meta init --with-skill     Also install the bundled Claude Code skill\n  \
                       meta init --all            Install every optional component\n  \
                       meta init --repair         Restore missing artifacts without touching .meta\n  \
                       meta init --force          Overwrite existing .meta with defaults",
                )
                .visible_aliases(vec!["i"])
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    clap::Arg::new("force")
                        .long("force")
                        .short('f')
                        .action(clap::ArgAction::SetTrue)
                        .help("Overwrite existing .meta with default configuration"),
                )
                .arg(
                    clap::Arg::new("repair")
                        .long("repair")
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with("force")
                        .help("Restore missing artifacts (gitignore, skill) without rewriting .meta"),
                )
                .arg(
                    clap::Arg::new("with-skill")
                        .long("with-skill")
                        .action(clap::ArgAction::SetTrue)
                        .help("Install the bundled Claude Code meta-tool skill"),
                )
                .arg(
                    clap::Arg::new("all")
                        .long("all")
                        .action(clap::ArgAction::SetTrue)
                        .help("Install every optional component (currently: --with-skill)"),
                )
                .arg(
                    clap::Arg::new("format")
                        .long("format")
                        .value_name("FORMAT")
                        .value_parser(["json", "yaml", "yml", "toml"])
                        .help("Format of the new config file (json|yaml|toml). Only applies on fresh init; existing configs keep their current format."),
                ),
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let format = match matches.get_one::<String>("format") {
            Some(s) => ConfigFormat::parse(s)?,
            None => ConfigFormat::Json,
        };
        let options = InitOptions {
            force: matches.get_flag("force"),
            repair: matches.get_flag("repair"),
            with_skill: matches.get_flag("with-skill"),
            all: matches.get_flag("all"),
            format,
        };

        println!(
            "\n  {} {}",
            "📦".cyan(),
            "Initializing meta repository".bold()
        );
        let report = initialize_meta_repo_with_options(&config.working_dir, options)?;

        // Re-use the module-internal printer via a small inline summary so the
        // CLI gets a polished status output. We intentionally don't re-export
        // print_report to keep the public surface small.
        let path_label = report
            .config_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(".metarepo");
        if report.meta_created {
            println!(
                "  {} Created {} with default configuration",
                "✓".green(),
                path_label
            );
        } else if report.meta_overwritten {
            println!(
                "  {} Overwrote {} with default configuration (--force)",
                "✓".yellow(),
                path_label
            );
        } else if report.meta_skipped_existing {
            println!(
                "  {} {} already present (use --force to overwrite)",
                "·".bright_black(),
                path_label
            );
        }
        if report.gitignore_updated {
            println!("  {} Updated .gitignore", "✓".green());
        } else {
            println!("  {} .gitignore already current", "·".bright_black());
        }
        if report.skill_installed {
            println!(
                "  {} Installed Claude Code skill at .claude/skills/meta-tool/",
                "✓".green()
            );
        } else if report.skill_already_present {
            println!(
                "  {} Claude Code skill already present at .claude/skills/meta-tool/",
                "·".bright_black()
            );
        }

        Ok(())
    }
}

impl BasePlugin for InitPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Initialize a new meta repository")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for InitPlugin {
    fn default() -> Self {
        Self::new()
    }
}
