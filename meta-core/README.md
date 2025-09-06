# metarepo-core

Core library for building [metarepo](https://github.com/codyaverett/metarepo) plugins. This crate provides the essential interfaces and types needed to create custom plugins that extend the metarepo CLI functionality.

## Overview

`metarepo-core` is the foundation for the metarepo plugin system. It defines the `MetaPlugin` trait that all plugins must implement, along with configuration structures and utilities for working with meta repositories.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
metarepo-core = "0.3"
```

## Quick Start

Here's a minimal example of creating a metarepo plugin:

```rust
use anyhow::Result;
use clap::{ArgMatches, Command};
use metarepo_core::{MetaPlugin, RuntimeConfig};

pub struct MyPlugin {
    name: String,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            name: "myplugin".to_string(),
        }
    }
}

impl MetaPlugin for MyPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("myplugin")
                .about("My custom plugin")
                .subcommand(
                    Command::new("hello")
                        .about("Say hello")
                )
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("hello", _)) => {
                println!("Hello from my plugin!");
                println!("Working directory: {:?}", config.working_dir);
                Ok(())
            }
            _ => Ok(())
        }
    }
}
```

## Core Components

### MetaPlugin Trait

The `MetaPlugin` trait is the main interface that all plugins must implement:

```rust
pub trait MetaPlugin: Send + Sync {
    /// Returns the plugin name (used for command routing)
    fn name(&self) -> &str;
    
    /// Register CLI commands for this plugin
    fn register_commands(&self, app: Command) -> Command;
    
    /// Handle a command for this plugin
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()>;
    
    /// Returns true if this plugin is experimental (default: false)
    fn is_experimental(&self) -> bool {
        false
    }
}
```

### RuntimeConfig

The `RuntimeConfig` struct provides access to the meta repository configuration and environment:

```rust
pub struct RuntimeConfig {
    pub meta_config: MetaConfig,      // The loaded .meta file configuration
    pub working_dir: PathBuf,         // Current working directory
    pub meta_file_path: Option<PathBuf>, // Path to the .meta file (if found)
    pub experimental: bool,           // Whether experimental features are enabled
}
```

Key methods:
- `has_meta_file()` - Check if a .meta file was found
- `meta_root()` - Get the root directory of the meta repository
- `is_experimental()` - Check if experimental features are enabled

### MetaConfig

The `MetaConfig` struct represents the contents of a `.meta` file:

```rust
pub struct MetaConfig {
    pub ignore: Vec<String>,                    // Patterns to ignore
    pub projects: HashMap<String, String>,      // Project paths -> repository URLs
    pub plugins: Option<HashMap<String, String>>, // Plugin configurations
    pub nested: Option<NestedConfig>,           // Nested repository settings
}
```

Methods for working with configurations:
- `load()` - Load from the nearest .meta file
- `load_from_file()` - Load from a specific file path
- `save_to_file()` - Save configuration to a file
- `find_meta_file()` - Search for .meta file in parent directories

## Complete Example

For a complete working example of a metarepo plugin, see the [metarepo-plugin-example](https://github.com/codyaverett/metarepo/tree/main/examples/metarepo-plugin-example) in the main repository.

This example demonstrates:
- Command registration with subcommands and arguments
- Accessing the runtime configuration
- Working with meta repository projects
- Handling different command patterns

## Plugin Development Guide

### 1. Create a New Plugin Project

```bash
cargo new --lib my-metarepo-plugin
cd my-metarepo-plugin
```

### 2. Add Dependencies

```toml
[dependencies]
metarepo-core = "0.3"
anyhow = "1.0"
clap = "4.0"
```

### 3. Implement the Plugin

Create your plugin implementation in `src/lib.rs`:

```rust
use anyhow::Result;
use clap::{ArgMatches, Command, Arg};
use metarepo_core::{MetaPlugin, RuntimeConfig};

pub struct MyPlugin;

impl MetaPlugin for MyPlugin {
    fn name(&self) -> &str {
        "myplugin"
    }

    fn register_commands(&self, app: Command) -> Command {
        app.subcommand(
            Command::new("myplugin")
                .about("My custom metarepo plugin")
                .subcommand(
                    Command::new("list")
                        .about("List all projects")
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a new project")
                        .arg(
                            Arg::new("path")
                                .required(true)
                                .help("Path to the project")
                        )
                )
        )
    }

    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        match matches.subcommand() {
            Some(("list", _)) => {
                for (path, url) in &config.meta_config.projects {
                    println!("{}: {}", path, url);
                }
                Ok(())
            }
            Some(("add", sub_matches)) => {
                let path = sub_matches.get_one::<String>("path").unwrap();
                println!("Adding project at: {}", path);
                // Implementation here
                Ok(())
            }
            _ => Ok(())
        }
    }
}
```

### 4. Testing Your Plugin

You can test your plugin by creating a binary that uses it:

```rust
// src/main.rs
use anyhow::Result;
use clap::Command;
use metarepo_core::{MetaConfig, RuntimeConfig};
use my_metarepo_plugin::MyPlugin;

fn main() -> Result<()> {
    let plugin = MyPlugin;
    
    let app = Command::new("test-plugin");
    let app = plugin.register_commands(app);
    
    let matches = app.get_matches();
    
    // Create a test runtime config
    let config = RuntimeConfig {
        meta_config: MetaConfig::default(),
        working_dir: std::env::current_dir()?,
        meta_file_path: None,
        experimental: false,
    };
    
    plugin.handle_command(&matches, &config)?;
    
    Ok(())
}
```

## Advanced Features

### Experimental Plugins

Mark your plugin as experimental to require the `--experimental` flag:

```rust
impl MetaPlugin for MyPlugin {
    fn is_experimental(&self) -> bool {
        true
    }
    // ... other methods
}
```

### Working with Nested Repositories

The `NestedConfig` struct provides configuration for handling nested repositories:

```rust
pub struct NestedConfig {
    pub recursive_import: bool,     // Import nested repos recursively
    pub max_depth: usize,          // Maximum nesting depth
    pub flatten: bool,              // Flatten nested structure
    pub cycle_detection: bool,      // Detect circular dependencies
    pub ignore_nested: Vec<String>, // Patterns to ignore
    pub namespace_separator: Option<String>, // Separator for namespaces
    pub preserve_structure: bool,   // Preserve directory structure
}
```

### Error Handling

Use `anyhow::Result` for error handling:

```rust
use anyhow::{anyhow, Context};

fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
    let project_path = matches
        .get_one::<String>("path")
        .ok_or_else(|| anyhow!("Project path is required"))?;
    
    std::fs::read_dir(project_path)
        .context("Failed to read project directory")?;
    
    Ok(())
}
```

## Integration with Metarepo CLI

Once your plugin is built, it can be integrated with the metarepo CLI in several ways:

1. **Built-in plugins**: Compile directly into the metarepo binary
2. **Dynamic plugins**: Load at runtime (requires dynamic loading support)
3. **External plugins**: Run as separate processes communicating via IPC

Refer to the [main metarepo documentation](https://github.com/codyaverett/metarepo) for details on plugin integration.

## API Stability

The `metarepo-core` API follows semantic versioning. The `MetaPlugin` trait and core types are considered stable from version 1.0 onwards. Minor versions may add new optional methods with default implementations.

## Contributing

Contributions are welcome! Please see the [main repository](https://github.com/codyaverett/metarepo) for contribution guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/codyaverett/metarepo/blob/main/LICENSE) file for details.

## Resources

- [Main Repository](https://github.com/codyaverett/metarepo)
- [Example Plugin](https://github.com/codyaverett/metarepo/tree/main/examples/metarepo-plugin-example)
- [Documentation](https://docs.rs/metarepo-core)
- [Issue Tracker](https://github.com/codyaverett/metarepo/issues)