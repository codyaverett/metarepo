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

✅ **Phase 3 Complete**: Core Plugins Implementation
- ✅ Working `git` plugin with clone, status, and update commands
- ✅ Working `project` plugin with create and import commands
- ✅ Full repository management workflow
- ✅ Real-world testing with GitHub repositories
- ✅ Comprehensive error handling and validation

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
│   ├── git/               # ✅ Git operations across repositories
│   ├── project/           # ✅ Project management (create/import)
│   ├── exec/              # Execute commands (planned)
│   └── loop/              # Directory iteration (planned)
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

### Git Plugin (Fully Working)
- ✅ `meta git clone <url>` - Clone meta repo and all child repositories
- ✅ `meta git status` - Show git status across all repositories
- ✅ `meta git update` - Clone missing repositories
- ✅ Handles missing repositories gracefully
- ✅ Real git operations using `git2` crate

### Project Plugin (Fully Working)
- ✅ `meta project create <path> <repo_url>` - Create and clone new project
- ✅ `meta project import <path> <repo_url>` - Import existing project
- ✅ Automatically updates `.meta` file and `.gitignore`
- ✅ Validates project doesn't already exist
- ✅ Handles both new and existing directories

## Usage

### Building
```bash
cargo build
```

### Running
```bash
# Show help
cargo run --bin meta -- --help

# Initialize a meta repository
cargo run --bin meta -- init

# Create a new project (clones and adds to .meta)
cargo run --bin meta -- project create my-project https://github.com/user/repo.git

# Import an existing project
cargo run --bin meta -- project import existing-dir https://github.com/user/existing.git

# Show git status across all repositories
cargo run --bin meta -- git status

# Clone missing repositories
cargo run --bin meta -- git update

# Clone a meta repository and all its children
cargo run --bin meta -- git clone https://github.com/user/meta-repo.git

# Use verbose output
cargo run --bin meta -- --verbose git status
```

### Example Workflow
```bash
# 1. Initialize a new meta repository
cargo run --bin meta -- init

# 2. Add some projects
cargo run --bin meta -- project create frontend https://github.com/user/frontend.git
cargo run --bin meta -- project create backend https://github.com/user/backend.git

# 3. Check status of all repositories
cargo run --bin meta -- git status

# 4. If someone else adds projects, update to get missing ones
cargo run --bin meta -- git update
```

### Testing
```bash
cargo test
```

## Verified Real-World Testing

The implementation has been thoroughly tested with actual GitHub repositories:

- ✅ **Tested Repositories**: 
  - `https://github.com/codyaverett/container-codes-2.git`
  - `https://github.com/codyaverett/container-codes.git`
  - `https://github.com/octocat/Hello-World.git`

- ✅ **Verified Workflows**:
  - Repository initialization and configuration
  - Project creation with automatic cloning
  - Project importing with validation
  - Git status checking across multiple repositories
  - Missing repository detection and cloning
  - Automatic `.meta` and `.gitignore` management

## Next Steps

1. **Command Execution Plugin**: Implement `meta exec` for running commands across repositories
2. **Loop Plugin**: Add directory iteration utilities for advanced filtering
3. **Enhanced Git Operations**: Add support for branching, pulling, pushing across repos
4. **Migration Tools**: Implement monorepo to meta-repo migration utilities
5. **Advanced Features**: Template support, parallel operations, and enhanced CLI experience

## Compatibility

- ✅ Compatible `.meta` file format with Node.js version
- ✅ Similar command-line interface structure
- ✅ Core workflow compatibility verified
- ✅ Real-world repository management works identically

## Development

This project follows Test-Driven Development (TDD) principles:
- All core functionality has corresponding tests
- Plugin system designed for extensibility
- Comprehensive error handling
- Cross-platform support

See `docs/IMPLEMENTATION_PLAN.md` for detailed development roadmap and `docs/ARCHITECTURE.md` for system design details.