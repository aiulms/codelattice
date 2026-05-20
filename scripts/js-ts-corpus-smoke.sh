#!/usr/bin/env bash
set -euo pipefail

# JS/TS Real-World Corpus Smoke
# 对 fixture 和可选真实项目运行只读 analyze，输出结构化 baseline。
# 不执行目标项目代码，不 npm install，不 build。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$ROOT/target/debug/codelattice"
FEATURES="tree-sitter-javascript,tree-sitter-typescript"
MANIFEST="$ROOT/fixtures/corpus/js-ts-corpus-manifest.json"
REPORT="/tmp/codelattice-js-ts-corpus-report.json"

PROJECT_ENTRIES=()
FIXTURE_ONLY=false
CORPUS_DIR=""
CLONE_MISSING=false
OFFLINE=false
DEFAULT_CORPUS_DIR="/Users/jiangxuanyang/Desktop/codelattice-corpus/js-ts"
PASS=0
WARN=0
FAIL=0
SKIP=0
RESULTS=()

usage() {
  cat <<'USAGE'
Usage: scripts/js-ts-corpus-smoke.sh [options]

Options:
  --fixture-only           Only run fixture projects (default if no projects specified)
  --corpus-dir <path>      Directory for cloned real projects
  --clone-missing          Clone repos from manifest if not present in corpus-dir
  --offline                Skip clone and skip missing projects
  --project <path>         Add a specific project path (can repeat)
  --help                   Show this help

Default corpus dir: /Users/jiangxuanyang/Desktop/codelattice-corpus/js-ts
Report output: /tmp/codelattice-js-ts-corpus-report.json
USAGE
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --fixture-only) FIXTURE_ONLY=true; shift ;;
        --corpus-dir) CORPUS_DIR="$2"; shift 2 ;;
        --clone-missing) CLONE_MISSING=true; shift ;;
        --offline) OFFLINE=true; shift ;;
        --project)
            PROJECT_PATH="$2"
            PROJECT_ID="$(basename "$PROJECT_PATH")"
            PROJECT_ENTRIES+=("${PROJECT_PATH}"$'\t'"auto"$'\t'"${PROJECT_ID}")
            shift 2
            ;;
        --help) usage; exit 0 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

if [ -z "$CORPUS_DIR" ] && { [ "$CLONE_MISSING" = true ] || [ "$OFFLINE" = true ]; }; then
    CORPUS_DIR="$DEFAULT_CORPUS_DIR"
fi

echo "Building codelattice with JS/TS parser features..."
cargo build --manifest-path "$ROOT/crates/cli/Cargo.toml" --features "$FEATURES" --bin codelattice 2>/dev/null

# Always include fixtures. 真实 corpus 的语言来自 manifest，fixture 语言在这里显式固定；
# 避免用 tsconfig/package.json 重新猜测导致 baseline 被错误语言污染。
JS_FIXTURE="$ROOT/fixtures/javascript/portable-smoke"
TS_FIXTURE="$ROOT/fixtures/typescript/portable-smoke"
[ -d "$JS_FIXTURE" ] && PROJECT_ENTRIES+=("${JS_FIXTURE}"$'\t'"javascript"$'\t'"javascript-fixture")
[ -d "$TS_FIXTURE" ] && PROJECT_ENTRIES+=("${TS_FIXTURE}"$'\t'"typescript"$'\t'"typescript-fixture")

# Add real corpus projects from manifest
if [ "$FIXTURE_ONLY" = false ] && [ -n "$CORPUS_DIR" ]; then
    if [ ! -f "$MANIFEST" ]; then
        echo "Warning: manifest not found at $MANIFEST, skipping corpus"
    else
        while IFS=$'\t' read -r status path proj_id language info; do
            if [ "$status" = "exists" ]; then
                PROJECT_ENTRIES+=("${path}"$'\t'"${language}"$'\t'"${proj_id}")
                echo "  Added corpus project: $proj_id ($language)"
            else
                echo "  Skipping $proj_id: $info"
                SKIP=$((SKIP + 1))
                RESULTS+=("{\"project\":\"$proj_id\",\"status\":\"skipped\",\"language\":\"$language\",\"reason\":\"$info\"}")
            fi
        done < <(CLONE_MISSING="${CLONE_MISSING}" OFFLINE="${OFFLINE}" python3 - "$MANIFEST" "$CORPUS_DIR" <<'PY'
import json, sys, os, subprocess

manifest_path = sys.argv[1]
corpus_dir = sys.argv[2]
clone_missing = os.environ.get("CLONE_MISSING", "") == "true"
offline = os.environ.get("OFFLINE", "") == "true"

with open(manifest_path) as f:
    manifest = json.load(f)

for proj in manifest.get("projects", []):
    subdir = proj.get("subdir", proj["id"])
    path = os.path.join(corpus_dir, subdir)
    repo_url = proj.get("repoUrl", "")
    language = proj.get("language", "auto")
    optional = proj.get("optional", False)
    depth = proj.get("cloneDepth", 1)

    if os.path.isdir(path) and os.listdir(path):
        print(f"exists\t{path}\t{proj['id']}\t{language}\t")
    elif clone_missing and repo_url and not offline:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        try:
            subprocess.run(
                ["git", "clone", "--depth", str(depth), repo_url, path],
                capture_output=True, timeout=120, check=True
            )
            print(f"exists\t{path}\t{proj['id']}\t{language}\t")
        except Exception as e:
            reason = str(e)[:80]
            if optional:
                print(f"skip\t{path}\t{proj['id']}\t{language}\toptional clone failed: {reason}")
            else:
                print(f"skip\t{path}\t{proj['id']}\t{language}\tclone failed: {reason}")
    elif optional:
        print(f"skip\t{path}\t{proj['id']}\t{language}\toptional and not cloned")
    else:
        print(f"skip\t{path}\t{proj['id']}\t{language}\tnot cloned (use --clone-missing)")
PY
        )
    fi
fi

echo ""
echo "=== JS/TS Real-World Corpus Smoke ==="
echo "Projects: ${#PROJECT_ENTRIES[@]} analyzed, $SKIP skipped"
echo "Report: $REPORT"
echo ""

for ENTRY in "${PROJECT_ENTRIES[@]}"; do
    IFS=$'\t' read -r PROJECT LANG PROJECT_ID <<< "$ENTRY"
    PROJECT_NAME="$PROJECT_ID"

    # Manually supplied projects default to auto, while fixtures/corpus use explicit languages.
    if [ -z "$LANG" ] || [ "$LANG" = "auto" ]; then
        if [ -f "$PROJECT/tsconfig.json" ]; then
            LANG="typescript"
        elif [ -f "$PROJECT/package.json" ]; then
            LANG="javascript"
        else
            LANG="auto"
        fi
    fi

    echo "--- Analyzing: $PROJECT_NAME ($LANG) ---"

    # Get HEAD commit if git repo
    HEAD_COMMIT="N/A"
    if [ -d "$PROJECT/.git" ]; then
        HEAD_COMMIT=$(git -C "$PROJECT" rev-parse --short HEAD 2>/dev/null || echo "N/A")
    fi

    # Run analyze to temp file
    OUTPUT_FILE="$(mktemp)"
    "$BINARY" analyze --root "$PROJECT" --language "$LANG" --format json >"$OUTPUT_FILE" 2>/dev/null || {
        echo "  ❌ Analysis failed for $PROJECT_NAME"
        rm -f "$OUTPUT_FILE"
        FAIL=$((FAIL + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"failed\",\"language\":\"$LANG\",\"headCommit\":\"$HEAD_COMMIT\"}")
        continue
    }

    # Extract all metrics in one python3 call from file.
    # Prefer summary fields (authoritative) over manual edge counting.
    METRICS=$(python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
s = d.get('summary', {})
g = d.get('graph', {})
nodes = g.get('nodes', d.get('nodes', []))
edges = g.get('edges', d.get('edges', []))
source_files = s.get('sourceFileCount') or len([x for x in nodes if x.get('kind') == 'source-file'])
symbols = s.get('symbolCount') or len([x for x in nodes if x.get('kind') == 'symbol'])
all_edges = s.get('edgeCount') or len(edges)
# Prefer summary.callEdgeCount; fallback to counting edges with type=='CALLS'
call_edges = s.get('callEdgeCount') or len([e for e in edges if e.get('type') == 'CALLS' or e.get('kind') == 'CALLS'])
diagnostics = s.get('diagnosticCount') or len(g.get('diagnostics', d.get('diagnostics', [])))
fw_hints = s.get('frameworkHintCount') or len(g.get('framework_hints', d.get('frameworkHints', [])))
public_surface = s.get('publicSurfaceCandidateCount') or len(g.get('public_surface', d.get('publicSurface', [])))
print(f'{source_files}\t{symbols}\t{all_edges}\t{call_edges}\t{diagnostics}\t{fw_hints}\t{public_surface}')
" "$OUTPUT_FILE" 2>/dev/null)
    rm -f "$OUTPUT_FILE"

    if [ -z "$METRICS" ]; then
        echo "  ❌ $PROJECT_NAME: failed to extract metrics"
        FAIL=$((FAIL + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"failed\",\"language\":\"$LANG\",\"headCommit\":\"$HEAD_COMMIT\"}")
        continue
    fi

    SOURCE_COUNT=$(echo "$METRICS" | cut -f1)
    SYMBOL_COUNT=$(echo "$METRICS" | cut -f2)
    EDGE_COUNT=$(echo "$METRICS" | cut -f3)
    CALL_EDGES=$(echo "$METRICS" | cut -f4)
    DIAG_COUNT=$(echo "$METRICS" | cut -f5)
    FW_HINTS=$(echo "$METRICS" | cut -f6)
    PUB_SURF=$(echo "$METRICS" | cut -f7)

    echo "  Language:         $LANG"
    echo "  HEAD:             $HEAD_COMMIT"
    echo "  Source files:     $SOURCE_COUNT"
    echo "  Symbols:          $SYMBOL_COUNT"
    echo "  Edges:            $EDGE_COUNT"
    echo "  Call edges:       $CALL_EDGES"
    echo "  Diagnostics:      $DIAG_COUNT"
    echo "  Framework hints:  $FW_HINTS"
    echo "  Public surface:   $PUB_SURF"

    if [ "$SOURCE_COUNT" -gt 0 ] && [ "$SYMBOL_COUNT" -gt 0 ] && [ "$CALL_EDGES" -gt 0 ]; then
        echo "  ✅ $PROJECT_NAME passed"
        PASS=$((PASS + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"pass\",\"language\":\"$LANG\",\"headCommit\":\"$HEAD_COMMIT\",\"sourceFiles\":$SOURCE_COUNT,\"symbols\":$SYMBOL_COUNT,\"edges\":$EDGE_COUNT,\"callEdgeCount\":$CALL_EDGES,\"diagnostics\":$DIAG_COUNT,\"frameworkHints\":$FW_HINTS,\"publicSurface\":$PUB_SURF}")
    elif [ "$SOURCE_COUNT" -gt 0 ] && [ "$SYMBOL_COUNT" -gt 0 ]; then
        echo "  ⚠️ $PROJECT_NAME: passed but zero call edges"
        WARN=$((WARN + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"warning\",\"language\":\"$LANG\",\"headCommit\":\"$HEAD_COMMIT\",\"sourceFiles\":$SOURCE_COUNT,\"symbols\":$SYMBOL_COUNT,\"edges\":$EDGE_COUNT,\"callEdgeCount\":$CALL_EDGES}")
    else
        echo "  ❌ $PROJECT_NAME: zero source files or symbols"
        FAIL=$((FAIL + 1))
        RESULTS+=("{\"project\":\"$PROJECT_NAME\",\"status\":\"failed\",\"language\":\"$LANG\",\"headCommit\":\"$HEAD_COMMIT\",\"sourceFiles\":$SOURCE_COUNT,\"symbols\":$SYMBOL_COUNT}")
    fi
    echo ""
done

TOTAL=$(( ${#PROJECT_ENTRIES[@]} + SKIP ))
echo "=== Summary ==="
echo "Total: $TOTAL, Passed: $PASS, Warnings: $WARN, Failed: $FAIL, Skipped: $SKIP"

# Write JSON report
python3 - "$REPORT" "$TOTAL" "$PASS" "$WARN" "$FAIL" "$SKIP" "${RESULTS[*]}" <<'PY'
import sys, json
path = sys.argv[1]
total, passed, warnings, failed, skipped = sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]
results_raw = sys.argv[7] if len(sys.argv) > 7 else ""
results = []
if results_raw:
    for r in results_raw.split("} {"):
        r = r.strip()
        if not r.startswith("{"):
            r = "{" + r
        if not r.endswith("}"):
            r = r + "}"
        try:
            results.append(json.loads(r))
        except:
            pass
report = {
    "schemaVersion": "codelattice.corpus.report.v1",
    "total": int(total),
    "passed": int(passed),
    "warnings": int(warnings),
    "failed": int(failed),
    "skipped": int(skipped),
    "results": results
}
with open(path, 'w') as f:
    json.dump(report, f, indent=2)
print(f"Report written to {path}")
PY

echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "⚠️ Some projects failed. Check output above."
    exit 1
fi

echo "✅ All corpus smoke tests passed (warnings: $WARN)."
