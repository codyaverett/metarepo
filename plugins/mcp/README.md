# MCP (Model Context Protocol) Plugin

A comprehensive MCP implementation for Gestalt that provides both client and server functionality.

## Features

### MCP Client
- Connect to and interact with any MCP server
- List resources, tools, and prompts from MCP servers
- Execute tools on remote MCP servers
- Manage MCP server processes (start, stop, restart)

### MCP Server
- Run Gestalt as an MCP server
- Expose all Gestalt plugins as MCP tools
- Compatible with VS Code, Claude Desktop, and other MCP clients
- Full JSON-RPC protocol implementation

## Installation

The MCP plugin is included in the Gestalt workspace. Build it with:

```bash
cargo build
```

## Usage

### Client Mode - Interacting with MCP Servers

```bash
# Connect to an MCP server
gest mcp connect npx -y @modelcontextprotocol/server-filesystem /path

# List available tools
gest mcp list-tools npx -y @modelcontextprotocol/server-filesystem /path

# Call a tool
gest mcp call-tool npx -y @modelcontextprotocol/server-filesystem /path read_file --args '{"path": "file.txt"}'

# Manage server processes
gest mcp start myserver npx -y @modelcontextprotocol/server-git
gest mcp status
gest mcp stop myserver
```

### Server Mode - Running Gestalt as an MCP Server

```bash
# Get configuration for VS Code or Claude Desktop
gest mcp config

# Run as MCP server
gest mcp serve
```

## Available Tools (Server Mode)

When running as an MCP server, Gestalt exposes:

- **Git Tools**: `git_status`, `git_diff`, `git_commit`, `git_pull`, `git_push`
- **Project Tools**: `project_list`, `project_add`, `project_remove`
- **Execution Tools**: `exec` (run commands across projects)
- **MCP Tools**: `mcp_server_start`, `mcp_server_stop`, `mcp_server_status`

## Configuration

### VS Code
Add to `settings.json`:
```json
{
  "mcpServers": {
    "gestalt": {
      "command": "gest",
      "args": ["mcp", "serve"]
    }
  }
}
```

### Claude Desktop
Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "gestalt": {
      "command": "/path/to/gest",
      "args": ["mcp", "serve"]
    }
  }
}
```

## Testing

Run the test scripts from the project root:

```bash
# Test client functionality
./plugins/mcp/tests/test_mcp_client.sh

# Test server functionality
./plugins/mcp/tests/test_mcp_server.sh
```

## Documentation

- [MCP Usage Guide](mcp_usage.md) - Detailed usage examples
- [Server Documentation](docs/MCP_SERVER_USAGE.md) - Running Gestalt as an MCP server
- [Test Documentation](tests/README.md) - Testing guide

## Architecture

```
plugins/mcp/
├── src/
│   ├── lib.rs           # Module exports
│   ├── plugin.rs        # CLI plugin interface
│   ├── client.rs        # MCP client implementation
│   ├── server.rs        # Process management for MCP servers
│   └── mcp_server.rs    # Gestalt as MCP server
├── tests/               # Test scripts
├── docs/                # Documentation
└── Cargo.toml
```

## Common MCP Servers

Popular MCP servers you can connect to:

- `@modelcontextprotocol/server-filesystem` - File operations
- `@modelcontextprotocol/server-git` - Git repository management
- `@modelcontextprotocol/server-memory` - Persistent memory storage
- `@modelcontextprotocol/server-sqlite` - SQLite database operations
- `@modelcontextprotocol/server-fetch` - Web content fetching

## Contributing

When adding new Gestalt plugins, consider exposing their functionality through the MCP server by updating `mcp_server.rs`.