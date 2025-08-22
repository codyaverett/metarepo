use crate::{MetaError, RuntimeConfig};
use anyhow::Result;
use clap::{ArgMatches, Command};
use std::collections::HashMap;
use meta_core::MetaPlugin;

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
        // Register built-in workspace plugins
        self.register(Box::new(meta_init::InitPlugin::new()));
        // TODO: Enable more plugins as they're fixed
        // self.register(Box::new(meta_git::GitPlugin::new()));
        // self.register(Box::new(meta_project::ProjectPlugin::new()));
        // self.register(Box::new(meta_exec::ExecPlugin::new()));
        // self.register(Box::new(meta_loop::LoopPlugin::new()));
    }
    
    pub fn build_cli(&self, base_app: Command) -> Command {
        self.plugins.values().fold(base_app, |app, plugin| {
            plugin.register_commands(app)
        })
    }
    
    pub fn handle_command(&self, command_name: &str, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if let Some(plugin) = self.plugins.get(command_name) {
            plugin.handle_command(matches, config)
        } else {
            Err(MetaError::Plugin(format!("Unknown command: {}", command_name)).into())
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