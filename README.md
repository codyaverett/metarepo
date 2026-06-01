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
│   └── src/
│       ├── lib.rs          # Plugin traits and data types
│       └── protocol.rs     # v1 external-plugin wire protocol (shared)
├── metarepo-plugin-sdk/    # SDK for authoring external plugins (Plugin trait + serve())
├── examples/
│   ├── metarepo-plugin-example/  # Reference protocol plugin built on the SDK (Rust)
│   ├── metarepo-plugin-shell/    # Reference manifest plugin (shell script + manifest)
│   ├── plugin-node/              # Node.js protocol-plugin template
│   ├── plugin-python/            # Python protocol-plugin template
│   └── plugin-go/                # Go protocol-plugin template
├── meta/                   # Core binary crate with built-in plugins
│   ├── src/
│   │   ├── lib.rs          # Main library
│   │   ├── config.rs       # Configuration handling
│   │   ├── plugin.rs       # Plugin system
│   │   ├── cli.rs          # CLI framework
│   │   ├── main.rs         # Binary entry point
│   │   └── plugins/        # Built-in plugins
│   │       ├── init/       # Initialize new meta repositories
│   │       ├── skill/      # Manage the bundled Claude Code skill
│   │       ├── git/        # Git operations across repositories
│   │       ├── project/    # Project management
│   │       ├── config/     # Configuration management
│   │       ├── exec/       # Execute commands across repositories
│   │       ├── run/        # Run project-specific scripts from .meta
│   │       ├── worktree/   # Git worktree management
│   │       ├── rules/      # Project structure enforcement (experimental)
│   │       ├── mcp/        # Model Context Protocol integration (experimental)
│   │       ├── plugin_manager/ # External plugin management (experimental)
│   │       └── shared/     # Shared utilities for plugins
│   └── Cargo.toml
└── README.md
```

## Installation

```bash
# Install from source
cargo install --path meta

# Verify installation
meta --version
```

> **Development:** Use `cargo run --bin meta --` to run from source without installing (e.g., `cargo run --bin meta -- git status`).

## Quick Start

```bash
# Initialize a new meta workspace
meta init

# Add projects
meta project add frontend https://github.com/user/frontend.git
meta project add backend https://github.com/user/backend.git

# Pick a config format on init (default: .metarepo / JSON)
meta init --format yaml      # writes .metarepo.yaml
meta init --format toml      # writes .metarepo.toml

# Use an explicit config file (overrides discovery)
meta --config ./tools/.metarepo.yaml git status

# Convert between formats
meta config migrate yaml             # writes .metarepo.yaml; keeps original
meta config migrate toml --replace   # writes .metarepo.toml; removes original

# Check status across all repositories
meta git status

# Clone missing repositories defined in .meta
meta git update

# Execute commands across all projects
meta exec --all npm install

# Run in parallel
meta exec --all --parallel npm test

# Run scripts defined in .meta
meta run build
meta run --list

# Manage worktrees for feature branches
meta worktree add feature/new-feature --all
meta worktree list
```

To clone an existing meta workspace:

```bash
meta git clone https://github.com/user/meta-repo.git
cd meta-repo
meta git update
```

## Built-in Plugins

| Plugin | Command Pattern | Description |
|--------|----------------|-------------|
| **init** | `meta init [--with-skill\|--with-completions\|--all]` | Initialize a meta repository; optionally install the Claude Code skill and shell completions |
| **git** | `meta git <clone\|status\|update>` | Git operations across repositories |
| **project** | `meta project <add\|list\|remove\|rename\|tree\|update\|convert-to-bare\|update-gitignore>` | Project management |
| **config** | `meta config <edit\|show\|get\|set\|validate>` | Configuration management with interactive TUI |
| **exec** | `meta exec [flags] <command>` | Execute commands across repositories |
| **run** | `meta run [flags] <script>` | Run scripts defined in `.meta` |
| **worktree** | `meta worktree <add\|remove\|list\|prune\|clean\|repair>` | Git worktree management across workspace (`clean` removes merged worktrees) |
| **rules** | `meta -x rules <check\|init\|list\|...>` | Project structure enforcement (experimental) |
| **plugin** | `meta -x plugin <add\|install\|remove\|list\|update>` | External plugin management (experimental) |
| **mcp** | `meta -x mcp <add\|list\|connect\|serve\|...>` | Model Context Protocol integration (experimental) |

See [CLI Reference](.claude/skills/meta-tool/SKILL.md) for full command documentation with all flags and examples.

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--version` | `-v` | Print version information |
| `--experimental` | `-x` | Enable experimental features (rules, plugin, mcp) |
| `--non-interactive` | | Non-interactive mode: `fail` or `defaults` (for CI) |
| `--config` | `-c` | Use a specific config file, overriding auto-discovery |
| `--workspace` | `-w` | Operate on every project, ignoring the current directory |
| `--root` | | Resolve the outermost enclosing metarepo instead of the nearest one |

## Directory-aware scope

Multi-project commands (`git status`/`pull`, `exec`, `run`, `project list`/`tree`,
and `worktree`) act on a set of projects determined by your current directory:

- **inside a project** → just that project
- **inside a subdirectory** that contains projects → the projects beneath it
- **at the workspace root** → every project

```bash
cd plugins/
meta git status        # only the projects under plugins/
meta -w git status     # every project (run from anywhere)
```

Use `--workspace`/`-w` to force the whole workspace from anywhere, or
`--project`/`--projects` (where supported) to target specific projects. For a
metarepo nested inside another, `--root` drives the **outermost** one (combine
with `--workspace` to span all of its projects). Commands that are inherently
whole-workspace or target a named project — `git clone`/`update`, `project
add`/`remove`/`rename` — are not directory-scoped.

## Advanced Configuration

### Worktree Post-Create Hooks

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
meta worktree add feature/new-feature

# Skip post-create hooks
meta worktree add feature/quick-test --no-hooks
```

### Bare Repository Mode (Default)

All projects use bare repositories by default for cleaner structure:

```bash
meta project add my-app git@github.com:user/my-app.git
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

## Testing

```bash
cargo test
```

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

### Quick Issue Creation

```bash
make issue-bug       # Bug report with prompts
make issue-feature   # Feature request with prompts
make issue-idea      # Quick idea capture
make list-issues     # View recent issues
```

All scripts support JSON input and silent mode for automation. See [.github/scripts/README.md](.github/scripts/README.md).

### Web Interface

Create issues at [github.com/caavere/metarepo/issues/new/choose](https://github.com/caavere/metarepo/issues/new/choose).

### Pre-commit Hooks

Install pre-commit hooks for automatic code quality checks:

```bash
make install-hooks
```

Hooks auto-format code, run clippy, check for merge conflicts, validate config files, and more. See [CONTRIBUTING.md](CONTRIBUTING.md#pre-commit-hooks) for full details on what the hooks do, troubleshooting, and bypassing.

### Pull Requests

1. Fork and clone the repository
2. Install pre-commit hooks: `make install-hooks`
3. Create a feature branch
4. Make your changes with clear commit messages (commitizen format)
5. Submit a pull request

### Security

For security vulnerabilities, please follow our [Security Policy](SECURITY.md). **Do not** create public issues for security concerns.

## Compatibility

- Compatible `.meta` file format with Node.js version
- Similar command-line interface structure
- Core workflow compatibility verified

## Documentation

- [CLI Reference](.claude/skills/meta-tool/SKILL.md) - Full command reference (source of truth)
- [Architecture](docs/ARCHITECTURE.md) - System design and structure
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md) - Development roadmap
- [Plugin Development](docs/PLUGIN_DEVELOPMENT.md) - Guide for creating external plugins (SDK quick start, install, security, testing)
- [Plugin Protocol v1](docs/PLUGIN_PROTOCOL_V1.md) - External-plugin wire protocol specification
- [Harness Integration](docs/HARNESS_INTEGRATION.md) - Making AI agent harnesses (Claude Code, opencode, MCP clients, a custom TUI) fluent in metarepo
- [Rules System](docs/RULES.md) - Defining project rules and metadata
- [Worktree Configuration](docs/WORKTREE.md) - Advanced worktree features and configuration
- [Shell Completions](docs/SHELL_COMPLETIONS.md) - Generating and installing tab-completion scripts
- [Testing Guidelines](docs/qa/) - QA and testing strategy
- [Security Testing](docs/security/) - Security testing strategy
- [Enhancement Ideas](docs/ENHANCEMENT_IDEAS.md) - Future improvement ideas
- [TODO & Future Ideas](docs/TODO.md) - Planned features and improvement ideas
