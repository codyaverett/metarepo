# TODO - Future Features and Improvements

This document tracks ideas, enhancements, and features for future development.

## Interactive Configuration

### Priority: Medium
### Status: ✅ Completed (v0.9.0)

**Goal:** Make all plugins support interactive parameter collection when required arguments are missing.

**Current Behavior:**
```bash
meta project add
# Error: missing required argument 'path'
```

**Desired Behavior:**
```bash
meta project add
# Interactive prompts:
# → Project name: my-app
# → Repository URL (optional): https://github.com/user/my-app.git
# → Clone as bare repository? [Y/n]: y
# → Post-create command (optional): npm install
#
# ✅ Adding project 'my-app'...
```

**Applies To:**
- ✅ `meta project add` - Prompts for name, URL when not provided
- ✅ `meta worktree add` - Prompts for branch name and project selection
- ✅ `meta worktree remove` - Prompts for branch name and project selection
- ✅ `meta project remove` - Interactive project selection from list
- ✅ `meta plugin add` - Prompts for plugin path
- ✅ `meta run` - Interactive script selection when not specified

**Implementation Notes:**
- ✅ Use consistent prompting style across all plugins (dialoguer + colored)
- ✅ Support both interactive and non-interactive modes (detect TTY)
- ✅ Provide sensible defaults
- ✅ `--non-interactive=fail` flag to fail on missing input (CI/CD friendly)
- ✅ `--non-interactive=defaults` flag to use sensible defaults
- ✅ Use colored cyan prompts with error messages in red
- ✅ Support Ctrl+C to cancel at any point
- ✅ TTY detection via `io::stdin().is_terminal()`
- ✅ Shared utility module in `meta-core::interactive` for reusable prompts

**Benefits:**
- Better user experience for beginners
- Reduces need to remember exact command syntax
- Guided workflows for complex operations
- Less frustration when forgetting arguments

---

## Worktree Improvements

### Auto-sync Worktrees Across Projects

**Priority:** Low
**Status:** Idea

When creating a worktree for a branch in one project, optionally create the same worktree in related projects.

```bash
meta worktree add feature/auth --sync
# Creates feature/auth in all projects (or prompts which ones)
```

### Worktree Templates

**Priority:** Low
**Status:** Idea

Define templates for common worktree setups:

```json
{
  "worktree_templates": {
    "feature": {
      "init": "npm ci && npm run setup:dev",
      "env": {
        "NODE_ENV": "development"
      }
    },
    "hotfix": {
      "init": "npm ci --production",
      "env": {
        "NODE_ENV": "production"
      }
    }
  }
}
```

```bash
meta worktree add feature/new-ui --template feature
```

---

## Configuration Enhancements

### Configuration Validation

**Priority:** Medium
**Status:** Idea

Add `meta config validate` command to check `.meta` file for:
- Invalid JSON syntax
- Missing required fields
- Invalid URL formats
- Broken project references
- Circular dependencies in nested repos

### Configuration Migration

**Priority:** Low
**Status:** Idea

Support migrating from other multi-repo tools:

```bash
meta config import --from meta.json  # Node.js meta
meta config import --from workspace.json  # Other tools
```

### Environment-Specific Configurations

**Priority:** Low
**Status:** Idea

Support multiple configuration profiles:

```json
{
  "profiles": {
    "development": {
      "worktree_init": "npm ci",
      "default_bare": true
    },
    "production": {
      "worktree_init": "npm ci --production",
      "default_bare": false
    }
  }
}
```

```bash
meta --profile production project add my-app
```

---

## Plugin System

### Plugin Marketplace

**Priority:** Low
**Status:** Idea

Create a registry/marketplace for community plugins:

```bash
meta plugin search <query>
meta plugin info <plugin-name>
meta plugin install <plugin-name>
meta plugin publish
```

### Plugin Hooks System

**Priority:** Medium
**Status:** Idea

Allow plugins to register hooks for lifecycle events:

```rust
pub trait PluginHooks {
    fn before_clone(&self, url: &str) -> Result<()>;
    fn after_clone(&self, path: &Path) -> Result<()>;
    fn before_worktree_create(&self, branch: &str) -> Result<()>;
    fn after_worktree_create(&self, path: &Path) -> Result<()>;
}
```

Use cases:
- Auto-run linters after clone
- Send notifications after operations
- Update external tools/databases
- Custom validation logic

---

## Performance Improvements

### Parallel Operations

**Priority:** Medium
**Status:** Partially Implemented

Enhance existing parallel execution:
- Make it default for safe operations
- Better progress reporting with multiple bars
- Configurable concurrency limits
- Smart dependency detection (don't parallelize dependent operations)

### Caching

**Priority:** Low
**Status:** Idea

Cache expensive operations:
- Git status results (invalidate on file changes)
- Project tree structure
- Remote URL lookups
- Plugin metadata

---

## Developer Experience

### Shell Completions

**Priority:** High
**Status:** Planned

Generate shell completions for:
- Bash
- Zsh
- Fish
- PowerShell

```bash
meta completions bash > /etc/bash_completion.d/meta
meta completions zsh > ~/.zsh/completion/_meta
```

### Better Error Messages

**Priority:** Medium
**Status:** Ongoing

Improve error messages with:
- Suggestions for common mistakes
- Links to documentation
- Examples of correct usage
- Color-coded severity levels

### Logging and Debugging

**Priority:** Medium
**Status:** Idea

Enhanced logging capabilities:

```bash
meta --log-level debug project add my-app
meta --log-file meta.log worktree add feature/test
meta logs view
meta logs clear
```

---

## Git Integration

### Git Worktree Cleanup

**Priority:** Medium
**Status:** Idea

Automatically detect and clean up abandoned worktrees:

```bash
meta worktree doctor
# Checks for:
# - Worktrees with uncommitted changes
# - Worktrees behind remote
# - Stale worktrees (no activity for N days)
# - Orphaned worktree references
```

### Branch Synchronization

**Priority:** Low
**Status:** Idea

Keep branches in sync across projects:

```bash
meta git sync
# For each project:
# - Fetch from remote
# - Update all worktrees to latest
# - Report conflicts
```

### Stacked Diffs Support

**Priority:** Low
**Status:** Idea

Better support for stacked diff workflows:

```bash
meta worktree stack feature/base feature/step1 feature/step2
# Creates worktrees with proper base branches
```

---

## Repository Management

### Monorepo Support

**Priority:** Low
**Status:** Idea

Better support for monorepos with workspace features:

```json
{
  "type": "monorepo",
  "packages": [
    "packages/*",
    "apps/*"
  ]
}
```

### Submodule Alternative

**Priority:** Low
**Status:** Idea

Use meta as a smarter alternative to git submodules:
- Easier to update
- Better branch management
- No nested .git issues

---

## Testing and Quality

### Integration Tests

**Priority:** High
**Status:** Needed

Add comprehensive integration tests:
- Test real git operations
- Test worktree creation/removal
- Test bare repo conversions
- Test error conditions

### Benchmark Suite

**Priority:** Low
**Status:** Idea

Create benchmarks for:
- Large workspace operations
- Many projects/worktrees
- Deep nested repositories
- Parallel vs sequential performance

---

## Documentation

### Video Tutorials

**Priority:** Low
**Status:** Idea

Create video walkthroughs for:
- Getting started
- Advanced worktree workflows
- Plugin development
- Migration guides

### Interactive Tutorial

**Priority:** Low
**Status:** Idea

Build an interactive tutorial:

```bash
meta tutorial start
# Step-by-step guide with actual operations
# Safe sandbox environment
# Undo capability
```

---

## Ideas for Consideration

### CI/CD Integration
- GitHub Actions plugin
- GitLab CI templates
- Jenkins integration

### IDE Integration
- VS Code extension
- IntelliJ plugin
- Vim/Neovim plugin

### Metrics and Analytics
- Track workspace health
- Report usage patterns
- Identify bottlenecks

### Backup and Restore
- Backup entire workspace state
- Restore to previous state
- Export/import configurations

### Team Collaboration
- Shared workspace configurations
- Team-specific scripts
- Role-based permissions

---

## Contributing

Have an idea? Please add it to this document or open an issue on GitHub!

Format:
```markdown
### Feature Name

**Priority:** High/Medium/Low
**Status:** Planned/Idea/In Progress/Completed

Description of the feature...

**Benefits:**
- List benefits

**Implementation Notes:**
- Technical considerations
```
