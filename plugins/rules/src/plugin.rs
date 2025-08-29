use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use meta_core::{MetaPlugin, RuntimeConfig};
use crate::{RulesConfig, RuleEngine};
use colored::*;

pub struct RulesPlugin;

impl RulesPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("gest rules")
            .about("Enforce project file structure rules")
            .subcommand(
                Command::new("check")
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
                    .about("Initialize rules configuration file")
                    .arg(
                        Arg::new("output")
                            .long("output")
                            .short('o')
                            .value_name("FILE")
                            .help("Output file path (default: .rules.yaml)")
                            .default_value(".rules.yaml")
                    )
            )
            .subcommand(
                Command::new("list")
                    .about("List all configured rules")
            );
        
        app.print_help()?;
        println!();
        Ok(())
    }
    
    fn handle_check(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let fix = matches.get_flag("fix");
        let project = matches.get_one::<String>("project");
        
        // Load rules configuration
        let rules_config = self.load_rules_config(config)?;
        let engine = RuleEngine::new(rules_config);
        
        // Determine which projects to check
        let projects = if let Some(project_name) = project {
            vec![project_name.clone()]
        } else {
            config.meta_config.projects.keys().cloned().collect()
        };
        
        let mut total_violations = 0;
        
        for project_name in projects {
            let project_path = if config.meta_root().is_some() {
                config.meta_root().unwrap().join(&project_name)
            } else {
                config.working_dir.join(&project_name)
            };
            
            if !project_path.exists() {
                println!("{}: {}", project_name.yellow(), "Directory not found".red());
                continue;
            }
            
            println!("\n{} {}", "Checking project:".bold(), project_name.cyan());
            println!("{}", "=".repeat(50));
            
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
                    let fixable: Vec<_> = violations.iter().filter(|v| v.fixable).cloned().collect();
                    
                    if !fixable.is_empty() {
                        crate::engine::fix_violations(&project_path, &fixable)?;
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
    
    fn handle_init(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let output_path = matches.get_one::<String>("output").unwrap();
        let full_path = if config.meta_root().is_some() {
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
        println!("\n{}", "Example configuration:".bold());
        println!("{}", yaml);
        
        Ok(())
    }
    
    fn handle_list(&self, config: &RuntimeConfig) -> Result<()> {
        let rules_config = self.load_rules_config(config)?;
        
        println!("{}", "Configured Rules:".bold().underline());
        println!();
        
        if !rules_config.directories.is_empty() {
            println!("{}", "📁 Directory Rules:".cyan().bold());
            for dir_rule in &rules_config.directories {
                let required = if dir_rule.required { "required" } else { "optional" };
                println!("   • {} ({})", dir_rule.path, required.dimmed());
            }
            println!();
        }
        
        if !rules_config.components.is_empty() {
            println!("{}", "🧩 Component Rules:".cyan().bold());
            for comp_rule in &rules_config.components {
                println!("   • Pattern: {}", comp_rule.pattern.yellow());
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
    
    fn load_rules_config(&self, config: &RuntimeConfig) -> Result<RulesConfig> {
        // Try to find .rules.yaml in the meta root or working directory
        let rules_path = if config.meta_root().is_some() {
            config.meta_root().unwrap().join(".rules.yaml")
        } else {
            config.working_dir.join(".rules.yaml")
        };
        
        if rules_path.exists() {
            crate::config::load_config(rules_path)
        } else {
            // Return a default minimal config if no rules file exists
            Ok(RulesConfig::minimal())
        }
    }
}

impl MetaPlugin for RulesPlugin {
    fn name(&self) -> &str {
        "rules"
    }
    
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("rules")
                .about("Enforce project file structure rules")
                .allow_external_subcommands(true)
                .subcommand(
                    Command::new("check")
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
                        .about("Initialize rules configuration file")
                        .arg(
                            Arg::new("output")
                                .long("output")
                                .short('o')
                                .value_name("FILE")
                                .help("Output file path (default: .rules.yaml)")
                                .default_value(".rules.yaml")
                        )
                )
                .subcommand(
                    Command::new("list")
                        .about("List all configured rules")
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if matches.subcommand().is_none() {
            return self.show_help();
        }
        
        match matches.subcommand() {
            Some(("check", sub_matches)) => self.handle_check(sub_matches, config),
            Some(("init", sub_matches)) => self.handle_init(sub_matches, config),
            Some(("list", _)) => self.handle_list(config),
            Some((external_cmd, _)) => {
                println!("Unknown rules subcommand: '{}'", external_cmd);
                println!();
                self.show_help()
            }
            None => self.show_help()
        }
    }
}

impl Default for RulesPlugin {
    fn default() -> Self {
        Self::new()
    }
}