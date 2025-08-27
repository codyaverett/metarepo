# Gestalt as an MCP Server

Gestalt can run as an MCP (Model Context Protocol) server, exposing all its plugin functionality as MCP tools that can be used by AI assistants like Claude Desktop, VS Code Copilot, and other MCP-compatible clients.

## Features

When running as an MCP server, Gestalt exposes the following tools:

### Git Tools
- `git_status` - Show git status for all repositories
- `git_diff` - Show git diff across repositories
- `git_commit` - Commit changes across repositories
- `git_pull` - Pull changes from remote repositories
- `git_push` - Push changes to remote repositories

### Project Management Tools
- `project_list` - List all projects in the workspace
- `project_add` - Add a new project to the workspace
- `project_remove` - Remove a project from the workspace

### Execution Tools
- `exec` - Execute a command across multiple projects

### MCP Management Tools
- `mcp_server_start` - Start an MCP server
- `mcp_server_stop` - Stop an MCP server
- `mcp_server_status` - Get status of MCP servers

## Configuration

### 1. Get the Configuration

Run this command to get the configuration for your MCP client:

```bash
cargo run -- mcp config
# or if installed:
gest mcp config
```

### 2. For VS Code / Claude Desktop

Add the output to your configuration file:

**VS Code** (`settings.json`):
```json
{
  "mcpServers": {
    "gestalt": {
      "command": "gest",
      "args": ["mcp", "serve"],
      "name": "Gestalt Multi-Project Manager",
      "description": "MCP server exposing Gestalt CLI tools"
    }
  }
}
```

**Claude Desktop** (`claude_desktop_config.json`):
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

### 3. Run as Standalone Server

You can also run Gestalt as a standalone MCP server:

```bash
# Run the server (it will listen on stdin/stdout)
cargo run -- mcp serve

# Or if installed globally:
gest mcp serve
```

## Testing the Server

You can test the MCP server using JSON-RPC commands:

```bash
# Initialize connection
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | gest mcp serve

# List available tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | gest mcp serve

# Call a tool (example: list projects)
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"project_list","arguments":{}}}' | gest mcp serve
```

## Use Cases

### 1. AI-Assisted Multi-Repository Management

With Gestalt running as an MCP server, AI assistants can:
- Check the status of all your git repositories
- Commit changes across multiple projects with consistent messages
- Execute commands across your entire workspace
- Manage project dependencies

### 2. Automated Workflows

MCP clients can use Gestalt tools to:
- Run tests across all projects
- Update dependencies systematically
- Apply patches or fixes to multiple repositories
- Generate reports on project status

### 3. Integration with Development Tools

VS Code and other editors can use Gestalt's MCP interface to:
- Provide context about multiple projects to AI assistants
- Enable AI to make changes across repository boundaries
- Coordinate complex refactoring operations

## Example Workflow

Here's how an AI assistant might use Gestalt's MCP tools:

1. **Check Status**: Call `git_status` to see what's changed
2. **Review Changes**: Call `git_diff` to understand modifications
3. **Run Tests**: Call `exec` with `{"command": "npm test"}` 
4. **Commit**: Call `git_commit` with a descriptive message
5. **Push**: Call `git_push` to share changes

## Security Considerations

- The MCP server runs with the same permissions as the user
- It can only access directories and files the user can access
- All operations are logged to stderr for auditing
- Consider running in a restricted environment if exposing to untrusted clients

## Troubleshooting

If the server isn't working:

1. Check that Gestalt is properly installed: `gest --version`
2. Verify the path in your MCP client configuration
3. Test with simple JSON-RPC commands first
4. Check stderr output for error messages
5. Ensure all required plugins are compiled

## Advanced Usage

You can extend the MCP server by:
1. Adding new tools in `mcp_server.rs`
2. Exposing additional plugin functionality
3. Adding resource providers for project data
4. Implementing custom prompts for common workflows