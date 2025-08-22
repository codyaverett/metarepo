#!/bin/bash

# Test script for meta functionality
set -e

echo "Testing meta binary..."

# Test help
echo "1. Testing help command:"
cargo run --bin meta -- --help

echo -e "\n2. Testing version:"
cargo run --bin meta -- --version

echo -e "\n3. Testing init command:"
cargo run --bin meta -- init

echo -e "\n4. Testing exec command:"
cargo run --bin meta -- exec "echo hello"

echo -e "\n5. Testing git command:"
cargo run --bin meta -- git

echo -e "\nAll tests completed successfully!"