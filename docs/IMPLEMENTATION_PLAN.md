# Rust Meta Tool Implementation Plan

## Overview
This project recreates the tooling and experience of the Node.js [meta](https://github.com/mateodelnorte/meta) project in Rust. Meta is a tool for managing multi-project systems and libraries, answering the conundrum of choosing between a mono repo or many repos by saying "both", with a meta repo.

## Project Architecture

### Monorepo Structure
The project will be organized as a Cargo workspace with the following structure:
```
metarep/
├── Cargo.toml              # Workspace configuration
├── docs/                   # Architecture and user documentation
├── meta/                   # Core binary crate
├── plugins/                # Built-in plugin crates
│   ├── init/              # Initialize new meta repositories
│   ├── git/               # Git operations across multiple repos
│   ├── project/           # Project management (create/import)
│   ├── exec/              # Execute commands across repos
│   └── loop/              # Directory iteration utilities
└── tests/                  # Integration tests
```

### Core Components

#### 1. Meta Configuration
- `.meta` file parsing (JSON format compatible with Node.js version)
- Project references with repository URLs
- Ignore patterns for directories
- Configuration validation and error handling

#### 2. Plugin System
- Plugin trait defining standard interface
- Dynamic plugin discovery and loading
- Plugin registration with CLI framework
- Support for both workspace plugins and external crates
- Plugin naming convention (without `meta-` prefix for internal plugins)

#### 3. CLI Framework
- Built on `clap` for robust command-line parsing
- Subcommand routing to appropriate plugins
- Help generation and documentation
- Error handling and user feedback

## Implementation Phases

### Phase 1: Core Infrastructure

#### 1.1 Project Structure Setup
- [x] Convert to Cargo workspace for monorepo architecture
- [ ] Create core `meta` binary crate
- [ ] Setup plugin system foundation with separate crates
- [ ] Configure workspace dependencies and build system

#### 1.2 Configuration & Data Models
- [ ] Implement `.meta` file parsing (JSON format)
- [ ] Create project configuration structs
- [ ] Add ignore patterns support
- [ ] Configuration validation and migration utilities

#### 1.3 CLI Framework Foundation
- [ ] Integrate `clap` for command-line parsing
- [ ] Design plugin registration system
- [ ] Implement subcommand routing
- [ ] Basic help and version commands

### Phase 2: Plugin System

#### 2.1 Plugin Architecture
- [ ] Create plugin trait/interface
- [ ] Implement plugin discovery mechanism
- [ ] Add plugin registration system
- [ ] Plugin lifecycle management

#### 2.2 Core Plugin Crates
The following plugins will be implemented as separate crates within the workspace:

- **`init`**: Initialize new meta repositories
  - Create `.meta` file
  - Setup gitignore patterns
  - Initialize git repository

- **`git`**: Git operations across multiple repos
  - Clone meta repo and all child repositories
  - Update missing repositories
  - Status checking across all projects
  - Bulk git operations (pull, push, status, etc.)

- **`project`**: Project management (create/import)
  - Create new projects within meta repo
  - Import existing repositories
  - Project migration utilities
  - Subtree splitting for monorepo migration

- **`exec`**: Execute commands across repos
  - Parallel command execution
  - Output aggregation and formatting
  - Directory filtering and targeting
  - Command result reporting

- **`loop`**: Directory iteration utilities
  - Project discovery and filtering
  - Directory traversal utilities
  - Include/exclude pattern matching
  - Shared utilities for other plugins

### Phase 3: Essential Features

#### 3.1 Repository Management
- [ ] Clone operations for meta repos and children
- [ ] Update missing repositories (`meta git update`)
- [ ] Status checking across projects
- [ ] Repository synchronization

#### 3.2 Command Execution
- [ ] Parallel execution support with `--parallel` flag
- [ ] Output aggregation and formatting
- [ ] Error handling and reporting
- [ ] Command targeting with include/exclude patterns

#### 3.3 Migration Tools
- [ ] Monorepo to meta-repo migration
- [ ] Git history preservation
- [ ] Subtree splitting utilities
- [ ] Migration validation

### Phase 4: Advanced Features

#### 4.1 Enhanced CLI Experience
- [ ] Shell completion support
- [ ] Interactive mode for complex operations
- [ ] Progress indicators for long-running operations
- [ ] Colored output and formatting

#### 4.2 Configuration Management
- [ ] Multiple meta file support
- [ ] Environment-specific configurations
- [ ] Configuration inheritance
- [ ] Validation and linting tools

### Phase 5: Documentation & Testing

#### 5.1 Documentation Structure
- [ ] Architecture documentation in `docs/`
- [ ] Plugin development guide
- [ ] User guides and tutorials
- [ ] Migration guide from Node.js version
- [ ] API documentation

#### 5.2 Test-Driven Development
- [ ] Unit tests for core functionality
- [ ] Integration tests for plugin system
- [ ] End-to-end workflow tests
- [ ] Performance benchmarks
- [ ] Continuous integration setup

## Technical Considerations

### Dependencies
- `clap`: Command-line argument parsing
- `serde`: Serialization/deserialization for `.meta` files
- `tokio`: Async runtime for parallel operations
- `git2`: Git operations
- `anyhow`: Error handling
- `tracing`: Logging and debugging

### Compatibility
- Full compatibility with existing `.meta` file format
- Command-line interface compatibility where possible
- Plugin API designed for future extensibility
- Cross-platform support (Windows, macOS, Linux)

### Performance Goals
- Faster startup than Node.js version
- Efficient parallel operations
- Minimal memory footprint
- Optimized for large repository sets

## Success Criteria

1. **Functional Compatibility**: Core workflows from Node.js version work identically
2. **Plugin Ecosystem**: Extensible plugin system for additional functionality
3. **Performance**: Measurably faster than Node.js equivalent
4. **Documentation**: Comprehensive guides for users and plugin developers
5. **Testing**: High test coverage with reliable CI/CD
6. **Migration Path**: Clear upgrade path from Node.js version

## Future Enhancements

- Web UI for repository management
- Integration with CI/CD systems
- Repository analytics and reporting
- Advanced dependency management
- Plugin marketplace and discovery