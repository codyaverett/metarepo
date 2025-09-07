use crate::{MetarepoError, RuntimeConfig};
use anyhow::Result;
use clap::{ArgMatches, Command};
use std::collections::HashMap;
use metarepo_core::MetaPlugin;

pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn MetaPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, plugin: Box<dyn MetaPlugin>) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, plugin);
    }
    
    pub fn register_all_workspace_plugins(&mut self) {
        self.register_all_workspace_plugins_with_flags(false);
    }
    
    pub fn register_all_workspace_plugins_with_flags(&mut self, experimental: bool) {
        use crate::plugins;
        
        // Register built-in workspace plugins
        self.register(Box::new(plugins::init::InitPlugin::new()));
        self.register(Box::new(plugins::git::GitPlugin::new()));
        self.register(Box::new(plugins::project::ProjectPlugin::new()));
        self.register(Box::new(plugins::exec::ExecPlugin::new()));
        self.register(Box::new(plugins::rules::RulesPlugin::new()));
        self.register(Box::new(plugins::plugin_manager::PluginManagerPlugin::new()));
        
        // Only register experimental plugins if flag is set
        if experimental {
            self.register(Box::new(plugins::mcp::McpPlugin::new()));
        }
    }

    pub fn load_external_plugins(&mut self, config: &metarepo_core::MetaConfig) {
        use crate::plugins::PluginLoader;
        
        // Load plugins from configuration
        let external_plugins = PluginLoader::load_from_config(config);
        for plugin in external_plugins {
            tracing::debug!("Loaded external plugin: {}", plugin.name());
            self.register(plugin);
        }
        
        // Discover plugins in standard locations
        let discovered = PluginLoader::discover_plugins();
        for plugin in discovered {
            tracing::debug!("Discovered plugin: {}", plugin.name());
            self.register(plugin);
        }
    }
    
    pub fn build_cli(&self, base_app: Command) -> Command {
        self.build_cli_with_flags(base_app, false)
    }
    
    pub fn build_cli_with_flags(&self, base_app: Command, experimental: bool) -> Command {
        // First, register all non-experimental plugins
        let mut app_with_regular = self.plugins.values()
            .filter(|plugin| !plugin.is_experimental())
            .fold(base_app, |app, plugin| {
                plugin.register_commands(app)
            });
        
        // Then, if experimental is enabled, add experimental plugins with indicators
        if experimental {
            app_with_regular = self.plugins.values()
                .filter(|plugin| plugin.is_experimental())
                .fold(app_with_regular, |app, plugin| {
                    let mut registered_app = plugin.register_commands(app);
                    
                    // Modify the experimental command's about text
                    let plugin_name = plugin.name();
                    registered_app = registered_app.mut_subcommand(plugin_name, |subcmd| {
                        let current_about = subcmd.get_about()
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        // Add yellow color to [EXPERIMENTAL] tag using ANSI codes
                        subcmd.about(format!("\x1b[93m[EXPERIMENTAL]\x1b[0m {}", current_about))
                    });
                    
                    registered_app
                })
        }
        
        app_with_regular
    }
    
    pub fn handle_command(&self, command_name: &str, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if let Some(plugin) = self.plugins.get(command_name) {
            plugin.handle_command(matches, config)
        } else {
            Err(MetarepoError::Plugin(format!("Unknown command: {}", command_name)).into())
        }
    }
    
    pub fn get_plugin(&self, name: &str) -> Option<&Box<dyn MetaPlugin>> {
        self.plugins.get(name)
    }
    
    pub fn list_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_registry() {
        let registry = PluginRegistry::new();
        
        assert!(registry.get_plugin("test").is_none());
        assert!(registry.get_plugin("nonexistent").is_none());
        
        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 0);
    }
}