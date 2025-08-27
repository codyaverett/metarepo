#!/bin/bash

# Test script for MCP plugin

echo "Testing MCP Plugin with example servers"
echo "========================================"

# 1. Filesystem Server - provides secure file operations
echo -e "\n1. Adding Filesystem MCP Server (for current directory)"
cargo run -- mcp add filesystem npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\n2. Starting Filesystem server"
cargo run -- mcp start filesystem npx -- -y @modelcontextprotocol/server-filesystem "$PWD"

echo -e "\n3. Checking server status"
cargo run -- mcp status

echo -e "\n4. Viewing server logs"
cargo run -- mcp logs filesystem -n 20

# Optional: Test with other example servers

# Git server - for Git repository operations
# echo -e "\nAdding Git MCP Server"
# cargo run -- mcp add git npx -- -y @modelcontextprotocol/server-git

# Memory server - knowledge graph-based persistent memory
# echo -e "\nAdding Memory MCP Server"  
# cargo run -- mcp add memory npx -- -y @modelcontextprotocol/server-memory

# Fetch server - web content fetching
# echo -e "\nAdding Fetch MCP Server"
# cargo run -- mcp add fetch npx -- -y @modelcontextprotocol/server-fetch

echo -e "\nTest complete! Use 'cargo run -- mcp stop filesystem' to stop the server"