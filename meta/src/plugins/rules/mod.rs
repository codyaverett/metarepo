pub mod config;
pub mod create;
pub mod docs;
pub mod engine;
pub mod plugin;
pub mod project;
pub mod validators;

// Export the main plugin
pub use config::{
    ComponentRule, DependencyRule, DirectoryRule, DocumentationRule, FileRule, ImportRule,
    NamingRule, RulesConfig, SecurityRule, SizeRule,
};
pub use engine::{RuleEngine, Severity, Violation};
pub use plugin::RulesPlugin;

use anyhow::Result;
use std::path::Path;

pub fn load_rules_config<P: AsRef<Path>>(path: P) -> Result<RulesConfig> {
    config::load_config(path)
}

pub fn validate_project<P: AsRef<Path>>(
    project_path: P,
    config: &RulesConfig,
) -> Result<Vec<Violation>> {
    let engine = RuleEngine::new(config.clone());
    engine.validate(project_path)
}

pub fn fix_violations<P: AsRef<Path>>(project_path: P, violations: &[Violation]) -> Result<()> {
    engine::fix_violations(project_path, violations)
}
