# Gestalt Rules Plugin

A file structure enforcement plugin for Gestalt that helps maintain consistent project organization across repositories.

## Features

- **Configurable Rules**: Define directory structure, file patterns, and component conventions
- **Multi-Project Validation**: Check rules across all projects in your workspace
- **Project-Specific Rules**: Override workspace rules with project-specific configurations
- **Auto-Fix Capabilities**: Automatically create missing directories
- **Rule Creation Tools**: Interactive commands to create new rules
- **Documentation System**: Built-in docs for learning rule syntax
- **Flexible Configuration**: Support for YAML and JSON rule definitions
- **Claude-Friendly**: Can be used by AI assistants to understand project structure

## Usage

### Initialize Rules Configuration

```bash
# Initialize workspace rules
gest rules init

# Initialize project-specific rules
gest rules init --project frontend
```

Creates a `.rules.yaml` file with example configuration.

### Check Project Structure

```bash
# Check all projects
gest rules check

# Check specific project
gest rules check --project my-project

# Auto-fix violations
gest rules check --fix
```

### Create New Rules

```bash
# Create directory rule
gest rules create directory src/utils --required --description "Utility functions"

# Create component rule
gest rules create component 'components/**/' --structure '[ComponentName].vue,[ComponentName].test.js'

# Create file rule
gest rules create file '**/*.ts' --requires 'test:*.test.ts,doc:*.md'

# Add rule to specific project
gest rules create directory src/hooks --project frontend
```

### View Documentation

```bash
# Full documentation
gest rules docs

# Specific rule type docs
gest rules docs directory
gest rules docs component
gest rules docs file
```

### Manage Project Rules

```bash
# Show rules status for all projects
gest rules status

# Copy workspace rules to a project
gest rules copy frontend

# List rules for specific project
gest rules list --project frontend
```

## Rule Priority and Inheritance

The rules plugin supports multiple configuration levels:

1. **Project-specific rules** (`<project>/.rules.yaml`) - Highest priority
2. **Workspace rules** (`.rules.yaml` in workspace root) - Default for all projects
3. **Built-in minimal rules** - Used when no configuration exists

When checking a project:
- If the project has its own `.rules.yaml`, only those rules are used
- Otherwise, workspace rules are applied
- Use `gest rules copy <project>` to start with workspace rules and customize

## Configuration Format

Rules are defined in `.rules.yaml`:

```yaml
directories:
  - path: src
    required: true
    description: Source code directory
  - path: tests
    required: false
    description: Test files directory

components:
  - pattern: "components/**/"
    structure:
      - "[ComponentName].vue"
      - "__tests__/"
      - "__tests__/[ComponentName].test.js"
      - "[ComponentName].stories.js"
    description: Vue component structure

files:
  - pattern: "**/*.vue"
    requires:
      test: "__tests__/*.test.js"
      story: "*.stories.js"
    description: Vue files must have tests and stories
```

## Rule Types

### Directory Rules
Ensure specific directories exist in your projects.

```yaml
directories:
  - path: src
    required: true  # Violations are errors
  - path: docs
    required: false # Violations are info-level
```

### Component Rules
Validate component folder structures using patterns.

```yaml
components:
  - pattern: "components/**/"  # Matches component directories
    structure:                 # Required files/folders within
      - "[ComponentName].vue"   # [ComponentName] is replaced with actual name
      - "__tests__/"
```

### File Rules
Ensure files have required companions (tests, documentation, etc.).

```yaml
files:
  - pattern: "**/*.rs"
    requires:
      test: "#[test]"  # Special case: looks for test annotations in file
```

## Integration with Claude/AI

This plugin is particularly useful for AI assistants to:

1. **Quick Context Building**: Run `gest rules check` to understand project structure before making changes
2. **Ensure Consistency**: Validate that new components follow conventions
3. **Auto-Generate Structure**: Use `--fix` to create required directories

Example workflow for Claude:
```bash
# Check current structure
gest rules check --project frontend

# Create new component following rules
mkdir -p components/Button
touch components/Button/Button.vue
mkdir -p components/Button/__tests__
touch components/Button/__tests__/Button.test.js

# Verify structure
gest rules check --project frontend
```

## Examples

### React TypeScript Project

```yaml
directories:
  - path: src/components
    required: true
  - path: src/__tests__
    required: true

components:
  - pattern: "src/components/**/"
    structure:
      - "[ComponentName].tsx"
      - "[ComponentName].test.tsx"
      - "[ComponentName].stories.tsx"
      - "index.ts"

files:
  - pattern: "**/*.tsx"
    requires:
      test: "*.test.tsx"
```

### Rust Project

```yaml
directories:
  - path: src
    required: true
  - path: benches
    required: false

files:
  - pattern: "src/**/*.rs"
    requires:
      test: "#[test]"  # Looks for test annotations
```

## Severity Levels

- **Error**: Required rules that must be followed
- **Warning**: Important but not critical violations
- **Info**: Optional improvements or suggestions

## Advanced Features

### Project-Specific Rules

Each project can have its own rules that completely override workspace rules:

```bash
# Initialize rules for a specific project
gest rules init --project frontend

# Edit frontend/.rules.yaml to customize
# Now frontend will use its own rules instead of workspace rules
```

### Interactive Rule Creation

The plugin supports creating rules through command-line arguments or interactively:

```bash
# With arguments
gest rules create directory src/config --required

# Interactive mode (when structure items aren't provided)
gest rules create component 'components/**/'
> Enter structure items (empty line to finish):
> [ComponentName].tsx
> [ComponentName].test.tsx
> index.ts
>
```

### Built-in Documentation

Access comprehensive documentation without leaving the terminal:

```bash
# Learn about all rule types
gest rules docs

# Get examples for specific rule type
gest rules docs component
```

### Integration with Project Plugin

The rules plugin integrates with the Gestalt project plugin to:
- Automatically discover projects from `.meta` configuration
- Support project-specific rule paths
- Validate multiple projects in one command

## Dependencies

This plugin depends on:
- `meta-core` - Core Gestalt plugin interfaces
- `meta-project` - Project management functionality

## Future Enhancements

- Pattern-based file content validation
- Custom validators via scripts
- CI/CD integration with JSON output
- Parallel validation for large workspaces
- Template generation for common structures
- Rule inheritance and composition
- Watch mode for real-time validation