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
│   │       ├── run/        # Run project-specific scripts from .meta
│   │       ├── rules/      # Project structure enforcement
│   │       ├── worktree/   # Git worktree management
│   │       ├── mcp/        # Model Context Protocol integration
│   │       ├── plugin_manager/ # External plugin management
│   │       └── shared/     # Shared utilities for plugins
│   └── Cargo.toml
└── README.md
```

## Features

### Core
- CLI framework with subcommands and help system
- Configuration file (`.meta`) parsing and validation
- Plugin discovery and registration system
- Compatible with Node.js meta `.meta` file format

### Built-in Plugins

**Init Plugin** - Initialize a new meta repository
- `meta init` - Initialize a new meta repository
- Creates `.meta` file with proper JSON structure
- Updates `.gitignore` with meta-specific patterns

**Git Plugin** - Git operations across multiple repositories
- `meta git clone <url>` - Clone meta repo and all child repositories
- `meta git status` - Show git status across all repositories
- `meta git update` - Clone missing repositories

**Project Plugin** - Project management operations
- `meta project create <path> <repo_url>` - Create and clone new project
- `meta project import <path> <repo_url>` - Import existing project

**Exec Plugin** - Execute commands across multiple repositories
- `meta exec <command>` - Execute a command in all project directories
- `meta exec --projects <project1,project2> <command>` - Execute in specific projects
- `meta exec --include-only <patterns> <command>` - Only include matching projects
- `meta exec --exclude <patterns> <command>` - Exclude matching projects
- `meta exec --existing-only <command>` - Only iterate over existing projects
- `meta exec --git-only <command>` - Only iterate over git repositories
- `meta exec --parallel <command>` - Execute commands in parallel
- `meta exec --include-main <command>` - Include the main meta repository
- `meta exec --no-progress` - Disable progress indicators (useful for CI)
- `meta exec --streaming` - Show output as it happens instead of buffered

**Run Plugin** - Run project-specific scripts defined in .meta
- `meta run <script>` - Run a named script from .meta configuration
- `meta run --list` - List all available scripts
- `meta run --project <project> <script>` - Run script in a specific project
- `meta run --projects <project1,project2> <script>` - Run in multiple projects
- `meta run --all <script>` - Run script in all projects
- `meta run --parallel <script>` - Execute scripts in parallel
- `meta run --env KEY=VALUE <script>` - Set environment variables
- `meta run --existing-only <script>` - Only run in existing directories
- `meta run --git-only <script>` - Only run in git repositories
- `meta run --no-progress` - Disable progress indicators
- `meta run --streaming` - Show output as it happens

**Rules Plugin** - Enforce project file structure rules
- `meta rules check` - Check project structure against configured rules
- `meta rules init` - Initialize rules configuration file (.metarules.json)
- `meta rules list` - List all configured rules
- `meta rules docs` - Show documentation for creating and using rules
- `meta rules create` - Create a new rule interactively
- `meta rules status` - Show rules status for all projects
- `meta rules copy <project>` - Copy workspace rules to a specific project

**Worktree Plugin** - Git worktree management across workspace projects
- `meta worktree add <branch>` - Create worktrees for selected projects
- `meta worktree add <branch> --no-hooks` - Create worktrees without running post-create commands
- `meta worktree remove <worktree>` - Remove worktrees from selected projects
- `meta worktree list` - List all worktrees across the workspace
- `meta worktree prune` - Remove stale worktrees that no longer exist
- Supports post-create hooks via `worktree_init` configuration
- Supports bare repository mode for cleaner project structure

**Plugin Manager** - Manage metarepo plugins
- `meta plugin add <path>` - Add a plugin from a local path
- `meta plugin install <name>` - Install a plugin from crates.io
- `meta plugin remove <name>` - Remove an installed plugin
- `meta plugin list` - List all installed plugins
- `meta plugin update` - Update all plugins to their latest versions

**MCP Plugin** - Model Context Protocol server management (Experimental)
- Manage MCP (Model Context Protocol) servers for AI integration
- Configuration and server lifecycle management

## Usage

### Building

#### Linux Prerequisites
Before building on Linux, ensure you have the following dependencies installed:

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y libssl-dev pkg-config

# Fedora/RHEL/CentOS
sudo dnf install openssl-devel pkg-config

# Arch Linux
sudo pacman -S openssl pkg-config
```

#### Build Command
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

# Run scripts defined in .meta
cargo run --bin meta -- run build
cargo run --bin meta -- run --list
cargo run --bin meta -- run --parallel test

# Check project structure against rules
cargo run --bin meta -- rules check
cargo run --bin meta -- rules init
cargo run --bin meta -- rules status

# Manage git worktrees
cargo run --bin meta -- worktree add feature/new-feature
cargo run --bin meta -- worktree list
cargo run --bin meta -- worktree remove feature/old-feature

# Manage plugins
cargo run --bin meta -- plugin list
cargo run --bin meta -- plugin install meta-plugin-example
cargo run --bin meta -- plugin update
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

### Advanced Configuration

#### Worktree Post-Create Hooks

Automatically run commands when creating worktrees (e.g., install dependencies):

```json
{
  "worktree_init": "npm ci",
  "projects": {
    "frontend": {
      "url": "git@github.com:user/frontend.git",
      "worktree_init": "pnpm install && pnpm run setup"
    },
    "backend": {
      "url": "git@github.com:user/backend.git",
      "worktree_init": "cargo build"
    }
  }
}
```

```bash
# Creates worktree and automatically runs worktree_init command
cargo run --bin meta -- worktree add feature/new-feature

# Skip post-create hooks
cargo run --bin meta -- worktree add feature/quick-test --no-hooks
```

#### Bare Repository Mode (Default)

**New in v0.8.2:** All projects now use bare repositories by default for cleaner structure!

```bash
# Simple add - uses bare repository automatically
cargo run --bin meta -- project add my-app git@github.com:user/my-app.git
```

This creates:
```
workspace/
├── my-app/
│   ├── .git/           # Bare repository
│   ├── main/           # Default branch worktree
│   └── feature-1/      # Additional worktrees
```

**To use traditional clones**, set `"default_bare": false` in `.meta` or `"bare": false` per-project.

See [Worktree Configuration](docs/WORKTREE.md) for detailed documentation.

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
- [Worktree Configuration](docs/WORKTREE.md) - Advanced worktree features and configuration
- [TODO & Future Ideas](docs/TODO.md) - Planned features and improvement ideas