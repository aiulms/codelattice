#!/usr/bin/env bash
# webui-snapshot.sh — Generate CodeLattice WebUI Snapshot (V1)
#
# Aggregates CLI/MCP analysis results into a single JSON snapshot
# conforming to docs/webui/webui-snapshot-contract.md
#
# Usage:
#   bash scripts/webui-snapshot.sh --root <path> --language rust --output snapshot.json
#   bash scripts/webui-snapshot.sh --root . --language auto --output -
#   bash scripts/webui-snapshot.sh --root fixtures/rust/portable-smoke --language rust --output /tmp/s.json --compact
#
# Exit codes:
#   0 — success
#   1 — usage error or runtime error
#   2 — project language unclear
#
# Hard boundaries (per AGENTS.md):
#   - Only modifies files under the specified output path
#   - Does not modify GitNexus-RC / Tool / CodeLattice-Tool
#   - Does not introduce new dependencies
#   - Does not run promote to CodeLattice-Tool

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────
ROOT=""
LANGUAGE="auto"
OUTPUT=""
COMPACT=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ── Help ───────────────────────────────────────────────────────────
usage() {
  cat <<EOF
Usage: $(basename "$0") --root <path> --language <lang> --output <path> [options]

Generate a CodeLatticeWebSnapshotV1 JSON for WebUI consumption.

Required:
  --root <path>        Project root directory (absolute or relative)
  --language <lang>    Language: rust|cangjie|arkts|typescript|c|cpp|python|auto
  --output <path>      Output JSON file path, or '-' for stdout

Options:
  --compact            Minify JSON output (no pretty-print)
  -h, --help           Show this help

Examples:
  $(basename "$0") --root fixtures/rust/portable-smoke --language rust --output /tmp/snapshot.json
  $(basename "$0") --root . --language auto --output -
  $(basename "$0") --root ./my-project --language typescript --output snapshot.json --compact
EOF
  exit 0
}

# ── Parse args ─────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --language) LANGUAGE="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --compact) COMPACT=true; shift ;;
    -h|--help) usage ;;
    *) echo "Error: Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# ── Validate ───────────────────────────────────────────────────────
if [[ -z "$ROOT" ]]; then
  echo "Error: --root is required" >&2; exit 1
fi

# Resolve to absolute path
if [[ ! "$ROOT" = /* ]]; then
  ROOT="$(cd "$ROOT" 2>/dev/null && pwd)" || {
    echo "Error: root directory does not exist: $ROOT" >&2; exit 1
  }
fi

if [[ ! -d "$ROOT" ]]; then
  echo "Error: root is not a directory: $ROOT" >&2; exit 1
fi

if [[ -z "$OUTPUT" ]]; then
  echo "Error: --output is required" >&2; exit 1
fi

# Validate output path (allow '-' for stdout)
if [[ "$OUTPUT" != "-" ]]; then
  OUTDIR="$(dirname "$OUTPUT")"
  if [[ ! -d "$OUTDIR" ]]; then
    echo "Error: output directory does not exist: $OUTDIR" >&2; exit 1
  fi
fi

# ── Find codelattice binary ────────────────────────────────────────
CODELATTICE=""
# Prefer workspace target/release, then target/debug, then PATH
for candidate in \
  "${WORKSPACE_ROOT}/target/release/codelattice" \
  "${WORKSPACE_ROOT}/target/debug/codelattice" \
  "$(command -v codelattice 2>/dev/null)"; do
  if [[ -n "${candidate:-}" && -x "${candidate}" ]]; then
    CODELATTICE="$candidate"
    break
  fi
done

if [[ -z "$CODELATTICE" ]]; then
  cat >&2 <<EOF
Error: Cannot find codelattice binary.

Build it first:
  cargo build --release --bins   # or --all-features for full language support

Or install from a release tarball and ensure 'codelattice' is on your PATH.
EOF
  exit 1
fi

# Verify binary supports analyze subcommand
if ! "$CODELATTICE" --help 2>&1 | grep -q "analyze"; then
  echo "Error: '$CODELATTICE' does not appear to be a valid CodeLattice binary" >&2
  exit 1
fi

# ── Language validation ────────────────────────────────────────────
VALID_LANGUAGES=("rust" "cangjie" "arkts" "typescript" "c" "cpp" "python" "auto")
LANG_VALID=0
for vl in "${VALID_LANGUAGES[@]}"; do
  if [[ "$LANGUAGE" == "$vl" ]]; then LANG_VALID=1; break; fi
done
if [[ $LANG_VALID -eq 0 ]]; then
  echo "Error: unsupported language: $LANGUAGE. Supported: ${VALID_LANGUAGES[*]}" >&2; exit 1
fi

# ── Helper: run CLI and capture JSON safely ────────────────────────
run_analyze() {
  local out
  out=$("$CODELATTICE" analyze --root "$ROOT" --language "$LANGUAGE" --format json 2>/dev/null) || true
  # Check if output looks like JSON (starts with '{')
  if [[ -z "$out" ]] || ! echo "$out" | head -c1 | grep -q '{'; then
    echo '{"_error": "analyze_failed", "_detail": "empty or non-JSON output"}'
    return 1
  fi
  echo "$out"
}

run_quality() {
  local out
  out=$("$CODELATTICE" quality --root "$ROOT" --language "$LANGUAGE" 2>/dev/null) || true
  if [[ -z "$out" ]] || ! echo "$out" | head -c1 | grep -q '{'; then
    echo '{"overall": "unknown", "gates": []}'
    return 1
  fi
  echo "$out"
}

# ── Main generation logic (via embedded Python) ────────────────────
generate_snapshot() {
  local analyze_json quality_json timestamp version_str

  analyze_json=$(run_analyze)
  quality_json=$(run_quality)

  timestamp=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00" 2>/dev/null || date +"%Y-%m-%dT%H:%M:%S%z")

  # Extract version from binary
  version_str=$("$CODELATTICE" --version 2>/dev/null | head -1 || echo "unknown")

  # Use Python stdlib to build the snapshot JSON
  python3 - "$analyze_json" "$quality_json" "$timestamp" "$version_str" "$ROOT" "$LANGUAGE" <<'PYEOF'
import sys, json, os, datetime

def safe_parse(s):
    try:
        return json.loads(s) if s else {}
    except json.JSONDecodeError:
        return {}

analyze_raw = safe_parse(sys.argv[1])
quality_raw = safe_parse(sys.argv[2])
generated_at = sys.argv[3]
generator_version = sys.argv[4]
root_path = sys.argv[5]
language = sys.argv[6]

# ── Build snapshot ───────────────────────────────────────────────
snapshot = {
    "schemaVersion": "webui.snapshot.v1",
    "generatedAt": generated_at,
    "generatorVersion": generator_version,
    "root": root_path,
    "language": language,
    "generatedFrom": {
        "staticAnalysis": True,
        "runtimeVerified": False,
        "externalUsageVerified": False,
        "coverageVerified": False,
        "deletionSafetyVerified": False
    },
    # Sections populated below
    "summary": {},
    "quality": {},
    "insights": {"status": "not_collected", "reason": "requires MCP aggregation; snapshot uses CLI data only"},
    "explore": {"status": "not_collected", "reason": "requires symbol-level query; use MCP tools for on-demand exploration"},
    "impact": {"status": "not_collected", "reason": "requires target symbol parameter; use MCP impact_preview for on-demand analysis", "sampleEntries": []},
    "cleanup": {
        "deadCodeCandidates": {"status": "not_collected", "reason": "requires MCP dead_code_candidates tool"},
        "reachability": {"status": "not_collected", "reason": "requires MCP reachability_map tool"},
        "externalApiSurface": {"status": "not_collected", "reason": "requires MCP external_api_surface tool"},
        "frameworkEntries": {"status": "not_collected", "reason": "requires MCP framework_entry_hints tool"}
    },
    "releaseReview": {
        "breakingChange": {"status": "not_collected", "reason": "requires changed symbols list"},
        "consistency": {"status": "not_collected", "reason": "requires changed symbols list"},
        "configExamples": {"status": "not_collected", "reason": "requires changed symbols list"}
    },
    "docsTestsConfig": {"status": "collected", "docs": {}, "tests": {}, "configFiles": {}},
    "workflowPresets": {"status": "not_collected", "reason": "workflow presets are planning-only metadata; see MCP workflow_presets tool"},
    "limitations": [
        "Static graph analysis only — no runtime behavior proof",
        "No full type inference or trait solving",
        "No macro expansion or proc-macro execution",
        "Call edges are heuristic with confidence scores — not compiler-verified",
        "Dynamic dispatch / reflection / plugins may hide actual callers",
        "Dead-code candidates are NOT deletion-proof — always verify manually",
        "External API surface is NOT external-usage-verified",
        "Consistency review does not run tests or execute scripts",
        "Config/examples review does not build Docker images or run CI",
        "Cross-crate/cross-package dependencies may be incomplete",
        "Snapshot is a point-in-time aggregate; run fresh for updated results"
    ]
}

# ── summary: from analyze output ─────────────────────────────────
summary = analyze_raw.get("summary", {})
snapshot["summary"] = {
    "nodeCount": summary.get("nodeCount", 0),
    "edgeCount": summary.get("edgeCount", 0),
    "symbolCount": summary.get("symbolCount", 0),
    "sourceFileCount": summary.get("sourceFileCount", 0),
    "packageCount": summary.get("packageCount", 0),
    "diagnosticCount": summary.get("diagnosticCount", 0),
    "callEdgeCount": summary.get("callEdgeCount", 0),
    "topNodeKinds": summary.get("topNodeKinds", []),
    "topEdgeKinds": summary.get("topEdgeKinds", [])
}

# ── quality: merge analyze gates + quality subcommand ─────────────
gates_from_analyze = analyze_raw.get("qualityGates", [])
quality_gates = quality_raw.get("gates", gates_from_analyze)
overall = quality_raw.get("overall", "pass")

# Count passed/failed
passed = sum(1 for g in quality_gates if g.get("passed", False))
failed = len(quality_gates) - passed

diags_summary = analyze_raw.get("diagnosticsSummary", {})

snapshot["quality"] = {
    "overall": overall,
    "totalGates": len(quality_gates),
    "passedGates": passed,
    "failedGates": failed,
    "gates": quality_gates,
    "metrics": {
        "graphCompleteness": {
            "nodeCount": summary.get("nodeCount", 0),
            "edgeCount": summary.get("edgeCount", 0),
            "symbolCount": summary.get("symbolCount", 0),
            "sourceFileCount": summary.get("sourceFileCount", 0),
            "danglingEdgeCount": 0  # CLI doesn't expose this directly in summary
        },
        "edgeConfidence": {
            "totalConfidenceEdgeCount": summary.get("edgeCount", 0),
            "highConfidenceEdgeCount": None,
            "mediumConfidenceEdgeCount": None,
            "lowConfidenceEdgeCount": None,
            "unknownConfidenceEdgeCount": None,
            "lowConfidenceEdgeRate": None,
            "unknownConfidenceEdgeRate": None
        },
        "callQuality": {
            "callEdgeCount": summary.get("callEdgeCount", 0),
            "highConfidenceCallEdgeCount": None,
            "mediumConfidenceCallEdgeCount": None,
            "lowConfidenceCallEdgeCount": None,
            "unknownConfidenceCallEdgeCount": None,
            "lowConfidenceCallRate": None
        },
        "dependencyQuality": {
            "importEdgeCount": None,
            "includeEdgeCount": None,
            "unresolvedImportOrIncludeCount": None
        },
        "diagnostics": diags_summary
    },
    "diagnosticsSummary": diags_summary
}

# ── docsTestsConfig: static file scan ─────────────────────────────
docs_tests = snapshot["docsTestsConfig"]
root_abs = os.path.abspath(root_path) if not os.path.isabs(root_path) else root_path

doc_files = []
test_files = []
config_patterns = []

# Scan for common doc/config patterns (shallow, non-recursive into node_modules etc.)
skip_dirs = {'node_modules', 'target', '.git', '__pycache__', '.tox', 'venv', '.venv',
             'dist', 'build', '.cargo', 'vendor', '.cache'}
for entry in os.listdir(root_abs):
    if entry.startswith('.'):
        continue
    full = os.path.join(root_abs, entry)
    if not os.path.isdir(full):
        # Check file type by name
        lower = entry.lower()
        if lower.endswith(('.md', '.txt', '.rst', '.adoc')):
            doc_files.append(entry)
        elif any(p in lower for p in ('test', 'spec', '_test', '.test')):
            test_files.append(entry)
        elif lower in ('cargo.toml', 'package.json', 'pyproject.toml', 'setup.py',
                        'makefile', 'dockerfile', 'docker-compose.yml', 'docker-compose.yaml',
                        '.gitignore', 'tsconfig.json', 'cjpm.toml', 'oh-package.json5'):
            config_patterns.append(entry)
    else:
        # Check for known doc/config directories
        if entry == 'docs':
            for f in os.listdir(full):
                if f.endswith('.md'):
                    doc_files.append(os.path.join(entry, f))
        elif entry in ('tests', 'test', '__tests__', 'spec'):
            for f in os.listdir(full):
                test_files.append(os.path.join(entry, f))

docs_tests["docs"] = {
    "docCount": len(doc_files),
    "topDocPaths": doc_files[:20]
}
docs_tests["tests"] = {
    "testFileCount": len(test_files),
    "topTestPaths": test_files[:20]
}
docs_tests["configFiles"] = {
    "paths": config_patterns
}

# ── Output ───────────────────────────────────────────────────────
indent_val = None if len(sys.argv) > 8 and sys.argv[8] == "--compact" else 2
print(json.dumps(snapshot, indent=indent_val, ensure_ascii=False))
PYEOF
}

# ── Write output ──────────────────────────────────────────────────
if [[ "$OUTPUT" == "-" ]]; then
  generate_snapshot
else
  generate_snapshot > "$OUTPUT"
  local_size=$(wc -c < "$OUTPUT" | tr -d ' ')
  echo "Snapshot written to $OUTPUT (${local_size} bytes)" >&2
fi
