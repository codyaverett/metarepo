use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig, MetaConfig};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

pub struct PluginManagerPlugin;

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self
    }

    fn plugin_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;
        
        let plugin_dir = PathBuf::from(home)
            .join(".config")
            .join("metarepo")
            .join("plugins");
        
        if !plugin_dir.exists() {
            fs::create_dir_all(&plugin_dir)
                .context("Failed to create plugin directory")?;
        }
        
        Ok(plugin_dir)
    }

    fn add_plugin(path: &str) -> Result<()> {
        let source_path = PathBuf::from(path);
        
        if !source_path.exists() {
            return Err(anyhow::anyhow!("Plugin path does not exist: {}", path));
        }

        let plugin_dir = Self::plugin_dir()?;
        let file_name = source_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid plugin path"))?;
        
        let dest_path = plugin_dir.join(file_name);

        // Copy the plugin to the plugins directory
        fs::copy(&source_path, &dest_path)
            .context("Failed to copy plugin")?;

        // Make it executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest_path, perms)?;
        }

        println!("Plugin added successfully: {:?}", dest_path);
        println!("The plugin will be available on next run of meta");
        
        Ok(())
    }

    fn add_to_meta_config(name: &str, spec: &str) -> Result<()> {
        // Find and update .meta file
        if let Some(meta_file) = MetaConfig::find_meta_file() {
            let mut config = MetaConfig::load_from_file(&meta_file)?;
            
            if config.plugins.is_none() {
                config.plugins = Some(std::collections::HashMap::new());
            }
            
            if let Some(plugins) = &mut config.plugins {
                plugins.insert(name.to_string(), spec.to_string());
            }
            
            config.save_to_file(&meta_file)?;
            println!("Added plugin '{}' to .meta configuration", name);
        } else {
            println!("Warning: No .meta file found. Plugin installed globally but not added to project configuration.");
        }
        
        Ok(())
    }

    fn install_plugin(name: &str) -> Result<()> {
        println!("Installing plugin from crates.io: {}", name);
        
        // Use cargo install to get the plugin
        let plugin_crate = format!("metarepo-plugin-{}", name);
        
        let output = ProcessCommand::new("cargo")
            .args(&["install", &plugin_crate])
            .output()
            .context("Failed to run cargo install")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to install plugin: {}", stderr));
        }

        println!("Plugin '{}' installed successfully", name);
        
        // Add to .meta config
        Self::add_to_meta_config(name, &format!("^latest"))?;
        
        Ok(())
    }

    fn remove_plugin(name: &str) -> Result<()> {
        // Remove from plugins directory
        let plugin_dir = Self::plugin_dir()?;
        
        // Look for plugin file
        let entries = fs::read_dir(&plugin_dir)?;
        let mut found = false;
        
        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            if file_name_str.contains(name) {
                fs::remove_file(entry.path())?;
                println!("Removed plugin: {}", file_name_str);
                found = true;
            }
        }

        // Also try to uninstall from cargo
        let plugin_crate = format!("metarepo-plugin-{}", name);
        let _ = ProcessCommand::new("cargo")
            .args(&["uninstall", &plugin_crate])
            .output();

        // Remove from .meta config
        if let Some(meta_file) = MetaConfig::find_meta_file() {
            let mut config = MetaConfig::load_from_file(&meta_file)?;
            
            if let Some(plugins) = &mut config.plugins {
                if plugins.remove(name).is_some() {
                    config.save_to_file(&meta_file)?;
                    println!("Removed plugin '{}' from .meta configuration", name);
                }
            }
        }

        if !found {
            return Err(anyhow::anyhow!("Plugin '{}' not found", name));
        }

        Ok(())
    }

    fn list_plugins() -> Result<()> {
        println!("Installed Plugins:");
        println!("==================");

        // List plugins in plugin directory
        let plugin_dir = Self::plugin_dir()?;
        if plugin_dir.exists() {
            println!("\nLocal plugins ({:?}):", plugin_dir);
            
            let entries = fs::read_dir(&plugin_dir)?;
            let mut count = 0;
            
            for entry in entries {
                let entry = entry?;
                if entry.path().is_file() {
                    println!("  - {}", entry.file_name().to_string_lossy());
                    count += 1;
                }
            }
            
            if count == 0 {
                println!("  (none)");
            }
        }

        // List plugins in .meta config
        if let Some(meta_file) = MetaConfig::find_meta_file() {
            let config = MetaConfig::load_from_file(&meta_file)?;
            
            if let Some(plugins) = &config.plugins {
                if !plugins.is_empty() {
                    println!("\nProject plugins (from .meta):");
                    for (name, spec) in plugins {
                        println!("  - {}: {}", name, spec);
                    }
                }
            }
        }

        // List plugins installed via cargo
        println!("\nPlugins from crates.io:");
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;
        
        let cargo_bin = PathBuf::from(home).join(".cargo").join("bin");
        if cargo_bin.exists() {
            let entries = fs::read_dir(&cargo_bin)?;
            let mut found = false;
            
            for entry in entries {
                let entry = entry?;
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                
                if file_name_str.starts_with("metarepo-plugin-") {
                    println!("  - {}", file_name_str);
                    found = true;
                }
            }
            
            if !found {
                println!("  (none)");
            }
        }

        Ok(())
    }

    fn update_plugins() -> Result<()> {
        println!("Updating plugins...");

        // Update plugins from crates.io
        if let Some(meta_file) = MetaConfig::find_meta_file() {
            let config = MetaConfig::load_from_file(&meta_file)?;
            
            if let Some(plugins) = &config.plugins {
                for (name, spec) in plugins {
                    if !spec.starts_with("file:") && !spec.starts_with("git+") {
                        println!("Updating {}", name);
                        let plugin_crate = format!("metarepo-plugin-{}", name);
                        
                        let output = ProcessCommand::new("cargo")
                            .args(&["install", "--force", &plugin_crate])
                            .output()
                            .context("Failed to run cargo install")?;

                        if output.status.success() {
                            println!("  ✓ Updated {}", name);
                        } else {
                            eprintln!("  ✗ Failed to update {}", name);
                        }
                    }
                }
            }
        }

        println!("Plugin update complete");
        Ok(())
    }
}

impl MetaPlugin for PluginManagerPlugin {
    fn name(&self) -> &str {
        "plugin"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("plugin")
                .about("Manage metarepo plugins")
                .long_about("Install, remove, and manage external plugins for metarepo")
                .subcommand(
                    Command::new("add")
                        .about("Add a plugin from a local path")
                        .arg(
                            Arg::new("path")
                                .help("Path to the plugin executable")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("install")
                        .about("Install a plugin from crates.io")
                        .arg(
                            Arg::new("name")
                                .help("Name of the plugin to install")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove an installed plugin")
                        .arg(
                            Arg::new("name")
                                .help("Name of the plugin to remove")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List all installed plugins"),
                )
                .subcommand(
                    Command::new("update")
                        .about("Update all plugins to their latest versions"),
                ),
        )
    }

    fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("add", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                Self::add_plugin(path)?;
            }
            Some(("install", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name").unwrap();
                Self::install_plugin(name)?;
            }
            Some(("remove", sub_matches)) => {
                let name = sub_matches.get_one::<String>("name").unwrap();
                Self::remove_plugin(name)?;
            }
            Some(("list", _)) => {
                Self::list_plugins()?;
            }
            Some(("update", _)) => {
                Self::update_plugins()?;
            }
            _ => {
                println!("Use 'meta plugin --help' for available commands");
            }
        }
        
        Ok(())
    }

    fn is_experimental(&self) -> bool {
        false // Plugin management is a core feature
    }
}

impl Default for PluginManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}