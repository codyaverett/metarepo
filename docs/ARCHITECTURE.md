# Meta Tool Architecture

## Overview

The Rust meta tool is designed as a modular, extensible system for managing multi-repository projects. It follows a plugin-based architecture that allows core functionality to be distributed across specialized crates while maintaining a unified command-line interface.

## Core Architecture Principles

### 1. Plugin-Centric Design
All functionality beyond basic CLI parsing is implemented as plugins. This includes:
- Core operations (git, project management)
- Utility functions (directory iteration, command execution)
- Extension points for custom workflows

### 2. Workspace-Based Monorepo
The project itself uses Cargo workspaces to demonstrate the meta-repository concept:
- Core binary provides the CLI framework
- Each plugin is a separate crate
- Shared utilities and types in common crates
- Integration tests at the workspace level

### 3. Compatibility with Node.js Version
- Identical `.meta` file format (JSON)
- Compatible command-line interface
- Similar plugin discovery and loading mechanisms
- Equivalent workflow semantics

## System Components

### Core Binary (`meta/`)

The main executable responsible for:
- CLI argument parsing and routing
- Plugin discovery and registration
- Configuration file loading
- Error handling and user feedback

```rust
// Simplified core structure
pub struct MetaCli {
    config: MetaConfig,
    plugins: PluginRegistry,
}

impl MetaCli {
    pub fn run(&self, args: Vec<String>) -> Result<()> {
        let command = self.parse_args(args)?;
        let plugin = self.plugins.find_handler(&command)?;
        plugin.execute(command, &self.config)
    }
}
```

### Plugin System

#### Plugin Trait
```rust
pub trait MetaPlugin {
    fn name(&self) -> &str;
    fn register_commands(&self, app: &mut clap::App) -> clap::App;
    fn handle_command(&self, matches: &clap::ArgMatches, config: &MetaConfig) -> Result<()>;
}
```

#### Plugin Discovery
1. **Workspace Plugins**: Built-in plugins in the `plugins/` directory
2. **External Plugins**: Crates with `meta-plugin` in `Cargo.toml` metadata
3. **Global Plugins**: Installed via `cargo install` with discovery via registry

#### Plugin Registration
```rust
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn MetaPlugin>>,
}

impl PluginRegistry {
    pub fn discover_and_register(&mut self) -> Result<()> {
        // 1. Register workspace plugins
        self.register_workspace_plugins()?;
        
        // 2. Discover external plugins
        self.discover_external_plugins()?;
        
        // 3. Load configuration-specified plugins
        self.load_config_plugins()?;
        
        Ok(())
    }
}
```

### Configuration System

#### Meta Configuration (`.meta` file)
```rust
#[derive(Serialize, Deserialize)]
pub struct MetaConfig {
    pub ignore: Vec<String>,
    pub projects: HashMap<String, String>, // path -> repo_url
    pub plugins: Option<HashMap<String, String>>, // name -> version/path
}
```

#### Runtime Configuration
```rust
pub struct RuntimeConfig {
    pub meta_config: MetaConfig,
    pub working_dir: PathBuf,
    pub global_options: GlobalOptions,
}
```

## Plugin Implementations

### Init Plugin (`plugins/init/`)
Initializes new meta repositories.

**Responsibilities:**
- Create `.meta` file with default configuration
- Initialize git repository if needed
- Setup initial gitignore patterns
- Validate repository structure

**Commands:**
- `meta init` - Initialize current directory as meta repo

### Git Plugin (`plugins/git/`)
Handles git operations across multiple repositories.

**Responsibilities:**
- Clone meta repo and all child repositories
- Update missing repositories
- Aggregate git status across projects
- Coordinate git operations (pull, push, etc.)

**Commands:**
- `meta git clone <url>` - Clone meta repo and all children
- `meta git update` - Clone missing repositories
- `meta git status` - Show status across all repos
- `meta git pull/push/fetch` - Execute git commands on all repos

### Project Plugin (`plugins/project/`)
Manages individual projects within the meta repository.

**Responsibilities:**
- Create new projects
- Import existing repositories
- Remove projects from meta repo
- Migrate monorepo directories to separate repositories

**Commands:**
- `meta project create <path> <repo_url>` - Create new project
- `meta project import <path> <repo_url>` - Import existing repo
- `meta project remove <path>` - Remove project from meta repo
- `meta project migrate <path> <repo_url>` - Migrate directory to separate repo

### Exec Plugin (`plugins/exec/`)
Executes commands across multiple repositories.

**Responsibilities:**
- Run arbitrary commands in each project directory
- Aggregate and format output
- Handle parallel execution
- Provide filtering and targeting options

**Commands:**
- `meta exec <command>` - Execute command in all projects
- `meta exec <command> --parallel` - Execute in parallel
- `meta exec <command> --include-only <projects>` - Target specific projects

### Loop Plugin (`plugins/loop/`)
Provides directory iteration utilities used by other plugins.

**Responsibilities:**
- Project discovery and enumeration
- Directory filtering based on patterns
- Shared iteration logic
- Path resolution utilities

**API:**
```rust
pub struct ProjectIterator<'a> {
    config: &'a MetaConfig,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl<'a> Iterator for ProjectIterator<'a> {
    type Item = ProjectInfo;
    
    fn next(&mut self) -> Option<Self::Item> {
        // Implementation
    }
}
```

## Data Flow

### Command Execution Flow
```
1. User Input → CLI Parser
2. CLI Parser → Plugin Registry
3. Plugin Registry → Specific Plugin
4. Plugin → Configuration + Arguments
5. Plugin → Execute Logic
6. Plugin → Format Output
7. Output → User
```

### Plugin Loading Flow
```
1. Startup → Discover Workspace Plugins
2. Workspace → Load Plugin Metadata
3. Metadata → Register Commands
4. Configuration → Load External Plugins
5. External → Validate and Register
6. Registry → Ready for Command Routing
```

## Error Handling Strategy

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    #[error("Git operation failed: {0}")]
    Git(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Error Propagation
- Plugins return `Result<(), MetaError>`
- Core CLI handles error formatting and user feedback
- Detailed errors for debugging, concise errors for users
- Exit codes follow Unix conventions

## Performance Considerations

### Parallel Operations
- Use `tokio` for async operations where beneficial
- Parallel git operations with controlled concurrency
- Efficient plugin loading and initialization
- Lazy loading of heavy dependencies

### Memory Management
- Stream processing for large outputs
- Minimal configuration caching
- Plugin isolation to prevent memory leaks
- Efficient string handling for path operations

## Security Considerations

### Plugin Security
- Workspace plugins are trusted (compiled with main binary)
- External plugins require explicit installation
- No dynamic code execution
- Configuration validation prevents path traversal

### Git Operations
- Use `git2` library for safe git operations
- Validate repository URLs before cloning
- Sandbox operations to working directory tree
- Respect git security configurations

## Testing Strategy

### Unit Testing
- Each plugin has comprehensive unit tests
- Mock filesystem and git operations for testing
- Test error conditions and edge cases
- Property-based testing for complex logic

### Integration Testing
- End-to-end workflow testing
- Plugin interaction testing
- Real repository operations in sandboxed environments
- Performance regression testing

### Compatibility Testing
- Test against existing `.meta` files
- Verify Node.js workflow compatibility
- Cross-platform testing (Windows, macOS, Linux)
- Version compatibility matrix

## Future Architecture Extensions

### Plugin Marketplace
- Plugin discovery service
- Version management for external plugins
- Security scanning and verification
- Dependency resolution for plugin ecosystems

### Advanced Configuration
- Multi-environment configurations
- Template-based project creation
- Conditional plugin loading
- Configuration inheritance and composition

### Performance Optimizations
- Incremental operations based on change detection
- Caching layer for expensive operations
- Background operations for long-running tasks
- Optimized data structures for large repository sets