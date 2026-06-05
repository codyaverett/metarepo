use super::config::RulesConfig;
use super::create::RuleCreator;
use super::engine::RuleEngine;
use super::project::{ProjectRulesManager, RulesStats};
use anyhow::Result;
use clap::ArgMatches;
use colored::*;
use metarepo_core::{arg, command, plugin, BasePlugin, MetaPlugin, RuntimeConfig};
use std::collections::HashMap;

/// RulesPlugin using the new simplified plugin architecture
pub struct RulesPlugin;

impl RulesPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("rules")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Enforce project file structure rules")
            .author("Metarepo Contributors")
            .help_description(
                "Validate and enforce project structure conventions across the workspace.\n\
                 \n\
                 The rules system checks each project against a .rules.yaml configuration\n\
                 covering directory layout, component folders, required companion files,\n\
                 naming conventions, documentation, size limits, and security patterns.\n\
                 Rules resolve per project: a project's own .rules.yaml takes priority,\n\
                 otherwise the workspace-root .rules.yaml applies, falling back to a\n\
                 built-in minimal rule set when neither exists.\n\
                 \n\
                 Use check to validate (optionally auto-fixing), init to scaffold a config,\n\
                 create to add rules, and list/docs/status/copy to inspect and manage them.\n\
                 \n\
                 Examples:\n\
                 \n\
                   meta rules init                       scaffold a workspace .rules.yaml\n\
                   meta rules check --fix                validate everything and fix what it can\n\
                   meta rules check --project frontend   validate a single project",
            )
            .command(
                command("check")
                    .about("Check project structure against configured rules")
                    .help_description(
                        "Validate one or all projects against their resolved rules.\n\
                         \n\
                         For each project the engine loads the applicable .rules.yaml\n\
                         (project-specific, then workspace, then built-in minimal), prints\n\
                         the rule source and counts, then reports every violation as an\n\
                         error, warning, or info with its path. Without --project, all\n\
                         projects listed in .meta are checked; missing project directories\n\
                         are skipped with a notice.\n\
                         \n\
                         Pass --fix to auto-create missing directories and other fixable\n\
                         items; violations marked fixable are flagged with a hint. A summary\n\
                         of total violations is printed at the end.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules check\n\
                           meta rules check --project meta-core\n\
                           meta rules check --fix",
                    )
                    .aliases(vec!["c".to_string(), "chk".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Specific project to check (defaults to all)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("fix")
                            .long("fix")
                            .help("Automatically fix violations where possible")
                    )
            )
            .command(
                command("init")
                    .about("Initialize rules configuration file")
                    .help_description(
                        "Scaffold a starter .rules.yaml so you can begin enforcing structure.\n\
                         \n\
                         Writes a sample configuration (directory, component, file, naming,\n\
                         size, and security rules) to the workspace root by default, or to a\n\
                         project directory when --project is given. The target path comes\n\
                         from --output (default .rules.yaml). If a file already exists at the\n\
                         target it is left untouched and a warning is printed; the generated\n\
                         YAML is echoed to stdout so you can review and edit it.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules init\n\
                           meta rules init --project frontend\n\
                           meta rules init --output custom-rules.yaml",
                    )
                    .aliases(vec!["i".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("output")
                            .long("output")
                            .short('o')
                            .help("Output file path (default: .rules.yaml)")
                            .default_value(".rules.yaml")
                            .takes_value(true)
                    )
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("Initialize rules for specific project")
                            .takes_value(true)
                    )
            )
            .command(
                command("list")
                    .about("List all configured rules")
                    .help_description(
                        "Print the directory, component, and file rules currently in effect.\n\
                         \n\
                         Loads the resolved configuration and lists directory rules (with\n\
                         required/optional status), component rules (pattern plus expected\n\
                         structure), and file rules (pattern plus required companions), each\n\
                         with its description. With --project it lists that project's resolved\n\
                         rules; otherwise it lists the workspace rules (or the built-in\n\
                         minimal set when no .rules.yaml exists).\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules list\n\
                           meta rules list --project frontend",
                    )
                    .aliases(vec!["ls".to_string(), "l".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("project")
                            .long("project")
                            .short('p')
                            .help("List rules for specific project")
                            .takes_value(true)
                    )
            )
            .command(
                command("docs")
                    .about("Show documentation for creating and using rules")
                    .help_description(
                        "Print built-in reference documentation for the rule types.\n\
                         \n\
                         With no argument it prints the full documentation covering every\n\
                         rule type. Pass a type to show just that section; accepted values\n\
                         (with aliases) are directory (dir), component (comp), file (files),\n\
                         naming (name), dependency (dep, deps), import (imports),\n\
                         documentation (doc, docs), size, and security (sec). An unknown\n\
                         type prints the list of valid types.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules docs\n\
                           meta rules docs component\n\
                           meta rules docs security",
                    )
                    .aliases(vec!["d".to_string()])
                    .with_help_formatting()
                    .arg(
                        arg("type")
                            .help("Show docs for specific rule type (directory, component, file, naming, dependency, import, documentation, size, security)")
                            .takes_value(true)
                    )
            )
            .command(
                command("create")
                    .about("Create a new rule")
                    .help_description(
                        "Add a new rule to a .rules.yaml configuration.\n\
                         \n\
                         Choose a subcommand for the rule type you want to add: directory,\n\
                         component, or file. Each writes into the workspace .rules.yaml by\n\
                         default, or a project's .rules.yaml when --project is supplied,\n\
                         creating the file if needed and skipping rules that already exist.\n\
                         Running create with no subcommand prints help for the available\n\
                         rule types.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules create directory src --required\n\
                           meta rules create component \"src/components/*/\"\n\
                           meta rules create file \"**/*.rs\" --requires test:#[test]",
                    )
                    .aliases(vec!["new".to_string()])
                    .with_help_formatting()
                    .subcommand(
                        command("directory")
                            .about("Create a directory rule")
                            .help_description(
                                "Add a rule that requires (or suggests) a directory to exist.\n\
                                 \n\
                                 Records a directory rule for the given path. Pass --required\n\
                                 to make a missing directory an error (otherwise it is reported\n\
                                 as info), and --description to document its purpose. Targets\n\
                                 the workspace .rules.yaml unless --project is given. A rule for\n\
                                 a path that already exists is left unchanged.\n\
                                 \n\
                                 Examples:\n\
                                 \n\
                                   meta rules create directory src --required\n\
                                   meta rules create dir docs -d \"Project documentation\"",
                            )
                            .aliases(vec!["dir".to_string()])
                            .arg(
                                arg("path")
                                    .help("Directory path")
                                    .required(true)
                                    .takes_value(true)
                            )
                            .arg(
                                arg("required")
                                    .long("required")
                                    .help("Mark as required")
                            )
                            .arg(
                                arg("description")
                                    .long("description")
                                    .short('d')
                                    .help("Rule description")
                                    .takes_value(true)
                            )
                            .arg(
                                arg("project")
                                    .long("project")
                                    .short('p')
                                    .help("Add to specific project")
                                    .takes_value(true)
                            )
                    )
                    .subcommand(
                        command("component")
                            .about("Create a component rule")
                            .help_description(
                                "Add a rule that validates the internal layout of component folders.\n\
                                 \n\
                                 Records a component rule whose pattern matches component\n\
                                 directories and whose structure lists the files each must\n\
                                 contain; use [ComponentName] in structure items as a\n\
                                 placeholder for the folder name. Provide the structure with\n\
                                 --structure as a comma-separated list, or omit it to enter\n\
                                 items interactively (one per line, blank to finish). Use\n\
                                 --description to document it and --project to target a\n\
                                 project's .rules.yaml.\n\
                                 \n\
                                 Examples:\n\
                                 \n\
                                   meta rules create component \"src/components/*/\" -s \"[ComponentName].vue,index.ts\"\n\
                                   meta rules create comp \"src/plugins/*/\"",
                            )
                            .aliases(vec!["comp".to_string()])
                            .arg(
                                arg("pattern")
                                    .help("Component directory pattern")
                                    .required(true)
                                    .takes_value(true)
                            )
                            .arg(
                                arg("structure")
                                    .long("structure")
                                    .short('s')
                                    .help("Structure items (comma-separated)")
                                    .takes_value(true)
                            )
                            .arg(
                                arg("description")
                                    .long("description")
                                    .short('d')
                                    .help("Rule description")
                                    .takes_value(true)
                            )
                            .arg(
                                arg("project")
                                    .long("project")
                                    .short('p')
                                    .help("Add to specific project")
                                    .takes_value(true)
                            )
                    )
                    .subcommand(
                        command("file")
                            .about("Create a file rule")
                            .help_description(
                                "Add a rule that requires matching files to have companions or content.\n\
                                 \n\
                                 Records a file rule for the given glob pattern. Use --requires\n\
                                 to list requirements as comma-separated type:pattern pairs;\n\
                                 each pair checks for a companion file or, for content checks\n\
                                 such as #[test], a pattern inside the file. Add --description\n\
                                 to document it and --project to target a project's .rules.yaml.\n\
                                 \n\
                                 Examples:\n\
                                 \n\
                                   meta rules create file \"**/*.rs\" --requires test:#[test]\n\
                                   meta rules create f \"src/**/*.tsx\" -r test:.test.tsx,types:interface",
                            )
                            .aliases(vec!["f".to_string()])
                            .arg(
                                arg("pattern")
                                    .help("File pattern")
                                    .required(true)
                                    .takes_value(true)
                            )
                            .arg(
                                arg("requires")
                                    .long("requires")
                                    .short('r')
                                    .help("Required files (format: type:pattern)")
                                    .takes_value(true)
                            )
                            .arg(
                                arg("description")
                                    .long("description")
                                    .short('d')
                                    .help("Rule description")
                                    .takes_value(true)
                            )
                            .arg(
                                arg("project")
                                    .long("project")
                                    .short('p')
                                    .help("Add to specific project")
                                    .takes_value(true)
                            )
                    )
            )
            .command(
                command("status")
                    .about("Show rules status for all projects")
                    .help_description(
                        "Show which projects have their own rules versus inheriting the workspace.\n\
                         \n\
                         Lists every project in .meta and marks those with a project-specific\n\
                         .rules.yaml; the rest are noted as using the workspace rules. Use it\n\
                         to see at a glance where structure enforcement is customized before\n\
                         running check or copy.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules status",
                    )
            )
            .command(
                command("copy")
                    .about("Copy workspace rules to a specific project")
                    .help_description(
                        "Seed a project's .rules.yaml from the current workspace rules.\n\
                         \n\
                         Loads the workspace rules (or the built-in minimal set when none\n\
                         exist) and writes them to the target project's .rules.yaml so you\n\
                         can customize them per project. If the project already has its own\n\
                         .rules.yaml it is left untouched and a warning is printed.\n\
                         \n\
                         Examples:\n\
                         \n\
                           meta rules copy frontend",
                    )
                    .arg(
                        arg("project")
                            .help("Target project")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .handler("check", handle_check)
            .handler("init", handle_init)
            .handler("list", handle_list)
            .handler("docs", handle_docs)
            .handler("create", handle_create)
            .handler("status", handle_status)
            .handler("copy", handle_copy)
            .build()
    }
}

/// Handler for the check command
fn handle_check(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let fix = matches.get_flag("fix");
    let project = matches.get_one::<String>("project");

    let manager = ProjectRulesManager::new(config);

    // Determine which projects to check
    let projects = if let Some(project_name) = project {
        vec![project_name.clone()]
    } else {
        config.meta_config.projects.keys().cloned().collect()
    };

    let mut total_violations = 0;

    for project_name in projects {
        let project_path = manager.get_project_path(&project_name)?;

        if !project_path.exists() {
            println!("{}: {}", project_name.yellow(), "Directory not found".red());
            continue;
        }

        // Load project-specific or workspace rules
        let rules_config = manager.load_project_rules(&project_name)?;
        let engine = RuleEngine::new(rules_config.clone());

        println!("\n{} {}", "Checking project:".bold(), project_name.cyan());
        println!("{}", "=".repeat(50));

        // Show rules source
        let stats = RulesStats::from_config(
            &rules_config,
            super::project::check_project_rules_inheritance(&project_path),
        );
        stats.print();
        println!();

        let violations = engine.validate(&project_path)?;

        if violations.is_empty() {
            println!("✅ {}", "All rules passed!".green());
        } else {
            total_violations += violations.len();

            for violation in &violations {
                match violation.severity {
                    super::engine::Severity::Error => {
                        println!("❌ {} {}", "ERROR:".red().bold(), violation.message);
                    }
                    super::engine::Severity::Warning => {
                        println!("⚠️  {} {}", "WARNING:".yellow().bold(), violation.message);
                    }
                    super::engine::Severity::Info => {
                        println!("ℹ️  {} {}", "INFO:".blue().bold(), violation.message);
                    }
                }

                if let Some(path) = &violation.path {
                    println!("   {}: {}", "Path".dimmed(), path.display());
                }

                if violation.fixable {
                    println!("   {} This can be auto-fixed", "→".green());
                }
            }

            if fix {
                println!("\n{}", "Attempting to fix violations...".yellow());
                let fixable: Vec<_> = violations.iter().filter(|v| v.fixable).cloned().collect();

                if !fixable.is_empty() {
                    super::engine::fix_violations(&project_path, &fixable)?;
                    println!("✅ Fixed {} violations", fixable.len());
                }
            }
        }
    }

    if total_violations > 0 {
        println!(
            "\n{} Found {} total violations",
            "Summary:".bold(),
            total_violations
        );
        if !fix {
            println!("💡 Run with --fix to automatically fix fixable violations");
        }
    }

    Ok(())
}

/// Handler for the init command
fn handle_init(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let output_path = matches.get_one::<String>("output").unwrap();
    let project = matches.get_one::<String>("project");

    let full_path = if let Some(project_name) = project {
        let manager = ProjectRulesManager::new(config);
        let project_path = manager.get_project_path(project_name)?;
        project_path.join(output_path)
    } else if config.meta_root().is_some() {
        config.meta_root().unwrap().join(output_path)
    } else {
        config.working_dir.join(output_path)
    };

    if full_path.exists() {
        println!(
            "{} Rules configuration already exists at: {}",
            "Warning:".yellow(),
            full_path.display()
        );
        return Ok(());
    }

    let default_config = RulesConfig::default_config();
    let yaml = serde_yaml::to_string(&default_config)?;
    std::fs::write(&full_path, &yaml)?;

    println!("✅ Created rules configuration at: {}", full_path.display());
    if let Some(project_name) = project {
        println!("📝 Project {} now has specific rules", project_name.cyan());
    }
    println!("\n{}", "Example configuration:".bold());
    println!("{}", yaml);

    Ok(())
}

/// Handler for the list command
fn handle_list(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let project = matches.get_one::<String>("project");

    let rules_config = if let Some(project_name) = project {
        let manager = ProjectRulesManager::new(config);
        manager.load_project_rules(project_name)?
    } else {
        load_rules_config(config)?
    };

    println!("{}", "Configured Rules:".bold().underline());
    println!();

    if !rules_config.directories.is_empty() {
        println!("{}", "📁 Directory Rules:".cyan().bold());
        for dir_rule in &rules_config.directories {
            let required = if dir_rule.required {
                "required"
            } else {
                "optional"
            };
            println!("   • {} ({})", dir_rule.path, required.dimmed());
            if let Some(desc) = &dir_rule.description {
                println!("     {}", desc.dimmed());
            }
        }
        println!();
    }

    if !rules_config.components.is_empty() {
        println!("{}", "🧩 Component Rules:".cyan().bold());
        for comp_rule in &rules_config.components {
            println!("   • Pattern: {}", comp_rule.pattern.yellow());
            if let Some(desc) = &comp_rule.description {
                println!("     {}", desc.dimmed());
            }
            println!("     Structure:");
            for item in &comp_rule.structure {
                println!("       - {}", item);
            }
        }
        println!();
    }

    if !rules_config.files.is_empty() {
        println!("{}", "📄 File Rules:".cyan().bold());
        for file_rule in &rules_config.files {
            println!("   • Pattern: {}", file_rule.pattern.yellow());
            if let Some(desc) = &file_rule.description {
                println!("     {}", desc.dimmed());
            }
            if !file_rule.requires.is_empty() {
                println!("     Requires:");
                for (key, pattern) in &file_rule.requires {
                    println!("       - {}: {}", key, pattern);
                }
            }
        }
    }

    Ok(())
}

/// Handler for the docs command
fn handle_docs(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    if let Some(rule_type) = matches.get_one::<String>("type") {
        match rule_type.as_str() {
            "directory" | "dir" => super::docs::print_directory_rule_docs(),
            "component" | "comp" => super::docs::print_component_rule_docs(),
            "file" | "files" => super::docs::print_file_rule_docs(),
            "naming" | "name" => super::docs::print_naming_rule_docs(),
            "dependency" | "dep" | "deps" => super::docs::print_dependency_rule_docs(),
            "import" | "imports" => super::docs::print_import_rule_docs(),
            "documentation" | "doc" | "docs" => super::docs::print_documentation_rule_docs(),
            "size" => super::docs::print_size_rule_docs(),
            "security" | "sec" => super::docs::print_security_rule_docs(),
            _ => {
                println!("{} Unknown rule type: {}", "Error:".red(), rule_type);
                println!("Valid types: directory, component, file, naming, dependency, import, documentation, size, security");
            }
        }
    } else {
        super::docs::print_full_documentation();
    }
    Ok(())
}

/// Handler for the create command
fn handle_create(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    match matches.subcommand() {
        Some(("directory", sub_matches)) => {
            let path = sub_matches.get_one::<String>("path").unwrap();
            let required = sub_matches.get_flag("required");
            let description = sub_matches.get_one::<String>("description").cloned();
            let project = sub_matches.get_one::<String>("project").map(|s| s.as_str());

            let creator = RuleCreator::new(None);
            creator.create_directory_rule(path, required, description, project)?;
        }
        Some(("component", sub_matches)) => {
            let pattern = sub_matches.get_one::<String>("pattern").unwrap();
            let structure = if let Some(structure_str) = sub_matches.get_one::<String>("structure")
            {
                structure_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            } else {
                // Interactive mode to get structure
                println!("Enter structure items (empty line to finish):");
                let mut items = Vec::new();
                use std::io::{self, Write};
                loop {
                    print!("> ");
                    io::stdout().flush()?;
                    let mut line = String::new();
                    io::stdin().read_line(&mut line)?;
                    let line = line.trim();
                    if line.is_empty() {
                        break;
                    }
                    items.push(line.to_string());
                }
                items
            };
            let description = sub_matches.get_one::<String>("description").cloned();
            let project = sub_matches.get_one::<String>("project").map(|s| s.as_str());

            let creator = RuleCreator::new(None);
            creator.create_component_rule(pattern, structure, description, project)?;
        }
        Some(("file", sub_matches)) => {
            let pattern = sub_matches.get_one::<String>("pattern").unwrap();
            let mut requires = HashMap::new();

            if let Some(requires_str) = sub_matches.get_one::<String>("requires") {
                for req in requires_str.split(',') {
                    if let Some((key, value)) = req.split_once(':') {
                        requires.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
            }

            let description = sub_matches.get_one::<String>("description").cloned();
            let project = sub_matches.get_one::<String>("project").map(|s| s.as_str());

            let creator = RuleCreator::new(None);
            creator.create_file_rule(pattern, requires, description, project)?;
        }
        _ => {
            super::docs::print_create_help();
        }
    }
    Ok(())
}

/// Handler for the status command
fn handle_status(_matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let manager = ProjectRulesManager::new(config);
    manager.list_project_rules_status()?;
    Ok(())
}

/// Handler for the copy command
fn handle_copy(matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let project_name = matches.get_one::<String>("project").unwrap();
    let manager = ProjectRulesManager::new(config);
    manager.copy_workspace_rules_to_project(project_name)?;
    Ok(())
}

/// Load rules config helper function
fn load_rules_config(config: &RuntimeConfig) -> Result<RulesConfig> {
    let rules_path = if config.meta_root().is_some() {
        config.meta_root().unwrap().join(".rules.yaml")
    } else {
        config.working_dir.join(".rules.yaml")
    };

    if rules_path.exists() {
        super::config::load_config(rules_path)
    } else {
        Ok(RulesConfig::minimal())
    }
}

// Traditional implementation for backward compatibility
impl MetaPlugin for RulesPlugin {
    fn name(&self) -> &str {
        "rules"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.handle_command(matches, config)
    }
}

impl BasePlugin for RulesPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> Option<&str> {
        Some("Enforce project file structure rules")
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for RulesPlugin {
    fn default() -> Self {
        Self::new()
    }
}
