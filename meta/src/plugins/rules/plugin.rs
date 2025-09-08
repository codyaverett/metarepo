use anyhow::Result;
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, HelpFormat,
    plugin, command, arg,
};
use super::config::RulesConfig;
use super::engine::RuleEngine;
use super::project::{ProjectRulesManager, RulesStats};
use super::create::RuleCreator;
use colored::*;
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
            .command(
                command("check")
                    .about("Check project structure against configured rules")
                    .alias("c")
                    .alias("chk")
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
                    .alias("i")
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
                    .alias("ls")
                    .alias("l")
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
                    .alias("d")
                    .arg(
                        arg("type")
                            .help("Show docs for specific rule type (directory, component, file, naming, dependency, import, documentation, size, security)")
                            .takes_value(true)
                    )
                    .arg(
                        arg("ai")
                            .long("ai")
                            .help("Output in AI-optimized format (structured markdown)")
                    )
            )
            .command(
                command("create")
                    .about("Create a new rule")
                    .alias("new")
                    .subcommand(
                        command("directory")
                            .about("Create a directory rule")
                            .alias("dir")
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
                            .alias("comp")
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
                            .alias("f")
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
            )
            .command(
                command("copy")
                    .about("Copy workspace rules to a specific project")
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
        let stats = RulesStats::from_config(&rules_config, 
            super::project::check_project_rules_inheritance(&project_path));
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
        println!("\n{} Found {} total violations", "Summary:".bold(), total_violations);
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
        println!("{} Rules configuration already exists at: {}", 
                "Warning:".yellow(), 
                full_path.display());
        return Ok(());
    }
    
    let default_config = RulesConfig::default_config();
    let yaml = serde_yaml::to_string(&default_config)?;
    std::fs::write(&full_path, &yaml)?;
    
    println!("✅ Created rules configuration at: {}", full_path.display());
    if project.is_some() {
        println!("📝 Project {} now has specific rules", project.unwrap().cyan());
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
            let required = if dir_rule.required { "required" } else { "optional" };
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
    let ai_mode = matches.get_flag("ai");
    
    if let Some(rule_type) = matches.get_one::<String>("type") {
        if ai_mode {
            // In AI mode, just print the optimized full docs
            super::docs::print_full_documentation_ai();
        } else {
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
        }
    } else {
        if ai_mode {
            super::docs::print_full_documentation_ai();
        } else {
            super::docs::print_full_documentation();
        }
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
            let structure = if let Some(structure_str) = sub_matches.get_one::<String>("structure") {
                structure_str.split(',').map(|s| s.trim().to_string()).collect()
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
    
    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Check for output format flag
        if let Some(format_str) = matches.get_one::<String>("output-format") {
            if let Some(format) = HelpFormat::from_str(format_str) {
                return self.show_help(format);
            }
        }
        
        // Check for AI help flag
        if matches.get_flag("ai") {
            return self.show_ai_help();
        }
        
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