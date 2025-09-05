use crate::{GestaltError, RuntimeConfig};
use anyhow::Result;
use clap::{ArgMatches, Command};
use meta_core::MetaPlugin;
use std::collections::HashMap;

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
        // Register built-in workspace plugins
        self.register(Box::new(meta_init::InitPlugin::new()));
        self.register(Box::new(meta_git::GitPlugin::new()));
        self.register(Box::new(meta_project::ProjectPlugin::new()));
        self.register(Box::new(gestalt_exec::ExecPlugin::new()));
        self.register(Box::new(gestalt_rules::RulesPlugin::new()));

        // Only register experimental plugins if flag is set
        if experimental {
            self.register(Box::new(gestalt_plugin_mcp::McpPlugin::new()));
        }

        // TODO: Enable more plugins as they're implemented
        // self.register(Box::new(meta_loop::LoopPlugin::new()));
    }

    pub fn build_cli(&self, base_app: Command) -> Command {
        self.build_cli_with_flags(base_app, false)
    }

    pub fn build_cli_with_flags(&self, base_app: Command, experimental: bool) -> Command {
        self.plugins
            .values()
            .filter(|plugin| experimental || !plugin.is_experimental())
            .fold(base_app, |app, plugin| plugin.register_commands(app))
    }

    pub fn handle_command(
        &self,
        command_name: &str,
        matches: &ArgMatches,
        config: &RuntimeConfig,
    ) -> Result<()> {
        if let Some(plugin) = self.plugins.get(command_name) {
            plugin.handle_command(matches, config)
        } else {
            Err(GestaltError::Plugin(format!("Unknown command: {}", command_name)).into())
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
