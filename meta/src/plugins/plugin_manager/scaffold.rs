use anyhow::Result;
use std::fs;
use std::path::Path;
use metarepo_core::PluginManifest;

/// Plugin scaffolding utilities
pub struct PluginScaffold;

impl PluginScaffold {
    /// Create a new plugin project structure
    pub fn create_plugin(name: &str, path: &Path, template: PluginTemplate) -> Result<()> {
        // Create directory structure
        let plugin_dir = path.join(name);
        fs::create_dir_all(&plugin_dir)?;
        fs::create_dir_all(plugin_dir.join("src"))?;
        fs::create_dir_all(plugin_dir.join("tests"))?;
        
        // Generate files based on template
        match template {
            PluginTemplate::Rust => Self::create_rust_plugin(name, &plugin_dir)?,
            PluginTemplate::Python => Self::create_python_plugin(name, &plugin_dir)?,
            PluginTemplate::JavaScript => Self::create_js_plugin(name, &plugin_dir)?,
            PluginTemplate::Binary => Self::create_binary_plugin(name, &plugin_dir)?,
        }
        
        // Create plugin manifest
        let manifest_path = plugin_dir.join("plugin.toml");
        PluginManifest::write_example(&manifest_path)?;
        println!("✓ Created plugin manifest at: {}", manifest_path.display());
        
        // Create README
        Self::create_readme(name, &plugin_dir)?;
        
        println!("✓ Plugin '{}' scaffolded successfully at: {}", name, plugin_dir.display());
        println!("\nNext steps:");
        println!("  1. Edit plugin.toml to define your plugin's commands");
        println!("  2. Implement your plugin logic in the src directory");
        println!("  3. Test your plugin: meta plugin test {}", name);
        println!("  4. Install locally: meta plugin install --local {}", plugin_dir.display());
        
        Ok(())
    }
    
    fn create_rust_plugin(name: &str, dir: &Path) -> Result<()> {
        // Create Cargo.toml
        let cargo_toml = format!(r#"[package]
name = "metarepo-plugin-{}"
version = "0.1.0"
edition = "2021"

[dependencies]
metarepo-core = "0.4"
anyhow = "1.0"
clap = "4.0"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

[[bin]]
name = "{}-plugin"
path = "src/main.rs"
"#, name, name);
        
        fs::write(dir.join("Cargo.toml"), cargo_toml)?;
        
        // Create main.rs with example implementation
        let main_rs = format!(r#"use anyhow::Result;
use metarepo_core::{{plugin, command, arg}};
use std::env;

fn main() -> Result<()> {{
    // Check if running in plugin mode
    if env::var("METAREPO_PLUGIN_MODE").is_ok() {{
        // Run as metarepo plugin
        run_as_plugin()
    }} else {{
        // Run standalone
        println!("{} plugin v0.1.0", "{}");
        println!("This is a metarepo plugin. Install it with:");
        println!("  meta plugin install --local .");
        Ok(())
    }}
}}

fn run_as_plugin() -> Result<()> {{
    use std::io::{{self, BufRead, Write}};
    use serde_json::{{json, Value}};
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    for line in stdin.lock().lines() {{
        let line = line?;
        let request: Value = serde_json::from_str(&line)?;
        
        let response = match request["type"].as_str() {{
            Some("GetInfo") => json!({{
                "type": "Info",
                "name": "{}",
                "version": "0.1.0",
                "experimental": false
            }}),
            Some("RegisterCommands") => json!({{
                "type": "Commands",
                "commands": [{{
                    "name": "{}",
                    "about": "Example command",
                    "subcommands": [],
                    "args": []
                }}]
            }}),
            Some("HandleCommand") => json!({{
                "type": "Success",
                "message": "Command executed successfully"
            }}),
            _ => json!({{
                "type": "Error",
                "message": "Unknown request type"
            }})
        }};
        
        writeln!(stdout, "{{}}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }}
    
    Ok(())
}}
"#, name, name, name, name);
        
        fs::write(dir.join("src").join("main.rs"), main_rs)?;
        
        // Create lib.rs for shared functionality
        let lib_rs = format!(r#"use anyhow::Result;
use metarepo_core::{{MetaPlugin, RuntimeConfig, BasePlugin, plugin, command, arg}};

/// Create the {} plugin using the builder pattern
pub fn create_plugin() -> impl MetaPlugin {{
    plugin("{}")
        .version("0.1.0")
        .description("{} plugin for metarepo")
        .command(
            command("example")
                .about("Example command")
                .arg(
                    arg("input")
                        .short('i')
                        .long("input")
                        .help("Input file")
                        .takes_value(true)
                )
        )
        .handler("example", |_matches, _config| {{
            println!("Example command executed!");
            Ok(())
        }})
        .build()
}}
"#, name, name, name);
        
        fs::write(dir.join("src").join("lib.rs"), lib_rs)?;
        
        Ok(())
    }
    
    fn create_python_plugin(name: &str, dir: &Path) -> Result<()> {
        // Create main.py
        let main_py = format!(r#"#!/usr/bin/env python3
import os
import sys
import json

def main():
    if os.environ.get('METAREPO_PLUGIN_MODE'):
        run_as_plugin()
    else:
        print(f"{} plugin v0.1.0")
        print("This is a metarepo plugin. Install it with:")
        print("  meta plugin install --local .")

def run_as_plugin():
    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break
            
            request = json.loads(line)
            response = handle_request(request)
            
            print(json.dumps(response))
            sys.stdout.flush()
        except Exception as e:
            response = {{"type": "Error", "message": str(e)}}
            print(json.dumps(response))
            sys.stdout.flush()

def handle_request(request):
    req_type = request.get('type')
    
    if req_type == 'GetInfo':
        return {{
            "type": "Info",
            "name": "{}",
            "version": "0.1.0",
            "experimental": False
        }}
    elif req_type == 'RegisterCommands':
        return {{
            "type": "Commands",
            "commands": [{{
                "name": "{}",
                "about": "Example command",
                "subcommands": [],
                "args": []
            }}]
        }}
    elif req_type == 'HandleCommand':
        return {{
            "type": "Success",
            "message": "Command executed successfully"
        }}
    else:
        return {{
            "type": "Error",
            "message": f"Unknown request type: {{req_type}}"
        }}

if __name__ == "__main__":
    main()
"#, name, name, name);
        
        fs::write(dir.join("main.py"), main_py)?;
        
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dir.join("main.py"))?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(dir.join("main.py"), perms)?;
        }
        
        Ok(())
    }
    
    fn create_js_plugin(name: &str, dir: &Path) -> Result<()> {
        // Create package.json
        let package_json = format!(r#"{{
  "name": "metarepo-plugin-{}",
  "version": "0.1.0",
  "description": "{} plugin for metarepo",
  "main": "index.js",
  "bin": {{
    "{}-plugin": "./index.js"
  }},
  "scripts": {{
    "test": "echo \"Error: no test specified\" && exit 1"
  }},
  "keywords": ["metarepo", "plugin"],
  "author": "",
  "license": "MIT"
}}
"#, name, name, name);
        
        fs::write(dir.join("package.json"), package_json)?;
        
        // Create index.js
        let index_js = format!(r#"#!/usr/bin/env node

const readline = require('readline');

if (process.env.METAREPO_PLUGIN_MODE) {{
    runAsPlugin();
}} else {{
    console.log('{} plugin v0.1.0');
    console.log('This is a metarepo plugin. Install it with:');
    console.log('  meta plugin install --local .');
}}

function runAsPlugin() {{
    const rl = readline.createInterface({{
        input: process.stdin,
        output: process.stdout,
        terminal: false
    }});

    rl.on('line', (line) => {{
        try {{
            const request = JSON.parse(line);
            const response = handleRequest(request);
            console.log(JSON.stringify(response));
        }} catch (error) {{
            console.log(JSON.stringify({{
                type: 'Error',
                message: error.message
            }}));
        }}
    }});
}}

function handleRequest(request) {{
    switch (request.type) {{
        case 'GetInfo':
            return {{
                type: 'Info',
                name: '{}',
                version: '0.1.0',
                experimental: false
            }};
        case 'RegisterCommands':
            return {{
                type: 'Commands',
                commands: [{{
                    name: '{}',
                    about: 'Example command',
                    subcommands: [],
                    args: []
                }}]
            }};
        case 'HandleCommand':
            return {{
                type: 'Success',
                message: 'Command executed successfully'
            }};
        default:
            return {{
                type: 'Error',
                message: `Unknown request type: ${{request.type}}`
            }};
    }}
}}
"#, name, name, name);
        
        fs::write(dir.join("index.js"), index_js)?;
        
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dir.join("index.js"))?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(dir.join("index.js"), perms)?;
        }
        
        Ok(())
    }
    
    fn create_binary_plugin(name: &str, dir: &Path) -> Result<()> {
        // Create a simple shell script template
        let script = format!(r#"#!/bin/bash

# {} plugin for metarepo
# This is a template for a binary plugin

if [[ "$METAREPO_PLUGIN_MODE" == "1" ]]; then
    # Running as metarepo plugin
    while IFS= read -r line; do
        request=$(echo "$line" | jq -r '.type')
        
        case "$request" in
            "GetInfo")
                echo '{{"type":"Info","name":"{}","version":"0.1.0","experimental":false}}'
                ;;
            "RegisterCommands")
                echo '{{"type":"Commands","commands":[{{"name":"{}","about":"Example command","subcommands":[],"args":[]}}]}}'
                ;;
            "HandleCommand")
                echo '{{"type":"Success","message":"Command executed successfully"}}'
                ;;
            *)
                echo '{{"type":"Error","message":"Unknown request type"}}'
                ;;
        esac
    done
else
    # Running standalone
    echo "{} plugin v0.1.0"
    echo "This is a metarepo plugin. Install it with:"
    echo "  meta plugin install --local ."
fi
"#, name, name, name, name);
        
        let script_path = dir.join(format!("{}-plugin", name));
        fs::write(&script_path, script)?;
        
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
        
        Ok(())
    }
    
    fn create_readme(name: &str, dir: &Path) -> Result<()> {
        let readme = format!(r#"# {} Plugin

A metarepo plugin created with the plugin scaffold.

## Installation

### Local Installation
```bash
meta plugin install --local .
```

### From Git
```bash
meta plugin install git+https://github.com/yourusername/{}.git
```

## Usage

```bash
meta {} example --input file.txt
```

## Development

Edit `plugin.toml` to define your plugin's commands and arguments.

### Testing
```bash
meta plugin test {}
```

### Building (Rust plugins)
```bash
cargo build --release
```

## License

MIT
"#, name, name, name, name);
        
        fs::write(dir.join("README.md"), readme)?;
        Ok(())
    }
}

/// Plugin template types
#[derive(Debug, Clone, Copy)]
pub enum PluginTemplate {
    Rust,
    Python,
    JavaScript,
    Binary,
}

impl PluginTemplate {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(PluginTemplate::Rust),
            "python" | "py" => Some(PluginTemplate::Python),
            "javascript" | "js" | "node" => Some(PluginTemplate::JavaScript),
            "binary" | "bin" | "shell" | "bash" => Some(PluginTemplate::Binary),
            _ => None,
        }
    }
}