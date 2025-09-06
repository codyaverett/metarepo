# Nested Meta Repositories

## Overview

The project plugin supports importing and managing nested meta repositories - projects that are themselves meta repositories containing their own `.meta` files and sub-projects. This feature enables hierarchical organization of complex multi-repository projects while preventing issues like infinite recursion and circular dependencies.

## Features

### Automatic Nested Repository Detection
When importing a project, the system automatically detects if it contains a `.meta` file, identifying it as a meta repository that may have its own sub-projects.

### Cycle Detection
Built-in cycle detection prevents infinite recursion when meta repositories reference each other in circular patterns. The system maintains a dependency graph and visited set to detect and report cycles before they cause issues.

### Configurable Recursion Depth
Control how deep the automatic import process goes with configurable depth limits, preventing runaway recursion and managing resource usage.

### Flexible Import Strategies
Choose between:
- **Recursive Import**: Automatically import all nested projects
- **Shallow Import**: Only import the top-level project
- **Flattened Import**: Import all nested projects at the root level
- **Selective Import**: Choose which nested projects to import

## Configuration

### Meta File Configuration

Add these options to your `.meta` file to control nested repository behavior:

```json
{
  "projects": {
    "my-project": "https://github.com/user/repo.git"
  },
  "nested": {
    "recursive_import": true,
    "max_depth": 3,
    "flatten": false,
    "cycle_detection": true,
    "ignore_nested": ["specific-project"]
  }
}
```

#### Configuration Options

- `recursive_import` (boolean, default: false): Enable automatic importing of nested projects
- `max_depth` (integer, default: 3): Maximum nesting depth for recursive imports
- `flatten` (boolean, default: false): Import nested projects at root level instead of maintaining hierarchy
- `cycle_detection` (boolean, default: true): Enable circular dependency detection
- `ignore_nested` (array, default: []): List of nested project names to skip during recursive import

### Command-Line Options

Control nested repository behavior with CLI flags:

```bash
# Import with recursive processing of nested repos
meta project import my-project https://github.com/user/meta-repo.git --recursive

# Limit recursion depth
meta project import my-project repo-url --recursive --max-depth 2

# Import with flattening (all nested projects at root level)
meta project import my-project repo-url --recursive --flatten

# Skip recursive import even if configured
meta project import my-project repo-url --no-recursive

# Show nested repository tree structure
meta project tree

# List projects with hierarchy
meta project list --tree
```

## Use Cases

### 1. Microservices Architecture
Organize microservices with a top-level meta repository that references service-specific meta repositories:

```
company-platform/           # Top-level meta repo
├── .meta
├── frontend/              # Meta repo for all frontend apps
│   ├── .meta
│   ├── web-app/
│   ├── mobile-app/
│   └── admin-panel/
├── backend/               # Meta repo for all backend services
│   ├── .meta
│   ├── auth-service/
│   ├── payment-service/
│   └── notification-service/
└── infrastructure/        # Meta repo for infrastructure code
    ├── .meta
    ├── terraform/
    └── kubernetes/
```

### 2. Multi-Team Projects
Each team maintains their own meta repository while still being part of a larger project:

```
product-suite/
├── .meta
├── team-alpha/           # Team Alpha's meta repo
│   ├── .meta
│   └── [team projects]
├── team-beta/            # Team Beta's meta repo
│   ├── .meta
│   └── [team projects]
└── shared-libraries/     # Shared components meta repo
    ├── .meta
    └── [shared projects]
```

### 3. Versioned Dependencies
Manage different versions of the same project structure:

```
platform/
├── .meta
├── v1/                   # Version 1 meta repo
│   ├── .meta
│   └── [v1 projects]
├── v2/                   # Version 2 meta repo
│   ├── .meta
│   └── [v2 projects]
└── common/               # Common components
    ├── .meta
    └── [shared projects]
```

## Safety Features

### Cycle Detection Algorithm
The system uses a depth-first search algorithm with a visited set to detect cycles:

1. Before importing a project, check if it's already in the current import chain
2. If found, report the cycle and abort the operation
3. Maintain a global visited set across the entire import session
4. Provide clear error messages showing the cycle path

Example cycle detection output:
```
Error: Circular dependency detected!
  product-suite → team-alpha → shared-lib → product-suite
  
Cycle path:
  1. product-suite imports team-alpha
  2. team-alpha imports shared-lib
  3. shared-lib imports product-suite (CYCLE!)
```

### Depth Limiting
Prevents runaway recursion with configurable depth limits:

- Default maximum depth: 3 levels
- Configurable per-repository or globally
- Clear reporting when depth limit is reached
- Option to override for specific imports

### Safe Import Process
The import process follows these safety steps:

1. **Pre-flight Check**: Analyze the entire import tree before making changes
2. **Cycle Detection**: Check for circular dependencies
3. **Conflict Detection**: Identify naming conflicts before import
4. **Dry Run Option**: Preview what would be imported without making changes
5. **Rollback Support**: Ability to undo failed partial imports

## Advanced Features

### Namespace Management
Nested projects can be namespaced to avoid conflicts:

```json
{
  "nested": {
    "namespace_separator": "/",
    "preserve_structure": true
  }
}
```

This creates project names like `frontend/web-app` instead of just `web-app`.

### Selective Sync
Choose which nested repositories to work with:

```bash
# Only import specific nested projects
meta project import my-project repo-url --recursive --only frontend,backend

# Exclude specific nested projects
meta project import my-project repo-url --recursive --exclude tests,docs
```

### Update Strategies
Different strategies for updating nested repositories:

```bash
# Update all nested repositories
meta project update --recursive

# Update only direct children
meta project update --depth 1

# Update specific branch of nested repos
meta project update --recursive --branch develop
```

## Implementation Details

### Dependency Graph
The system maintains an in-memory dependency graph during import operations:

- Nodes represent repositories
- Edges represent import relationships
- Graph is checked for cycles before each new import
- Graph is used to determine import order

### Import Queue
Repositories are processed using a breadth-first queue:

1. Add root repository to queue
2. Process repository (clone/import)
3. If it's a meta repo and recursive is enabled:
   - Parse its `.meta` file
   - Add child projects to queue (if not already visited)
4. Continue until queue is empty or depth limit reached

### State Management
Import state is tracked throughout the operation:

- Visited repositories set
- Current depth counter
- Import chain for cycle detection
- Success/failure status per repository

## Troubleshooting

### Common Issues

#### "Maximum recursion depth exceeded"
- Check your depth configuration
- Look for unexpected meta repositories
- Use `--max-depth` flag to override

#### "Circular dependency detected"
- Review the reported cycle path
- Restructure repositories to break the cycle
- Use `--no-recursive` for specific imports

#### "Naming conflict detected"
- Use namespacing to avoid conflicts
- Rename conflicting projects
- Use `--flatten=false` to maintain hierarchy

### Debug Mode
Enable detailed logging for troubleshooting:

```bash
# Verbose output for import operations
RUST_LOG=debug meta project import my-project repo-url --recursive

# Dry run to preview import
meta project import my-project repo-url --recursive --dry-run
```

## Best Practices

1. **Limit Nesting Depth**: Keep hierarchies shallow (2-3 levels) for maintainability
2. **Avoid Circular Dependencies**: Design repository relationships as a directed acyclic graph
3. **Use Namespacing**: Enable namespacing for large projects to avoid conflicts
4. **Document Structure**: Maintain a README in meta repositories explaining the structure
5. **Test Imports**: Use dry-run mode before importing large repository trees
6. **Version Control**: Tag meta repository states for reproducible imports
7. **Regular Maintenance**: Periodically review and clean up repository relationships

## Migration Guide

### From Flat Structure to Nested
1. Identify logical groupings of repositories
2. Create intermediate meta repositories for each group
3. Update the root `.meta` file to reference group repositories
4. Test with `--dry-run` before actual migration
5. Use `meta project tree` to verify structure

### From Nested to Flat
1. Use `--flatten` flag during import
2. Update project references in code
3. Remove intermediate meta repositories
4. Update CI/CD configurations

## Future Enhancements

- Parallel import processing for better performance
- Repository template support for nested structures
- Visual graph representation of repository relationships
- Automated conflict resolution strategies
- Integration with CI/CD for nested repository management