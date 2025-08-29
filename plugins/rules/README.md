# Gestalt Rules Plugin

A file structure enforcement plugin for Gestalt that helps maintain consistent project organization across repositories.

## Features

- **Configurable Rules**: Define directory structure, file patterns, and component conventions
- **Multi-Project Validation**: Check rules across all projects in your workspace
- **Auto-Fix Capabilities**: Automatically create missing directories
- **Flexible Configuration**: Support for YAML and JSON rule definitions
- **Claude-Friendly**: Can be used by AI assistants to understand project structure

## Usage

### Initialize Rules Configuration

```bash
gest rules init
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

### List Configured Rules

```bash
gest rules list
```

## Configuration Format

Rules are defined in `.rules.yaml` in your workspace root:

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

## Future Enhancements

- Pattern-based file content validation
- Custom validators via scripts
- CI/CD integration with JSON output
- Parallel validation for large workspaces
- Template generation for common structures