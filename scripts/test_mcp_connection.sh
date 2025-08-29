#!/bin/bash

echo "Testing Gestalt MCP Server..."
echo ""

# Test 1: Initialize
echo "Test 1: Initialize"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | \
    /Users/caavere/.cargo/bin/gest mcp serve 2>/dev/null | \
    jq -r '.result.name'

echo ""

# Test 2: List tools
echo "Test 2: Counting tools"
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | \
    /Users/caavere/.cargo/bin/gest mcp serve 2>/dev/null | \
    tail -1 | jq '.result.tools | length'

echo ""

# Test 3: Call help tool
echo "Test 3: Call help tool"
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"help","arguments":{}}}\n' | \
    /Users/caavere/.cargo/bin/gest mcp serve 2>/dev/null | \
    tail -1 | jq -r '.result.content[0].text' | head -5

echo ""
echo "If you see 'gestalt-mcp-server', '13', and help output above, the server is working correctly."
echo ""
echo "The 'Server disconnected' messages in Claude Desktop are NORMAL."
echo "Claude connects only when it needs to use a tool."
echo ""
echo "To test in Claude Desktop, try asking:"
echo "  - 'Can you use the gestalt help tool?'"
echo "  - 'Show me the git status using gestalt'"
echo "  - 'List the available gestalt tools'"