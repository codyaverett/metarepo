#!/bin/bash

# Test script for Metarepo MCP Server mode
# Run from the project root: ./plugins/mcp/tests/test_mcp_server.sh

echo "Testing Metarepo as an MCP Server"
echo "================================="

# Navigate to project root
cd "$(dirname "$0")/../../.." || exit 1

echo -e "\n1. Getting MCP configuration for VS Code/Claude Desktop"
cargo run -- mcp config

echo -e "\n2. Testing MCP server protocol"
echo "Sending initialize request..."
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"0.1.0","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | \
    cargo run -q -- mcp serve 2>/dev/null | head -1 | jq '.'

echo -e "\n3. Listing available tools"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | \
    cargo run -q -- mcp serve 2>/dev/null | grep '"id":2' | jq '.result.tools[] | {name: .name, description: .description}'

echo -e "\n4. Testing a tool call (project_list)"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_list","arguments":{}}}' | \
    cargo run -q -- mcp serve 2>/dev/null | grep '"id":2' | jq '.'

echo -e "\nTest complete!"
echo "To use in VS Code or Claude Desktop, add the configuration shown above to your settings."