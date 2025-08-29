use anyhow::Result;
use crate::config::{RulesConfig, DirectoryRule, ComponentRule, FileRule};
use std::path::Path;
use std::collections::HashMap;
use colored::*;

pub struct RuleCreator {
    config_path: Option<String>,
}

impl RuleCreator {
    pub fn new(config_path: Option<String>) -> Self {
        Self { config_path }
    }
    
    pub fn create_directory_rule(
        &self,
        path: &str,
        required: bool,
        description: Option<String>,
        project: Option<&str>,
    ) -> Result<()> {
        let mut config = self.load_or_create_config(project)?;
        
        // Check if rule already exists
        if config.directories.iter().any(|r| r.path == path) {
            println!("{} Directory rule for '{}' already exists", "Warning:".yellow(), path);
            return Ok(());
        }
        
        let new_rule = DirectoryRule {
            path: path.to_string(),
            required,
            description,
        };
        
        config.directories.push(new_rule);
        self.save_config(&config, project)?;
        
        println!("✅ Created directory rule:");
        println!("   Path: {}", path.green());
        println!("   Required: {}", if required { "yes".green() } else { "no".yellow() });
        if let Some(desc) = &description {
            println!("   Description: {}", desc);
        }
        
        Ok(())
    }
    
    pub fn create_component_rule(
        &self,
        pattern: &str,
        structure: Vec<String>,
        description: Option<String>,
        project: Option<&str>,
    ) -> Result<()> {
        let mut config = self.load_or_create_config(project)?;
        
        // Check if rule already exists
        if config.components.iter().any(|r| r.pattern == pattern) {
            println!("{} Component rule for '{}' already exists", "Warning:".yellow(), pattern);
            return Ok(());
        }
        
        let new_rule = ComponentRule {
            pattern: pattern.to_string(),
            structure,
            description,
        };
        
        config.components.push(new_rule.clone());
        self.save_config(&config, project)?;
        
        println!("✅ Created component rule:");
        println!("   Pattern: {}", pattern.green());
        println!("   Structure:");
        for item in &new_rule.structure {
            println!("     - {}", item.cyan());
        }
        if let Some(desc) = &description {
            println!("   Description: {}", desc);
        }
        
        Ok(())
    }
    
    pub fn create_file_rule(
        &self,
        pattern: &str,
        requires: HashMap<String, String>,
        description: Option<String>,
        project: Option<&str>,
    ) -> Result<()> {
        let mut config = self.load_or_create_config(project)?;
        
        // Check if rule already exists
        if config.files.iter().any(|r| r.pattern == pattern) {
            println!("{} File rule for '{}' already exists", "Warning:".yellow(), pattern);
            return Ok(());
        }
        
        let new_rule = FileRule {
            pattern: pattern.to_string(),
            requires,
            description,
        };
        
        config.files.push(new_rule.clone());
        self.save_config(&config, project)?;
        
        println!("✅ Created file rule:");
        println!("   Pattern: {}", pattern.green());
        if !new_rule.requires.is_empty() {
            println!("   Requires:");
            for (key, value) in &new_rule.requires {
                println!("     - {}: {}", key.yellow(), value.cyan());
            }
        }
        if let Some(desc) = &description {
            println!("   Description: {}", desc);
        }
        
        Ok(())
    }
    
    fn load_or_create_config(&self, project: Option<&str>) -> Result<RulesConfig> {
        let config_path = self.get_config_path(project)?;
        
        if config_path.exists() {
            crate::config::load_config(&config_path)
        } else {
            Ok(RulesConfig::new())
        }
    }
    
    fn save_config(&self, config: &RulesConfig, project: Option<&str>) -> Result<()> {
        let config_path = self.get_config_path(project)?;
        crate::config::save_config(&config_path, config)?;
        
        println!("📝 Updated rules configuration: {}", config_path.display());
        Ok(())
    }
    
    fn get_config_path(&self, project: Option<&str>) -> Result<std::path::PathBuf> {
        if let Some(project_name) = project {
            // Project-specific rules
            let project_path = self.get_project_path(project_name)?;
            Ok(project_path.join(".rules.yaml"))
        } else if let Some(path) = &self.config_path {
            // Custom path provided
            Ok(std::path::PathBuf::from(path))
        } else {
            // Default workspace rules
            Ok(std::path::PathBuf::from(".rules.yaml"))
        }
    }
    
    fn get_project_path(&self, project_name: &str) -> Result<std::path::PathBuf> {
        // This will be enhanced when we add project plugin dependency
        // For now, assume projects are in the current directory
        Ok(std::path::PathBuf::from(project_name))
    }
}

pub fn interactive_create_directory_rule() -> Result<()> {
    println!("{}", "Creating Directory Rule".cyan().bold());
    println!("{}", "═══════════════════════".blue());
    println!();
    
    print!("Directory path: ");
    use std::io::{self, Write};
    io::stdout().flush()?;
    let mut path = String::new();
    io::stdin().read_line(&mut path)?;
    let path = path.trim();
    
    print!("Required? (y/n) [y]: ");
    io::stdout().flush()?;
    let mut required = String::new();
    io::stdin().read_line(&mut required)?;
    let required = !required.trim().eq_ignore_ascii_case("n");
    
    print!("Description (optional): ");
    io::stdout().flush()?;
    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim();
    let description = if description.is_empty() { None } else { Some(description.to_string()) };
    
    let creator = RuleCreator::new(None);
    creator.create_directory_rule(path, required, description, None)?;
    
    Ok(())
}

pub fn interactive_create_component_rule() -> Result<()> {
    println!("{}", "Creating Component Rule".cyan().bold());
    println!("{}", "═══════════════════════".blue());
    println!();
    
    print!("Component pattern (e.g., 'components/**/'): ");
    use std::io::{self, Write};
    io::stdout().flush()?;
    let mut pattern = String::new();
    io::stdin().read_line(&mut pattern)?;
    let pattern = pattern.trim();
    
    println!("Enter structure items (one per line, empty line to finish):");
    println!("Use [ComponentName] as placeholder");
    let mut structure = Vec::new();
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut item = String::new();
        io::stdin().read_line(&mut item)?;
        let item = item.trim();
        if item.is_empty() {
            break;
        }
        structure.push(item.to_string());
    }
    
    print!("Description (optional): ");
    io::stdout().flush()?;
    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim();
    let description = if description.is_empty() { None } else { Some(description.to_string()) };
    
    let creator = RuleCreator::new(None);
    creator.create_component_rule(pattern, structure, description, None)?;
    
    Ok(())
}

pub fn interactive_create_file_rule() -> Result<()> {
    println!("{}", "Creating File Rule".cyan().bold());
    println!("{}", "══════════════════".blue());
    println!();
    
    print!("File pattern (e.g., '**/*.vue'): ");
    use std::io::{self, Write};
    io::stdout().flush()?;
    let mut pattern = String::new();
    io::stdin().read_line(&mut pattern)?;
    let pattern = pattern.trim();
    
    println!("Enter required files (format: 'type:pattern', empty line to finish):");
    println!("Example: test:*.test.js");
    let mut requires = HashMap::new();
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut item = String::new();
        io::stdin().read_line(&mut item)?;
        let item = item.trim();
        if item.is_empty() {
            break;
        }
        
        if let Some((key, value)) = item.split_once(':') {
            requires.insert(key.trim().to_string(), value.trim().to_string());
        } else {
            println!("Invalid format. Use 'type:pattern'");
        }
    }
    
    print!("Description (optional): ");
    io::stdout().flush()?;
    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim();
    let description = if description.is_empty() { None } else { Some(description.to_string()) };
    
    let creator = RuleCreator::new(None);
    creator.create_file_rule(pattern, requires, description, None)?;
    
    Ok(())
}