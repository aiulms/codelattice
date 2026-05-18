#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_MODE="${BUILD_MODE:-debug}"

source "${ROOT_DIR}/scripts/common.sh"

build_project() {
  log_info "building in ${BUILD_MODE} mode"
  prepare_dist
  cargo build
  ./scripts/test.sh
}

prepare_dist() {
  mkdir -p "${ROOT_DIR}/dist"
}

build_project "$@"
