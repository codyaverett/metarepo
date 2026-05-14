// Integration tests for #5: multi-format config + --config override + migrate.

use metarepo::{create_runtime_config_full, MetaConfig};
use metarepo_core::{ConfigFormat, ProjectEntry};
use std::collections::HashMap;
use tempfile::TempDir;

fn write_default_yaml(tmp: &TempDir) -> std::path::PathBuf {
    let path = tmp.path().join(".metarepo.yaml");
    let mut config = MetaConfig::default();
    config.projects.insert(
        "alpha".to_string(),
        ProjectEntry::Url("https://example.com/alpha.git".to_string()),
    );
    config
        .save_to_file_with_format(&path, ConfigFormat::Yaml)
        .unwrap();
    path
}

#[test]
fn explicit_config_override_loads_arbitrary_path() {
    let tmp = TempDir::new().unwrap();
    let path = write_default_yaml(&tmp);

    // create_runtime_config_full bypasses discovery when an override is set.
    // We pass it directly here (the CLI does the same after parsing --config).
    let rc = create_runtime_config_full(false, None, Some(path.clone())).unwrap();
    assert_eq!(rc.meta_file_path, Some(path));
    assert!(rc.meta_config.projects.contains_key("alpha"));
}

#[test]
fn explicit_override_rejects_unreadable_path() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("nope.yaml");
    let err = create_runtime_config_full(false, None, Some(missing)).err();
    assert!(
        err.is_some(),
        "missing override path should produce an error"
    );
}

#[test]
fn save_then_load_works_for_each_extension() {
    let cases = [
        (".metarepo", ConfigFormat::Json),
        (".metarepo.json", ConfigFormat::Json),
        (".metarepo.yaml", ConfigFormat::Yaml),
        (".metarepo.yml", ConfigFormat::Yaml),
        (".metarepo.toml", ConfigFormat::Toml),
    ];

    for (name, format) in cases {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(name);

        let mut config = MetaConfig::default();
        let mut scripts = HashMap::new();
        scripts.insert("build".to_string(), "cargo build".to_string());
        config.projects.insert(
            "alpha".to_string(),
            ProjectEntry::Metadata(metarepo_core::ProjectMetadata {
                url: "https://example.com/x.git".to_string(),
                aliases: vec!["a".to_string()],
                scripts,
                env: HashMap::new(),
                worktree_init: None,
                bare: None,
            }),
        );

        config
            .save_to_file_with_format(&path, format)
            .unwrap_or_else(|e| panic!("save {} failed: {}", name, e));
        let loaded = MetaConfig::load_from_file(&path).unwrap();
        assert!(loaded.projects.contains_key("alpha"), "{}", name);
    }
}

#[test]
fn discover_errors_when_multiple_configs_coexist() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".meta"), "{}").unwrap();
    std::fs::write(tmp.path().join(".metarepo.yaml"), "ignore: []\n").unwrap();

    // With no override, runtime config builder surfaces the structured error
    // via its Display impl. Ensure both filenames + a fix hint are visible.
    std::env::remove_var("METAREPO_CONFIG");
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let err = create_runtime_config_full(false, None, None).err().unwrap();
    std::env::set_current_dir(orig).unwrap();

    let msg = err.to_string();
    assert!(msg.contains(".meta"));
    assert!(msg.contains(".metarepo.yaml"));
    assert!(msg.contains("--config") || msg.contains("migrate"));
}
