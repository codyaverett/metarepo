#!/bin/bash
(
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{"roots":{"listChanged":true},"sampling":{}},"clientInfo":{"name":"test","version":"0.1.0"}}}'
sleep 0.2
echo '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}'
sleep 0.2
# Respond to roots/list request
echo '{"jsonrpc":"2.0","id":0,"result":{"roots":[]}}'
sleep 0.2
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
sleep 0.5
) | npx @playwright/mcp@latest 2>&1
