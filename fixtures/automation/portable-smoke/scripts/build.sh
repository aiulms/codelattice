#!/usr/bin/env bash
set -euo pipefail

cargo test
rm -rf dist
