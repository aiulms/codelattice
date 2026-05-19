#!/bin/bash
# Build script — references rust-core project
set -e
cd "$(dirname "$0")/.."
echo "Building core-lib..."
cd rust-core && cargo build --release
