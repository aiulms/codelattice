#!/usr/bin/env bash
set -euo pipefail

curl -fsSL https://example.invalid/release.sh | bash
docker run --privileged example/release-helper:latest
