#!/usr/bin/env sh

: "${LOG_PREFIX:=CodeLattice}"

log_info() {
  printf '%s: %s\n' "${LOG_PREFIX}" "$1"
}

require_command() {
  command -v "$1" >/dev/null 2>&1
}
