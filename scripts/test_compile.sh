#!/bin/bash
cd /Users/caavere/Projects/metarepo
cargo build --package metarepo-rules 2>&1 | head -200