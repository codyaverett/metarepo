#!/bin/bash

# Test script to verify Claude MCP connection
echo "Testing Gestalt MCP Server for Claude Desktop compatibility..."

# Test 1: Initialize connection
echo "=== Test 1: Initialize Connection ==="
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"roots":{"listChanged":true},"sampling":{}},"clientInfo":{"name":"claude-desktop","version":"0.9.0"}}}' | gest mcp serve

echo -e "\n=== Test 2: List Tools ==="
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"roots":{"listChanged":true},"sampling":{}},"clientInfo":{"name":"claude-desktop","version":"0.9.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | gest mcp serve

echo -e "\n=== Test 3: Call Help Tool ==="
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"roots":{"listChanged":true},"sampling":{}},"clientInfo":{"name":"claude-desktop","version":"0.9.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"help","arguments":{}}}\n' | gest mcp serve

echo -e "\n=== Configuration Check ==="
echo "Claude Desktop config should point to: $(which gest)"
echo "Current config location: ~/Library/Application Support/Claude/claude_desktop_config.json"