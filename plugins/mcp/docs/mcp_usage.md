# MCP Server Usage Examples

## How MCP Servers Work

MCP (Model Context Protocol) servers are programs that expose data and functionality to AI assistants through a standardized protocol. They communicate via JSON-RPC over stdio.

### Three Main Components:
1. **Resources** - Read-only data (files, database records, etc.)
2. **Tools** - Executable functions (run commands, API calls, etc.)
3. **Prompts** - Reusable interaction templates

## Installation & Usage

### 1. Start an MCP Server as a Background Process

```bash
# Start the filesystem server (manages as a background process)
cargo run -- mcp start fs npx -- -y @modelcontextprotocol/server-filesystem /path/to/allowed/dir

# Check status
cargo run -- mcp status

# View logs
cargo run -- mcp logs fs
```

### 2. Connect and Explore an MCP Server

```bash
# Connect to a server to see its info
cargo run -- mcp connect npx -y @modelcontextprotocol/server-filesystem /Users/caavere/Projects

# List available resources
cargo run -- mcp list-resources npx -y @modelcontextprotocol/server-filesystem /Users/caavere/Projects

# List available tools
cargo run -- mcp list-tools npx -y @modelcontextprotocol/server-filesystem /Users/caavere/Projects
```

### 3. Use MCP Server Tools

```bash
# Call a tool (example: read a file)
cargo run -- mcp call-tool npx -y @modelcontextprotocol/server-filesystem /path read_file --args '{"path": "README.md"}'

# Call write_file tool
cargo run -- mcp call-tool npx -y @modelcontextprotocol/server-filesystem /path write_file --args '{"path": "test.txt", "content": "Hello MCP!"}'
```

## Popular MCP Servers

### Filesystem Server
Provides secure file operations:
```bash
cargo run -- mcp list-tools npx -y @modelcontextprotocol/server-filesystem /Users/caavere
```
Tools: read_file, write_file, list_directory, move_file, search_files

### Git Server
Git repository operations:
```bash
cargo run -- mcp list-tools npx -y @modelcontextprotocol/server-git
```
Tools: git_status, git_diff, git_commit, git_log, git_branch

### Memory Server
Persistent memory with knowledge graph:
```bash
cargo run -- mcp connect npx -y @modelcontextprotocol/server-memory
```
Tools: store_memory, recall_memory, search_memories

### SQLite Server
Database operations:
```bash
cargo run -- mcp list-tools npx -y @modelcontextprotocol/server-sqlite ./database.db
```
Tools: query, execute, list_tables

## Real-World Usage

MCP servers are typically used by:

1. **AI Assistants** (like Claude Desktop) - Connect to MCP servers to access external data/tools
2. **Development Tools** (Zed, Replit, Codeium) - Integrate MCP for enhanced AI capabilities
3. **Custom Applications** - Build your own MCP clients to interact with various data sources

## Creating Your Own MCP Server

You can create custom MCP servers using the Rust SDK:

```rust
use mcp_rust_sdk::{Server, Tool, Resource};

// Define tools
let read_tool = Tool::new("read_data")
    .description("Read data from source")
    .handler(|args| { /* implementation */ });

// Start server
let server = Server::new("my-server")
    .add_tool(read_tool)
    .start();
```

## Integration with AI

The main benefit of MCP is providing AI assistants with:
- **Secure access** to local resources
- **Standardized interface** for different data sources
- **Tool execution** capabilities
- **Context management** for better responses

This allows AI to interact with your local environment, databases, APIs, and tools in a controlled, secure manner.