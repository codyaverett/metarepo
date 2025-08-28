# Gestalt MCP Server Setup

The Gestalt MCP server exposes all Gestalt CLI functionality as MCP tools, allowing AI assistants like Claude to interact with your development environment.

## Available Tools

The Gestalt MCP server exposes the following tools:

### General
- `help` - Get help and list available commands

### Git Operations
- `git_status` - Show git status for all repositories
- `git_diff` - Show git diff across repositories
- `git_commit` - Commit changes across repositories
- `git_pull` - Pull changes from remote repositories
- `git_push` - Push changes to remote repositories

### Project Management
- `project_list` - List all projects in the workspace
- `project_add` - Add a new project to the workspace
- `project_remove` - Remove a project from the workspace

### Command Execution
- `exec` - Execute a command across multiple projects

### MCP Management
- `mcp_add_server` - Add an MCP server configuration
- `mcp_list_servers` - List configured MCP servers
- `mcp_remove_server` - Remove an MCP server configuration

## Testing the Server

Test the server manually:
```bash
# Start the server
gest mcp serve

# In another terminal, send JSON-RPC commands:
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | gest mcp serve

# List available tools
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | gest mcp serve
```

## Claude Desktop Configuration

Add to your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "gestalt": {
      "command": "/path/to/gest",
      "args": ["mcp", "serve"],
      "env": {}
    }
  }
}
```

Replace `/path/to/gest` with the full path to your Gestalt binary. You can find this with:
```bash
which gest
```

## VS Code Configuration

Add to your VS Code `settings.json`:

```json
{
  "mcp.servers": {
    "gestalt": {
      "command": "/path/to/gest",
      "args": ["mcp", "serve"],
      "name": "Gestalt Multi-Project Manager",
      "description": "MCP server exposing Gestalt CLI tools for git, project, and execution management"
    }
  }
}
```

## Usage Examples

Once configured, Claude or other MCP clients can use Gestalt tools:

```
"Show me the git status of all my projects"
"Add /Users/me/new-project to the workspace"
"Commit all changes with message 'feat: add new feature'"
"Execute 'npm test' across all projects"
```

## Troubleshooting

1. **Server not starting**: Ensure the Gestalt binary path is correct and executable
2. **Tools not appearing**: Check that the MCP client properly initialized the connection
3. **Commands failing**: Verify that Gestalt has access to the directories and git repositories

## Development

To add new tools to the MCP server:

1. Edit `plugins/mcp/src/mcp_server.rs`
2. Add tool definition in `build_tools()`
3. Add execution logic in `execute_tool()`
4. Rebuild: `cargo build --release`