use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default)]
    pub directories: Vec<DirectoryRule>,
    
    #[serde(default)]
    pub components: Vec<ComponentRule>,
    
    #[serde(default)]
    pub files: Vec<FileRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryRule {
    pub path: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentRule {
    pub pattern: String,
    pub structure: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRule {
    pub pattern: String,
    #[serde(default)]
    pub requires: HashMap<String, String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RuleType {
    Directory(DirectoryRule),
    Component(ComponentRule),
    File(FileRule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub rule_type: String,
    pub config: serde_json::Value,
}

impl RulesConfig {
    pub fn new() -> Self {
        Self {
            directories: Vec::new(),
            components: Vec::new(),
            files: Vec::new(),
        }
    }
    
    pub fn minimal() -> Self {
        Self {
            directories: vec![
                DirectoryRule {
                    path: "src".to_string(),
                    required: true,
                    description: Some("Source code directory".to_string()),
                },
                DirectoryRule {
                    path: "tests".to_string(),
                    required: false,
                    description: Some("Test files directory".to_string()),
                },
            ],
            components: Vec::new(),
            files: Vec::new(),
        }
    }
    
    pub fn default_config() -> Self {
        Self {
            directories: vec![
                DirectoryRule {
                    path: "components".to_string(),
                    required: true,
                    description: Some("Vue/React components directory".to_string()),
                },
                DirectoryRule {
                    path: "tests".to_string(),
                    required: true,
                    description: Some("Test files directory".to_string()),
                },
                DirectoryRule {
                    path: "docs".to_string(),
                    required: false,
                    description: Some("Documentation directory".to_string()),
                },
            ],
            components: vec![
                ComponentRule {
                    pattern: "components/**/".to_string(),
                    structure: vec![
                        "[ComponentName].vue".to_string(),
                        "__tests__/".to_string(),
                        "__tests__/[ComponentName].test.js".to_string(),
                        "[ComponentName].stories.js".to_string(),
                    ],
                    description: Some("Vue component structure".to_string()),
                },
            ],
            files: vec![
                FileRule {
                    pattern: "**/*.vue".to_string(),
                    requires: HashMap::from([
                        ("test".to_string(), "__tests__/*.test.js".to_string()),
                        ("story".to_string(), "*.stories.js".to_string()),
                    ]),
                    description: Some("Vue files must have tests and stories".to_string()),
                },
                FileRule {
                    pattern: "src/**/*.rs".to_string(),
                    requires: HashMap::from([
                        ("test".to_string(), "#[test]".to_string()),
                    ]),
                    description: Some("Rust files should have tests".to_string()),
                },
            ],
        }
    }
    
    pub fn example_react_config() -> Self {
        Self {
            directories: vec![
                DirectoryRule {
                    path: "src/components".to_string(),
                    required: true,
                    description: Some("React components directory".to_string()),
                },
                DirectoryRule {
                    path: "src/__tests__".to_string(),
                    required: true,
                    description: Some("Test files directory".to_string()),
                },
            ],
            components: vec![
                ComponentRule {
                    pattern: "src/components/**/".to_string(),
                    structure: vec![
                        "[ComponentName].tsx".to_string(),
                        "[ComponentName].test.tsx".to_string(),
                        "[ComponentName].stories.tsx".to_string(),
                        "index.ts".to_string(),
                    ],
                    description: Some("React TypeScript component structure".to_string()),
                },
            ],
            files: vec![
                FileRule {
                    pattern: "**/*.tsx".to_string(),
                    requires: HashMap::from([
                        ("test".to_string(), "*.test.tsx".to_string()),
                    ]),
                    description: Some("TypeScript React files must have tests".to_string()),
                },
            ],
        }
    }
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<RulesConfig> {
    let content = std::fs::read_to_string(path)?;
    
    // Try to parse as YAML first, then JSON
    if let Ok(config) = serde_yaml::from_str::<RulesConfig>(&content) {
        Ok(config)
    } else if let Ok(config) = serde_json::from_str::<RulesConfig>(&content) {
        Ok(config)
    } else {
        Err(anyhow::anyhow!("Failed to parse rules configuration as YAML or JSON"))
    }
}

pub fn save_config<P: AsRef<Path>>(path: P, config: &RulesConfig) -> Result<()> {
    let yaml = serde_yaml::to_string(config)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_minimal_config() {
        let config = RulesConfig::minimal();
        assert_eq!(config.directories.len(), 2);
        assert!(config.components.is_empty());
        assert!(config.files.is_empty());
    }
    
    #[test]
    fn test_default_config() {
        let config = RulesConfig::default_config();
        assert!(!config.directories.is_empty());
        assert!(!config.components.is_empty());
        assert!(!config.files.is_empty());
    }
    
    #[test]
    fn test_serialize_deserialize() {
        let config = RulesConfig::default_config();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: RulesConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.directories.len(), parsed.directories.len());
        assert_eq!(config.components.len(), parsed.components.len());
        assert_eq!(config.files.len(), parsed.files.len());
    }
}