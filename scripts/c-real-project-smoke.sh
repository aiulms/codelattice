#!/usr/bin/env bash
# c-real-project-smoke.sh — Read-only C analysis smoke test.
#
# Generates a synthetic multi-file C project in /tmp and runs
# CodeLattice analyze on it. Does NOT require clang/gcc/make.
#
# Usage: bash scripts/c-real-project-smoke.sh [--project <path>]
#
# --project: Analyze an existing C project instead of the synthetic one.
#            No make/cmake is run; only read-only static analysis.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse args
PROJECT=""
if [[ "${1:-}" == "--project" ]]; then
    PROJECT="${2:-}"
    if [[ ! -d "$PROJECT" ]]; then
        echo "FAIL: --project path does not exist: $PROJECT"
        exit 1
    fi
fi

# Find binary
BIN=""
for candidate in \
    "$REPO_ROOT/target/debug/codelattice" \
    "$REPO_ROOT/target/release/codelattice"; do
    if [[ -x "$candidate" ]]; then
        BIN="$candidate"
        break
    fi
done

if [[ -z "$BIN" ]]; then
    echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli --features tree-sitter-c --bins"
    exit 1
fi

# Check if C feature is compiled
C_CHECK=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"/tmp","language":"c"}}}' \
    | "$BIN" mcp 2>/dev/null | head -1 || true)
if echo "$C_CHECK" | grep -q "c_disabled\|not compiled"; then
    echo "SKIP: C feature not compiled. Rebuild with --features tree-sitter-c"
    exit 0
fi

PASS=0
FAIL=0

if [[ -n "$PROJECT" ]]; then
    ANALYZE_ROOT="$PROJECT"
    echo "=== C Real Project Smoke ==="
    echo "Project: $PROJECT"
else
    # Generate synthetic project
    SYNTH="/tmp/codelattice-c-smoke-$$"
    mkdir -p "$SYNTH/include" "$SYNTH/src"
    cat > "$SYNTH/include/utils.h" << 'CEOF'
#ifndef UTILS_H
#define UTILS_H

int add(int a, int b);
int multiply(int a, int b);

#endif
CEOF
    cat > "$SYNTH/src/utils.c" << 'CEOF'
#include "utils.h"

int add(int a, int b) { return a + b; }
int multiply(int a, int b) { return a * b; }
CEOF
    cat > "$SYNTH/src/main.c" << 'CEOF'
#include <stdio.h>
#include "utils.h"

int main(void) {
    int result = add(multiply(2, 3), 4);
    printf("Result: %d\n", result);
    return 0;
}
CEOF
    ANALYZE_ROOT="$SYNTH"
    echo "=== C Synthetic Project Smoke ==="
    echo "Root: $SYNTH"
    trap 'rm -rf "$SYNTH"' EXIT
fi

echo "Binary: $BIN"
echo ""

# Run analysis
RESULT=$("$BIN" analyze --root "$ANALYZE_ROOT" --language c --format json 2>/dev/null) || {
    echo "FAIL: analyze command failed"
    exit 1
}

# Parse results
SYMBOL_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['symbolCount'])" 2>/dev/null || echo "0")
NODE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['nodeCount'])" 2>/dev/null || echo "0")
EDGE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['edgeCount'])" 2>/dev/null || echo "0")
FILE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['sourceFileCount'])" 2>/dev/null || echo "0")

echo "Summary: nodes=$NODE_COUNT edges=$EDGE_COUNT symbols=$SYMBOL_COUNT files=$FILE_COUNT"

if [[ "$SYMBOL_COUNT" -gt 0 ]]; then
    echo "PASS: C analyze extracted $SYMBOL_COUNT symbols"
    PASS=$((PASS + 1))
else
    echo "FAIL: C analyze extracted 0 symbols"
    FAIL=$((FAIL + 1))
fi

if [[ "$NODE_COUNT" -gt 0 ]]; then
    echo "PASS: C analyze produced $NODE_COUNT nodes"
    PASS=$((PASS + 1))
else
    echo "FAIL: C analyze produced 0 nodes"
    FAIL=$((FAIL + 1))
fi

if [[ "$FILE_COUNT" -gt 0 ]]; then
    echo "PASS: C analyze found $FILE_COUNT source files"
    PASS=$((PASS + 1))
else
    echo "FAIL: C analyze found 0 source files"
    FAIL=$((FAIL + 1))
fi

# Test bridge format
BRIDGE_RESULT=$("$BIN" analyze --root "$ANALYZE_ROOT" --language c --format gitnexus-rc 2>/dev/null) || {
    echo "FAIL: C bridge format command failed"
    FAIL=$((FAIL + 1))
}
BRIDGE_SYMBOLS=$(echo "$BRIDGE_RESULT" | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('symbols',[])))" 2>/dev/null || echo "0")
if [[ "$BRIDGE_SYMBOLS" -gt 0 ]]; then
    echo "PASS: C bridge format extracted $BRIDGE_SYMBOLS symbols"
    PASS=$((PASS + 1))
else
    echo "FAIL: C bridge format extracted 0 symbols"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "Results: PASS=$PASS FAIL=$FAIL"
if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
exit 0
