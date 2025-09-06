# Metarepo Plugin Development Guide

This guide explains how to create custom plugins for the metarepo CLI tool.

## Overview

Metarepo supports two types of plugins:
1. **Built-in plugins**: Compiled directly into the main binary (init, git, project, exec, rules, mcp)
2. **External plugins**: Developed as separate crates and loaded dynamically

## Creating an External Plugin

### Step 1: Set Up Your Plugin Crate

Create a new Rust project for your plugin:

```bash
cargo new --lib metarepo-plugin-example
cd metarepo-plugin-example
```

### Step 2: Add Dependencies

Update your `Cargo.toml`:

```toml
[package]
name = "metarepo-plugin-example"
version = "0.1.0"
edition = "2021"
description = "Example plugin for metarepo"
license = "MIT"

[dependencies]
metarepo-core = "0.2.1"  # Use the latest version
anyhow = "1.0"
clap = "4.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[lib]
crate-type = ["cdylib", "rlib"]
```

### Step 3: Implement the MetaPlugin Trait

Create your plugin implementation in `src/lib.rs`:

```rust
use anyhow::Result;
use clap::{ArgMatches, Command, Arg};
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
                .about("Example plugin command")
                .arg(
                    Arg::new("message")
                        .help("Message to display")
                        .required(true)
                        .index(1)
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if let Some(message) = matches.get_one::<String>("message") {
            println!("Example plugin says: {}", message);
            println!("Working directory: {:?}", config.working_dir);
            
            if config.has_meta_file() {
                println!("Meta repository detected!");
                println!("Projects: {:?}", config.meta_config.projects);
            }
        }
        
        Ok(())
    }
    
    fn is_experimental(&self) -> bool {
        false  // Set to true if your plugin is experimental
    }
}

// Export the plugin constructor
#[no_mangle]
pub extern "C" fn create_plugin() -> Box<dyn MetaPlugin> {
    Box::new(ExamplePlugin::new())
}
```

### Step 4: Create a Binary Wrapper (For Subprocess-based Loading)

For subprocess-based external plugins, create `src/main.rs`:

```rust
use anyhow::Result;
use clap::Command;
use metarepo_core::{MetaPlugin, RuntimeConfig, MetaConfig};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

mod lib;
use lib::ExamplePlugin;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum PluginMessage {
    GetInfo,
    RegisterCommands,
    HandleCommand { args: Vec<String>, config: RuntimeConfig },
    InfoResponse { name: String, experimental: bool },
    CommandsResponse { commands: String },
    Success,
    Error { message: String },
}

fn main() -> Result<()> {
    let plugin = ExamplePlugin::new();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    
    for line in stdin.lock().lines() {
        let line = line?;
        let message: PluginMessage = serde_json::from_str(&line)?;
        
        let response = match message {
            PluginMessage::GetInfo => {
                PluginMessage::InfoResponse {
                    name: plugin.name().to_string(),
                    experimental: plugin.is_experimental(),
                }
            }
            PluginMessage::RegisterCommands => {
                let app = Command::new("dummy");
                let app = plugin.register_commands(app);
                // Serialize command structure
                PluginMessage::CommandsResponse {
                    commands: format!("{:?}", app),
                }
            }
            PluginMessage::HandleCommand { args, config } => {
                // Parse args and handle command
                match plugin.handle_command(&Default::default(), &config) {
                    Ok(_) => PluginMessage::Success,
                    Err(e) => PluginMessage::Error { message: e.to_string() },
                }
            }
            _ => PluginMessage::Error { message: "Unknown message type".to_string() },
        };
        
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    
    Ok(())
}
```

## Plugin Configuration

### Declaring Plugins in .meta File

External plugins can be declared in your `.meta` file:

```json
{
  "projects": {
    "project1": "https://github.com/user/project1.git"
  },
  "plugins": {
    "example": "0.1.0",
    "local-plugin": "file:../my-local-plugin",
    "git-plugin": "git+https://github.com/user/plugin.git"
  }
}
```

### Plugin Resolution Order

1. Built-in plugins (highest priority)
2. Local file paths
3. Installed crates from crates.io
4. Git repositories

## Testing Your Plugin

### Local Testing

1. Build your plugin:
```bash
cargo build --release
```

2. Test with a local metarepo installation:
```bash
# In your metarepo directory
meta plugin add ../metarepo-plugin-example/target/release/libmetarepo_plugin_example.so
```

3. Use your plugin:
```bash
meta example "Hello from my plugin!"
```

### Integration Testing

Create integration tests in `tests/integration.rs`:

```rust
#[cfg(test)]
mod tests {
    use metarepo_core::{MetaPlugin, RuntimeConfig, MetaConfig};
    use std::path::PathBuf;
    
    #[test]
    fn test_plugin_creation() {
        let plugin = super::super::ExamplePlugin::new();
        assert_eq!(plugin.name(), "example");
    }
    
    #[test]
    fn test_command_handling() {
        let plugin = super::super::ExamplePlugin::new();
        let config = RuntimeConfig {
            meta_config: MetaConfig::default(),
            working_dir: PathBuf::from("."),
            meta_file_path: None,
            experimental: false,
        };
        
        // Test your command handling
        let result = plugin.handle_command(&Default::default(), &config);
        assert!(result.is_ok());
    }
}
```

## Publishing Your Plugin

### To crates.io

1. Ensure your `Cargo.toml` has all required fields:
```toml
[package]
name = "metarepo-plugin-yourname"
version = "0.1.0"
authors = ["Your Name <email@example.com>"]
edition = "2021"
description = "Your plugin description"
documentation = "https://docs.rs/metarepo-plugin-yourname"
homepage = "https://github.com/yourusername/metarepo-plugin-yourname"
repository = "https://github.com/yourusername/metarepo-plugin-yourname"
license = "MIT OR Apache-2.0"
keywords = ["metarepo", "plugin", "meta", "monorepo"]
categories = ["development-tools"]
```

2. Publish:
```bash
cargo publish
```

### Installation by Users

Users can install your plugin using:

```bash
# From crates.io
meta plugin install metarepo-plugin-yourname

# From local path
meta plugin add /path/to/plugin

# From git
meta plugin add git+https://github.com/user/plugin.git
```

## Best Practices

1. **Error Handling**: Use `anyhow::Result` for consistent error handling
2. **Configuration**: Respect the `RuntimeConfig` provided by metarepo
3. **Documentation**: Document all public APIs and commands
4. **Testing**: Include comprehensive unit and integration tests
5. **Versioning**: Follow semantic versioning for your plugin
6. **Dependencies**: Keep dependencies minimal to avoid conflicts
7. **Performance**: Lazy-load heavy dependencies when possible
8. **Security**: Validate all user input and file paths

## Advanced Topics

### Accessing Meta Repository State

```rust
fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    // Check if we're in a meta repository
    if let Some(meta_root) = config.meta_root() {
        println!("Meta repository root: {:?}", meta_root);
        
        // Access projects
        for (name, url) in &config.meta_config.projects {
            println!("Project {}: {}", name, url);
        }
    }
    
    Ok(())
}
```

### Working with Experimental Features

```rust
impl MetaPlugin for MyPlugin {
    fn is_experimental(&self) -> bool {
        true  // Mark as experimental
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if !config.is_experimental() {
            return Err(anyhow::anyhow!(
                "This plugin requires the --experimental flag"
            ));
        }
        
        // Experimental functionality here
        Ok(())
    }
}
```

### Plugin Communication

For complex plugins that need to communicate with other plugins:

```rust
use metarepo_core::{MetaPlugin, RuntimeConfig};

pub trait PluginCommunication {
    fn send_message(&self, target_plugin: &str, message: &str) -> Result<String>;
    fn receive_message(&mut self) -> Result<Option<String>>;
}
```

## Examples

### File System Plugin

```rust
use std::fs;
use std::path::Path;

impl MetaPlugin for FsPlugin {
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("fs")
                .about("File system operations")
                .subcommand(
                    Command::new("clean")
                        .about("Clean build artifacts")
                        .arg(Arg::new("pattern")
                            .help("Pattern to match")
                            .default_value("target"))
                )
        )
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        if let Some(("clean", sub_matches)) = matches.subcommand() {
            let pattern = sub_matches.get_one::<String>("pattern").unwrap();
            
            for (project, _) in &config.meta_config.projects {
                let project_path = config.working_dir.join(project);
                let target_path = project_path.join(pattern);
                
                if target_path.exists() {
                    fs::remove_dir_all(&target_path)?;
                    println!("Cleaned: {:?}", target_path);
                }
            }
        }
        
        Ok(())
    }
}
```

### HTTP API Plugin

```rust
use reqwest;

impl MetaPlugin for ApiPlugin {
    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("api")
                .about("API operations")
                .arg(Arg::new("endpoint")
                    .help("API endpoint")
                    .required(true))
        )
    }
    
    async fn handle_command(&self, matches: &ArgMatches, _config: &RuntimeConfig) -> Result<()> {
        let endpoint = matches.get_one::<String>("endpoint").unwrap();
        let response = reqwest::get(endpoint).await?;
        println!("Response: {}", response.text().await?);
        Ok(())
    }
}
```

## Troubleshooting

### Common Issues

1. **Plugin not loading**: Ensure the plugin binary is in the correct location
2. **Version conflicts**: Check that metarepo-core version matches
3. **Command conflicts**: Ensure plugin names don't conflict with built-in commands
4. **Permission errors**: Check file permissions on plugin binaries

### Debug Mode

Enable debug output:
```bash
RUST_LOG=debug meta example "test"
```

## Support

- Report issues: https://github.com/metarepo/metarepo/issues
- Documentation: https://docs.rs/metarepo-core
- Examples: https://github.com/metarepo/plugin-examples

## License

Plugins can use any license, but should be compatible with metarepo's MIT license for distribution.