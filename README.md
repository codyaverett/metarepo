# Metarepo - Multi-Project Management Tool

A Rust implementation inspired by the Node.js [meta](https://github.com/mateodelnorte/meta) tool for managing multi-project systems and libraries.

## Project Structure

```
metarepo/
├── Cargo.toml              # Workspace configuration
├── docs/                   # Architecture and implementation docs
│   ├── IMPLEMENTATION_PLAN.md
│   └── ARCHITECTURE.md
├── meta-core/              # Shared plugin interfaces
│   └── src/lib.rs          # Plugin traits and data types
├── meta/                   # Core binary crate with built-in plugins
│   ├── src/
│   │   ├── lib.rs          # Main library
│   │   ├── config.rs       # Configuration handling
│   │   ├── plugin.rs       # Plugin system
│   │   ├── cli.rs          # CLI framework
│   │   ├── main.rs         # Binary entry point
│   │   └── plugins/        # Built-in plugins
│   │       ├── init/       # Initialize new meta repositories
│   │       ├── git/        # Git operations across repositories
│   │       ├── project/    # Project management (create/import)
│   │       ├── exec/       # Execute commands across repositories
│   │       ├── rules/      # Project structure enforcement
│   │       ├── mcp/        # Model Context Protocol integration
│   │       └── plugin_manager/ # External plugin management
│   └── Cargo.toml
└── README.md
```

## Features

### Core
- CLI framework with subcommands and help system
- Configuration file (`.meta`) parsing and validation
- Plugin discovery and registration system
- Compatible with Node.js meta `.meta` file format

### Available Plugins

**Init Plugin**
- `meta init` - Initialize a new meta repository
- Creates `.meta` file with proper JSON structure
- Updates `.gitignore` with meta-specific patterns

**Git Plugin**
- `meta git clone <url>` - Clone meta repo and all child repositories
- `meta git status` - Show git status across all repositories
- `meta git update` - Clone missing repositories

**Project Plugin**
- `meta project create <path> <repo_url>` - Create and clone new project
- `meta project import <path> <repo_url>` - Import existing project

**Exec Plugin**
- `meta exec <command>` - Execute a command in all project directories
- `meta exec --projects <project1,project2> <command>` - Execute in specific projects
- `meta exec --include-only <patterns> <command>` - Only include matching projects
- `meta exec --exclude <patterns> <command>` - Exclude matching projects
- `meta exec --existing-only <command>` - Only iterate over existing projects
- `meta exec --git-only <command>` - Only iterate over git repositories
- `meta exec --parallel <command>` - Execute commands in parallel
- `meta exec --include-main <command>` - Include the main meta repository

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

# Execute a command in all projects
cargo run --bin meta -- exec npm install

# Execute in specific projects only
cargo run --bin meta -- exec --projects frontend,backend npm test

# Execute with filters
cargo run --bin meta -- exec --git-only git status
cargo run --bin meta -- exec --exclude node_modules,target ls -la

# Execute in parallel
cargo run --bin meta -- exec --parallel npm test

# Include main repository
cargo run --bin meta -- exec --include-main git status
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

## Compatibility

- Compatible `.meta` file format with Node.js version
- Similar command-line interface structure
- Core workflow compatibility verified

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - System design and structure
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md) - Development roadmap
- [Plugin Development](docs/PLUGIN_DEVELOPMENT.md) - Guide for creating plugins
- [Rules System](docs/RULES.md) - Defining project rules and metadata