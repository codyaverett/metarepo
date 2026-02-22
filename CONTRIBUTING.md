# Contributing to metarepo

Thank you for your interest in contributing to metarepo! This document provides guidelines and workflows for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
- [Issue Creation](#issue-creation)
- [Pull Requests](#pull-requests)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Documentation](#documentation)
- [Security](#security)

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment. Please be kind and considerate in all interactions.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/metarepo.git
   cd metarepo
   ```
3. **Build the project**:
   ```bash
   cargo build
   ```
4. **Run tests**:
   ```bash
   cargo test
   ```

## How to Contribute

There are many ways to contribute:

- Report bugs
- Suggest new features or enhancements
- Improve documentation
- Write tests
- Submit code changes
- Review pull requests
- Help others in discussions

## Issue Creation

We've streamlined issue creation with multiple options:

### Quick CLI Commands (Fastest!)

**Interactive mode:**
```bash
.github/scripts/new-bug.sh        # Bug report with prompts
.github/scripts/new-feature.sh    # Feature request with prompts
.github/scripts/new-idea.sh       # Quick idea capture
```

**Programmatic mode** (for automation/AI agents):
```bash
# Command-line arguments
.github/scripts/new-bug.sh "Title" "Description" "Steps" "Expected" "Actual"
.github/scripts/new-idea.sh "Idea title" "Optional notes"

# JSON input (perfect for Claude agents)
echo '{"title":"Bug title","description":"..."}' | .github/scripts/new-bug.sh --json --silent

# Environment variables
BUG_TITLE="..." BUG_DESC="..." .github/scripts/new-bug.sh
```

**Using Makefile shortcuts:**
```bash
make issue-bug      # Interactive bug report
make issue-feature  # Interactive feature request
make issue-idea     # Quick idea capture
make list-issues    # View recent issues
```

**See [.github/scripts/README.md](.github/scripts/README.md) for complete programmatic usage examples.**

### Web Interface (Structured Forms)

Visit [github.com/caavere/metarepo/issues/new/choose](https://github.com/caavere/metarepo/issues/new/choose) to use:

- **Bug Report** - Comprehensive form with environment details
- **Feature Request** - Detailed proposal template
- **Quick Idea** - Minimal template for fast capture
- **Security Vulnerability** - Private security reporting

### Issue Types

#### Bug Reports
Include:
- Clear description of the bug
- Steps to reproduce
- Expected vs. actual behavior
- Environment details (OS, metarepo version, Rust version)
- Error messages or logs

#### Feature Requests
Include:
- Problem statement and use case
- Proposed solution
- Alternative approaches considered
- Usage examples

#### Quick Ideas
For:
- Future improvements
- Small todos
- Brainstorming
- Notes for later expansion

## Pull Requests

### Before Submitting

1. **Search existing PRs** to avoid duplicates
2. **Link related issues** in your PR description
3. **Follow coding standards** (run `cargo fmt` and `cargo clippy`)
4. **Add tests** for new functionality
5. **Update documentation** if needed

### PR Process

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**:
   - Write clear, focused commits
   - Use commitizen format for commit messages
   - Include issue references (e.g., `fixes #123`)

3. **Run quality checks**:
   ```bash
   cargo fmt           # Format code
   cargo clippy        # Run linter
   cargo test          # Run tests
   cargo build         # Ensure it builds
   ```

4. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a pull request** on GitHub

### PR Requirements

- **Description**: Clear explanation of changes and motivation
- **Tests**: All tests pass (`cargo test`)
- **Formatting**: Code is formatted (`cargo fmt`)
- **Linting**: No clippy warnings (`cargo clippy`)
- **Documentation**: Updated if needed
- **Linked Issues**: Reference related issues

### Commit Message Format

Use commitizen format:

```
type(scope): subject

body (optional)

footer (optional)
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Test additions or changes
- `chore`: Build process or auxiliary tool changes

**Example:**
```
feat(git): add support for git worktree pruning

Implement `meta git worktree prune` command to clean up stale worktrees.

Fixes #42
```

## Development Workflow

### Project Structure

```
metarepo/
├── Cargo.toml              # Workspace configuration
├── metarepo-core/          # Core plugin API
│   ├── src/
│   └── Cargo.toml
├── metarepo/               # Main CLI with built-in plugins
│   ├── src/
│   │   ├── main.rs
│   │   ├── cli/
│   │   ├── plugins/
│   │   └── tui/
│   └── Cargo.toml
├── docs/                   # Documentation
└── .github/                # CI/CD and templates
```

### Building

```bash
cargo build                 # Debug build
cargo build --release       # Release build
```

### Testing

```bash
cargo test                  # Run all tests
cargo test --package metarepo-core  # Test specific package
cargo test integration      # Run integration tests
```

### Code Quality

```bash
cargo fmt                   # Format code
cargo clippy                # Run linter
cargo clippy -- -D warnings # Fail on warnings
```

### Pre-commit Hooks

Pre-commit hooks automatically check code quality before each commit, catching issues early and maintaining consistent code standards.

#### Installation

Install the hooks once for your local repository:

```bash
make install-hooks
```

This creates a pre-commit hook at `.git/hooks/pre-commit` that runs automatically.

#### When Hooks Run

The hook runs **automatically** at this point in your git workflow:

```bash
git add <files>           # 1. Stage your changes
git commit -m "message"   # 2. Hook runs HERE (before commit is created)
                          # 3. If hook passes, commit is created
```

The hook only checks **staged files** (files you've run `git add` on), not all files in your working directory.

#### What the Hook Does

**Auto-fixes (applied automatically):**
- Code formatting (`cargo fmt`) - Auto-formats all Rust code
- Trailing whitespace - Removes from all text files
- End-of-file newlines - Ensures files end with a newline

When auto-fixes are applied, the hook:
1. Makes the fixes
2. Re-stages the fixed files
3. Allows the commit to proceed
4. Shows what was fixed in the output

**Validations (must pass for commit to succeed):**
- Clippy linting (`cargo clippy -- -D warnings`) - No warnings allowed
- Merge conflict markers - Prevents committing `<<<<<<<`, `=======`, `>>>>>>>`
- Large files - Warns if files >1MB are being committed
- YAML syntax - Validates `.yml` and `.yaml` files
- TOML syntax - Validates `Cargo.toml` and other `.toml` files
- Security audit - Checks for vulnerabilities (if `cargo-audit` installed)

#### Hook Outcomes

**Success (all checks pass):**
```bash
$ git commit -m "fix: update logic"
Running pre-commit checks...

Auto-fix checks:
  Formatting Rust code... ok
  Removing trailing whitespace... ok
  Ensuring files end with newline... ok

Validation checks:
  Running clippy... ok
  Checking for merge conflicts... ok
  Checking for large files... ok
  Running security audit... ok

All pre-commit checks passed

[main abc1234] fix: update logic
```

**Auto-fixed (changes were formatted):**
```bash
$ git commit -m "feat: add feature"
Running pre-commit checks...

Auto-fix checks:
  Running: cargo fmt --all
  Code formatted and staged

Pre-commit checks passed (1 auto-fixes applied)

[main def5678] feat: add feature
```

**Failure (must fix issues):**
```bash
$ git commit -m "broken: bad code"
Running pre-commit checks...

Validation checks:
  Running clippy... FAILED
Error output:
error: unused variable: `x`
  --> src/main.rs:5:9

Pre-commit checks failed (1 errors)

To fix:
  1. Review the errors above
  2. Fix the issues
  3. Re-stage your changes: git add .
  4. Commit again

To bypass (not recommended):
  git commit --no-verify
```

#### Bypassing the Hook

Sometimes you need to commit even when checks fail. Use `--no-verify` (or `-n`):

```bash
# Skip ALL pre-commit checks
git commit --no-verify -m "wip: work in progress"

# Short form
git commit -n -m "wip: work in progress"
```

**When to bypass:**
- Creating a WIP commit to save progress
- Committing intentionally broken code for later fixing
- Emergency hotfixes when hooks are blocking
- Resolving git hook issues

**When NOT to bypass:**
- Avoiding fixing clippy warnings
- Pushing to shared branches
- Creating pull requests
- As a regular practice

> **Note:** Even if you bypass the hook, CI will still run all checks. You'll need to fix issues before merging.

#### Troubleshooting

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

#### Manual Checks (Alternative to Hooks)

If you prefer not to use hooks, run these commands before committing:

```bash
make fmt          # Format code
make lint         # Run clippy with -D warnings
make test         # Run tests
make audit        # Security audit

# Or run all quality checks at once
make all          # Runs: fmt check test build
```

### Documentation

```bash
cargo doc                   # Build documentation
cargo doc --open            # Build and open docs
```

## Testing

### Test Categories

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test component interactions
- **Smoke Tests**: Basic functionality verification
- **Security Tests**: Vulnerability and safety checks

### Writing Tests

- Place unit tests in the same file as the code being tested
- Place integration tests in `tests/` directory
- Use descriptive test names
- Test edge cases and error conditions

### Test Guidelines

See [docs/qa/TESTING_GUIDELINES.md](docs/qa/TESTING_GUIDELINES.md) for comprehensive testing guidelines.

## Documentation

### What to Document

- **Code**: Use doc comments (`///`) for public APIs
- **Architecture**: Update `docs/ARCHITECTURE.md` for design changes
- **Features**: Document new features in README
- **Plugins**: Update `docs/PLUGIN_DEVELOPMENT.md` for plugin APIs

### Documentation Style

- Write clear, concise explanations
- Include code examples
- Use proper markdown formatting
- Keep documentation up-to-date with code changes

## Security

For security vulnerabilities:

1. **DO NOT** create a public issue
2. Follow the [Security Policy](SECURITY.md)
3. Report privately via GitHub Security Advisory or email

For security questions or general concerns, use the Security issue template.

## Getting Help

- **Documentation**: Check [docs/](docs/) directory
- **Discussions**: Ask questions in [GitHub Discussions](https://github.com/caavere/metarepo/discussions)
- **Issues**: Search existing issues or create a new one
- **Architecture**: Review [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Plugin Development**: See [docs/PLUGIN_DEVELOPMENT.md](docs/PLUGIN_DEVELOPMENT.md)

## Recognition

Contributors are recognized in:
- Release notes
- GitHub contributors page
- Project documentation

Thank you for contributing to metarepo!

---

**Questions?** Open a [discussion](https://github.com/caavere/metarepo/discussions) or ask in your PR/issue.
