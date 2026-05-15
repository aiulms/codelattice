#!/usr/bin/env bash
# mcp-cache-smoke.sh — Verify MCP cache behavior: miss→hit, cross-tool, clear→re-miss.
#
# Usage: bash scripts/mcp-cache-smoke.sh [fixture-path]
#
# Outputs a concise performance table.
# Exit 0 if all checks pass, 1 on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE="${1:-$REPO_ROOT/fixtures/call-resolution/c1-same-module}"
FIXTURE_ABS="$(cd "$(dirname "$FIXTURE")" && pwd)/$(basename "$FIXTURE")"

BIN="$REPO_ROOT/target/debug/codelattice"
if [[ ! -x "$BIN" ]]; then
    BIN="$REPO_ROOT/target/release/codelattice"
fi
if [[ ! -x "$BIN" ]]; then
    BIN="$REPO_ROOT/target/debug/gitnexus-rust-core-cli"
fi
if [[ ! -x "$BIN" ]]; then
    BIN="$REPO_ROOT/target/release/gitnexus-rust-core-cli"
fi
if [[ ! -x "$BIN" ]]; then
    echo "ERROR: No binary found. Run 'cargo build -p gitnexus-rust-core-cli --bins' first."
    exit 1
fi

echo "=== MCP Cache Smoke ==="
echo "Binary:   $BIN"
echo "Fixture:  $FIXTURE_ABS"
echo ""

PASS=0
FAIL=0

# Helper: run multi-request session and extract cache/perf data
# Args: request_lines... (each is a JSON-RPC request)
# Outputs: one line per response with "id cacheHit durationMs tool"
run_session() {
    local input="$1"
    echo "$input" | "$BIN" mcp 2>/dev/null | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    d = json.loads(line)
    rid = d.get('id', '?')
    text = d.get('result', {}).get('content', [{}])[0].get('text', '{}')
    data = json.loads(text)
    hit = data.get('cacheHit', 'N/A')
    dur = data.get('analysisDurationMs', '-')
    tool = 'unknown'
    # figure out tool name from the request pattern in the input
    print(f'{rid}\t{hit}\t{dur}')
" 2>/dev/null
}

# --- Test 1: Miss → Hit (same tool) ---
echo "Test 1: Miss → Hit (codelattice_analyze x2)"
REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n' "$FIXTURE_ABS" "$FIXTURE_ABS")
RESULT=$(run_session "$REQ")
LINE1=$(echo "$RESULT" | head -1)
LINE2=$(echo "$RESULT" | tail -1)

HIT1=$(echo "$LINE1" | awk -F'\t' '{print $2}')
HIT2=$(echo "$LINE2" | awk -F'\t' '{print $2}')
DUR1=$(echo "$LINE1" | awk -F'\t' '{print $3}')
DUR2=$(echo "$LINE2" | awk -F'\t' '{print $3}')

echo "  Call 1: cacheHit=$HIT1 durationMs=$DUR1"
echo "  Call 2: cacheHit=$HIT2 durationMs=$DUR2"

if [[ "$HIT1" == "False" && "$HIT2" == "True" ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected miss then hit"
    FAIL=$((FAIL + 1))
fi
echo ""

# --- Test 2: Cross-tool cache reuse ---
echo "Test 2: Cross-tool cache reuse (calls_from → symbol_context)"
REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_calls_from","arguments":{"root":"%s","language":"rust","symbol":"main"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_symbol_context","arguments":{"root":"%s","language":"rust","name":"main","includeSnippet":false}}}\n' "$FIXTURE_ABS" "$FIXTURE_ABS")
RESULT=$(run_session "$REQ")
LINE1=$(echo "$RESULT" | head -1)
LINE2=$(echo "$RESULT" | tail -1)

HIT1=$(echo "$LINE1" | awk -F'\t' '{print $2}')
HIT2=$(echo "$RESULT" | tail -1 | awk -F'\t' '{print $2}')

echo "  calls_from:   cacheHit=$HIT1"
echo "  symbol_context: cacheHit=$HIT2"

if [[ "$HIT1" == "False" && "$HIT2" == "True" ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected miss then cross-tool hit"
    FAIL=$((FAIL + 1))
fi
echo ""

# --- Test 3: cache_clear → re-miss ---
echo "Test 3: cache_clear → re-miss"
REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_cache_clear","arguments":{}}}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n' "$FIXTURE_ABS" "$FIXTURE_ABS")
RESULT=$(run_session "$REQ")

LINE1=$(echo "$RESULT" | head -1)
LINE3=$(echo "$RESULT" | tail -1)

HIT1=$(echo "$LINE1" | awk -F'\t' '{print $2}')
HIT3=$(echo "$LINE3" | awk -F'\t' '{print $2}')
DUR1=$(echo "$LINE1" | awk -F'\t' '{print $3}')
DUR3=$(echo "$LINE3" | awk -F'\t' '{print $3}')

echo "  Before clear: cacheHit=$HIT1 durationMs=$DUR1"
echo "  After clear:  cacheHit=$HIT3 durationMs=$DUR3"

if [[ "$HIT1" == "False" && "$HIT3" == "False" ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected both misses (clear should evict)"
    FAIL=$((FAIL + 1))
fi
echo ""

# --- Test 4: Source snippet with cache hit ---
echo "Test 4: Source snippet available on cache hit"
REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_symbol_context","arguments":{"root":"%s","language":"rust","name":"helper"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_symbol_context","arguments":{"root":"%s","language":"rust","name":"helper"}}}\n' "$FIXTURE_ABS" "$FIXTURE_ABS")
SNIPPET_RESULT=$(echo "$REQ" | "$BIN" mcp 2>/dev/null | python3 -c "
import json, sys
lines = sys.stdin.read().strip().split('\n')
for i, line in enumerate(lines):
    d = json.loads(line)
    text = d.get('result',{}).get('content',[{}])[0].get('text','{}')
    data = json.loads(text)
    hit = data.get('cacheHit', 'N/A')
    snip = data.get('selected',{}).get('sourceSnippet',{})
    has_snip = bool(snip.get('lines',''))
    print(f'Call {i+1}: cacheHit={hit} hasSnippet={has_snip}')
" 2>/dev/null)

echo "$SNIPPET_RESULT"
SNIP_OK=$(echo "$SNIPPET_RESULT" | grep -c "hasSnippet=True" || true)
if [[ "$SNIP_OK" -eq 2 ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected snippet on both calls"
    FAIL=$((FAIL + 1))
fi
echo ""

# --- Test 5: Persistent cache (cross-process hit) ---
echo "Test 5: Persistent cache — cross-process hit"
CACHE_DIR="$(mktemp -d /tmp/codelattice-smoke-cache-XXXXXX)"
# Session 1: miss → populate persistent cache
HIT_S1=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n' "$FIXTURE_ABS" \
    | CODELATTICE_CACHE_DIR="$CACHE_DIR" "$BIN" mcp 2>/dev/null \
    | python3 -c "import json,sys; d=json.loads(sys.stdin.readline()); data=json.loads(d['result']['content'][0]['text']); print(data.get('cacheHit','N/A'))" 2>/dev/null)

# Session 2: should hit persistent cache
HIT_S2=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n' "$FIXTURE_ABS" \
    | CODELATTICE_CACHE_DIR="$CACHE_DIR" "$BIN" mcp 2>/dev/null \
    | python3 -c "import json,sys; d=json.loads(sys.stdin.readline()); data=json.loads(d['result']['content'][0]['text']); print(data.get('cacheHit','N/A'))" 2>/dev/null)

echo "  Session 1: cacheHit=$HIT_S1"
echo "  Session 2: cacheHit=$HIT_S2"

if [[ "$HIT_S1" == "False" && "$HIT_S2" == "True" ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected session1=False, session2=True (persistent hit)"
    FAIL=$((FAIL + 1))
fi
rm -rf "$CACHE_DIR"
echo ""

# --- Test 6: cache_status shows both layers ---
echo "Test 6: cache_status shows memory + persistent layers"
CACHE_DIR2="$(mktemp -d /tmp/codelattice-smoke-cache-XXXXXX)"
STATUS_RESULT=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}\n' "$FIXTURE_ABS" \
    | CODELATTICE_CACHE_DIR="$CACHE_DIR2" "$BIN" mcp 2>/dev/null \
    | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    d = json.loads(line)
    if d.get('id') == 2:
        data = json.loads(d['result']['content'][0]['text'])
        has_mem = 'memory' in data
        has_pers = 'persistent' in data
        mem_count = data.get('memory',{}).get('entryCount', 0)
        pers_enabled = data.get('persistent',{}).get('enabled', False)
        print(f'memory={has_mem} persistent={has_pers} memEntries={mem_count} persEnabled={pers_enabled}')
" 2>/dev/null)

echo "  $STATUS_RESULT"
STATUS_OK=$(echo "$STATUS_RESULT" | grep -c "memory=True persistent=True" || true)
if [[ "$STATUS_OK" -eq 1 ]]; then
    echo "  PASS"
    PASS=$((PASS + 1))
else
    echo "  FAIL — expected both layers in cache_status"
    FAIL=$((FAIL + 1))
fi
rm -rf "$CACHE_DIR2"
echo ""

# --- Summary ---
echo "============================================"
echo " Cache Smoke Results"
echo "============================================"
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo ""

if [[ "$FAIL" -eq 0 ]]; then
    echo "All cache smoke tests passed."
    exit 0
else
    echo "Some cache smoke tests failed."
    exit 1
fi
