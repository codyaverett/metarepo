use anyhow::Result;
use crate::config::RulesConfig;
use meta_core::RuntimeConfig;
use std::path::{Path, PathBuf};
use colored::*;

pub struct ProjectRulesManager<'a> {
    runtime_config: &'a RuntimeConfig,
}

impl<'a> ProjectRulesManager<'a> {
    pub fn new(runtime_config: &'a RuntimeConfig) -> Self {
        Self { runtime_config }
    }
    
    /// Load rules for a specific project, with fallback to workspace rules
    pub fn load_project_rules(&self, project_name: &str) -> Result<RulesConfig> {
        // Try project-specific rules first
        if let Ok(project_rules) = self.load_project_specific_rules(project_name) {
            println!("📋 Using project-specific rules for {}", project_name.cyan());
            return Ok(project_rules);
        }
        
        // Try rules from .meta file
        if let Ok(meta_rules) = self.load_meta_project_rules(project_name) {
            println!("📋 Using rules from .meta for {}", project_name.cyan());
            return Ok(meta_rules);
        }
        
        // Fall back to workspace rules
        self.load_workspace_rules()
    }
    
    /// Load rules from project's .rules.yaml file
    fn load_project_specific_rules(&self, project_name: &str) -> Result<RulesConfig> {
        let project_path = self.get_project_path(project_name)?;
        let rules_file = project_path.join(".rules.yaml");
        
        if rules_file.exists() {
            crate::config::load_config(rules_file)
        } else {
            Err(anyhow::anyhow!("No project-specific rules found"))
        }
    }
    
    /// Load rules from .meta file's project configuration
    fn load_meta_project_rules(&self, _project_name: &str) -> Result<RulesConfig> {
        // Check if project has rules defined in .meta
        // This would require extending the .meta format to include rules
        // For now, return error to fall back to workspace rules
        Err(anyhow::anyhow!("No rules in .meta for project"))
    }
    
    /// Load workspace-wide rules
    fn load_workspace_rules(&self) -> Result<RulesConfig> {
        let rules_path = if self.runtime_config.meta_root().is_some() {
            self.runtime_config.meta_root().unwrap().join(".rules.yaml")
        } else {
            self.runtime_config.working_dir.join(".rules.yaml")
        };
        
        if rules_path.exists() {
            println!("📋 Using workspace rules");
            crate::config::load_config(rules_path)
        } else {
            println!("📋 Using default minimal rules");
            Ok(RulesConfig::minimal())
        }
    }
    
    /// Get the full path to a project directory
    pub fn get_project_path(&self, project_name: &str) -> Result<PathBuf> {
        if self.runtime_config.meta_config.projects.contains_key(project_name) {
            let base_path = if self.runtime_config.meta_root().is_some() {
                self.runtime_config.meta_root().unwrap()
            } else {
                self.runtime_config.working_dir.clone()
            };
            Ok(base_path.join(project_name))
        } else {
            Err(anyhow::anyhow!("Project '{}' not found in .meta", project_name))
        }
    }
    
    /// List all projects and their rules status
    pub fn list_project_rules_status(&self) -> Result<()> {
        println!("{}", "PROJECT RULES STATUS".cyan().bold());
        println!("{}", "════════════════════".blue());
        println!();
        
        for (project_name, _) in &self.runtime_config.meta_config.projects {
            let project_path = self.get_project_path(project_name)?;
            let has_specific_rules = project_path.join(".rules.yaml").exists();
            
            if has_specific_rules {
                println!("✅ {} - {}", project_name.green(), "Has project-specific rules".dimmed());
            } else {
                println!("   {} - {}", project_name.yellow(), "Using workspace rules".dimmed());
            }
        }
        
        Ok(())
    }
    
    /// Copy workspace rules to a specific project
    pub fn copy_workspace_rules_to_project(&self, project_name: &str) -> Result<()> {
        let workspace_rules = self.load_workspace_rules()?;
        let project_path = self.get_project_path(project_name)?;
        let project_rules_path = project_path.join(".rules.yaml");
        
        if project_rules_path.exists() {
            println!("{} Project {} already has specific rules", 
                    "Warning:".yellow(), 
                    project_name);
            return Ok(());
        }
        
        crate::config::save_config(&project_rules_path, &workspace_rules)?;
        
        println!("✅ Copied workspace rules to project {}", project_name.green());
        println!("   Edit {} to customize", project_rules_path.display());
        
        Ok(())
    }
    
    /// Merge project rules with workspace rules
    pub fn merge_rules(&self, project_rules: RulesConfig, workspace_rules: RulesConfig) -> RulesConfig {
        RulesConfig {
            directories: [project_rules.directories, workspace_rules.directories].concat(),
            components: [project_rules.components, workspace_rules.components].concat(),
            files: [project_rules.files, workspace_rules.files].concat(),
            naming: [project_rules.naming, workspace_rules.naming].concat(),
            dependencies: [project_rules.dependencies, workspace_rules.dependencies].concat(),
            imports: [project_rules.imports, workspace_rules.imports].concat(),
            documentation: [project_rules.documentation, workspace_rules.documentation].concat(),
            size: [project_rules.size, workspace_rules.size].concat(),
            security: [project_rules.security, workspace_rules.security].concat(),
        }
    }
}

/// Check if a project uses specific rules or inherits workspace rules
pub fn check_project_rules_inheritance(project_path: &Path) -> String {
    let rules_file = project_path.join(".rules.yaml");
    if rules_file.exists() {
        "project-specific".to_string()
    } else {
        "workspace".to_string()
    }
}

/// Get rules statistics for reporting
pub struct RulesStats {
    pub total_directories: usize,
    pub total_components: usize,
    pub total_files: usize,
    pub source: String,
}

impl RulesStats {
    pub fn from_config(config: &RulesConfig, source: String) -> Self {
        Self {
            total_directories: config.directories.len(),
            total_components: config.components.len(),
            total_files: config.files.len(),
            source,
        }
    }
    
    pub fn print(&self) {
        println!("📊 Rules Statistics:");
        println!("   Source: {}", self.source.cyan());
        println!("   Directory rules: {}", self.total_directories);
        println!("   Component rules: {}", self.total_components);
        println!("   File rules: {}", self.total_files);
    }
}