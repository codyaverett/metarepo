# Metarepo Example Plugin

This is an example external plugin for the metarepo CLI tool, demonstrating how to create plugins that can be loaded dynamically.

## Features

This example plugin provides three simple commands:
- `meta example hello <name>` - Greets the user with a personalized message
- `meta example info` - Displays information about the current meta repository
- `meta example count` - Counts the number of projects in the repository

## Building

Build the plugin in release mode:

```bash
cargo build --release
```

This will create:
- A dynamic library (`target/release/libmetarepo_plugin_example.so` on Linux/Mac, `.dll` on Windows)
- An executable binary (`target/release/metarepo-plugin-example`)

## Installation

### Method 1: Subprocess Mode (Recommended)

The plugin can run as a subprocess, communicating with metarepo via JSON-RPC:

```bash
# Add the plugin executable
meta plugin add ./target/release/metarepo-plugin-example

# Or install from the examples directory
meta plugin add ./examples/metarepo-plugin-example/target/release/metarepo-plugin-example
```

### Method 2: Dynamic Library (Future)

Once dynamic loading is implemented in metarepo:

```bash
meta plugin add ./target/release/libmetarepo_plugin_example.so
```

### Method 3: From crates.io (Future)

Once published:

```bash
meta plugin install metarepo-plugin-example
```

## Usage

After installation, the plugin commands are available:

```bash
# Greet someone
meta example hello World

# Show repository information
meta example info

# Count projects
meta example count
```

## Development

### Running Tests

```bash
cargo test
```

### Debugging

Run the plugin standalone to see usage information:

```bash
cargo run
```

Run in subprocess mode for testing:

```bash
METAREPO_PLUGIN_MODE=1 cargo run
```

Then send JSON requests via stdin:

```json
{"type":"GetInfo"}
{"type":"RegisterCommands"}
{"type":"HandleCommand","command":"example","args":["hello","World"],"config":{"meta_config":{"ignore":[],"projects":{}},"working_dir":"/tmp","meta_file_path":null,"experimental":false}}
```

## Plugin Protocol

The plugin communicates with metarepo using JSON-RPC over stdio when in subprocess mode.

### Request Types

1. **GetInfo**: Returns plugin metadata
2. **RegisterCommands**: Returns command structure
3. **HandleCommand**: Executes a command with given arguments

### Response Types

1. **Info**: Plugin information response
2. **Commands**: Available commands structure
3. **Success**: Command executed successfully
4. **Error**: Command execution failed

## Creating Your Own Plugin

1. Copy this example as a template
2. Update the `Cargo.toml` with your plugin details
3. Modify `src/lib.rs` to implement your plugin logic
4. Keep `src/main.rs` for subprocess communication
5. Build and test locally
6. Publish to crates.io when ready

## License

MIT - See the main metarepo project for details