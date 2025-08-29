use anyhow::Result;
use crate::config::RulesConfig;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use glob::Pattern;

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct Violation {
    pub rule: String,
    pub message: String,
    pub severity: Severity,
    pub path: Option<PathBuf>,
    pub fixable: bool,
}

pub struct RuleEngine {
    config: RulesConfig,
}

impl RuleEngine {
    pub fn new(config: RulesConfig) -> Self {
        Self { config }
    }
    
    pub fn validate<P: AsRef<Path>>(&self, project_path: P) -> Result<Vec<Violation>> {
        let project_path = project_path.as_ref();
        let mut violations = Vec::new();
        
        // Check directory rules
        violations.extend(self.check_directory_rules(project_path)?);
        
        // Check component rules
        violations.extend(self.check_component_rules(project_path)?);
        
        // Check file rules
        violations.extend(self.check_file_rules(project_path)?);
        
        Ok(violations)
    }
    
    fn check_directory_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.directories {
            let dir_path = project_path.join(&rule.path);
            
            if !dir_path.exists() {
                if rule.required {
                    violations.push(Violation {
                        rule: format!("directory:{}", rule.path),
                        message: format!("Required directory '{}' is missing", rule.path),
                        severity: Severity::Error,
                        path: Some(dir_path.clone()),
                        fixable: true,
                    });
                } else {
                    violations.push(Violation {
                        rule: format!("directory:{}", rule.path),
                        message: format!("Optional directory '{}' is missing", rule.path),
                        severity: Severity::Info,
                        path: Some(dir_path.clone()),
                        fixable: true,
                    });
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_component_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.components {
            let pattern = if rule.pattern.ends_with('/') {
                &rule.pattern[..rule.pattern.len()-1]
            } else {
                &rule.pattern
            };
            
            // Find all directories matching the pattern
            let component_dirs = self.find_matching_dirs(project_path, pattern)?;
            
            for component_dir in component_dirs {
                let component_name = component_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");
                
                for structure_item in &rule.structure {
                    let expected_path = self.resolve_component_path(&component_dir, structure_item, component_name);
                    
                    if !expected_path.exists() {
                        let is_dir = structure_item.ends_with('/');
                        let item_type = if is_dir { "directory" } else { "file" };
                        
                        violations.push(Violation {
                            rule: format!("component:{}", rule.pattern),
                            message: format!(
                                "Component '{}' is missing {} '{}'",
                                component_name,
                                item_type,
                                structure_item.replace("[ComponentName]", component_name)
                            ),
                            severity: Severity::Error,
                            path: Some(expected_path.clone()),
                            fixable: is_dir,
                        });
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_file_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.files {
            let files = self.find_matching_files(project_path, &rule.pattern)?;
            
            for file_path in files {
                for (req_type, req_pattern) in &rule.requires {
                    let required_exists = self.check_required_file(&file_path, req_pattern)?;
                    
                    if !required_exists {
                        violations.push(Violation {
                            rule: format!("file:{}", rule.pattern),
                            message: format!(
                                "File '{}' is missing required {} matching '{}'",
                                file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                req_type,
                                req_pattern
                            ),
                            severity: Severity::Warning,
                            path: Some(file_path.clone()),
                            fixable: false,
                        });
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn find_matching_dirs(&self, base_path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut matching_dirs = Vec::new();
        let glob_pattern = Pattern::new(pattern)?;
        
        for entry in WalkDir::new(base_path).follow_links(true) {
            let entry = entry?;
            if entry.file_type().is_dir() {
                if let Ok(relative) = entry.path().strip_prefix(base_path) {
                    if glob_pattern.matches_path(relative) {
                        matching_dirs.push(entry.path().to_path_buf());
                    }
                }
            }
        }
        
        Ok(matching_dirs)
    }
    
    fn find_matching_files(&self, base_path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();
        let glob_pattern = Pattern::new(pattern)?;
        
        for entry in WalkDir::new(base_path).follow_links(true) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Ok(relative) = entry.path().strip_prefix(base_path) {
                    if glob_pattern.matches_path(relative) {
                        matching_files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
        
        Ok(matching_files)
    }
    
    fn resolve_component_path(&self, component_dir: &Path, structure_item: &str, component_name: &str) -> PathBuf {
        let resolved = structure_item.replace("[ComponentName]", component_name);
        component_dir.join(resolved.trim_end_matches('/'))
    }
    
    fn check_required_file(&self, file_path: &Path, pattern: &str) -> Result<bool> {
        // Special case: check for test annotations in the file itself
        if pattern == "#[test]" {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                return Ok(content.contains("#[test]") || content.contains("#[cfg(test)]"));
            }
            return Ok(false);
        }
        
        // Check for related files in the same directory or nearby
        let parent = file_path.parent().unwrap_or(Path::new("."));
        let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        
        // Replace wildcards with the actual file stem
        let actual_pattern = pattern.replace("*", file_stem);
        let glob_pattern = Pattern::new(&actual_pattern)?;
        
        // Look in the parent directory and common test directories
        let search_dirs = vec![
            parent.to_path_buf(),
            parent.join("__tests__"),
            parent.join("tests"),
            parent.parent().unwrap_or(Path::new(".")).join("__tests__"),
        ];
        
        for search_dir in search_dirs {
            if search_dir.exists() {
                for entry in std::fs::read_dir(search_dir)? {
                    let entry = entry?;
                    if entry.file_type()?.is_file() {
                        if let Some(name) = entry.file_name().to_str() {
                            if glob_pattern.matches(name) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(false)
    }
}

pub fn fix_violations<P: AsRef<Path>>(project_path: P, violations: &[Violation]) -> Result<()> {
    let _project_path = project_path.as_ref();
    
    for violation in violations {
        if !violation.fixable {
            continue;
        }
        
        if let Some(path) = &violation.path {
            // Only fix directory creation for now
            if violation.rule.starts_with("directory:") || violation.rule.starts_with("component:") {
                if !path.exists() {
                    std::fs::create_dir_all(path)?;
                    println!("  Created directory: {}", path.display());
                }
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DirectoryRule;
    use tempfile::tempdir;
    
    #[test]
    fn test_directory_rule_violation() {
        let temp = tempdir().unwrap();
        let config = RulesConfig {
            directories: vec![
                DirectoryRule {
                    path: "src".to_string(),
                    required: true,
                    description: None,
                },
            ],
            components: Vec::new(),
            files: Vec::new(),
        };
        
        let engine = RuleEngine::new(config);
        let violations = engine.validate(temp.path()).unwrap();
        
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Required directory 'src' is missing"));
        assert!(violations[0].fixable);
    }
    
    #[test]
    fn test_fix_directory_violation() {
        let temp = tempdir().unwrap();
        let src_path = temp.path().join("src");
        
        let violation = Violation {
            rule: "directory:src".to_string(),
            message: "Required directory 'src' is missing".to_string(),
            severity: Severity::Error,
            path: Some(src_path.clone()),
            fixable: true,
        };
        
        fix_violations(temp.path(), &[violation]).unwrap();
        assert!(src_path.exists());
    }
}