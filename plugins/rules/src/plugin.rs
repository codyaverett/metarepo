use crate::create::RuleCreator;
use crate::project::{ProjectRulesManager, RulesStats};
use crate::{RuleEngine, RulesConfig};
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use colored::*;
use meta_core::{MetaPlugin, RuntimeConfig};
use std::collections::HashMap;

pub struct RulesPlugin;

impl RulesPlugin {
    pub fn new() -> Self {
        Self
    }

    fn show_help(&self) -> Result<()> {
        let mut app = self.build_cli();
        app.print_help()?;
        println!();
        Ok(())
    }

    fn build_cli(&self) -> Command {
        Command::new("rules")
            .visible_alias("r")
            .about("Enforce project file structure rules")
            .disable_help_subcommand(true)
            .subcommand(
                Command::new("check")
                    .visible_aliases(["c", "chk"])
                    .about("Check project structure against configured rules")
                    .arg(
                        Arg::new("project")
                            .long("project")
                            .short('p')
                            .value_name("PROJECT")
                            .help("Specific project to check (defaults to all)")
                    )
                    .arg(
                        Arg::new("fix")
                            .long("fix")
                            .help("Automatically fix violations where possible")
                            .action(clap::ArgAction::SetTrue)
                    )
            )
            .subcommand(
                Command::new("init")
                    .visible_alias("i")
                    .about("Initialize rules configuration file")
                    .arg(
                        Arg::new("output")
                            .long("output")
                            .short('o')
                            .value_name("FILE")
                            .help("Output file path (default: .rules.yaml)")
                            .default_value(".rules.yaml")
                    )
                    .arg(
                        Arg::new("project")
                            .long("project")
                            .short('p')
                            .value_name("PROJECT")
                            .help("Initialize rules for specific project")
                    )
            )
            .subcommand(
                Command::new("list")
                    .visible_aliases(["ls", "l"])
                    .about("List all configured rules")
                    .arg(
                        Arg::new("project")
                            .long("project")
                            .short('p')
                            .value_name("PROJECT")
                            .help("List rules for specific project")
                    )
            )
            .subcommand(
                Command::new("docs")
                    .visible_alias("d")
                    .about("Show documentation for creating and using rules")
                    .arg(
                        Arg::new("type")
                            .value_name("TYPE")
                            .help("Show docs for specific rule type (directory, component, file, naming, dependency, import, documentation, size, security)")
                    )
                    .arg(
                        Arg::new("ai")
                            .long("ai")
                            .help("Output in AI-optimized format (structured markdown)")
                            .action(clap::ArgAction::SetTrue)
                    )
            )
            .subcommand(
                Command::new("create")
                    .visible_alias("new")
                    .about("Create a new rule")
                    .subcommand(
                        Command::new("directory")
                            .visible_alias("dir")
                            .about("Create a directory rule")
                            .arg(
                                Arg::new("path")
                                    .value_name("PATH")
                                    .help("Directory path")
                                    .required(true)
                            )
                            .arg(
                                Arg::new("required")
                                    .long("required")
                                    .help("Mark as required")
                                    .action(clap::ArgAction::SetTrue)
                            )
                            .arg(
                                Arg::new("description")
                                    .long("description")
                                    .short('d')
                                    .value_name("TEXT")
                                    .help("Rule description")
                            )
                            .arg(
                                Arg::new("project")
                                    .long("project")
                                    .short('p')
                                    .value_name("PROJECT")
                                    .help("Add to specific project")
                            )
                    )
                    .subcommand(
                        Command::new("component")
                            .visible_alias("comp")
                            .about("Create a component rule")
                            .arg(
                                Arg::new("pattern")
                                    .value_name("PATTERN")
                                    .help("Component directory pattern")
                                    .required(true)
                            )
                            .arg(
                                Arg::new("structure")
                                    .long("structure")
                                    .short('s')
                                    .value_name("ITEMS")
                                    .help("Structure items (comma-separated)")
                                    .value_delimiter(',')
                            )
                            .arg(
                                Arg::new("description")
                                    .long("description")
                                    .short('d')
                                    .value_name("TEXT")
                                    .help("Rule description")
                            )
                            .arg(
                                Arg::new("project")
                                    .long("project")
                                    .short('p')
                                    .value_name("PROJECT")
                                    .help("Add to specific project")
                            )
                    )
                    .subcommand(
                        Command::new("file")
                            .visible_alias("f")
                            .about("Create a file rule")
                            .arg(
                                Arg::new("pattern")
                                    .value_name("PATTERN")
                                    .help("File pattern")
                                    .required(true)
                            )
                            .arg(
                                Arg::new("requires")
                                    .long("requires")
                                    .short('r')
                                    .value_name("KEY:PATTERN")
                                    .help("Required files (format: type:pattern)")
                                    .value_delimiter(',')
                            )
                            .arg(
                                Arg::new("description")
                                    .long("description")
                                    .short('d')
                                    .value_name("TEXT")
                                    .help("Rule description")
                            )
                            .arg(
                                Arg::new("project")
                                    .long("project")
                                    .short('p')
                                    .value_name("PROJECT")
                                    .help("Add to specific project")
                            )
                    )
            )
            .subcommand(
                Command::new("status")
                    .about("Show rules status for all projects")
            )
            .subcommand(
                Command::new("copy")
                    .about("Copy workspace rules to a specific project")
                    .arg(
                        Arg::new("project")
                            .value_name("PROJECT")
                            .help("Target project")
                            .required(true)
                    )
            )
    }

    fn handle_check(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
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
                crate::project::check_project_rules_inheritance(&project_path),
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
                        crate::engine::Severity::Error => {
                            println!("❌ {} {}", "ERROR:".red().bold(), violation.message);
                        }
                        crate::engine::Severity::Warning => {
                            println!("⚠️  {} {}", "WARNING:".yellow().bold(), violation.message);
                        }
                        crate::engine::Severity::Info => {
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
                    let fixable: Vec<_> =
                        violations.iter().filter(|v| v.fixable).cloned().collect();

                    if !fixable.is_empty() {
                        crate::engine::fix_violations(&project_path, &fixable)?;
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

    fn handle_init(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
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
        if project.is_some() {
            println!(
                "📝 Project {} now has specific rules",
                project.unwrap().cyan()
            );
        }
        println!("\n{}", "Example configuration:".bold());
        println!("{}", yaml);

        Ok(())
    }

    fn handle_list(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let project = matches.get_one::<String>("project");

        let rules_config = if let Some(project_name) = project {
            let manager = ProjectRulesManager::new(config);
            manager.load_project_rules(project_name)?
        } else {
            self.load_rules_config(config)?
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

    fn handle_docs(&self, matches: &ArgMatches) -> Result<()> {
        let ai_mode = matches.get_flag("ai");

        if let Some(rule_type) = matches.get_one::<String>("type") {
            if ai_mode {
                // In AI mode, just print the optimized full docs
                crate::docs::print_full_documentation_ai();
            } else {
                match rule_type.as_str() {
                    "directory" | "dir" => crate::docs::print_directory_rule_docs(),
                    "component" | "comp" => crate::docs::print_component_rule_docs(),
                    "file" | "files" => crate::docs::print_file_rule_docs(),
                    "naming" | "name" => crate::docs::print_naming_rule_docs(),
                    "dependency" | "dep" | "deps" => crate::docs::print_dependency_rule_docs(),
                    "import" | "imports" => crate::docs::print_import_rule_docs(),
                    "documentation" | "doc" | "docs" => {
                        crate::docs::print_documentation_rule_docs()
                    }
                    "size" => crate::docs::print_size_rule_docs(),
                    "security" | "sec" => crate::docs::print_security_rule_docs(),
                    _ => {
                        println!("{} Unknown rule type: {}", "Error:".red(), rule_type);
                        println!("Valid types: directory, component, file, naming, dependency, import, documentation, size, security");
                    }
                }
            }
        } else {
            if ai_mode {
                crate::docs::print_full_documentation_ai();
            } else {
                crate::docs::print_full_documentation();
            }
        }
        Ok(())
    }

    fn handle_create(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
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
                let structure = if let Some(items) = sub_matches.get_many::<String>("structure") {
                    items.cloned().collect()
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

                if let Some(reqs) = sub_matches.get_many::<String>("requires") {
                    for req in reqs {
                        if let Some((key, value)) = req.split_once(':') {
                            requires.insert(key.to_string(), value.to_string());
                        }
                    }
                }

                let description = sub_matches.get_one::<String>("description").cloned();
                let project = sub_matches.get_one::<String>("project").map(|s| s.as_str());

                let creator = RuleCreator::new(None);
                creator.create_file_rule(pattern, requires, description, project)?;
            }
            _ => {
                crate::docs::print_create_help();
            }
        }
        Ok(())
    }

    fn handle_status(&self, config: &RuntimeConfig) -> Result<()> {
        let manager = ProjectRulesManager::new(config);
        manager.list_project_rules_status()?;
        Ok(())
    }

    fn handle_copy(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let project_name = matches.get_one::<String>("project").unwrap();
        let manager = ProjectRulesManager::new(config);
        manager.copy_workspace_rules_to_project(project_name)?;
        Ok(())
    }

    fn load_rules_config(&self, config: &RuntimeConfig) -> Result<RulesConfig> {
        let rules_path = if config.meta_root().is_some() {
            config.meta_root().unwrap().join(".rules.yaml")
        } else {
            config.working_dir.join(".rules.yaml")
        };

        if rules_path.exists() {
            crate::config::load_config(rules_path)
        } else {
            Ok(RulesConfig::minimal())
        }
    }
}

impl MetaPlugin for RulesPlugin {
    fn name(&self) -> &str {
        "rules"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(self.build_cli().allow_external_subcommands(true))
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if matches.subcommand().is_none() {
            return self.show_help();
        }

        match matches.subcommand() {
            Some(("check", sub_matches)) => self.handle_check(sub_matches, config),
            Some(("init", sub_matches)) => self.handle_init(sub_matches, config),
            Some(("list", sub_matches)) => self.handle_list(sub_matches, config),
            Some(("docs", sub_matches)) => self.handle_docs(sub_matches),
            Some(("create", sub_matches)) => self.handle_create(sub_matches, config),
            Some(("status", _)) => self.handle_status(config),
            Some(("copy", sub_matches)) => self.handle_copy(sub_matches, config),
            Some((external_cmd, _)) => {
                println!("Unknown rules subcommand: '{}'", external_cmd);
                println!();
                self.show_help()
            }
            None => self.show_help(),
        }
    }
}

impl Default for RulesPlugin {
    fn default() -> Self {
        Self::new()
    }
}
