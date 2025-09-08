use anyhow::Result;
use super::config::RulesConfig;
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
        
        // Check naming rules
        violations.extend(self.check_naming_rules(project_path)?);
        
        // Check dependency rules
        violations.extend(self.check_dependency_rules(project_path)?);
        
        // Check import rules
        violations.extend(self.check_import_rules(project_path)?);
        
        // Check documentation rules
        violations.extend(self.check_documentation_rules(project_path)?);
        
        // Check size rules
        violations.extend(self.check_size_rules(project_path)?);
        
        // Check security rules
        violations.extend(self.check_security_rules(project_path)?);
        
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
    
    fn check_naming_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.naming {
            let files = self.find_matching_files(project_path, &rule.pattern)?;
            let naming_regex = regex::Regex::new(&rule.naming_pattern)?;
            
            for file_path in files {
                if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                    if !naming_regex.is_match(file_name) {
                        let case_msg = rule.case_style.as_ref()
                            .map(|s| format!(" (should be {})", s))
                            .unwrap_or_default();
                        
                        violations.push(Violation {
                            rule: format!("naming:{}", rule.pattern),
                            message: format!(
                                "File '{}' does not match naming pattern '{}'{}",
                                file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                rule.naming_pattern,
                                case_msg
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
    
    fn check_dependency_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.dependencies {
            // Check package.json
            let package_json_path = project_path.join("package.json");
            if package_json_path.exists() {
                let content = std::fs::read_to_string(&package_json_path)?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let deps = json["dependencies"].as_object();
                    let dev_deps = json["devDependencies"].as_object();
                    
                    // Check forbidden dependencies
                    for forbidden in &rule.forbidden {
                        if deps.map_or(false, |d| d.contains_key(forbidden)) ||
                           dev_deps.map_or(false, |d| d.contains_key(forbidden)) {
                            violations.push(Violation {
                                rule: "dependency:forbidden".to_string(),
                                message: format!("Forbidden dependency '{}' found in package.json", forbidden),
                                severity: Severity::Error,
                                path: Some(package_json_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                    
                    // Check required dependencies
                    for (pkg, version) in &rule.required {
                        if !deps.map_or(false, |d| d.contains_key(pkg)) &&
                           !dev_deps.map_or(false, |d| d.contains_key(pkg)) {
                            violations.push(Violation {
                                rule: "dependency:required".to_string(),
                                message: format!("Required dependency '{}' ({}) is missing", pkg, version),
                                severity: Severity::Error,
                                path: Some(package_json_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                }
            }
            
            // Check Cargo.toml for Rust projects
            let cargo_toml_path = project_path.join("Cargo.toml");
            if cargo_toml_path.exists() {
                let content = std::fs::read_to_string(&cargo_toml_path)?;
                if let Ok(toml) = toml::from_str::<toml::Value>(&content) {
                    let deps = toml.get("dependencies");
                    let dev_deps = toml.get("dev-dependencies");
                    
                    for forbidden in &rule.forbidden {
                        if deps.and_then(|d| d.get(forbidden)).is_some() ||
                           dev_deps.and_then(|d| d.get(forbidden)).is_some() {
                            violations.push(Violation {
                                rule: "dependency:forbidden".to_string(),
                                message: format!("Forbidden dependency '{}' found in Cargo.toml", forbidden),
                                severity: Severity::Error,
                                path: Some(cargo_toml_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_import_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.imports {
            let files = self.find_matching_files(project_path, &rule.source_pattern)?;
            
            for file_path in files {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    // Check for forbidden imports
                    for forbidden in &rule.forbidden_imports {
                        if content.contains(forbidden) {
                            violations.push(Violation {
                                rule: format!("import:{}", rule.source_pattern),
                                message: format!(
                                    "File '{}' contains forbidden import '{}'",
                                    file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                    forbidden
                                ),
                                severity: Severity::Error,
                                path: Some(file_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                    
                    // Check for relative imports if absolute required
                    if rule.require_absolute {
                        let relative_import_regex = regex::Regex::new(r#"(import|from)\s+['"]\.\./?"#)?;
                        if relative_import_regex.is_match(&content) {
                            violations.push(Violation {
                                rule: format!("import:{}", rule.source_pattern),
                                message: format!(
                                    "File '{}' uses relative imports but absolute imports are required",
                                    file_path.strip_prefix(project_path).unwrap_or(&file_path).display()
                                ),
                                severity: Severity::Warning,
                                path: Some(file_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_documentation_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.documentation {
            let files = self.find_matching_files(project_path, &rule.pattern)?;
            
            for file_path in files {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    // Check for header comments
                    if rule.require_header {
                        let has_header = content.starts_with("//") || 
                                        content.starts_with("/*") ||
                                        content.starts_with("#");
                        if !has_header {
                            violations.push(Violation {
                                rule: format!("documentation:{}", rule.pattern),
                                message: format!(
                                    "File '{}' is missing required header documentation",
                                    file_path.strip_prefix(project_path).unwrap_or(&file_path).display()
                                ),
                                severity: Severity::Info,
                                path: Some(file_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                    
                    // Check for required sections in documentation files
                    if file_path.extension().and_then(|s| s.to_str()) == Some("md") {
                        for section in &rule.required_sections {
                            if !content.contains(section) {
                                violations.push(Violation {
                                    rule: format!("documentation:{}", rule.pattern),
                                    message: format!(
                                        "Documentation file '{}' is missing required section '{}'",
                                        file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                        section
                                    ),
                                    severity: Severity::Warning,
                                    path: Some(file_path.clone()),
                                    fixable: false,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_size_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.size {
            let files = self.find_matching_files(project_path, &rule.pattern)?;
            
            for file_path in files {
                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    // Check file size in bytes
                    if let Some(max_bytes) = rule.max_bytes {
                        if metadata.len() as usize > max_bytes {
                            violations.push(Violation {
                                rule: format!("size:{}", rule.pattern),
                                message: format!(
                                    "File '{}' exceeds maximum size ({} bytes > {} bytes)",
                                    file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                    metadata.len(),
                                    max_bytes
                                ),
                                severity: Severity::Warning,
                                path: Some(file_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                    
                    // Check line count
                    if let Some(max_lines) = rule.max_lines {
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            let line_count = content.lines().count();
                            if line_count > max_lines {
                                violations.push(Violation {
                                    rule: format!("size:{}", rule.pattern),
                                    message: format!(
                                        "File '{}' exceeds maximum line count ({} lines > {} lines)",
                                        file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                        line_count,
                                        max_lines
                                    ),
                                    severity: Severity::Warning,
                                    path: Some(file_path.clone()),
                                    fixable: false,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(violations)
    }
    
    fn check_security_rules(&self, project_path: &Path) -> Result<Vec<Violation>> {
        let mut violations = Vec::new();
        
        for rule in &self.config.security {
            let files = self.find_matching_files(project_path, &rule.pattern)?;
            
            for file_path in files {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    // Check for forbidden patterns
                    for pattern_str in &rule.forbidden_patterns {
                        if let Ok(pattern) = regex::Regex::new(pattern_str) {
                            if pattern.is_match(&content) {
                                violations.push(Violation {
                                    rule: format!("security:{}", rule.pattern),
                                    message: format!(
                                        "File '{}' contains forbidden pattern matching '{}'",
                                        file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                        pattern_str
                                    ),
                                    severity: Severity::Error,
                                    path: Some(file_path.clone()),
                                    fixable: false,
                                });
                            }
                        }
                    }
                    
                    // Check for forbidden functions
                    for func in &rule.forbidden_functions {
                        if content.contains(func) {
                            violations.push(Violation {
                                rule: format!("security:{}", rule.pattern),
                                message: format!(
                                    "File '{}' uses forbidden function '{}'",
                                    file_path.strip_prefix(project_path).unwrap_or(&file_path).display(),
                                    func
                                ),
                                severity: Severity::Error,
                                path: Some(file_path.clone()),
                                fixable: false,
                            });
                        }
                    }
                    
                    // Check for non-HTTPS URLs
                    if rule.require_https && content.contains("http://") {
                        violations.push(Violation {
                            rule: format!("security:{}", rule.pattern),
                            message: format!(
                                "File '{}' contains non-HTTPS URL",
                                file_path.strip_prefix(project_path).unwrap_or(&file_path).display()
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
    use crate::plugins::rules::config::{DirectoryRule, NamingRule, SizeRule, SecurityRule};
    use tempfile::tempdir;
    use std::fs;
    
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
            naming: Vec::new(),
            dependencies: Vec::new(),
            imports: Vec::new(),
            documentation: Vec::new(),
            size: Vec::new(),
            security: Vec::new(),
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
    
    #[test]
    fn test_naming_rule_violation() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("badname.tsx");
        fs::write(&file_path, "export default function Component() {}").unwrap();
        
        let config = RulesConfig {
            directories: Vec::new(),
            components: Vec::new(),
            files: Vec::new(),
            naming: vec![
                NamingRule {
                    pattern: "*.tsx".to_string(),
                    naming_pattern: "^[A-Z][a-zA-Z0-9]+\\.tsx$".to_string(),
                    case_style: Some("PascalCase".to_string()),
                    description: Some("React components must be PascalCase".to_string()),
                },
            ],
            dependencies: Vec::new(),
            imports: Vec::new(),
            documentation: Vec::new(),
            size: Vec::new(),
            security: Vec::new(),
        };
        
        let engine = RuleEngine::new(config);
        let violations = engine.validate(temp.path()).unwrap();
        
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("does not match naming pattern"));
        assert!(violations[0].message.contains("PascalCase"));
    }
    
    #[test]
    fn test_size_rule_violation() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("large.js");
        
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("const line{} = {};\n", i, i));
        }
        fs::write(&file_path, content).unwrap();
        
        let config = RulesConfig {
            directories: Vec::new(),
            components: Vec::new(),
            files: Vec::new(),
            naming: Vec::new(),
            dependencies: Vec::new(),
            imports: Vec::new(),
            documentation: Vec::new(),
            size: vec![
                SizeRule {
                    pattern: "*.js".to_string(),
                    max_lines: Some(50),
                    max_bytes: None,
                    max_functions: None,
                    max_complexity: None,
                    description: Some("JavaScript files should be reasonably sized".to_string()),
                },
            ],
            security: Vec::new(),
        };
        
        let engine = RuleEngine::new(config);
        let violations = engine.validate(temp.path()).unwrap();
        
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("exceeds maximum line count"));
    }
    
    #[test]
    fn test_security_rule_violation() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("insecure.js");
        
        let content = r#"
        const apiKey = "sk-1234567890";
        eval("console.log('dangerous')");
        fetch("http://api.example.com/data");
        "#;
        fs::write(&file_path, content).unwrap();
        
        let config = RulesConfig {
            directories: Vec::new(),
            components: Vec::new(),
            files: Vec::new(),
            naming: Vec::new(),
            dependencies: Vec::new(),
            imports: Vec::new(),
            documentation: Vec::new(),
            size: Vec::new(),
            security: vec![
                SecurityRule {
                    pattern: "*.js".to_string(),
                    forbidden_patterns: vec![
                        r#"apiKey\s*=\s*["']"#.to_string(),
                    ],
                    require_https: true,
                    no_hardcoded_secrets: true,
                    forbidden_functions: vec!["eval".to_string()],
                    description: Some("Basic security checks".to_string()),
                },
            ],
        };
        
        let engine = RuleEngine::new(config);
        let violations = engine.validate(temp.path()).unwrap();
        
        // Should have violations for: forbidden pattern (apiKey), eval function, and http URL
        assert!(violations.len() >= 2);
        assert!(violations.iter().any(|v| v.message.contains("forbidden pattern")));
        assert!(violations.iter().any(|v| v.message.contains("forbidden function")));
    }
}