# Plugin Development Guide (Simplified Architecture)

This guide explains how to develop plugins for metarepo using the new simplified plugin architecture introduced in v0.4.0.

## Table of Contents
- [Overview](#overview)
- [Quick Start](#quick-start)
- [Plugin Architecture](#plugin-architecture)
- [Development Methods](#development-methods)
- [Plugin Manifest](#plugin-manifest)
- [Help System](#help-system)
- [Testing](#testing)
- [Distribution](#distribution)

## Overview

Metarepo's plugin system has been redesigned to reduce boilerplate and provide consistent interfaces for both internal and external plugins. The new architecture offers:

- **Less Boilerplate**: Use builder patterns or derive macros instead of manual implementations
- **Consistent Help**: Automatic help generation in multiple formats (terminal, JSON, YAML, Markdown)
- **Declarative Definitions**: Define plugins using manifests or builder APIs
- **Multiple Languages**: Support for Rust, Python, JavaScript, and shell scripts
- **Unified Output**: Consistent `--help`, `--ai`, and `--output-format` flags across all plugins

## Quick Start

### Creating a New Plugin

Use the plugin scaffold command to quickly create a new plugin:

```bash
# Create a Rust plugin
meta plugin scaffold my-plugin --template rust

# Create a Python plugin
meta plugin scaffold my-plugin --template python

# Create a JavaScript plugin
meta plugin scaffold my-plugin --template javascript

# Create a shell script plugin
meta plugin scaffold my-plugin --template binary
```

This creates a complete plugin structure with:
- `plugin.toml` - Plugin manifest
- Source code with example implementation
- README with usage instructions
- Test structure

### Installing Your Plugin

```bash
# Install from local directory
meta plugin install --local ./my-plugin

# Install from git
meta plugin install git+https://github.com/username/my-plugin.git

# Install from crates.io (Rust plugins)
meta plugin install my-plugin
```

## Plugin Architecture

### 1. Builder Pattern (Recommended for Rust)

The builder pattern provides a declarative way to define plugins with minimal boilerplate:

```rust
use metarepo_core::{plugin, command, arg};

fn create_plugin() -> impl MetaPlugin {
    plugin("my-plugin")
        .version("0.1.0")
        .description("My awesome plugin")
        .author("Your Name")
        .command(
            command("process")
                .about("Process files")
                .arg(
                    arg("input")
                        .short('i')
                        .long("input")
                        .help("Input file path")
                        .required(true)
                        .takes_value(true)
                )
                .arg(
                    arg("verbose")
                        .short('v')
                        .long("verbose")
                        .help("Enable verbose output")
                )
        )
        .handler("process", |matches, config| {
            let input = matches.get_one::<String>("input").unwrap();
            println!("Processing: {}", input);
            Ok(())
        })
        .build()
}
```

### 2. BasePlugin Trait

For more control, implement the `BasePlugin` trait which provides default implementations:

```rust
use metarepo_core::{BasePlugin, MetaPlugin, HelpFormat};

struct MyPlugin;

impl MetaPlugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    
    fn register_commands(&self, app: Command) -> Command {
        // Register your commands
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Handle commands
    }
}

impl BasePlugin for MyPlugin {
    fn version(&self) -> Option<&str> { Some("0.1.0") }
    fn description(&self) -> Option<&str> { Some("My plugin") }
    fn author(&self) -> Option<&str> { Some("Your Name") }
    
    // Automatic help generation in multiple formats!
}
```

### 3. Plugin Manifest

Define your plugin declaratively using `plugin.toml`:

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
description = "My awesome plugin"
author = "Your Name"
license = "MIT"
experimental = false
min_meta_version = "0.4.0"

[[commands]]
name = "process"
description = "Process files"
aliases = ["p", "proc"]

[[commands.args]]
name = "input"
short = "i"
long = "input"
help = "Input file path"
required = true
takes_value = true
value_type = "path"

[[commands.args]]
name = "verbose"
short = "v"
long = "verbose"
help = "Enable verbose output"
required = false
takes_value = false
value_type = "bool"

[[commands.examples]]
command = "meta my-plugin process -i file.txt -v"
description = "Process file.txt with verbose output"

[config.execution]
mode = "process"  # or "docker", "wasm"
binary = "./bin/my-plugin"
protocol = "cli"  # or "json-rpc", "grpc"
```

## Development Methods

### Rust Plugins

Best for performance and tight integration:

```rust
// src/main.rs
use metarepo_core::plugin_runner;

fn main() -> Result<()> {
    // Automatically handles plugin protocol
    plugin_runner::run(create_plugin())
}
```

### Python Plugins

Great for rapid development:

```python
#!/usr/bin/env python3
import metarepo

@metarepo.plugin("my-plugin")
class MyPlugin:
    @metarepo.command("process")
    @metarepo.arg("--input", required=True, help="Input file")
    def process(self, args):
        print(f"Processing: {args.input}")
        return 0

if __name__ == "__main__":
    MyPlugin().run()
```

### JavaScript Plugins

For Node.js ecosystem integration:

```javascript
const { Plugin, command, arg } = require('metarepo');

const plugin = new Plugin('my-plugin')
  .version('0.1.0')
  .command(
    command('process')
      .arg(arg('input').required())
      .handler((args) => {
        console.log(`Processing: ${args.input}`);
      })
  );

plugin.run();
```

### Shell Script Plugins

For simple integrations:

```bash
#!/bin/bash
# Follows the metarepo plugin protocol
# See scaffold output for complete example
```

## Help System

### Automatic Help Generation

All plugins automatically support multiple help formats:

```bash
# Traditional terminal help
meta my-plugin --help

# JSON format for programmatic use
meta my-plugin --output-format json

# YAML format
meta my-plugin --output-format yaml

# Markdown for documentation
meta my-plugin --output-format markdown

# AI-friendly output (alias for JSON)
meta my-plugin --ai
```

### Custom Help

Override the default help by implementing `show_help()`:

```rust
impl BasePlugin for MyPlugin {
    fn show_help(&self, format: HelpFormat) -> Result<()> {
        match format {
            HelpFormat::Terminal => {
                // Custom terminal output
            }
            HelpFormat::Json => {
                // Custom JSON output
            }
            _ => {
                // Delegate to default
                self.default_show_help(format)
            }
        }
    }
}
```

## Testing

### Unit Tests

Test your plugin logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_creation() {
        let plugin = create_plugin();
        assert_eq!(plugin.name(), "my-plugin");
    }
}
```

### Integration Tests

Test the full plugin:

```bash
# Test plugin commands
meta plugin test my-plugin

# Run specific test scenarios
meta plugin test my-plugin --scenario process-file
```

### Manual Testing

```bash
# Run plugin in development mode
meta plugin dev ./my-plugin process --input test.txt

# Enable debug output
RUST_LOG=debug meta my-plugin process --input test.txt
```

## Distribution

### Crates.io (Rust)

```toml
# Cargo.toml
[package]
name = "metarepo-plugin-my-plugin"
version = "0.1.0"

[dependencies]
metarepo-core = "0.4"
```

```bash
cargo publish
```

### NPM (JavaScript)

```json
{
  "name": "@metarepo/plugin-my-plugin",
  "version": "0.1.0"
}
```

```bash
npm publish
```

### PyPI (Python)

```python
# setup.py
setup(
    name="metarepo-plugin-my-plugin",
    version="0.1.0",
    entry_points={
        'metarepo.plugins': [
            'my-plugin = my_plugin:MyPlugin',
        ],
    }
)
```

```bash
python -m build
twine upload dist/*
```

### Direct Distribution

Include installation instructions in your README:

```markdown
## Installation

### From Source
\```bash
git clone https://github.com/username/my-plugin
cd my-plugin
meta plugin install --local .
\```

### Pre-built Binary
\```bash
curl -L https://github.com/username/my-plugin/releases/latest/download/my-plugin > ~/.config/metarepo/plugins/my-plugin
chmod +x ~/.config/metarepo/plugins/my-plugin
\```
```

## Best Practices

1. **Use the Builder Pattern**: Reduces boilerplate significantly
2. **Define a Manifest**: Makes your plugin discoverable and self-documenting
3. **Support All Output Formats**: Ensures compatibility with different tools and workflows
4. **Handle Errors Gracefully**: Return meaningful error messages
5. **Document Examples**: Include usage examples in your manifest
6. **Version Properly**: Follow semantic versioning
7. **Test Thoroughly**: Include unit and integration tests
8. **Consider Performance**: Use async/parallel processing where appropriate

## Advanced Topics

### Async Commands

```rust
.handler("async-command", |matches, config| {
    tokio::runtime::Runtime::new()?.block_on(async {
        // Async operations
        Ok(())
    })
})
```

### Progress Reporting

```rust
use indicatif::{ProgressBar, ProgressStyle};

let pb = ProgressBar::new(100);
pb.set_style(ProgressStyle::default_bar());
pb.inc(1);
```

### Configuration Files

```rust
// Support plugin-specific config
let config_path = config.meta_root()
    .unwrap()
    .join(".meta")
    .join("plugins")
    .join("my-plugin.toml");
```

### Inter-plugin Communication

```rust
// Call another plugin
let result = config.plugin_registry
    .get("other-plugin")?
    .handle_command(&args, config)?;
```

## Migration from Old Architecture

If you have an existing plugin using the old architecture, here's how to migrate:

### Before (Old Architecture)

```rust
pub struct MyPlugin;

impl MyPlugin {
    fn show_help(&self) -> Result<()> {
        let mut app = Command::new("meta my-plugin")
            .about("My plugin")
            .arg(/* ... */);
        app.print_help()?;
        Ok(())
    }
}

impl MetaPlugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    
    fn register_commands(&self, app: Command) -> Command {
        // Duplicate command definition
    }
    
    fn handle_command(&self, matches: &ArgMatches, config: &RuntimeConfig) -> Result<()> {
        // Manual help handling
        if matches.subcommand().is_none() {
            return self.show_help();
        }
        // ...
    }
}
```

### After (New Architecture)

```rust
use metarepo_core::{plugin, command, arg};

fn create_plugin() -> impl MetaPlugin {
    plugin("my-plugin")
        .version("0.1.0")
        .description("My plugin")
        .command(/* ... */)
        .handler("cmd", handle_cmd)
        .build()
}
```

## Troubleshooting

### Common Issues

1. **Plugin not found**: Ensure it's in `~/.config/metarepo/plugins/` or installed via `meta plugin install`
2. **Permission denied**: Make sure the plugin binary is executable (`chmod +x`)
3. **Protocol errors**: Check that your plugin implements the correct protocol version
4. **Help not showing**: Verify your manifest or command registration

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug meta my-plugin

# Test plugin protocol
echo '{"type":"GetInfo"}' | ./my-plugin

# Validate manifest
meta plugin validate ./my-plugin/plugin.toml
```

## Examples

See the [examples directory](../examples/plugins/) for complete plugin implementations in different languages:

- [Rust Plugin](../examples/plugins/rust/)
- [Python Plugin](../examples/plugins/python/)
- [JavaScript Plugin](../examples/plugins/javascript/)
- [Shell Script Plugin](../examples/plugins/shell/)

## Resources

- [Plugin API Reference](./API_REFERENCE.md)
- [MetaPlugin Trait Documentation](https://docs.rs/metarepo-core/latest/metarepo_core/trait.MetaPlugin.html)
- [Example Plugins](https://github.com/metarepo/plugins)
- [Community Plugins](https://github.com/topics/metarepo-plugin)