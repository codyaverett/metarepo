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
- `meta project add <path> [source]` - Add a project to the workspace (aliases: `import`, `i`, `a`)
- `meta project list` - List all projects (aliases: `ls`, `l`)
- `meta project tree` - Display project hierarchy as a tree
- `meta project update` - Update all projects (pull latest changes)
- `meta project remove <name>` - Remove a project from the workspace
- `meta project rename <old_name> <new_name>` - Rename a project
- `meta project tag add <project> <tags>` - Add tags to a project (use `--all` to apply to all projects)
- `meta project tag remove <project> <tags>` - Remove tags from a project (use `--all` to apply to all projects)
- `meta project tag list <project>` - List tags for a project (use `--all` to list tags for all projects)

**Exec Plugin** - Execute commands across multiple repositories
- `meta exec <command>` - Execute a command in all project directories
- `meta exec --projects <project1,project2> <command>` - Execute in specific projects
- `meta exec --include-only <patterns> <command>` - Only include matching projects
- `meta exec --exclude <patterns> <command>` - Exclude matching projects
- `meta exec --include-tags <tags> <command>` - Only include projects with these tags
- `meta exec --exclude-tags <tags> <command>` - Exclude projects with these tags
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
- `meta run --include-tags <tags> <script>` - Only run in projects with these tags
- `meta run --exclude-tags <tags> <script>` - Exclude projects with these tags
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

# Execute with tag filters
cargo run --bin meta -- exec --include-tags frontend,production -- git status
cargo run --bin meta -- exec --exclude-tags test -- cargo build

# Execute in parallel
cargo run --bin meta -- exec --parallel npm test

# Include main repository
cargo run --bin meta -- exec --include-main git status

# Run scripts defined in .meta
cargo run --bin meta -- run build
cargo run --bin meta -- run --list
cargo run --bin meta -- run --parallel test

# Run scripts with tag filters
cargo run --bin meta -- run test --include-tags frontend
cargo run --bin meta -- run deploy --include-tags production --parallel

# Tag management examples
cargo run --bin meta -- project tag add frontend-app frontend,production
cargo run --bin meta -- project tag add --all common,shared
cargo run --bin meta -- project tag list --all
cargo run --bin meta -- project tag remove --all deprecated

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
cargo run --bin meta -- project add frontend https://github.com/user/frontend.git
cargo run --bin meta -- project add backend https://github.com/user/backend.git

# 3. Tag projects for organization
cargo run --bin meta -- project tag add frontend frontend,production,react
cargo run --bin meta -- project tag add backend backend,production,rust

# 4. Check status of all repositories
cargo run --bin meta -- git status

# 5. Execute commands on tagged subsets
cargo run --bin meta -- exec --include-tags frontend -- npm install
cargo run --bin meta -- exec --include-tags production -- cargo build

# 6. If someone else adds projects, update to get missing ones
cargo run --bin meta -- git update
```

### Project Tags

**New in v0.11.0:** Tag functionality allows you to categorize and filter projects for more efficient workspace management.

#### Managing Tags

```bash
# Add tags to categorize projects
meta project tag add frontend-app frontend,production,react

# Add tags to all projects at once
meta project tag add --all common,shared

# List tags for a project
meta project tag list frontend-app

# List tags for all projects
meta project tag list --all

# Remove tags from a project
meta project tag remove frontend-app react

# Remove tags from all projects
meta project tag remove --all deprecated
```

Tags can be specified as comma-separated (`frontend,production`) or space-separated (`frontend production`). Use `--all` as the project name to operate on all projects in the workspace.

#### Using Tags for Filtering

Tags integrate seamlessly with existing filtering mechanisms:

```bash
# Execute commands only in tagged projects
meta exec --include-tags frontend,production -- git status
meta exec --exclude-tags test -- cargo build

# Run scripts only in tagged projects
meta run test --include-tags frontend
meta run deploy --include-tags production --parallel

# Combine tags with other filters
meta exec --include-tags frontend --git-only --parallel -- npm install
```

**Tag Filtering Logic:**
- `--include-tags`: Projects must have **at least one** matching tag (OR logic)
- `--exclude-tags`: Projects must **not have any** of the excluded tags (AND logic)
- Tags work together with pattern matching, `--git-only`, and `--existing-only` filters

**Example Workflow:**
```bash
# Tag projects by technology stack
meta project tag add react-app frontend,react,typescript
meta project tag add vue-app frontend,vue,javascript
meta project tag add api-server backend,rust,production

# Tag projects by environment
meta project tag add staging-api backend,staging
meta project tag add prod-api backend,production

# Execute commands on specific subsets
meta exec --include-tags frontend -- npm run lint
meta exec --include-tags production,backend -- cargo test
meta exec --include-tags staging --exclude-tags production -- ./deploy.sh
```

Tags are stored in the `.meta` file and are backward-compatible. Projects using the simple URL format are automatically upgraded to Metadata format when tags are added.

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

## Contributing

We welcome contributions! There are several ways to get involved:

### Quick Issue Creation

Create issues from the command line for fast capture:

```bash
# Interactive mode
make issue-bug                              # Bug report with prompts
make issue-feature                          # Feature request with prompts
make issue-idea                             # Quick idea capture

# Programmatic mode (automation/AI agents)
.github/scripts/new-bug.sh "Title" "Description" "Steps" "Expected" "Actual"
echo '{"title":"..."}' | .github/scripts/new-idea.sh --json --silent

# List recent issues
make list-issues
```

**For automation and AI agents:** All scripts support JSON input, environment variables, and silent mode. See [.github/scripts/README.md](.github/scripts/README.md) for details.

### Web Interface

Use structured templates at [github.com/caavere/metarepo/issues/new/choose](https://github.com/caavere/metarepo/issues/new/choose):
- **Bug Report** - Comprehensive form with environment details
- **Feature Request** - Detailed proposal with use cases
- **Quick Idea** - Fast capture for todos and future improvements
- **Security** - Private security vulnerability reporting

### Development Setup

#### Pre-commit Hooks (Recommended)

Pre-commit hooks automatically check code quality before each commit, catching issues early and maintaining consistent code standards.

##### Installation

Install the hooks once for your local repository:

```bash
make install-hooks
```

This creates a pre-commit hook at `.git/hooks/pre-commit` that runs automatically.

##### When Hooks Run

The hook runs **automatically** at this point in your git workflow:

```bash
git add <files>           # 1. Stage your changes
git commit -m "message"   # 2. Hook runs HERE (before commit is created)
                          # 3. If hook passes, commit is created
```

The hook only checks **staged files** (files you've run `git add` on), not all files in your working directory.

##### What the Hook Does

**Auto-fixes (applied automatically):**
- ✓ Code formatting (`cargo fmt`) - Auto-formats all Rust code
- ✓ Trailing whitespace - Removes from all text files
- ✓ End-of-file newlines - Ensures files end with a newline

When auto-fixes are applied, the hook:
1. Makes the fixes
2. Re-stages the fixed files
3. Allows the commit to proceed
4. Shows what was fixed in the output

**Validations (must pass for commit to succeed):**
- ✓ Clippy linting (`cargo clippy -- -D warnings`) - No warnings allowed
- ✓ Merge conflict markers - Prevents committing `<<<<<<<`, `=======`, `>>>>>>>`
- ✓ Large files - Warns if files >1MB are being committed
- ✓ YAML syntax - Validates `.yml` and `.yaml` files
- ✓ TOML syntax - Validates `Cargo.toml` and other `.toml` files
- ✓ Security audit - Checks for vulnerabilities (if `cargo-audit` installed)

##### Hook Outcomes

**✅ Success (all checks pass):**
```bash
$ git commit -m "fix: update logic"
Running pre-commit checks...

Auto-fix checks:
▶ Formatting Rust code... ✓
▶ Removing trailing whitespace... ✓
▶ Ensuring files end with newline... ✓

Validation checks:
▶ Running clippy... ✓
▶ Checking for merge conflicts... ✓
▶ Checking for large files... ✓
▶ Running security audit... ✓

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ All pre-commit checks passed
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[main abc1234] fix: update logic
```

**🔧 Auto-fixed (changes were formatted):**
```bash
$ git commit -m "feat: add feature"
Running pre-commit checks...

Auto-fix checks:
  ℹ Running: cargo fmt --all
  ✓ Code formatted and staged

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Pre-commit checks passed (1 auto-fixes applied)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

ℹ Auto-fixed files have been staged

[main def5678] feat: add feature
```

**❌ Failure (must fix issues):**
```bash
$ git commit -m "broken: bad code"
Running pre-commit checks...

Validation checks:
▶ Running clippy... ✗
Error output:
error: unused variable: `x`
  --> src/main.rs:5:9

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✗ Pre-commit checks failed (1 errors)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

To fix:
  1. Review the errors above
  2. Fix the issues
  3. Re-stage your changes: git add .
  4. Commit again

To bypass (not recommended):
  git commit --no-verify
```

##### Bypassing the Hook

Sometimes you need to commit even when checks fail. Use `--no-verify` (or `-n`):

```bash
# Skip ALL pre-commit checks
git commit --no-verify -m "wip: work in progress"

# Short form
git commit -n -m "wip: work in progress"
```

**When to bypass:**
- ✓ Creating a WIP commit to save progress
- ✓ Committing intentionally broken code for later fixing
- ✓ Emergency hotfixes when hooks are blocking
- ✓ Resolving git hook issues

**When NOT to bypass:**
- ✗ Avoiding fixing clippy warnings
- ✗ Pushing to shared branches
- ✗ Creating pull requests
- ✗ As a regular practice

> **Note:** Even if you bypass the hook, CI will still run all checks. You'll need to fix issues before merging.

##### Troubleshooting

**Hook not running?**
```bash
# Verify hook is installed
ls -la .git/hooks/pre-commit

# Reinstall if needed
make install-hooks
```

**Hook fails on clean code?**
```bash
# Manually run the checks to see detailed output
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

**Slow hook execution?**
The hook is designed to run in ~10-15 seconds. If it's slower:
- Security audit adds ~5 seconds (only if `cargo-audit` installed)
- Large number of files may slow down formatting checks
- First run after dependency changes may trigger rebuilds

**Disable hook temporarily:**
```bash
# Move hook out of the way
mv .git/hooks/pre-commit .git/hooks/pre-commit.disabled

# Re-enable when ready
mv .git/hooks/pre-commit.disabled .git/hooks/pre-commit
```

##### Manual Checks (Alternative to Hooks)

If you prefer not to use hooks, run these commands before committing:

```bash
make fmt          # Format code
make lint         # Run clippy with -D warnings
make test         # Run tests
make audit        # Security audit

# Or run all quality checks at once
make all          # Runs: fmt check test build
```

### Pull Requests

1. Fork and clone the repository
2. Install pre-commit hooks: `make install-hooks` (recommended)
3. Create a feature branch
4. Make your changes with clear commit messages (use commitizen format)
5. Hooks will auto-check quality, or manually run: `make fmt lint test`
6. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines including:
- Development workflow
- Coding standards
- Testing requirements
- Commit message format
- Documentation guidelines

### Security

For security vulnerabilities, please follow our [Security Policy](SECURITY.md). **Do not** create public issues for security concerns.

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
