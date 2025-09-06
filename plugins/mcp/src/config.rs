use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::server::McpServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn config_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;
        
        let config_dir = Path::new(&home).join(".config").join("metarepo").join("mcp");
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .context("Failed to create config directory")?;
        }
        
        Ok(config_dir)
    }

    pub fn config_file() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("servers.json"))
    }

    pub fn load() -> Result<Self> {
        let config_file = Self::config_file()?;
        
        if !config_file.exists() {
            return Ok(Self::new());
        }
        
        let content = fs::read_to_string(&config_file)
            .context("Failed to read config file")?;
        
        serde_json::from_str(&content)
            .context("Failed to parse config file")
    }

    pub fn save(&self) -> Result<()> {
        let config_file = Self::config_file()?;
        
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(&config_file, content)
            .context("Failed to write config file")?;
        
        Ok(())
    }

    pub fn add_server(&mut self, config: McpServerConfig) -> Result<()> {
        if self.servers.contains_key(&config.name) {
            return Err(anyhow::anyhow!("Server '{}' already exists in configuration", config.name));
        }
        
        self.servers.insert(config.name.clone(), config);
        self.save()?;
        
        Ok(())
    }

    pub fn update_server(&mut self, config: McpServerConfig) -> Result<()> {
        self.servers.insert(config.name.clone(), config);
        self.save()?;
        Ok(())
    }

    pub fn remove_server(&mut self, name: &str) -> Result<()> {
        if self.servers.remove(name).is_none() {
            return Err(anyhow::anyhow!("Server '{}' not found in configuration", name));
        }
        
        self.save()?;
        Ok(())
    }

    pub fn get_server(&self, name: &str) -> Option<&McpServerConfig> {
        self.servers.get(name)
    }

    pub fn list_servers(&self) -> Vec<&McpServerConfig> {
        self.servers.values().collect()
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self::new()
    }
}