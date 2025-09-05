use gestalt_rules::config::{ComponentRule, DirectoryRule, FileRule};
use gestalt_rules::project::RulesStats;
use gestalt_rules::{RuleEngine, RulesConfig};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_project_specific_rules() {
    let temp = tempdir().unwrap();
    let project_path = temp.path().join("frontend");
    fs::create_dir(&project_path).unwrap();

    // Create project-specific rules
    let project_rules = RulesConfig {
        directories: vec![DirectoryRule {
            path: "src/components".to_string(),
            required: true,
            description: Some("Components directory".to_string()),
        }],
        components: Vec::new(),
        files: Vec::new(),
        naming: Vec::new(),
        dependencies: Vec::new(),
        imports: Vec::new(),
        documentation: Vec::new(),
        size: Vec::new(),
        security: Vec::new(),
    };

    // Save project rules
    let rules_file = project_path.join(".rules.yaml");
    gestalt_rules::config::save_config(&rules_file, &project_rules).unwrap();

    // Verify file was created
    assert!(rules_file.exists());

    // Load and verify
    let loaded = gestalt_rules::config::load_config(&rules_file).unwrap();
    assert_eq!(loaded.directories.len(), 1);
    assert_eq!(loaded.directories[0].path, "src/components");
}

#[cfg(test)]
#[test]
fn test_rules_stats() {
    let config = RulesConfig {
        directories: vec![
            DirectoryRule {
                path: "src".to_string(),
                required: true,
                description: None,
            },
            DirectoryRule {
                path: "tests".to_string(),
                required: false,
                description: None,
            },
        ],
        components: vec![ComponentRule {
            pattern: "components/**/".to_string(),
            structure: vec!["[ComponentName].vue".to_string()],
            description: None,
        }],
        files: vec![FileRule {
            pattern: "**/*.js".to_string(),
            requires: HashMap::new(),
            description: None,
        }],
        naming: Vec::new(),
        dependencies: Vec::new(),
        imports: Vec::new(),
        documentation: Vec::new(),
        size: Vec::new(),
        security: Vec::new(),
    };

    let stats = RulesStats::from_config(&config, "test-source".to_string());
    assert_eq!(stats.total_directories, 2);
    assert_eq!(stats.total_components, 1);
    assert_eq!(stats.total_files, 1);
    assert_eq!(stats.source, "test-source");
}

#[test]
fn test_create_directory_rule() {
    let temp = tempdir().unwrap();
    let rules_file = temp.path().join(".rules.yaml");

    // Create initial config
    let config = RulesConfig::new();
    gestalt_rules::config::save_config(&rules_file, &config).unwrap();

    // Load, modify, and save
    let mut loaded = gestalt_rules::config::load_config(&rules_file).unwrap();
    loaded.directories.push(DirectoryRule {
        path: "src/utils".to_string(),
        required: true,
        description: Some("Utility functions".to_string()),
    });
    gestalt_rules::config::save_config(&rules_file, &loaded).unwrap();

    // Verify the rule was added
    let final_config = gestalt_rules::config::load_config(&rules_file).unwrap();
    assert_eq!(final_config.directories.len(), 1);
    assert_eq!(final_config.directories[0].path, "src/utils");
}

#[test]
fn test_component_rule_validation() {
    let temp = tempdir().unwrap();

    // Create component directory structure
    let comp_dir = temp.path().join("components").join("Button");
    fs::create_dir_all(&comp_dir).unwrap();
    fs::write(comp_dir.join("Button.vue"), "").unwrap();

    let config = RulesConfig {
        directories: Vec::new(),
        components: vec![ComponentRule {
            pattern: "components/**/".to_string(),
            structure: vec![
                "[ComponentName].vue".to_string(),
                "[ComponentName].test.js".to_string(),
            ],
            description: None,
        }],
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

    // Should have one violation for missing test file
    assert_eq!(violations.len(), 1);
    assert!(violations[0].message.contains("Button.test.js"));
}

#[test]
fn test_file_rule_with_requires() {
    let temp = tempdir().unwrap();

    // Create a Vue file without test
    let src_dir = temp.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    fs::write(src_dir.join("App.vue"), "").unwrap();

    let mut requires = HashMap::new();
    requires.insert("test".to_string(), "*.test.js".to_string());

    let config = RulesConfig {
        directories: Vec::new(),
        components: Vec::new(),
        files: vec![FileRule {
            pattern: "**/*.vue".to_string(),
            requires,
            description: None,
        }],
        naming: Vec::new(),
        dependencies: Vec::new(),
        imports: Vec::new(),
        documentation: Vec::new(),
        size: Vec::new(),
        security: Vec::new(),
    };

    let engine = RuleEngine::new(config);
    let violations = engine.validate(temp.path()).unwrap();

    // Should have violation for missing test
    assert!(!violations.is_empty());
    assert!(violations[0].message.contains("missing required test"));
}

#[test]
fn test_yaml_and_json_config_compatibility() {
    let temp = tempdir().unwrap();
    let config = RulesConfig::default_config();

    // Save as YAML
    let yaml_path = temp.path().join("rules.yaml");
    let yaml_content = serde_yaml::to_string(&config).unwrap();
    fs::write(&yaml_path, &yaml_content).unwrap();

    // Save as JSON
    let json_path = temp.path().join("rules.json");
    let json_content = serde_json::to_string_pretty(&config).unwrap();
    fs::write(&json_path, &json_content).unwrap();

    // Load both and compare
    let from_yaml = gestalt_rules::config::load_config(&yaml_path).unwrap();
    let from_json = gestalt_rules::config::load_config(&json_path).unwrap();

    assert_eq!(from_yaml.directories.len(), from_json.directories.len());
    assert_eq!(from_yaml.components.len(), from_json.components.len());
    assert_eq!(from_yaml.files.len(), from_json.files.len());
}
