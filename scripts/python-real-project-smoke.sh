#!/usr/bin/env bash
# python-real-project-smoke.sh — Read-only Python analysis smoke test.
#
# Generates a synthetic multi-file Python project in /tmp and runs
# CodeLattice analyze on it. Does NOT require python/pytest/pip.
#
# Usage: bash scripts/python-real-project-smoke.sh [--project <path>]
#
# --project: Analyze an existing Python project instead of the synthetic one.
#            No python/pytest/pip is run; only read-only static analysis.

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
    echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli --features tree-sitter-python --bins"
    exit 1
fi

# Check if Python feature is compiled
PYTHON_CHECK=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"/tmp","language":"python"}}}' \
    | "$BIN" mcp 2>/dev/null | head -1 || true)
if echo "$PYTHON_CHECK" | grep -q "python_disabled\|not compiled"; then
    echo "SKIP: Python feature not compiled. Rebuild with --features tree-sitter-python"
    exit 0
fi

PASS=0
FAIL=0

if [[ -n "$PROJECT" ]]; then
    ANALYZE_ROOT="$PROJECT"
    echo "=== Python Real Project Smoke ==="
    echo "Project: $PROJECT"
else
    # Graceful skip if no Python project found (when run without --project and no synthetic fallback desired)
    if [[ -z "${CI:-}" ]]; then
        # Not in CI: generate synthetic project
        SYNTH="/tmp/codelattice-python-smoke-$$"
        mkdir -p "$SYNTH/src/sample_app" "$SYNTH/tests"
        cat > "$SYNTH/pyproject.toml" << 'PYEOF'
[build-system]
requires = ["setuptools>=68.0"]
build-backend = "setuptools.backends._legacy:_Backend"

[project]
name = "sample-app"
version = "0.1.0"
description = "Portable smoke test fixture for CodeLattice Python"
requires-python = ">=3.8"
PYEOF
        cat > "$SYNTH/src/sample_app/__init__.py" << 'PYEOF'
"""Sample application package."""

__version__ = "0.1.0"
PYEOF
        cat > "$SYNTH/src/sample_app/math_utils.py" << 'PYEOF'
"""Math utility module."""

def add(a: int, b: int) -> int:
    """Add two integers."""
    return a + b

def multiply(a: int, b: int) -> int:
    """Multiply two integers."""
    return a * b

PI = 3.14159
PYEOF
        cat > "$SYNTH/src/sample_app/service.py" << 'PYEOF'
"""Service module."""

from sample_app.math_utils import add
from sample_app.math_utils import multiply as mul

class UserService:
    """User service class for testing."""

    def __init__(self, name: str):
        """Initialize user service."""
        self.name = name
        self._internal_id = add(id(self), 0)

    def run(self) -> str:
        """Run the service."""
        result = mul(len(self.name), 2)
        return f"{self.name}: {result}"

    def _private_helper(self) -> int:
        """Private helper method."""
        return add(1, 2)

async def fetch_data(url: str) -> dict:
    """Fetch data asynchronously."""
    return {"url": url, "status": "ok"}

def process_items(items: list) -> list:
    """Process items using add."""
    return [add(item, 1) for item in items]
PYEOF
        cat > "$SYNTH/src/sample_app/main.py" << 'PYEOF'
"""Main entry point."""

from sample_app.math_utils import add, multiply
from sample_app.service import UserService, fetch_data

def main() -> None:
    """Main function."""
    result = add(1, 2)
    product = multiply(3, 4)
    user = UserService("alice")
    user.run()
    print(f"Result: {result}, Product: {product}")

if __name__ == "__main__":
    main()
PYEOF
        cat > "$SYNTH/tests/test_math_utils.py" << 'PYEOF'
"""Tests for math_utils module."""

from sample_app.math_utils import add, multiply

def test_add():
    """Test add function."""
    assert add(1, 2) == 3
    assert add(-1, 1) == 0

def test_multiply():
    """Test multiply function."""
    assert multiply(2, 3) == 6
    assert multiply(0, 5) == 0

def test_add_with_multiply():
    """Test add combined with multiply."""
    result = add(multiply(2, 3), 4)
    assert result == 10
PYEOF
        ANALYZE_ROOT="$SYNTH"
        echo "=== Python Synthetic Project Smoke ==="
        echo "Root: $SYNTH"
        trap 'rm -rf "$SYNTH"' EXIT
    else
        echo "SKIP: No Python project specified (use --project <path>)"
        exit 0
    fi
fi

echo "Binary: $BIN"
echo ""

# Run analysis
RESULT=$("$BIN" analyze --root "$ANALYZE_ROOT" --language python --format json 2>/dev/null) || {
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
