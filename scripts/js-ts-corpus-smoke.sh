#!/usr/bin/env bash
set -euo pipefail

# JS/TS Real-World Corpus Smoke
# 对 fixture 和可选真实项目运行只读 analyze，输出结构化 baseline。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$ROOT/target/debug/codelattice"

if ! command -v "$BINARY" &>/dev/null; then
    echo "Building codelattice..."
    cargo build --manifest-path "$ROOT/Cargo.toml" --features tree-sitter-javascript,tree-sitter-typescript 2>&1
fi

PROJECTS=()
FIXTURE_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --fixture-only) FIXTURE_ONLY=true; shift ;;
        --project) PROJECTS+=("$2"); shift 2 ;;
        --help) echo "Usage: $0 [--fixture-only] [--project <path>]..."; exit 0 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

if [ "$FIXTURE_ONLY" = true ] || [ ${#PROJECTS[@]} -eq 0 ]; then
    JS_FIXTURE="$ROOT/fixtures/javascript/portable-smoke"
    TS_FIXTURE="$ROOT/fixtures/typescript"
    if [ -d "$JS_FIXTURE" ]; then
        PROJECTS+=("$JS_FIXTURE")
    fi
    if [ -d "$TS_FIXTURE" ]; then
        PROJECTS+=("$TS_FIXTURE")
    fi
fi

echo "=== JS/TS Real-World Corpus Smoke ==="
echo "Projects: ${#PROJECTS[@]}"
echo ""

PASS=0
FAIL=0
RESULTS=()

for PROJECT in "${PROJECTS[@]}"; do
    PROJECT_NAME="$(basename "$PROJECT")"
    echo "--- Analyzing: $PROJECT_NAME ($PROJECT) ---"

    LANG=""
    if [ -f "$PROJECT/package.json" ] && [ ! -f "$PROJECT/tsconfig.json" ]; then
        LANG="javascript"
    elif [ -f "$PROJECT/tsconfig.json" ]; then
        LANG="typescript"
    elif [ -f "$PROJECT/Cargo.toml" ]; then
        LANG="rust"
    else
        LANG="auto"
    fi

    OUTPUT=$("$Binary" analyze --root "$PROJECT" --language "$LANG" --format json 2>/dev/null) || {
        echo "  ❌ Analysis failed for $PROJECT_NAME"
        FAIL=$((FAIL + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"error\",\"language\":\"$LANG\"}")
        continue
    }

    SOURCE_COUNT=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    n = d.get('nodes', [])
    files = [x for x in n if x.get('kind') == 'source-file']
    print(len(files))
except:
    print(0)
" 2>/dev/null || echo "0")

    SYMBOL_COUNT=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    n = d.get('nodes', [])
    syms = [x for x in n if x.get('kind') == 'symbol']
    print(len(syms))
except:
    print(0)
" 2>/dev/null || echo "0")

    EDGE_COUNT=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(len(d.get('edges', [])))
except:
    print(0)
" 2>/dev/null || echo "0")

    DIAGNOSTIC_COUNT=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(len(d.get('diagnostics', [])))
except:
    print(0)
" 2>/dev/null || echo "0")

    FW_HINTS=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(len(d.get('frameworkHints', [])))
except:
    print(0)
" 2>/dev/null || echo "0")

    PUBLIC_SURFACE=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(len(d.get('publicSurface', [])))
except:
    print(0)
" 2>/dev/null || echo "0")

    SUMMARY=$(echo "$OUTPUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    s = d.get('summary', {})
    print(json.dumps(s))
except:
    print('{}')
" 2>/dev/null || echo "{}")

    echo "  Language:         $LANG"
    echo "  Source files:     $SOURCE_COUNT"
    echo "  Symbols:          $SYMBOL_COUNT"
    echo "  Edges:            $EDGE_COUNT"
    echo "  Diagnostics:      $DIAGNOSTIC_COUNT"
    echo "  Framework hints:  $FW_HINTS"
    echo "  Public surface:   $PUBLIC_SURFACE"
    echo "  Summary:          $SUMMARY"

    if [ "$SOURCE_COUNT" -gt 0 ] && [ "$SYMBOL_COUNT" -gt 0 ]; then
        echo "  ✅ $PROJECT_NAME passed"
        PASS=$((PASS + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"ok\",\"language\":\"$LANG\",\"sourceFiles\":$SOURCE_COUNT,\"symbols\":$SYMBOL_COUNT,\"edges\":$EDGE_COUNT,\"diagnostics\":$DIAGNOSTIC_COUNT,\"frameworkHints\":$FW_HINTS,\"publicSurface\":$PUBLIC_SURFACE}")
    else
        echo "  ⚠️ $PROJECT_NAME: zero source files or symbols"
        FAIL=$((FAIL + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"warning\",\"language\":\"$LANG\",\"sourceFiles\":$SOURCE_COUNT,\"symbols\":$SYMBOL_COUNT}")
    fi
    echo ""
done

echo "=== Summary ==="
echo "Total: ${#PROJECTS[@]}, Passed: $PASS, Failed: $FAIL"
echo ""
echo "Results:"
for R in "${RESULTS[@]}"; do
    echo "  $R"
done

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "⚠️ Some projects had issues. Check output above."
    exit 1
fi

echo ""
echo "✅ All corpus smoke tests passed."
