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

See [docs/qa/TESTING.md](docs/qa/TESTING.md) for comprehensive testing guidelines.

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
