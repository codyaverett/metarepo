---
name: meta-cli
description: This skill should be used when the user asks about "meta commands", "metarepo", "meta exec", "meta worktree", "meta project", "meta git", "meta run", "managing multiple repositories", "multi-repo management", or discusses workspace operations across git repositories. Provides comprehensive guidance for using the meta CLI tool.
version: 0.11.0
---

# Meta CLI Reference

The `meta` CLI is a powerful multi-project management tool for managing multiple git repositories as a unified workspace.

## Quick Reference

### Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--version` | `-v` | Print version information |
| `--experimental` | `-x` | Enable experimental features (rules, plugin, mcp) |
| `--non-interactive` | | Non-interactive mode: 'fail' or 'defaults' |

### Common Project Selection Flags

These flags are available on most commands that operate on projects:

| Flag | Short | Description |
|------|-------|-------------|
| `--project` | `-p` | Single project to operate on |
| `--projects` | | Comma-separated list of projects |
| `--all` | `-a` | Operate on all projects |
| `--include-only` | | Only include projects matching patterns |
| `--exclude` | | Exclude projects matching patterns |
| `--existing-only` | | Only iterate over existing projects |
| `--git-only` | | Only iterate over git repositories |

### Execution Flags

| Flag | Description |
|------|-------------|
| `--parallel` | Execute commands in parallel |
| `--no-progress` | Disable progress indicators (CI environments) |
| `--streaming` | Show output as it happens (legacy behavior) |
| `--include-main` | Include the main meta repository |

---

## Command Reference

### `meta init`

Initialize a new meta repository in the current directory.

```bash
meta init
```

Creates a `.meta` configuration file and updates `.gitignore` patterns.

---

### `meta git` - Git Operations

#### `meta git clone <url>`

Clone a meta repository and all child repositories.

```bash
meta git clone https://github.com/user/meta-workspace.git
```

Aliases: `c`

#### `meta git status`

Show git status across all repositories.

```bash
meta git status
```

Aliases: `st`, `s`

#### `meta git update`

Clone missing repositories defined in `.meta`.

```bash
meta git update
```

Aliases: `up`, `u`

---

### `meta project` - Project Management

#### `meta project add [path] [source]`

Add a project to the workspace.

```bash
# Clone from URL
meta project add myproject https://github.com/user/repo.git

# Import existing local directory as symlink
meta project add myproject ../external-repo

# Interactive mode (no arguments)
meta project add
```

Aliases: `import`, `i`, `a`

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--recursive` | `-r` | Recursively import nested meta repositories |
| `--max-depth` | | Maximum depth for recursive imports (default: 3) |
| `--flatten` | | Import nested projects at root level |
| `--no-recursive` | | Disable recursive import |
| `--init-git` | | Auto-initialize git if not a repo |
| `--bare` | | Clone as bare repository with worktree structure |

#### `meta project list`

List all projects in the workspace (tree view by default).

```bash
meta project list           # Tree view
meta project list --flat    # Flat list with details
meta project list --minimal # Just project names
```

Aliases: `ls`, `l`

#### `meta project tree`

Display project hierarchy as a tree (same as `list` without flags).

```bash
meta project tree
```

#### `meta project update`

Update all projects (pull latest changes).

```bash
meta project update
meta project update --recursive
```

Aliases: `pull`

#### `meta project remove [name]`

Remove a project from the workspace.

```bash
meta project remove myproject
meta project remove myproject --force  # Force removal, delete directory
```

Aliases: `rm`, `r`

#### `meta project rename <old_name> <new_name>`

Rename a project in the workspace.

```bash
meta project rename old-name new-name
```

Aliases: `mv`, `move`

#### `meta project convert-to-bare <project>`

Convert a normal repository to bare repository with worktrees.

```bash
meta project convert-to-bare myproject
```

#### `meta project update-gitignore <name>`

Update .gitignore for a project that now has a remote.

```bash
meta project update-gitignore myproject
```

---

### `meta exec` - Execute Commands

Execute commands across multiple repositories.

```bash
# Run in current project context
meta exec pwd

# Run in all projects
meta exec --all git status

# Run in specific project
meta exec -p myproject npm install

# Run in parallel
meta exec --all --parallel npm test

# Filter projects
meta exec --all --include-only "frontend*" npm build
meta exec --all --exclude "legacy*" npm test
```

Aliases: `e`, `x`

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--project` | `-p` | Single project |
| `--projects` | | Comma-separated list |
| `--all` | `-a` | All projects |
| `--include-only` | | Include patterns (comma-separated) |
| `--exclude` | | Exclude patterns (comma-separated) |
| `--existing-only` | | Only existing directories |
| `--git-only` | | Only git repositories |
| `--parallel` | | Execute in parallel |
| `--include-main` | | Include main meta repository |
| `--no-progress` | | Disable progress indicators |
| `--streaming` | | Show output as it happens |

---

### `meta run` - Run Scripts

Run project-specific scripts defined in `.meta`.

```bash
# Run script in current project or all with the script
meta run test

# Run in specific project
meta run test --project foo

# Run in all projects
meta run build --all

# Run in parallel
meta run deploy --parallel

# List available scripts
meta run --list
meta run list
```

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--project` | `-p` | Single project |
| `--projects` | | Comma-separated list |
| `--all` | `-a` | All projects |
| `--parallel` | | Run in parallel |
| `--env` | `-e` | Set environment variable (KEY=VALUE) |
| `--list` | `-l` | List available scripts |
| `--existing-only` | | Only existing directories |
| `--git-only` | | Only git repositories |
| `--no-progress` | | Disable progress indicators |
| `--streaming` | | Show output as it happens |

---

### `meta config` - Configuration Management

Manage `.meta` configuration files.

#### `meta config edit`

Edit config with interactive TUI.

```bash
meta config edit
meta config edit --file path/to/.meta
```

Alias: `e`

#### `meta config show`

Display current configuration.

```bash
meta config show               # JSON (default)
meta config show --format yaml
meta config show --format toml
```

#### `meta config get <key>`

Get a specific config value.

```bash
meta config get default_bare
meta config get projects.myproject.url
```

#### `meta config set <key> <value>`

Set a specific config value.

```bash
meta config set default_bare true
```

#### `meta config validate`

Validate `.meta` file structure.

```bash
meta config validate
meta config validate --file path/to/.meta
```

---

### `meta worktree` - Worktree Management

Git worktree management across workspace projects.

#### `meta worktree add <branch> [commit]`

Create worktrees for selected projects.

```bash
# Smart branch detection
meta worktree add feature-123

# Create from specific branch
meta worktree add feature-123 --from origin/main

# Single project
meta worktree add feature-123 --project containers

# All projects
meta worktree add feature-123 --all

# Force create new branch
meta worktree add -b feature-123
```

Aliases: `create`, `new`

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--from` | `-f` | Starting point (e.g., origin/main, HEAD) |
| `--project` | `-p` | Single project |
| `--projects` | | Comma-separated list |
| `--all` | `-a` | All projects |
| `--create-branch` | `-b` | Create a new branch |
| `--path` | | Custom path suffix for worktree directory |
| `--no-hooks` | | Skip worktree_init command |

#### `meta worktree remove <branch>`

Remove worktrees from selected projects.

```bash
meta worktree remove feature-123
meta worktree remove feature-123 --all --force
```

Aliases: `rm`, `delete`

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--project` | `-p` | Single project |
| `--projects` | | Comma-separated list |
| `--all` | `-a` | All projects |
| `--force` | `-f` | Force removal with uncommitted changes |

#### `meta worktree list`

List all worktrees across the workspace.

```bash
meta worktree list
meta worktree list --verbose
```

Aliases: `ls`, `l`

#### `meta worktree prune`

Remove stale worktrees that no longer exist.

```bash
meta worktree prune
meta worktree prune --dry-run
```

---

### Experimental Commands

These commands require the `--experimental` or `-x` flag.

### `meta rules` (Experimental)

Enforce project file structure rules.

#### `meta -x rules check`

Check project structure against configured rules.

```bash
meta -x rules check
meta -x rules check --project myproject
meta -x rules check --fix  # Auto-fix violations
```

Aliases: `c`, `chk`

#### `meta -x rules init`

Initialize rules configuration file.

```bash
meta -x rules init
meta -x rules init --output .rules.yaml
meta -x rules init --project myproject
```

#### `meta -x rules list`

List all configured rules.

```bash
meta -x rules list
meta -x rules list --project myproject
```

Aliases: `ls`, `l`

#### `meta -x rules docs`

Show documentation for creating and using rules.

```bash
meta -x rules docs
meta -x rules docs directory
meta -x rules docs component
```

#### `meta -x rules create`

Create a new rule.

```bash
meta -x rules create directory src --required
meta -x rules create component "src/components/*" --structure "index.ts,styles.css"
meta -x rules create file "*.ts" --requires "test:*.test.ts"
```

#### `meta -x rules status`

Show rules status for all projects.

```bash
meta -x rules status
```

#### `meta -x rules copy <project>`

Copy workspace rules to a specific project.

```bash
meta -x rules copy myproject
```

---

### `meta plugin` (Experimental)

Manage metarepo plugins.

#### `meta -x plugin add [path]`

Add a plugin from a local path.

```bash
meta -x plugin add /path/to/plugin
```

#### `meta -x plugin install <name>`

Install a plugin from crates.io.

```bash
meta -x plugin install example
```

#### `meta -x plugin remove <name>`

Remove an installed plugin.

```bash
meta -x plugin remove example
```

#### `meta -x plugin list`

List all installed plugins.

```bash
meta -x plugin list
```

#### `meta -x plugin update`

Update all plugins to their latest versions.

```bash
meta -x plugin update
```

---

### `meta mcp` (Experimental)

Manage MCP (Model Context Protocol) servers.

#### `meta -x mcp add <name> <command> [args]`

Add a saved MCP server configuration.

```bash
meta -x mcp add playwright npx -- --yes @modelcontextprotocol/server-playwright
meta -x mcp add myserver /path/to/server --workdir /project --env "KEY=value"
```

#### `meta -x mcp list`

List saved MCP server configurations.

```bash
meta -x mcp list
```

#### `meta -x mcp remove <name>`

Remove a saved MCP server configuration.

```bash
meta -x mcp remove playwright
```

#### `meta -x mcp connect <name>`

Connect to an MCP server and show its info.

```bash
meta -x mcp connect playwright
```

#### `meta -x mcp list-tools <name>`

List tools from an MCP server.

```bash
meta -x mcp list-tools playwright
```

#### `meta -x mcp list-resources <name>`

List resources from an MCP server.

```bash
meta -x mcp list-resources playwright
```

#### `meta -x mcp call-tool <name> <tool> [--args JSON]`

Call a tool on an MCP server.

```bash
meta -x mcp call-tool playwright browser_navigate --args '{"url": "https://example.com"}'
```

#### `meta -x mcp serve`

Run Metarepo as an MCP server exposing CLI tools.

```bash
meta -x mcp serve
```

#### `meta -x mcp config`

Print MCP configuration for VS Code or Claude Desktop.

```bash
meta -x mcp config
```

---

## Common Workflows

### Initial Setup

```bash
# Clone an existing meta workspace
meta git clone https://github.com/org/workspace.git
cd workspace
meta git update  # Clone all child repos

# Or initialize a new workspace
mkdir my-workspace && cd my-workspace
git init
meta init
meta project add frontend https://github.com/org/frontend.git
meta project add backend https://github.com/org/backend.git
```

### Feature Branch Workflow

```bash
# Create worktrees for a feature across projects
meta worktree add feature-auth --from origin/main --all

# Work in the worktree
cd ../my-workspace-feature-auth
meta exec --all git status

# Remove worktrees when done
meta worktree remove feature-auth --all
```

### Running Tests Across Projects

```bash
# Run tests in all projects
meta exec --all npm test

# Run tests in parallel
meta exec --all --parallel npm test

# Run only in projects matching pattern
meta exec --all --include-only "service*" npm test
```

### CI/CD Integration

```bash
# Non-interactive mode for CI
meta --non-interactive defaults git update

# Disable progress bars
meta exec --all --no-progress npm ci
meta exec --all --no-progress --parallel npm run build
```

---

## Configuration Reference

### `.meta` File Format

```json
{
  "projects": {
    "frontend": {
      "url": "git@github.com:org/frontend.git"
    },
    "backend": {
      "url": "git@github.com:org/backend.git",
      "branch": "develop"
    },
    "shared": {}
  },
  "scripts": {
    "test": "npm test",
    "build": "npm run build"
  },
  "default_bare": true,
  "nested": {
    "recursive_import": true,
    "max_depth": 3
  },
  "plugins": {
    "custom": "^1.0.0"
  }
}
```

---

## Skill Maintenance

### How to Check for CLI Updates

When this skill may need updates:

1. Check current version: `meta --version`
2. Compare with skill version in frontmatter (currently 0.11.0)
3. If version differs, review source files for changes

### Source Files to Check

- `meta/Cargo.toml` - Version number
- `meta/src/cli.rs` - Global CLI flags
- `meta/src/plugins/*/plugin.rs` - Individual command implementations

### Known Version Changes

See `references/CHANGELOG_NOTES.md` for version history and breaking changes.

### Experimental Features Status

| Feature | Status | Notes |
|---------|--------|-------|
| `rules` | Experimental | Project structure enforcement |
| `plugin` | Experimental | External plugin management |
| `mcp` | Experimental | MCP server integration |

These require `-x` or `--experimental` flag to access.
