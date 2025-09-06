use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};

pub struct ExamplePlugin {
    name: String,
}

impl ExamplePlugin {
    pub fn new() -> Self {
        Self {
            name: "example".to_string(),
        }
    }
}

impl MetaPlugin for ExamplePlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("example")
                .about("Example plugin demonstrating external plugin development")
                .long_about(
                    "This is an example plugin that shows how to create external plugins \
                     for metarepo. It demonstrates command registration, argument handling, \
                     and accessing the meta repository configuration.",
                )
                .subcommand(
                    Command::new("hello")
                        .about("Print a greeting message")
                        .arg(
                            Arg::new("name")
                                .help("Name to greet")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("info")
                        .about("Display information about the current meta repository"),
                )
                .subcommand(
                    Command::new("count")
                        .about("Count the number of projects in the meta repository"),
                ),
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("hello", sub_matches)) => {
                if let Some(name) = sub_matches.get_one::<String>("name") {
                    eprintln!("Hello, {}! This is the example plugin.", name);
                    eprintln!("Working from: {:?}", config.working_dir);
                }
                Ok(())
            }
            Some(("info", _)) => {
                eprintln!("Meta Repository Information:");
                eprintln!("============================");
                eprintln!("Working directory: {:?}", config.working_dir);

                if config.has_meta_file() {
                    eprintln!("Meta file found: {:?}", config.meta_file_path);
                    if let Some(root) = config.meta_root() {
                        eprintln!("Repository root: {:?}", root);
                    }

                    if !config.meta_config.projects.is_empty() {
                        eprintln!("\nProjects:");
                        for (name, url) in &config.meta_config.projects {
                            eprintln!("  - {}: {}", name, url);
                        }
                    } else {
                        eprintln!("\nNo projects configured yet.");
                    }

                    if !config.meta_config.ignore.is_empty() {
                        eprintln!("\nIgnored patterns:");
                        for pattern in &config.meta_config.ignore {
                            eprintln!("  - {}", pattern);
                        }
                    }

                    if let Some(plugins) = &config.meta_config.plugins {
                        if !plugins.is_empty() {
                            eprintln!("\nConfigured plugins:");
                            for (name, version) in plugins {
                                eprintln!("  - {}: {}", name, version);
                            }
                        }
                    }
                } else {
                    eprintln!("No meta repository found in the current directory tree.");
                    eprintln!("Run 'meta init' to create one.");
                }

                Ok(())
            }
            Some(("count", _)) => {
                if config.has_meta_file() {
                    let count = config.meta_config.projects.len();
                    match count {
                        0 => eprintln!("No projects in this meta repository."),
                        1 => eprintln!("1 project in this meta repository."),
                        n => eprintln!("{} projects in this meta repository.", n),
                    }
                } else {
                    eprintln!("Not in a meta repository. Run 'meta init' first.");
                }
                Ok(())
            }
            _ => {
                eprintln!("Example plugin - use 'meta example --help' for available commands");
                Ok(())
            }
        }
    }

    fn is_experimental(&self) -> bool {
        false // This is a stable example plugin
    }
}

// Export function for dynamic loading (when compiled as cdylib)
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn MetaPlugin {
    Box::into_raw(Box::new(ExamplePlugin::new()))
}

// Safety function for proper cleanup
#[no_mangle]
pub extern "C" fn destroy_plugin(plugin: *mut dyn MetaPlugin) {
    if !plugin.is_null() {
        unsafe {
            let _ = Box::from_raw(plugin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metarepo_core::MetaConfig;
    use std::path::PathBuf;

    #[test]
    fn test_plugin_name() {
        let plugin = ExamplePlugin::new();
        assert_eq!(plugin.name(), "example");
    }

    #[test]
    fn test_plugin_not_experimental() {
        let plugin = ExamplePlugin::new();
        assert!(!plugin.is_experimental());
    }

    #[test]
    fn test_handle_info_without_meta_file() {
        let plugin = ExamplePlugin::new();
        let config = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: PathBuf::from("/tmp"),
            meta_file_path: None,
            experimental: false,
        };

        let app = Command::new("test");
        let app = plugin.register_commands(app);
        let matches = app.get_matches_from(vec!["test", "example", "info"]);

        if let Some(("example", sub_matches)) = matches.subcommand() {
            let result = plugin.handle_command(sub_matches, &config);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_handle_count_with_projects() {
        let plugin = ExamplePlugin::new();
        let mut meta_config = MetaConfig::default();
        meta_config
            .projects
            .insert("project1".to_string(), "https://example.com/p1".to_string());
        meta_config
            .projects
            .insert("project2".to_string(), "https://example.com/p2".to_string());

        let config = RuntimeConfig {
            meta_config,
            working_dir: PathBuf::from("/tmp"),
            meta_file_path: Some(PathBuf::from("/tmp/.meta")),
            experimental: false,
        };

        let app = Command::new("test");
        let app = plugin.register_commands(app);
        let matches = app.get_matches_from(vec!["test", "example", "count"]);

        if let Some(("example", sub_matches)) = matches.subcommand() {
            let result = plugin.handle_command(sub_matches, &config);
            assert!(result.is_ok());
        }
    }
}