use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Plugin manifest structure (plugin.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata
    pub plugin: PluginInfo,
    
    /// Commands provided by the plugin
    #[serde(default)]
    pub commands: Vec<ManifestCommand>,
    
    /// Plugin configuration options
    #[serde(default)]
    pub config: Option<PluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub min_meta_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestCommand {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub long_description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub args: Vec<ManifestArg>,
    #[serde(default)]
    pub subcommands: Vec<ManifestCommand>,
    #[serde(default)]
    pub examples: Vec<Example>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestArg {
    pub name: String,
    #[serde(default)]
    pub short: Option<char>,
    #[serde(default)]
    pub long: Option<String>,
    pub help: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub takes_value: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub possible_values: Vec<String>,
    #[serde(default)]
    pub value_type: ArgValueType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArgValueType {
    #[default]
    String,
    Number,
    Bool,
    Path,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// How the plugin should be executed
    #[serde(default)]
    pub execution: ExecutionConfig,
    
    /// Plugin capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,
    
    /// Required environment variables
    #[serde(default)]
    pub required_env: Vec<String>,
    
    /// Plugin dependencies
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionConfig {
    /// Execution mode: "process", "wasm", "docker"
    #[serde(default = "default_exec_mode")]
    pub mode: String,
    
    /// Path to the executable (relative to manifest)
    pub binary: Option<String>,
    
    /// Docker image for docker mode
    pub docker_image: Option<String>,
    
    /// WASM module for wasm mode
    pub wasm_module: Option<String>,
    
    /// Communication protocol: "json-rpc", "cli", "grpc"
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_exec_mode() -> String {
    "process".to_string()
}

fn default_protocol() -> String {
    "cli".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub optional: bool,
}

impl PluginManifest {
    /// Load manifest from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }
    
    /// Parse manifest from TOML string
    pub fn from_str(content: &str) -> Result<Self> {
        let manifest: PluginManifest = toml::from_str(content)?;
        manifest.validate()?;
        Ok(manifest)
    }
    
    /// Validate the manifest
    pub fn validate(&self) -> Result<()> {
        // Validate plugin info
        if self.plugin.name.is_empty() {
            return Err(anyhow::anyhow!("Plugin name cannot be empty"));
        }
        
        if self.plugin.version.is_empty() {
            return Err(anyhow::anyhow!("Plugin version cannot be empty"));
        }
        
        // Validate commands
        for cmd in &self.commands {
            Self::validate_command(cmd)?;
        }
        
        // Validate execution config if present
        if let Some(ref config) = self.config {
            let exec = &config.execution;
            match exec.mode.as_str() {
                "process" => {
                    if exec.binary.is_none() {
                        return Err(anyhow::anyhow!("Binary path required for process mode"));
                    }
                }
                "docker" => {
                    if exec.docker_image.is_none() {
                        return Err(anyhow::anyhow!("Docker image required for docker mode"));
                    }
                }
                "wasm" => {
                    if exec.wasm_module.is_none() {
                        return Err(anyhow::anyhow!("WASM module required for wasm mode"));
                    }
                }
                mode => {
                    return Err(anyhow::anyhow!("Unknown execution mode: {}", mode));
                }
            }
        }
        
        Ok(())
    }
    
    fn validate_command(cmd: &ManifestCommand) -> Result<()> {
        if cmd.name.is_empty() {
            return Err(anyhow::anyhow!("Command name cannot be empty"));
        }
        
        // Validate arguments
        for arg in &cmd.args {
            if arg.name.is_empty() {
                return Err(anyhow::anyhow!("Argument name cannot be empty"));
            }
            
            // Ensure either short or long flag is provided for non-positional args
            if !arg.required && arg.short.is_none() && arg.long.is_none() {
                return Err(anyhow::anyhow!(
                    "Argument '{}' must have either short or long flag",
                    arg.name
                ));
            }
        }
        
        // Recursively validate subcommands
        for subcmd in &cmd.subcommands {
            Self::validate_command(subcmd)?;
        }
        
        Ok(())
    }
    
    /// Generate a sample manifest
    pub fn example() -> Self {
        PluginManifest {
            plugin: PluginInfo {
                name: "example-plugin".to_string(),
                version: "0.1.0".to_string(),
                description: "An example metarepo plugin".to_string(),
                author: "Your Name".to_string(),
                license: "MIT".to_string(),
                homepage: "https://github.com/yourusername/example-plugin".to_string(),
                repository: "https://github.com/yourusername/example-plugin".to_string(),
                experimental: false,
                min_meta_version: Some("0.4.0".to_string()),
            },
            commands: vec![
                ManifestCommand {
                    name: "example".to_string(),
                    description: "Example command".to_string(),
                    long_description: Some("This is a longer description of the example command.".to_string()),
                    aliases: vec!["ex".to_string()],
                    args: vec![
                        ManifestArg {
                            name: "verbose".to_string(),
                            short: Some('v'),
                            long: Some("verbose".to_string()),
                            help: "Enable verbose output".to_string(),
                            required: false,
                            takes_value: false,
                            default_value: None,
                            possible_values: vec![],
                            value_type: ArgValueType::Bool,
                        },
                        ManifestArg {
                            name: "input".to_string(),
                            short: Some('i'),
                            long: Some("input".to_string()),
                            help: "Input file path".to_string(),
                            required: true,
                            takes_value: true,
                            default_value: None,
                            possible_values: vec![],
                            value_type: ArgValueType::Path,
                        },
                    ],
                    subcommands: vec![
                        ManifestCommand {
                            name: "run".to_string(),
                            description: "Run the example".to_string(),
                            long_description: None,
                            aliases: vec![],
                            args: vec![],
                            subcommands: vec![],
                            examples: vec![],
                        },
                    ],
                    examples: vec![
                        Example {
                            command: "meta example -v --input file.txt run".to_string(),
                            description: "Run the example with verbose output".to_string(),
                        },
                    ],
                },
            ],
            config: Some(PluginConfig {
                execution: ExecutionConfig {
                    mode: "process".to_string(),
                    binary: Some("./bin/example-plugin".to_string()),
                    docker_image: None,
                    wasm_module: None,
                    protocol: "cli".to_string(),
                },
                capabilities: vec!["filesystem".to_string(), "network".to_string()],
                required_env: vec![],
                dependencies: vec![],
            }),
        }
    }
    
    /// Write example manifest to file
    pub fn write_example(path: &Path) -> Result<()> {
        let manifest = Self::example();
        let content = toml::to_string_pretty(&manifest)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}