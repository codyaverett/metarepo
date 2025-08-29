pub mod plugin;
pub mod config;
pub mod engine;
pub mod validators;
pub mod docs;
pub mod create;
pub mod project;

pub use plugin::RulesPlugin;
pub use config::{
    RulesConfig, DirectoryRule, ComponentRule, FileRule,
    NamingRule, DependencyRule, ImportRule, DocumentationRule,
    SizeRule, SecurityRule
};
pub use engine::{RuleEngine, Violation, Severity};

use anyhow::Result;
use std::path::Path;

pub fn load_rules_config<P: AsRef<Path>>(path: P) -> Result<RulesConfig> {
    config::load_config(path)
}

pub fn validate_project<P: AsRef<Path>>(project_path: P, config: &RulesConfig) -> Result<Vec<Violation>> {
    let engine = RuleEngine::new(config.clone());
    engine.validate(project_path)
}

pub fn fix_violations<P: AsRef<Path>>(project_path: P, violations: &[Violation]) -> Result<()> {
    engine::fix_violations(project_path, violations)
}