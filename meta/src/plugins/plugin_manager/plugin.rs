use anyhow::{Context, Result};
use clap::ArgMatches;
use metarepo_core::{
    BasePlugin, MetaPlugin, RuntimeConfig, MetaConfig,
    plugin, command, arg,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

/// PluginManagerPlugin using the new simplified plugin architecture
pub struct PluginManagerPlugin;

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Create the plugin using the builder pattern
    pub fn create_plugin() -> impl MetaPlugin {
        plugin("plugin")
            .version(env!("CARGO_PKG_VERSION"))
            .description("Manage metarepo plugins")
            .author("Metarepo Contributors")
            .command(
                command("add")
                    .about("Add a plugin from a local path")
                    .with_help_formatting()
                    .arg(
                        arg("path")
                            .help("Path to the plugin executable")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("install")
                    .about("Install a plugin from crates.io")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Name of the plugin to install")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("remove")
                    .about("Remove an installed plugin")
                    .with_help_formatting()
                    .arg(
                        arg("name")
                            .help("Name of the plugin to remove")
                            .required(true)
                            .takes_value(true)
                    )
            )
            .command(
                command("list")
                    .about("List all installed plugins")
                    .with_help_formatting()
            )
            .command(
                command("update")
                    .about("Update all plugins to their latest versions")
                    .with_help_formatting()
            )
            .handler("add", handle_add)
            .handler("install", handle_install)
            .handler("remove", handle_remove)
            .handler("list", handle_list)
            .handler("update", handle_update)
            .build()
    }
}

/// Get plugin directory
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

/// Add plugin to .meta config
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

/// Handler for the add command
fn handle_add(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    add_plugin_from_path(path)
}

fn add_plugin_from_path(path: &str) -> Result<()> {
    let source_path = PathBuf::from(path);
    
    if !source_path.exists() {
        return Err(anyhow::anyhow!("Plugin path does not exist: {}", path));
    }

    let plugin_dir = plugin_dir()?;
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

/// Handler for the install command
fn handle_install(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    install_plugin(name)
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
    add_to_meta_config(name, &format!("^latest"))?;
    
    Ok(())
}

/// Handler for the remove command
fn handle_remove(matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    remove_plugin(name)
}

fn remove_plugin(name: &str) -> Result<()> {
    // Remove from plugins directory
    let plugin_dir = plugin_dir()?;
    
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

/// Handler for the list command
fn handle_list(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    list_plugins()
}

fn list_plugins() -> Result<()> {
    println!("Installed Plugins:");
    println!("==================");

    // List plugins in plugin directory
    let plugin_dir = plugin_dir()?;
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

/// Handler for the update command
fn handle_update(_matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
    update_plugins()
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

// Traditional implementation for backward compatibility
impl MetaPlugin for PluginManagerPlugin {
    fn name(&self) -> &str {
        "plugin"
    }
    
    fn is_experimental(&self) -> bool {
        false // Plugin management is a core feature
    }
    
    fn register_commands(&self, app: clap::Command) -> clap::Command {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.register_commands(app)
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Delegate to the builder-based plugin
        let plugin = Self::create_plugin();
        plugin.handle_command(matches, config)
    }
}

impl BasePlugin for PluginManagerPlugin {
    fn version(&self) -> Option<&str> {
        Some(env!("CARGO_PKG_VERSION"))
    }
    
    fn description(&self) -> Option<&str> {
        Some("Manage metarepo plugins")
    }
    
    fn author(&self) -> Option<&str> {
        Some("Metarepo Contributors")
    }
}

impl Default for PluginManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}