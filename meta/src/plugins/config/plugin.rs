//! Config plugin for managing .meta configuration

use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command, Arg};
use metarepo_core::{
    BasePlugin, MetaConfig, MetaPlugin, RuntimeConfig,
};
use std::path::PathBuf;

use super::tui_editor::ConfigEditor;

pub struct ConfigPlugin;

impl ConfigPlugin {
    pub fn new() -> Self {
        Self
    }

    fn handle_edit(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let meta_file = if let Some(file) = matches.get_one::<String>("file") {
            PathBuf::from(file)
        } else {
            config.meta_file_path.clone().ok_or_else(|| {
                anyhow!("Could not find .meta file. Use --file to specify path.")
            })?
        };

        // Launch TUI editor
        let mut editor = ConfigEditor::new(meta_file)?;
        editor.run()?;

        Ok(())
    }

    fn handle_show(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let format = matches.get_one::<String>("format").map(|s| s.as_str()).unwrap_or("json");

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&config.meta_config)?;
                println!("{}", json);
            }
            "yaml" => {
                let yaml = serde_yaml::to_string(&config.meta_config)?;
                println!("{}", yaml);
            }
            "toml" => {
                let toml = toml::to_string_pretty(&config.meta_config)?;
                println!("{}", toml);
            }
            _ => {
                return Err(anyhow!("Unknown format: {}. Use json, yaml, or toml", format));
            }
        }

        Ok(())
    }

    fn handle_get(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let key = matches.get_one::<String>("key").unwrap();

        // Parse the key path (e.g., "default_bare" or "projects.myproject.url")
        let parts: Vec<&str> = key.split('.').collect();

        // Convert config to a JSON value for easy navigation
        let config_json = serde_json::to_value(&config.meta_config)?;

        let mut current = &config_json;
        for part in &parts {
            current = current.get(part).ok_or_else(|| {
                anyhow!("Key '{}' not found in config", key)
            })?;
        }

        // Pretty print the value
        println!("{}", serde_json::to_string_pretty(current)?);

        Ok(())
    }

    fn handle_set(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let key = matches.get_one::<String>("key").unwrap();
        let value_str = matches.get_one::<String>("value").unwrap();

        // Load the config as a mutable JSON value
        let mut config_json = serde_json::to_value(&config.meta_config)?;

        // Parse the key path
        let parts: Vec<&str> = key.split('.').collect();

        // Navigate to the parent object
        let mut current = &mut config_json;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part - set the value
                // Try to parse as JSON first, fallback to string
                let value: serde_json::Value = serde_json::from_str(value_str)
                    .unwrap_or_else(|_| serde_json::Value::String(value_str.to_string()));

                current[part] = value;
            } else {
                // Navigate deeper
                current = current.get_mut(part).ok_or_else(|| {
                    anyhow!("Key path '{}' not found", parts[..=i].join("."))
                })?;
            }
        }

        // Convert back to MetaConfig and save
        let updated_config: MetaConfig = serde_json::from_value(config_json)?;

        let meta_file = config.meta_file_path.clone().ok_or_else(|| {
            anyhow!("Could not find .meta file path")
        })?;

        updated_config.save_to_file(&meta_file)?;

        println!("✓ Config updated: {} = {}", key, value_str);

        Ok(())
    }

    fn handle_validate(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        let meta_file = if let Some(file) = matches.get_one::<String>("file") {
            PathBuf::from(file)
        } else {
            config.meta_file_path.clone().ok_or_else(|| {
                anyhow!("Could not find .meta file. Use --file to specify path.")
            })?
        };

        // Try to load the config
        match MetaConfig::load_from_file(&meta_file) {
            Ok(_) => {
                println!("✓ Config file is valid: {}", meta_file.display());
                Ok(())
            }
            Err(e) => {
                println!("✗ Config file validation failed: {}", e);
                Err(e)
            }
        }
    }
}

impl MetaPlugin for ConfigPlugin {
    fn name(&self) -> &str {
        "config"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("config")
                .about("Manage .meta configuration files")
                .visible_alias("c")
                .subcommand_required(false)
                .subcommand(
                    Command::new("edit")
                        .about("Edit config with interactive TUI")
                        .visible_alias("e")
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .value_name("FILE")
                                .help("Path to .meta file")
                        )
                )
                .subcommand(
                    Command::new("show")
                        .about("Display current configuration")
                        .arg(
                            Arg::new("format")
                                .short('f')
                                .long("format")
                                .value_name("FORMAT")
                                .help("Output format (json, yaml, toml)")
                                .default_value("json")
                                .value_parser(["json", "yaml", "toml"])
                        )
                )
                .subcommand(
                    Command::new("get")
                        .about("Get a specific config value")
                        .arg(
                            Arg::new("key")
                                .required(true)
                                .value_name("KEY")
                                .help("Config key path (e.g., 'default_bare' or 'projects.myproject.url')")
                        )
                )
                .subcommand(
                    Command::new("set")
                        .about("Set a specific config value")
                        .arg(
                            Arg::new("key")
                                .required(true)
                                .value_name("KEY")
                                .help("Config key path")
                        )
                        .arg(
                            Arg::new("value")
                                .required(true)
                                .value_name("VALUE")
                                .help("Value to set")
                        )
                )
                .subcommand(
                    Command::new("validate")
                        .about("Validate .meta file structure")
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .value_name("FILE")
                                .help("Path to .meta file to validate")
                        )
                )
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("edit", sub_matches)) => self.handle_edit(sub_matches, config),
            Some(("show", sub_matches)) => self.handle_show(sub_matches, config),
            Some(("get", sub_matches)) => self.handle_get(sub_matches, config),
            Some(("set", sub_matches)) => self.handle_set(sub_matches, config),
            Some(("validate", sub_matches)) => self.handle_validate(sub_matches, config),
            _ => {
                // Default to edit if no subcommand provided
                self.handle_edit(matches, config)
            }
        }
    }
}

impl BasePlugin for ConfigPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }

    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }

    fn description(&self) -> Option<&str> {
        Some("Manage .meta configuration files")
    }
}
