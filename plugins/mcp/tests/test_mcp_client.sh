#!/bin/bash

# Test script for MCP client functionality
# Run from the project root: ./plugins/mcp/tests/test_mcp_client.sh

echo "Testing MCP Client Plugin"
echo "========================="

# Navigate to project root
cd "$(dirname "$0")/../../.." || exit 1

echo -e "\n1. Testing filesystem MCP server connection"
cargo run -- mcp connect npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\n2. Listing available resources"
cargo run -- mcp list-resources npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\n3. Listing available tools"
cargo run -- mcp list-tools npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\n4. Testing server management (start/stop)"
echo "Starting filesystem server as background process..."
cargo run -- mcp start test-fs npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\nChecking server status..."
cargo run -- mcp status

echo -e "\nViewing server logs..."
cargo run -- mcp logs test-fs -n 10

echo -e "\nStopping server..."
cargo run -- mcp stop test-fs

echo -e "\nTest complete!"