#!/bin/bash
# Deploy script — references ts-ui project
set -e
cd "$(dirname "$0")/.."
echo "Building ts-ui..."
cd ts-ui && npm run build
