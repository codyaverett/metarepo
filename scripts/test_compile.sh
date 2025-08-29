#!/bin/bash
cd /Users/caavere/Projects/gestalt
cargo build --package gestalt-rules 2>&1 | head -200