# MCP Plugin Tests

This directory contains test scripts for the MCP (Model Context Protocol) plugin functionality.

## Test Scripts

### `test_mcp_client.sh`
Tests the MCP client functionality - connecting to external MCP servers, listing resources/tools, and managing server processes.

```bash
# Run from project root
./plugins/mcp/tests/test_mcp_client.sh
```

### `test_mcp_server.sh`  
Tests Metarepo running as an MCP server, exposing plugin functionality as MCP tools.

```bash
# Run from project root
./plugins/mcp/tests/test_mcp_server.sh
```

## Manual Testing

### Test MCP Client Commands
```bash
# Connect to a filesystem server
cargo run -- mcp connect npx -y @modelcontextprotocol/server-filesystem /path

# List available tools
cargo run -- mcp list-tools npx -y @modelcontextprotocol/server-filesystem /path

# Call a tool
cargo run -- mcp call-tool npx -y @modelcontextprotocol/server-filesystem /path read_file --args '{"path": "README.md"}'
```

### Test MCP Server Mode
```bash
# Get configuration for VS Code
cargo run -- mcp config

# Run as MCP server (for testing with JSON-RPC)
cargo run -- mcp serve

# Send test commands via stdin
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run -- mcp serve
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run -- mcp serve
```

## Integration Testing

To test with real MCP clients:

1. **VS Code**: Add configuration from `cargo run -- mcp config` to settings.json
2. **Claude Desktop**: Add configuration to claude_desktop_config.json
3. **Custom Client**: Connect via stdio to `meta mcp serve`

## Common MCP Servers for Testing

- `@modelcontextprotocol/server-filesystem` - File operations
- `@modelcontextprotocol/server-git` - Git operations
- `@modelcontextprotocol/server-memory` - Persistent memory
- `@modelcontextprotocol/server-sqlite` - SQLite database
- `@modelcontextprotocol/server-fetch` - Web content fetching