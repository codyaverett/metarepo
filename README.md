# Meta - Rust Implementation

A Rust reimplementation of the Node.js [meta](https://github.com/mateodelnorte/meta) tool for managing multi-project systems and libraries.

## Current Status

✅ **Phase 1 Complete**: Core Infrastructure
- ✅ Cargo workspace structure setup
- ✅ Core `meta` binary crate created
- ✅ Basic CLI framework with `clap`
- ✅ Plugin trait and registry system
- ✅ `.meta` file parsing (JSON format)
- ✅ Initial tests passing

✅ **Phase 2 Complete**: Plugin System Integration
- ✅ Shared `meta-core` crate for plugin interfaces
- ✅ Circular dependency resolution
- ✅ Working `init` plugin with full functionality
- ✅ Plugin registration and command routing
- ✅ Real meta repository initialization

## Project Structure

```
metarep/
├── Cargo.toml              # Workspace configuration
├── docs/                   # Architecture and implementation docs
│   ├── IMPLEMENTATION_PLAN.md
│   └── ARCHITECTURE.md
├── meta-core/              # Shared plugin interfaces
│   └── src/lib.rs          # Plugin traits and data types
├── meta/                   # Core binary crate
│   ├── src/
│   │   ├── lib.rs          # Main library
│   │   ├── config.rs       # Configuration handling
│   │   ├── plugin.rs       # Plugin system
│   │   ├── cli.rs          # CLI framework
│   │   └── main.rs         # Binary entry point
│   └── Cargo.toml
├── plugins/                # Plugin crates
│   ├── init/              # ✅ Initialize new meta repositories
│   ├── git/               # Git operations (in progress)
│   ├── project/           # Project management (in progress)
│   ├── exec/              # Execute commands (in progress)
│   └── loop/              # Directory iteration (in progress)
└── README.md
```

## Features Implemented

### Core Meta Binary
- ✅ CLI framework with subcommands and help system
- ✅ Configuration file (`.meta`) parsing and validation
- ✅ Plugin discovery and registration system
- ✅ Error handling and user feedback
- ✅ Comprehensive test suite (14+ passing tests)

### Plugin System
- ✅ Shared `meta-core` crate with plugin interfaces
- ✅ Dynamic plugin loading and command registration
- ✅ Circular dependency resolution architecture
- ✅ Clean separation between core and plugin functionality

### Configuration System
- ✅ Compatible with Node.js meta `.meta` file format
- ✅ Default ignore patterns (`.git`, `node_modules`, `target`, etc.)
- ✅ Project repository mapping
- ✅ Configuration file operations (load/save)
- ✅ Auto-discovery of `.meta` files in parent directories

### Init Plugin (Fully Working)
- ✅ `meta init` command to initialize repositories
- ✅ Creates `.meta` file with proper JSON structure
- ✅ Updates `.gitignore` with meta-specific patterns
- ✅ Prevents double-initialization with error handling
- ✅ Compatible with existing Node.js meta configurations

## Usage

### Building
```bash
cargo build
```

### Running
```bash
# Show help
cargo run --bin meta -- --help

# Initialize a meta repository (fully functional)
cargo run --bin meta -- init

# Initialize with verbose output
cargo run --bin meta -- --verbose init

# Other commands (placeholders for future implementation)
cargo run --bin meta -- exec "command"
cargo run --bin meta -- git
```

### Testing
```bash
cargo test
```

## Next Steps

1. **Plugin System Integration**: Complete the plugin trait implementation to avoid circular dependencies
2. **Core Plugin Implementation**: Implement actual functionality for `init`, `git`, `project`, `exec`, and `loop` plugins
3. **Git Operations**: Real git cloning, status checking, and repository management
4. **Command Execution**: Parallel and sequential command execution across projects
5. **Advanced Features**: Template support, migration tools, and enhanced CLI experience

## Compatibility

- ✅ Compatible `.meta` file format with Node.js version
- ✅ Similar command-line interface structure
- ⏳ Full workflow compatibility (in progress)

## Development

This project follows Test-Driven Development (TDD) principles:
- All core functionality has corresponding tests
- Plugin system designed for extensibility
- Comprehensive error handling
- Cross-platform support

See `docs/IMPLEMENTATION_PLAN.md` for detailed development roadmap and `docs/ARCHITECTURE.md` for system design details.