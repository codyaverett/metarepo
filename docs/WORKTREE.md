# Worktree Configuration Guide

This guide covers advanced worktree configuration options in Metarepo, including post-create hooks and bare repository support.

## Table of Contents

- [Overview](#overview)
- [Post-Create Commands (worktree_init)](#post-create-commands-worktree_init)
- [Bare Repository Support](#bare-repository-support)
- [Configuration Examples](#configuration-examples)
- [Best Practices](#best-practices)

## Overview

Metarepo provides powerful worktree management capabilities that allow you to:
1. **Automatically run setup commands** when creating new worktrees
2. **Use bare repositories** for more efficient disk usage and cleaner project structure (enabled by default!)
3. **Configure behavior per-project or globally** across your workspace

**Default Behavior:** All new projects are cloned as bare repositories with worktrees unless explicitly configured otherwise.

## Post-Create Commands (worktree_init)

### What is worktree_init?

The `worktree_init` configuration allows you to specify a command that automatically runs after a new worktree is created. This is useful for:
- Installing dependencies (`npm install`, `pnpm install`, etc.)
- Setting up environment files
- Running build scripts
- Initializing project-specific tools

### Configuration Levels

You can configure `worktree_init` at two levels:

#### 1. Global Level (applies to all projects)

```json
{
  "worktree_init": "pnpm install",
  "projects": {
    "frontend": {
      "url": "git@github.com:user/frontend.git"
    },
    "backend": {
      "url": "git@github.com:user/backend.git"
    }
  }
}
```

#### 2. Project Level (overrides global)

```json
{
  "worktree_init": "pnpm install",
  "projects": {
    "frontend": {
      "url": "git@github.com:user/frontend.git",
      "worktree_init": "npm install && npm run setup"
    },
    "backend": {
      "url": "git@github.com:user/backend.git",
      "worktree_init": "cargo build"
    }
  }
}
```

### Usage

When you create a new worktree, the configured command runs automatically:

```bash
# Creates worktree and runs worktree_init command
meta worktree add feature/new-feature
```

Output:
```
  🌿 Creating worktree 'feature/new-feature' for 1 project(s)
  ══════════════════════════════════════════════════════════

  📦 frontend
     ✅ Created at /workspace/frontend/.worktrees/feature/new-feature
     🔄 Running worktree_init command...
     ✅ Hook completed successfully

  ────────────────────────────────────────────────────────────
  Summary: 1 worktrees created, 0 failed
```

### Skipping Post-Create Commands

You can skip the post-create command using the `--no-hooks` flag:

```bash
# Creates worktree WITHOUT running worktree_init
meta worktree add feature/quick-test --no-hooks
```

This is useful when:
- You're creating a temporary worktree for quick testing
- Dependencies are already installed
- You want to manually run setup later

### Environment Variables

Post-create commands have access to project-specific environment variables defined in your `.meta` configuration:

```json
{
  "projects": {
    "api": {
      "url": "git@github.com:user/api.git",
      "env": {
        "NODE_ENV": "development",
        "API_PORT": "3000"
      },
      "worktree_init": "npm install && npm run setup:dev"
    }
  }
}
```

### Command Execution Context

- **Working directory**: The newly created worktree directory
- **Shell**: Commands are executed via `sh -c` (supports pipes, redirects, etc.)
- **Environment**: Inherits project-specific environment variables from `.meta`
- **Failure handling**: Hook failures are reported but don't prevent worktree creation

## Bare Repository Support

### Quick Start: Adding Bare Repositories

**Bare repositories are now the default!** Simply add projects normally:

```bash
# Add a new project (bare by default)
meta project add my-app https://github.com/user/my-app.git
```

This will:
1. Clone the repository as bare (git data only at `<project>/.git/`)
2. Automatically create a worktree for the default branch at `<project>/<default-branch>/`
3. Configure the project as bare in `.meta`

**To use traditional (non-bare) clones**, explicitly disable bare mode:

```bash
# Add as traditional repository
meta project add my-app https://github.com/user/my-app.git

# With configuration:
{
  "default_bare": false,  # Disable bare by default
  "projects": {
    "my-app": {
      "url": "https://github.com/user/my-app.git"
    }
  }
}
```

### Configuration Options

**Default behavior:** Bare repositories are enabled by default (no configuration needed).

**Override per-project:**

```json
{
  "projects": {
    "bare-app": {
      "url": "git@github.com:user/bare-app.git"
      // Uses bare (default)
    },
    "normal-app": {
      "url": "git@github.com:user/normal-app.git",
      "bare": false  // Override: use traditional clone
    }
  }
}
```

**Disable bare globally:**

```json
{
  "default_bare": false,  // All projects use traditional clones
  "projects": {
    "app1": {
      "url": "git@github.com:user/app1.git"
      // Uses traditional clone
    },
    "app2": {
      "url": "git@github.com:user/app2.git",
      "bare": true  // Override: use bare
    }
  }
}
```

### What are Bare Repositories?

A bare repository contains only git data without a working directory. When combined with worktrees, this provides several benefits:

**Traditional Structure:**
```
workspace/
├── project/
│   ├── .git/                    # Full git data
│   ├── .worktrees/
│   │   ├── feature-1/           # Worktree
│   │   └── feature-2/           # Worktree
│   └── [main branch files]
```

**Bare Repository Structure:**
```
workspace/
├── project/
│   ├── .git/                    # Bare repository (git data only)
│   ├── main/                    # Default branch worktree
│   ├── feature-1/               # Worktree
│   └── feature-2/               # Worktree
```

### Benefits

1. **Cleaner structure**: All branches are at the same level
2. **No special "main" directory**: All branches are equal
3. **Better organization**: Easier to see all active branches
4. **Consistent paths**: `project/<branch>` for all branches

### Configuration

Enable bare repositories on a per-project basis:

```json
{
  "projects": {
    "frontend": {
      "url": "git@github.com:user/frontend.git",
      "bare": true
    },
    "backend": {
      "url": "git@github.com:user/backend.git",
      "bare": false
    }
  }
}
```

### Behavior

When you add a project with `bare: true`:

1. **Clone creates bare repo**: Repository cloned to `<project>/.git/`
2. **Default worktree created**: Automatically creates worktree for default branch (main/master)
3. **Worktrees at project level**: New worktrees created at `<project>/<branch>/`

### Example: Adding a Bare Repository Project

```bash
# First, configure in .meta
# {
#   "projects": {
#     "my-app": {
#       "url": "git@github.com:user/my-app.git",
#       "bare": true,
#       "worktree_init": "npm install"
#     }
#   }
# }

# Add the project
meta project add my-app
```

Output:
```
  🌱 Adding new project...
     Name: my-app
     Source: git@github.com:user/my-app.git
     Type: Bare repository
     Status: Cloning bare repository...
     Status: Creating default worktree...
     ✅ Created default worktree: /workspace/my-app/main
     ✅ Bare repository and default worktree created

  ✅ Successfully added 'my-app'
```

Result:
```
workspace/
├── my-app/
│   ├── .git/              # Bare repository
│   └── main/              # Default branch worktree
```

### Working with Bare Repositories

Creating new worktrees works the same way:

```bash
# Create a feature branch worktree
meta worktree add feature/auth --project my-app -b
```

Result:
```
workspace/
├── my-app/
│   ├── .git/              # Bare repository
│   ├── main/              # Default branch
│   └── feature/           # New feature branch
│       └── auth/
```

### Default Branch Detection

When creating the default worktree, Metarepo automatically detects your default branch:

1. Checks `refs/remotes/origin/HEAD`
2. Falls back to common names: `main`, `master`, `develop`
3. Ultimate fallback: `main`

### Converting Existing Repositories

You can convert an existing normal repository to bare format:

```bash
# Convert an existing project to bare repository
meta project convert-to-bare my-app
```

**What it does:**
1. Checks for uncommitted changes (aborts if found)
2. Backs up the current `.git` directory
3. Creates a bare repository at `<project>/.git/`
4. Creates a worktree for your current branch at `<project>/<current-branch>/`
5. Updates `.meta` configuration to mark project as bare
6. Cleans up the backup

**Important Notes:**
- ⚠️ Commit or stash all changes before converting
- ⚠️ The command prompts for confirmation before proceeding
- ✅ Your current branch working directory becomes `<project>/<branch>/`
- ✅ All git history and branches are preserved
- ✅ You can create new worktrees after conversion

**Example conversion:**

Before:
```
my-app/
├── .git/
├── src/
├── package.json
└── README.md
```

After:
```
my-app/
├── .git/              # Bare repository
└── main/              # Worktree for main branch
    ├── src/
    ├── package.json
    └── README.md
```

Then you can create additional worktrees:
```bash
meta worktree add feature/new-ui --project my-app
```

Result:
```
my-app/
├── .git/              # Bare repository
├── main/              # Main branch
└── feature/           # Feature worktree
    └── new-ui/
```

**Cannot convert back:**
There's currently no automated way to convert from bare back to normal. If needed:
1. Clone the repository fresh as a normal repo
2. Update `.meta` configuration manually
3. Remove the bare repository

Or manually:
```bash
cd my-app/main
git worktree remove ../feature/new-ui  # Remove other worktrees first
mv .git ../.git.backup
cp -r ../.git/. .git/  # Copy bare git to working directory
git config --unset core.bare
git reset --hard HEAD
```

## Configuration Examples

### Example 1: Node.js Projects with Global Default

```json
{
  "worktree_init": "npm ci",
  "projects": {
    "frontend": {
      "url": "git@github.com:company/frontend.git",
      "bare": true
    },
    "backend": {
      "url": "git@github.com:company/backend.git",
      "bare": true
    },
    "docs": {
      "url": "git@github.com:company/docs.git",
      "bare": true,
      "worktree_init": "npm ci && npm run build"
    }
  }
}
```

### Example 2: Mixed Language Projects

```json
{
  "projects": {
    "api": {
      "url": "git@github.com:company/api.git",
      "bare": true,
      "worktree_init": "cargo build",
      "env": {
        "RUST_LOG": "debug"
      }
    },
    "web": {
      "url": "git@github.com:company/web.git",
      "bare": true,
      "worktree_init": "pnpm install && pnpm run dev:setup"
    },
    "mobile": {
      "url": "git@github.com:company/mobile.git",
      "bare": false,
      "worktree_init": "flutter pub get"
    }
  }
}
```

### Example 3: Complex Setup Commands

```json
{
  "projects": {
    "monorepo": {
      "url": "git@github.com:company/monorepo.git",
      "bare": true,
      "worktree_init": "pnpm install && pnpm run setup:env && pnpm run build:deps",
      "env": {
        "NODE_ENV": "development",
        "PNPM_HOME": "/opt/pnpm"
      }
    }
  }
}
```

### Example 4: Conditional Setup

Use shell features for conditional setup:

```json
{
  "projects": {
    "app": {
      "url": "git@github.com:company/app.git",
      "bare": true,
      "worktree_init": "[ -f package-lock.json ] && npm ci || npm install"
    }
  }
}
```

## Best Practices

### 1. Choose the Right worktree_init Command

**Good:**
```json
"worktree_init": "npm ci"              // Fast, deterministic
"worktree_init": "pnpm install --frozen-lockfile"  // Ensures lockfile matches
"worktree_init": "cargo build"         // Pre-compile for immediate use
```

**Avoid:**
```json
"worktree_init": "npm install"         // May update dependencies unexpectedly
"worktree_init": "rm -rf node_modules && npm install"  // Unnecessary, slow
```

### 2. Use Bare Repositories When:

- ✅ You frequently work on multiple branches simultaneously
- ✅ You want a cleaner project structure
- ✅ You're starting a new project
- ✅ Disk space is a concern (shares git objects)

**Don't use bare repositories when:**
- ❌ Project is already established with normal structure
- ❌ Team members don't understand worktrees
- ❌ You rarely use worktrees

### 3. Optimize Hook Commands

**Fast setup:**
```json
"worktree_init": "npm ci --prefer-offline"
```

**Progressive setup:**
```json
"worktree_init": "npm ci && npm run quick-setup"
```

**Background setup:**
```json
"worktree_init": "npm ci && (npm run build > /dev/null 2>&1 &)"
```

### 4. Use --no-hooks Strategically

```bash
# Quick testing - skip setup
meta worktree add test/quick --no-hooks

# Production worktree - run full setup
meta worktree add release/v2.0

# Development worktree - run setup
meta worktree add feature/user-auth
```

### 5. Combine with Project Aliases

```json
{
  "projects": {
    "frontend": {
      "url": "git@github.com:company/web-app.git",
      "aliases": ["web", "ui"],
      "bare": true,
      "worktree_init": "pnpm install"
    }
  }
}
```

Now you can use aliases:
```bash
meta worktree add feature/new-ui --project web
```

### 6. Environment-Specific Setup

```json
{
  "projects": {
    "api": {
      "url": "git@github.com:company/api.git",
      "bare": true,
      "worktree_init": "npm ci && cp .env.development .env",
      "env": {
        "NODE_ENV": "development"
      }
    }
  }
}
```

### 7. Debugging Failed Hooks

If a hook fails:

1. Check the error message in the output
2. Try running the command manually in the worktree directory
3. Use `--no-hooks` to create the worktree, then debug
4. Verify environment variables are set correctly

Example debugging workflow:
```bash
# Create without hook to investigate
meta worktree add debug/test --no-hooks

# Navigate and test manually
cd project/debug/test
npm ci  # or whatever your hook command is

# Fix the issue, then remove and recreate
cd ../../..
meta worktree remove debug/test
meta worktree add debug/test
```

## Command Reference

### Creating Worktrees

```bash
# Basic usage (runs worktree_init)
meta worktree add <branch>

# Create in specific project
meta worktree add <branch> --project <name>

# Create in multiple projects
meta worktree add <branch> --projects proj1,proj2

# Create in all projects
meta worktree add <branch> --all

# Create new branch
meta worktree add -b <branch> <starting-point>

# Skip post-create command
meta worktree add <branch> --no-hooks

# Custom path
meta worktree add <branch> --path custom-name
```

### Managing Worktrees

```bash
# List all worktrees
meta worktree list

# Remove worktrees
meta worktree remove <branch>
meta worktree remove <branch> --force

# Clean up stale worktrees
meta worktree prune
meta worktree prune --dry-run
```

### Project Management

```bash
# Add project as bare repository
meta project add <name> <url> --bare

# Convert existing project to bare
meta project convert-to-bare <project>
```

## Troubleshooting

### Issue: Hook command not found

**Problem:** `sh: npm: command not found`

**Solution:** Ensure the command is in your PATH or use absolute paths:
```json
"worktree_init": "/usr/local/bin/npm ci"
```

### Issue: Hook fails but worktree created

**Behavior:** Worktree is created successfully, but the hook fails

**Solution:** This is by design. The worktree is usable, but setup didn't complete. Fix the hook command and either:
- Re-run manually: `cd worktree && npm ci`
- Remove and recreate: `meta worktree remove <branch> && meta worktree add <branch>`

### Issue: Bare repo confusion with git commands

**Problem:** Running `git status` in project root shows error

**Solution:** With bare repos, run git commands in worktree directories:
```bash
# Wrong (in bare repo root)
cd project
git status  # Error: not a git repository

# Correct (in worktree)
cd project/main
git status  # Works!
```

Or use `meta git status` to operate on all worktrees.

### Issue: Default worktree not on correct branch

**Problem:** Default worktree created on wrong branch

**Solution:** Manually set remote HEAD:
```bash
cd project/.git
git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/main
```

## See Also

- [README](../README.md) - Main documentation
- [Architecture](ARCHITECTURE.md) - System design
- [Plugin Development](PLUGIN_DEVELOPMENT.md) - Creating plugins
