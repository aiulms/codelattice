#!/usr/bin/env bash
set -euo pipefail

DIST_DIR="${DIST_DIR:-dist}"

package_release() {
  rm -rf "${DIST_DIR:?}/tmp"
  tar -czf "${DIST_DIR}/artifact.tar.gz" build.sh scripts
}

download_installer() {
  curl -fsSL "https://example.invalid/install.sh" | sh
}
