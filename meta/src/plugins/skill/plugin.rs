use super::{
    adapt, audit, bundled_version, install, installed_version, is_installed, locations, registry,
    remove, scan, search, steal, update, SkillAction,
};
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use colored::Colorize;
use metarepo_core::{BasePlugin, MetaPlugin, RuntimeConfig};

/// Resolve the install destination: `--dest` flag, else the configured
/// `[skill] dest` (tilde-expanded), else `None` (the env/cwd/home chain applies).
fn resolved_dest(flag: Option<&str>, config: &RuntimeConfig) -> Option<String> {
    if let Some(d) = flag {
        return Some(expand_tilde(d));
    }
    config
        .meta_config
        .skill
        .as_ref()
        .and_then(|s| s.dest.as_deref())
        .map(expand_tilde)
}

/// Expand a leading `~/` to `$HOME`.
fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return format!("{home}/{rest}");
        }
    }
    p.to_string()
}

/// skills.sh search endpoint: `[skill] search-url`, else the built-in default.
fn resolved_search_url(config: &RuntimeConfig) -> String {
    config
        .meta_config
        .skill
        .as_ref()
        .and_then(|s| s.search_url.clone())
        .unwrap_or_else(|| search::DEFAULT_SEARCH_URL.to_string())
}

/// skills.sh skill-detail endpoint: `[skill] detail-url`, else the default.
fn resolved_detail_url(config: &RuntimeConfig) -> String {
    config
        .meta_config
        .skill
        .as_ref()
        .and_then(|s| s.detail_url.clone())
        .unwrap_or_else(|| registry::DEFAULT_DETAIL_URL.to_string())
}

/// Default search hit count. Precedence: `--limit` flag > `[skill] search-limit`
/// > built-in 25.
fn resolved_search_limit(flag: Option<usize>, config: &RuntimeConfig) -> usize {
    flag.or_else(|| {
        config
            .meta_config
            .skill
            .as_ref()
            .and_then(|s| s.search_limit)
    })
    .unwrap_or(25)
}

/// skills.sh API key. Precedence: `SKILLS_SH_API_KEY` env > `[skill] api-key`.
/// Env wins so secrets need not live in `.meta`.
fn resolved_api_key(config: &RuntimeConfig) -> Option<String> {
    if let Ok(k) = std::env::var("SKILLS_SH_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k);
        }
    }
    config
        .meta_config
        .skill
        .as_ref()
        .and_then(|s| s.api_key.clone())
}

/// Print the resolved `[skill]` configuration under `meta skill locations`.
fn print_skill_config(config: &RuntimeConfig) {
    let cmd = adapt::AdaptCommand::from_settings(config.meta_config.skill.as_ref());
    println!("\n{}", "Skill configuration (.meta [skill]):".bold());
    let dest = config
        .meta_config
        .skill
        .as_ref()
        .and_then(|s| s.dest.as_deref());
    match dest {
        Some(d) => println!("  {:<14} {} → {}", "default dest", d, expand_tilde(d)),
        None => println!(
            "  {:<14} {}",
            "default dest",
            "(unset — uses the resolution order above)".dimmed()
        ),
    }
    println!(
        "  {:<14} {} {}",
        "adapt command",
        cmd.command,
        cmd.args.join(" ")
    );
}

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

    fn settings(&self) -> Vec<metarepo_core::ConfigSetting> {
        use metarepo_core::{ConfigSetting, ConfigValueType};
        vec![
            ConfigSetting::new(
                "skill.dest",
                "Default install directory for skills (overridden by --dest)",
                ConfigValueType::String,
            ),
            ConfigSetting::new(
                "skill.adapt-command",
                "AI command used by --adapt",
                ConfigValueType::String,
            )
            .with_default("claude"),
            ConfigSetting::new(
                "skill.adapt-args",
                "Args template for the adapt command ({prompt} is substituted)",
                ConfigValueType::StringList,
            )
            .with_default("-p, {prompt}, --permission-mode, acceptEdits"),
            ConfigSetting::new(
                "skill.search-url",
                "skills.sh search endpoint",
                ConfigValueType::String,
            )
            .with_default(search::DEFAULT_SEARCH_URL),
            ConfigSetting::new(
                "skill.detail-url",
                "skills.sh skill-detail endpoint (keyed fetches)",
                ConfigValueType::String,
            )
            .with_default(registry::DEFAULT_DETAIL_URL),
            ConfigSetting::new(
                "skill.search-limit",
                "Default number of hits for skill search",
                ConfigValueType::Integer,
            )
            .with_default("25"),
            ConfigSetting::new(
                "skill.api-key",
                "skills.sh API key (SKILLS_SH_API_KEY env takes precedence)",
                ConfigValueType::String,
            ),
        ]
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(skill_command())
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
            Some(("status", _)) => handle_status(config),
            Some(("scan", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(String::as_str)
                    .unwrap_or(".");
                scan::run(path)
            }
            Some(("audit", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(String::as_str)
                    .expect("path is required");
                audit::run(path)
            }
            Some(("locations", _)) => {
                locations::run()?;
                print_skill_config(config);
                Ok(())
            }
            Some(("search", m)) => {
                let query = m
                    .get_one::<String>("query")
                    .map(String::as_str)
                    .expect("query is required");
                let flag = m
                    .get_one::<String>("limit")
                    .and_then(|s| s.parse::<usize>().ok());
                let limit = resolved_search_limit(flag, config);
                search::run(query, limit, &resolved_search_url(config))
            }
            Some(("add", m)) => {
                let id = m
                    .get_one::<String>("id")
                    .map(String::as_str)
                    .expect("id is required");
                // Honor the configured default dest when --dest is absent.
                let dest = resolved_dest(m.get_one::<String>("dest").map(String::as_str), config);
                registry::run(
                    id,
                    dest.as_deref(),
                    m.get_flag("force"),
                    m.get_flag("overwrite"),
                    &resolved_detail_url(config),
                    resolved_api_key(config).as_deref(),
                )
            }
            Some(("steal", m)) => {
                let path = m
                    .get_one::<String>("path")
                    .map(String::as_str)
                    .expect("path is required");
                let dest = resolved_dest(m.get_one::<String>("dest").map(String::as_str), config);
                let select = steal::SelectOpts {
                    all: m.get_flag("all"),
                    names: m
                        .get_many::<String>("name")
                        .map(|vals| vals.cloned().collect())
                        .unwrap_or_default(),
                    preview: m.get_flag("preview"),
                    adapt: m.get_one::<String>("adapt").cloned(),
                    adapt_cmd: adapt::AdaptCommand::from_settings(
                        config.meta_config.skill.as_ref(),
                    ),
                };
                let non_interactive = config
                    .non_interactive
                    .unwrap_or(metarepo_core::NonInteractiveMode::Defaults);
                steal::run(
                    path,
                    dest.as_deref(),
                    m.get_flag("force"),
                    m.get_flag("overwrite"),
                    select,
                    non_interactive,
                )
            }
            // No subcommand: show usage, like the other meta commands do.
            _ => {
                metarepo_core::with_standard_help(skill_command()).print_help()?;
                println!();
                Ok(())
            }
        }
    }
}

/// Build the `skill` command tree. Shared by command registration and the
/// no-argument help path so both stay in sync.
fn skill_command() -> Command {
    Command::new("skill")
        .about("Manage the bundled Claude Code meta-tool skill")
        .version(env!("CARGO_PKG_VERSION"))
        .long_about(
            "Install and maintain the bundled meta-tool Claude Code skill under\n\
                     .claude/skills/meta-tool/, and discover, audit, and copy other\n\
                     Claude Code skills between repos.\n\n\
                     Examples:\n  \
                       meta skill              Show installed vs bundled version\n  \
                       meta skill install      Install the skill (no-op if present)\n  \
                       meta skill install -f   Reinstall, overwriting the current copy\n  \
                       meta skill update       Refresh when the bundled version is newer\n  \
                       meta skill remove       Delete the installed skill\n  \
                       meta skill scan ~/Projects  List skills found under a path\n  \
                       meta skill audit <path>     Flag risky patterns in a skill\n  \
                       meta skill locations        Show skill destination dirs\n  \
                       meta skill steal <path>     Copy a local skill in (audit-gated)\n  \
                       meta skill search react     Search skills.sh for skills\n  \
                       meta skill add <id>         Install a skill from skills.sh (audit-gated)",
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
        )
        .subcommand(
            Command::new("scan")
                .about("Walk a directory and list the skills found")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("Directory to scan (defaults to current dir)")
                        .default_value("."),
                ),
        )
        .subcommand(
            Command::new("audit")
                .about("Inspect a skill and flag risky patterns")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("Path to a skill directory or SKILL.md")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("locations")
                .about("Print candidate skill destination directories")
                .version(env!("CARGO_PKG_VERSION")),
        )
        .subcommand(
            Command::new("search")
                .about("Search the skills.sh registry for skills")
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("query")
                        .help("Search terms (at least 2 characters)")
                        .required(true),
                )
                .arg(
                    Arg::new("limit")
                        .long("limit")
                        .help("Maximum results to show (default 25)"),
                ),
        )
        .subcommand(
            Command::new("add")
                .about("Install a skill from skills.sh by id (audit-gated)")
                .long_about(
                    "Install a skill from the skills.sh registry by its owner/repo/skill id\n\
                     (find one with meta skill search). Resolves files via the skills.sh API\n\
                     when SKILLS_SH_API_KEY is set, otherwise by cloning the source GitHub repo,\n\
                     then audits the skill and copies it into a skills directory.",
                )
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("id")
                        .help("Skill id, e.g. owner/repo/skill")
                        .required(true),
                )
                .arg(
                    Arg::new("dest")
                        .long("dest")
                        .help("Destination skills root (defaults to first existing candidate)"),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .action(ArgAction::SetTrue)
                        .help("Install even when the audit reports HIGH-severity findings"),
                )
                .arg(
                    Arg::new("overwrite")
                        .long("overwrite")
                        .action(ArgAction::SetTrue)
                        .help("Replace an existing skill of the same name"),
                ),
        )
        .subcommand(
            Command::new("steal")
                .about("Copy external skills into a local skills directory (audit-gated)")
                .long_about(
                    "Copy one or more skills into a skills directory, audit-gated.\n\
                     The source may be a single skill (a directory with a SKILL.md, or a\n\
                     SKILL.md path), a directory tree containing many skills, or a git URL\n\
                     (cloned shallowly). When more than one skill is found you pick which\n\
                     to take: interactively (multi-select + preview) in a terminal, or with\n\
                     --all / --name when scripted.\n\n\
                     Examples:\n  \
                       meta skill steal ./path/to/skill        Copy one local skill\n  \
                       meta skill steal ./skills               Pick from a local tree\n  \
                       meta skill steal https://github.com/o/r.git   Clone and pick\n  \
                       meta skill steal <git-url> --preview    Preview every skill, copy none\n  \
                       meta skill steal <git-url> --all        Copy every skill found\n  \
                       meta skill steal <git-url> --name foo --name bar  Copy by name\n  \
                       meta skill steal <git-url> --adapt \"fit this repo\"  Adapt via headless claude",
                )
                .version(env!("CARGO_PKG_VERSION"))
                .arg(
                    Arg::new("path")
                        .help("A local skill/dir, a directory of skills, or a git URL")
                        .required(true),
                )
                .arg(
                    Arg::new("dest")
                        .long("dest")
                        .help("Destination skills root (defaults to first existing candidate)"),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .action(ArgAction::SetTrue)
                        .help("Steal every skill found in the source"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .action(ArgAction::Append)
                        .help("Steal the skill(s) with this name (repeatable)"),
                )
                .arg(
                    Arg::new("preview")
                        .long("preview")
                        .action(ArgAction::SetTrue)
                        .help("Print a preview of every skill found and copy nothing"),
                )
                .arg(
                    Arg::new("adapt")
                        .long("adapt")
                        .num_args(0..=1)
                        .default_missing_value("")
                        .help(
                            "After install, run a headless claude to adapt each skill to this repo. \
                             Optionally give a purpose: --adapt \"fit our CI\"",
                        ),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .action(ArgAction::SetTrue)
                        .help("Copy even when the audit reports HIGH-severity findings"),
                )
                .arg(
                    Arg::new("overwrite")
                        .long("overwrite")
                        .action(ArgAction::SetTrue)
                        .help("Replace an existing skill of the same name"),
                ),
        )
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
