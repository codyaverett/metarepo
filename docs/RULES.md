# Metarepo Rules System Documentation

## Overview

The Metarepo Rules System provides comprehensive project structure validation and enforcement. It helps maintain consistency across all projects in a metarepo workspace through configurable rules that check directory structures, file patterns, naming conventions, documentation, security, and more.

## Table of Contents

- [Quick Start](#quick-start)
- [Rule Types](#rule-types)
- [Configuration](#configuration)
- [Rule Priority](#rule-priority)
- [AI Assistant Integration](#ai-assistant-integration)
- [CI/CD Integration](#cicd-integration)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)

## Quick Start

### Check All Projects
```bash
# Check workspace rules compliance
meta rules check

# Auto-fix violations where possible
meta rules check --fix
```

### Check Specific Project
```bash
# Check a single project
meta rules check --project meta-core

# Fix violations in specific project
meta rules check --project meta --fix
```

### Initialize Rules
```bash
# Create workspace rules
meta rules init

# Create project-specific rules
meta rules init --project frontend
```

## Rule Types

### 1. Directory Rules

Ensure specific directories exist in your projects.

```yaml
directories:
  - path: src
    required: true  # Error if missing
    description: Source code directory
  - path: docs
    required: false # Info if missing
    description: Documentation
```

**Validation:**
- Checks if specified directories exist
- Required directories generate errors
- Optional directories generate info messages
- Auto-fixable by creating missing directories

### 2. Component Rules

Validate component folder structures using patterns.

```yaml
components:
  - pattern: "src/plugins/*/"
    structure:
      - "mod.rs"
      - "plugin.rs"
      - "README.md"
    description: Plugin structure
```

**Features:**
- Pattern matching with wildcards
- `[ComponentName]` placeholder replacement
- Validates internal structure of matched directories

### 3. File Rules

Ensure files have required companions or content.

```yaml
files:
  - pattern: "**/*.rs"
    requires:
      test: "#[test]"  # Content check
      doc: "///"       # Documentation check
    description: Rust files need tests
```

**Special Checks:**
- `#[test]` - Looks for test annotations in Rust files
- File path patterns for companion files
- Content patterns for in-file requirements

### 4. Naming Rules

Enforce naming conventions for files and directories.

```yaml
naming:
  - pattern: "**/*.rs"
    naming_pattern: "^[a-z_]+$"
    case_style: "snake_case"
    description: Rust files use snake_case
```

**Supported Case Styles:**
- `snake_case` - lowercase with underscores
- `camelCase` - first word lowercase
- `PascalCase` - all words capitalized
- `kebab-case` - lowercase with hyphens
- `UPPER_CASE` - uppercase with underscores

### 5. Documentation Rules

Ensure proper documentation standards.

```yaml
documentation:
  - pattern: "**/README.md"
    require_header: true
    require_examples: true
    min_description_length: 200
    required_sections:
      - "## Installation"
      - "## Usage"
    description: README standards
```

### 6. Size Rules

Control file size and complexity.

```yaml
size:
  - pattern: "src/**/*.rs"
    max_lines: 1000
    max_bytes: 100000
    max_functions: 30
    max_complexity: 10
    description: Keep files manageable
```

### 7. Security Rules

Enforce security best practices.

```yaml
security:
  - pattern: "**/*.rs"
    forbidden_patterns:
      - "password\\s*=\\s*\".*\""
      - "api_key\\s*=\\s*\".*\""
    no_hardcoded_secrets: true
    require_https: true
    forbidden_functions:
      - "eval"
      - "exec"
    description: Security requirements
```

### 8. Dependency Rules

Manage project dependencies.

```yaml
dependencies:
  allowed:
    - "serde"
    - "tokio"
  forbidden:
    - "openssl"  # Use rustls instead
  required:
    serde: ">=1.0"
  max_depth: 5
  description: Dependency restrictions
```

### 9. Import Rules

Organize and restrict imports.

```yaml
imports:
  - source_pattern: "src/**/*.rs"
    allowed_imports:
      - "crate::"
      - "std::"
    forbidden_imports:
      - "use .*::*;"  # No wildcards
    require_absolute: false
    max_depth: 3
    description: Import organization
```

## Configuration

### File Locations

1. **Workspace Rules**: `.rules.yaml` in workspace root
2. **Project Rules**: `<project>/.rules.yaml`
3. **Format**: YAML or JSON (`.rules.json`)

### Configuration Structure

```yaml
# Full configuration example
directories:
  - path: string
    required: boolean
    description: string

components:
  - pattern: string
    structure: [string]
    description: string

files:
  - pattern: string
    requires: {key: value}
    description: string

naming:
  - pattern: string
    naming_pattern: string
    case_style: string
    description: string

documentation:
  - pattern: string
    require_header: boolean
    require_examples: boolean
    min_description_length: number
    required_sections: [string]
    description: string

size:
  - pattern: string
    max_lines: number
    max_bytes: number
    max_functions: number
    max_complexity: number
    description: string

security:
  - pattern: string
    forbidden_patterns: [string]
    no_hardcoded_secrets: boolean
    require_https: boolean
    forbidden_functions: [string]
    description: string

dependencies:
  allowed: [string]
  forbidden: [string]
  required: {package: version}
  max_depth: number
  description: string

imports:
  - source_pattern: string
    allowed_imports: [string]
    forbidden_imports: [string]
    require_absolute: boolean
    max_depth: number
    description: string
```

## Rule Priority

Rules are applied in the following order:

1. **Project-specific rules** (highest priority)
   - Located in `<project>/.rules.yaml`
   - Completely override workspace rules

2. **Workspace rules** (default)
   - Located in workspace root `.rules.yaml`
   - Apply to all projects without specific rules

3. **Built-in minimal rules** (fallback)
   - Used when no configuration exists
   - Basic structure validation only

## AI Assistant Integration

The rules system is designed to work seamlessly with AI assistants like Claude, ChatGPT, and GitHub Copilot.

### For AI Assistants

1. **Check Current Structure**
```bash
# Understand project before making changes
meta rules check --project frontend
```

2. **Validate Changes**
```bash
# After creating new components
meta rules check --project frontend

# Auto-fix if needed
meta rules check --project frontend --fix
```

3. **Generate Compliant Code**
```bash
# View rules for reference
meta rules list --project frontend

# Create structure following rules
mkdir -p src/components/Button
touch src/components/Button/Button.tsx
touch src/components/Button/Button.test.tsx
```

### AI-Friendly Commands

```bash
# Get rules in structured format
meta rules docs --ai

# Check specific rule type
meta rules docs component --ai

# Get project-specific rules
meta rules list --project frontend
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Rules Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Build meta CLI
        run: cargo build --release
      
      - name: Check rules compliance
        run: ./target/release/meta rules check
      
      - name: Upload violations report
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: rules-violations
          path: rules-report.json
```

### GitLab CI

```yaml
rules-check:
  stage: test
  script:
    - cargo build --release
    - ./target/release/meta rules check
  artifacts:
    when: on_failure
    paths:
      - rules-report.json
```

### Pre-commit Hook

```bash
#!/bin/sh
# .git/hooks/pre-commit

# Check rules before committing
meta rules check --project $(basename $(pwd))

if [ $? -ne 0 ]; then
  echo "Rules validation failed. Fix violations before committing."
  exit 1
fi
```

## Examples

### React TypeScript Project

```yaml
# frontend/.rules.yaml
directories:
  - path: src/components
    required: true
  - path: src/hooks
    required: true
  - path: src/utils
    required: true
  - path: public
    required: true

components:
  - pattern: "src/components/*/"
    structure:
      - "[ComponentName].tsx"
      - "[ComponentName].test.tsx"
      - "[ComponentName].module.css"
      - "index.ts"

files:
  - pattern: "src/**/*.tsx"
    requires:
      test: ".test.tsx"
      types: "interface|type"

naming:
  - pattern: "src/components/*.tsx"
    case_style: "PascalCase"
  - pattern: "src/hooks/*.ts"
    naming_pattern: "^use[A-Z]"
```

### Rust Library Project

```yaml
# my-lib/.rules.yaml
directories:
  - path: src
    required: true
  - path: tests
    required: false
  - path: benches
    required: false
  - path: examples
    required: false

files:
  - pattern: "src/**/*.rs"
    requires:
      test: "#[test]|#[cfg(test)]"
      doc: "///"

documentation:
  - pattern: "src/lib.rs"
    require_header: true
    min_description_length: 100
    required_sections:
      - "//! # Examples"

size:
  - pattern: "src/**/*.rs"
    max_lines: 500
    max_functions: 20
```

### Node.js API Project

```yaml
# api/.rules.yaml
directories:
  - path: src/routes
    required: true
  - path: src/middleware
    required: true
  - path: src/models
    required: true
  - path: src/controllers
    required: true

components:
  - pattern: "src/routes/*/"
    structure:
      - "index.js"
      - "[name].route.js"
      - "[name].test.js"

files:
  - pattern: "src/**/*.js"
    requires:
      test: ".test.js|.spec.js"
      jsdoc: "/**"

security:
  - pattern: "**/*.js"
    forbidden_patterns:
      - "eval\\("
      - "require\\(.*\\$\\{"
    no_hardcoded_secrets: true
```

## Troubleshooting

### Common Issues

#### 1. Rules Not Found
```bash
Error: No rules configuration found
```
**Solution**: Initialize rules with `meta rules init`

#### 2. Pattern Not Matching
```bash
Warning: Pattern "src/**/*.tsx" matched no files
```
**Solution**: Check pattern syntax and file extensions

#### 3. Auto-fix Failing
```bash
Error: Cannot auto-fix: permission denied
```
**Solution**: Check file permissions or run with appropriate privileges

#### 4. Conflicting Rules
```bash
Error: Rule conflict detected
```
**Solution**: Review rule priorities and patterns for overlaps

### Debug Mode

```bash
# Verbose output for debugging
RUST_LOG=debug meta rules check

# Dry run (show what would be fixed)
meta rules check --fix --dry-run
```

### Rule Validation

```bash
# Validate rules configuration
meta rules validate

# Check specific rule file
meta rules validate --file custom-rules.yaml
```

## Best Practices

1. **Start Simple**: Begin with basic directory rules and expand gradually
2. **Project-Specific**: Use project rules for unique requirements
3. **Document Rules**: Always include descriptions for clarity
4. **Regular Checks**: Integrate rules checking into CI/CD
5. **Progressive Enhancement**: Start with warnings, upgrade to errors
6. **Team Agreement**: Discuss and agree on rules as a team
7. **Version Control**: Track rules files in git
8. **Regular Reviews**: Periodically review and update rules

## Advanced Features

### Custom Validators

Coming soon: Support for custom validation scripts

```yaml
custom:
  - name: "license-check"
    script: "./scripts/check-license.sh"
    pattern: "**/*.rs"
```

### Rule Inheritance

Coming soon: Extend and override base rules

```yaml
extends: "../base-rules.yaml"
overrides:
  directories:
    - path: src
      required: false
```

### Watch Mode

Coming soon: Real-time validation during development

```bash
meta rules watch --project frontend
```

## Contributing

To contribute to the rules system:

1. Check existing issues and discussions
2. Propose new rule types or features
3. Submit PRs with tests and documentation
4. Follow the project's contribution guidelines

## Support

- **Documentation**: This file and `meta rules docs`
- **Issues**: [GitHub Issues](https://github.com/codyaverett/metarepo/issues)
- **Discussions**: [GitHub Discussions](https://github.com/codyaverett/metarepo/discussions)
- **Examples**: See `examples/` directory in the repository