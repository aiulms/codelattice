#!/usr/bin/env bash
# cpp-real-project-smoke.sh — Read-only C++ analysis smoke test.
#
# Generates a synthetic multi-file C++ project in /tmp and runs
# CodeLattice analyze on it. Does NOT require clang/gcc/make/cmake.
#
# Usage: bash scripts/cpp-real-project-smoke.sh [--project <path>]
#
# --project: Analyze an existing C++ project instead of the synthetic one.
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
    echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli --features tree-sitter-cpp --bins"
    exit 1
fi

# Check if C++ feature is compiled
CPP_CHECK=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"/tmp","language":"cpp"}}}' \
    | "$BIN" mcp 2>/dev/null | head -1 || true)
if echo "$CPP_CHECK" | grep -q "cpp_disabled\|not compiled"; then
    echo "SKIP: C++ feature not compiled. Rebuild with --features tree-sitter-cpp"
    exit 0
fi

PASS=0
FAIL=0

if [[ -n "$PROJECT" ]]; then
    ANALYZE_ROOT="$PROJECT"
    echo "=== C++ Real Project Smoke ==="
    echo "Project: $PROJECT"
else
    # Generate synthetic project
    SYNTH="/tmp/codelattice-cpp-smoke-$$"
    mkdir -p "$SYNTH/include" "$SYNTH/src"
    cat > "$SYNTH/include/math_utils.hpp" << 'CPPEOF'
#ifndef MATH_UTILS_HPP
#define MATH_UTILS_HPP

namespace math {
    int add(int a, int b);
    int multiply(int a, int b);
    class Calculator {
    public:
        static int compute(int x, int y);
    };
}

#endif
CPPEOF
    cat > "$SYNTH/src/math_utils.cpp" << 'CPPEOF'
#include "math_utils.hpp"
#include <iostream>

namespace math {

int add(int a, int b) { return a + b; }
int multiply(int a, int b) { return a * b; }

int Calculator::compute(int x, int y) {
    return add(x, y) + multiply(x, y);
}

} // namespace math
CPPEOF
    cat > "$SYNTH/src/main.cpp" << 'CPPEOF'
#include <iostream>
#include "math_utils.hpp"

using namespace math;

int main() {
    int result = add(3, 4);
    int product = multiply(5, 6);
    int computed = Calculator::compute(result, product);
    std::cout << "Result: " << computed << std::endl;
    return 0;
}
CPPEOF
    cat > "$SYNTH/CMakeLists.txt" << 'CPPEOF'
cmake_minimum_required(VERSION 3.14)
project(SmokeTest LANGUAGES CXX)
add_executable(smoke src/main.cpp src/math_utils.cpp)
target_include_directories(smoke PRIVATE include)
CPPEOF
    ANALYZE_ROOT="$SYNTH"
    echo "=== C++ Synthetic Project Smoke ==="
    echo "Root: $SYNTH"
    trap 'rm -rf "$SYNTH"' EXIT
fi

echo "Binary: $BIN"
echo ""

# Run analysis
RESULT=$("$BIN" analyze --root "$ANALYZE_ROOT" --language cpp --format json 2>/dev/null) || {
    echo "FAIL: analyze command failed"
    exit 1
}

# Parse results
NODES=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['summary']['nodeCount'])" 2>/dev/null || echo "0")
EDGES=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['summary']['edgeCount'])" 2>/dev/null || echo "0")
SYMBOLS=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['summary']['symbolCount'])" 2>/dev/null || echo "0")
FILES=$(echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['summary']['sourceFileCount'])" 2>/dev/null || echo "0")

echo "Nodes: $NODES"
echo "Edges: $EDGES"
echo "Symbols: $SYMBOLS"
echo "Source files: $FILES"
echo ""

# Assertions
check() {
    local label="$1" actual="$2" min="$3"
    if [[ "$actual" -ge "$min" ]]; then
        echo "  PASS: $label ($actual >= $min)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label ($actual < $min)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Checks ==="
check "nodeCount > 0" "$NODES" 1
check "edgeCount > 0" "$EDGES" 1
check "symbolCount > 0" "$SYMBOLS" 1
check "sourceFileCount > 0" "$FILES" 1

echo ""
echo "=== Summary ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
echo "All checks passed!"
