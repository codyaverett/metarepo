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

/// Manages Claude Code skills: the bundled meta-tool skill
/// (install/update/status/remove) plus discovering, auditing, and importing
/// other skills (scan/audit/locations/search/steal/add).
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
        .about("Manage Claude Code skills: the bundled meta-tool skill plus discovering, auditing, and importing others")
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
        .after_long_help(metarepo_core::format_help_description(
            "Manage the full lifecycle of Claude Code skills from one command.\n\
             \n\
             Skills are directories holding a SKILL.md (YAML frontmatter plus a\n\
             markdown body) and optional supporting files. They are resolved from, in\n\
             order: $CLAUDE_SKILLS_HOME, the workspace ./.claude/skills, then\n\
             ~/.claude/skills. With no subcommand this shows the bundled meta-tool\n\
             skill's installed-vs-bundled status.\n\
             \n\
             Bundled meta-tool skill: install, update, status, remove. Finding skills:\n\
             scan a directory tree, search the skills.sh registry, list candidate\n\
             destination dirs with locations. Vetting: audit grades risky patterns\n\
             HIGH/MED/LOW. Importing: steal copies skills from a local path or git URL,\n\
             add installs from skills.sh by id; both pass an audit gate that refuses\n\
             HIGH findings unless forced.\n\
             \n\
             Defaults come from the [skill] block in .meta (dest, adapt command,\n\
             search/detail URLs, search limit, api-key); meta skill locations prints\n\
             the resolved values.\n\
             \n\
             Examples:\n  \
               meta skill                Show installed vs bundled meta-tool version\n  \
               meta skill search react   Find skills on skills.sh\n  \
               meta skill add owner/repo/skill   Install one (audit-gated)\n  \
               meta skill steal ./skills Pick and copy skills from a local tree",
        ))
        .subcommand_required(false)
        .subcommand(
            Command::new("install")
                .about("Install the skill into .claude/skills/meta-tool/")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Install the bundled meta-tool Claude Code skill into the workspace.\n\
                     \n\
                     Writes the skill to .claude/skills/meta-tool/ in the current\n\
                     working directory. This is a no-op when the skill is already\n\
                     present; pass --force / -f to overwrite the existing copy (for\n\
                     example to restore a locally modified skill to the bundled\n\
                     version).\n\
                     \n\
                     Examples:\n  \
                       meta skill install      Install if not already present\n  \
                       meta skill install -f   Reinstall, overwriting the current copy",
                ))
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
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Refresh the installed meta-tool skill to the bundled version.\n\
                     \n\
                     Compares the version installed under .claude/skills/meta-tool/\n\
                     against the version shipped with this binary and rewrites the\n\
                     skill when the bundled one is newer. When the installed copy is\n\
                     already current this reports that and makes no changes.\n\
                     \n\
                     Examples:\n  \
                       meta skill update   Update when a newer version is bundled",
                )),
        )
        .subcommand(
            Command::new("status")
                .about("Show installed vs bundled skill version")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Report the meta-tool skill's installed version against the bundled one.\n\
                     \n\
                     Prints whether the skill is installed under\n\
                     .claude/skills/meta-tool/ and, if so, whether it is up to date or\n\
                     an update is available (with the from/to versions). When the skill\n\
                     is absent it points you at meta skill install. This is the default\n\
                     action when meta skill is run with no subcommand.\n\
                     \n\
                     Examples:\n  \
                       meta skill status   Show installed vs bundled version\n  \
                       meta skill          Same, the no-argument default",
                )),
        )
        .subcommand(
            Command::new("remove")
                .about("Delete the installed skill")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Delete the bundled meta-tool skill from the workspace.\n\
                     \n\
                     Removes the .claude/skills/meta-tool/ directory if it exists.\n\
                     Reports when there was nothing installed to remove. Reinstall\n\
                     later with meta skill install.\n\
                     \n\
                     Examples:\n  \
                       meta skill remove   Delete the installed meta-tool skill",
                )),
        )
        .subcommand(
            Command::new("scan")
                .about("Walk a directory and list the skills found")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Discover Claude Code skills anywhere under a directory tree.\n\
                     \n\
                     Walks the given path recursively (skipping .git, node_modules,\n\
                     and target) and lists every skill found, showing each skill's\n\
                     name, description, and the path to its SKILL.md. The path defaults\n\
                     to the current directory. Use this to find skills worth auditing\n\
                     or stealing.\n\
                     \n\
                     Examples:\n  \
                       meta skill scan ~/Projects   List skills under a path\n  \
                       meta skill scan              Scan the current directory",
                ))
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
                .after_long_help(metarepo_core::format_help_description(
                    "Inspect a single skill and flag risky patterns before you trust it.\n\
                     \n\
                     Scans every file in the skill (not just SKILL.md) and grades\n\
                     findings HIGH / MED / LOW, each reported as file:line. HIGH covers\n\
                     remote-exec patterns (curl/wget, piping into a shell, eval),\n\
                     rm -rf, sudo, and wildcard allowed-tools; MED covers shipped\n\
                     executables, chmod +x, git push, --no-verify, ssh, and possible\n\
                     credential references; LOW covers missing frontmatter name or\n\
                     description. The auditor is heuristic and substring-based, so it\n\
                     can false-positive (a skill that merely documents curl | sh is\n\
                     flagged) — treat findings as a prompt to read the file. This is the\n\
                     same gate steal and add apply before importing.\n\
                     \n\
                     Examples:\n  \
                       meta skill audit ~/Downloads/some-skill   Audit a skill dir\n  \
                       meta skill audit ./skills/foo/SKILL.md    A SKILL.md path works",
                ))
                .arg(
                    Arg::new("path")
                        .help("Path to a skill directory or SKILL.md")
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("locations")
                .about("Print candidate skill destination directories")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Show where skills resolve from and the configured [skill] defaults.\n\
                     \n\
                     Prints the candidate skill destination directories in resolution\n\
                     order ($CLAUDE_SKILLS_HOME, ./.claude/skills, ~/.claude/skills),\n\
                     marking which already exist, followed by the resolved [skill]\n\
                     configuration from .meta (default dest and the adapt command). Run\n\
                     this to see where steal and add will write, and what --adapt will\n\
                     invoke.\n\
                     \n\
                     Examples:\n  \
                       meta skill locations   List destinations and [skill] config",
                )),
        )
        .subcommand(
            Command::new("search")
                .about("Search the skills.sh registry for skills")
                .version(env!("CARGO_PKG_VERSION"))
                .after_long_help(metarepo_core::format_help_description(
                    "Search the skills.sh registry for Claude Code skills to install.\n\
                     \n\
                     Queries the public, unauthenticated skills.sh search endpoint for\n\
                     the given terms (at least 2 characters) and prints matches with\n\
                     their install count and canonical owner/repo/skill id. Install a\n\
                     result with meta skill add <id>. Use --limit to change how many\n\
                     hits are shown (default 25, or the [skill] search-limit setting);\n\
                     the endpoint is overridable via [skill] search-url.\n\
                     \n\
                     Examples:\n  \
                       meta skill search react              Top matches for react\n  \
                       meta skill search next js --limit 50 Show up to 50 hits",
                ))
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
                .after_long_help(metarepo_core::format_help_description(
                    "Install a skill from the skills.sh registry by id, audit-gated.\n\
                     \n\
                     Takes an owner/repo/skill id (find one with meta skill search) and\n\
                     resolves its files one of two ways, chosen automatically: keyed\n\
                     fetch from the authenticated skills.sh API when SKILLS_SH_API_KEY\n\
                     (an sk_live_... key) is set, or keyless by shallow-cloning the\n\
                     source GitHub repo and fuzzy-matching the registry slug to a skill\n\
                     directory. Keyless install needs git; both paths need curl. The\n\
                     resolved skill runs through the same audit gate as steal before\n\
                     anything is written.\n\
                     \n\
                     Key flags mirror steal: --dest sets the destination skills root\n\
                     (else the [skill] dest / candidate chain), --overwrite replaces an\n\
                     existing skill of the same name, and --force / -f installs even when\n\
                     the audit reports HIGH-severity findings.\n\
                     \n\
                     Examples:\n  \
                       meta skill add owner/repo/skill            Install (audit-gated)\n  \
                       meta skill add owner/repo/skill --overwrite  Replace an existing copy\n  \
                       meta skill add owner/repo/skill --force    Install despite HIGH findings",
                ))
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
                .after_long_help(metarepo_core::format_help_description(
                    "Copy one or more external skills into a local skills directory, audit-gated.\n\
                     \n\
                     The source can be a single skill (a directory with a SKILL.md, or a\n\
                     SKILL.md path), a directory tree of many skills, or a git URL\n\
                     (shallow-cloned, then treated as a tree). The whole skill directory\n\
                     is copied recursively (scripts and data included; .git,\n\
                     node_modules, and target are skipped), and every copy passes the\n\
                     audit gate independently. When the source holds more than one skill\n\
                     you choose which to take: an interactive full-screen picker in a\n\
                     terminal, or --all / --name when scripted (required without a TTY).\n\
                     \n\
                     Key flags: --dest sets the destination skills root (else [skill]\n\
                     dest / candidate chain); --all takes every skill; --name <n>\n\
                     (repeatable) takes named skills; --preview shows findings and a body\n\
                     excerpt and copies nothing; --adapt [purpose] runs a headless AI\n\
                     command to tailor each copy to this repo; --overwrite replaces an\n\
                     existing copy; --force / -f proceeds despite HIGH findings. Skills\n\
                     with findings get a .meta-review.md trail and inline markers; git\n\
                     sources also get a .meta-source.toml provenance file.\n\
                     \n\
                     Examples:\n  \
                       meta skill steal ./path/to/skill                    Copy one local skill\n  \
                       meta skill steal https://github.com/owner/repo.git  Clone and pick\n  \
                       meta skill steal <git-url> --all --dest ~/.claude/skills  Copy all to a dest",
                ))
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
        Some("Manage Claude Code skills: the bundled meta-tool skill plus discovering, auditing, and importing others")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}
