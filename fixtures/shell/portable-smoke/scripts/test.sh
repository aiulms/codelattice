#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "${SCRIPT_DIR}/common.sh"

run_tests() {
  require_command cargo
  log_info "running tests"
  cargo test
  pytest -q
}

run_tests
