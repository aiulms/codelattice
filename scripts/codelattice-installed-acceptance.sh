#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEV_BINARY="$ROOT/target/debug/codelattice"
INSTALLED_DIR="/Users/jiangxuanyang/Desktop/CodeLattice-Tool"
INSTALLED_WRAPPER="$INSTALLED_DIR/codelattice-mcp.sh"
INSTALLED_BIN="$INSTALLED_DIR/bin/codelattice"
FIXTURE="$ROOT/fixtures/call-resolution/c1-same-module"

SYNC=false
DEV_ONLY=false
INSTALLED_ONLY=false
OPEN_NWE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --sync) SYNC=true; shift ;;
        --dev-only) DEV_ONLY=true; shift ;;
        --installed-only) INSTALLED_ONLY=true; shift ;;
        --open-nwe) OPEN_NWE=true; shift ;;
        --help) echo "Usage: $0 [--sync] [--dev-only] [--installed-only] [--open-nwe]"; exit 0 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

PASS=0
FAIL=0

section() { echo ""; echo "=== $1 ==="; }

ok() { echo "  ✅ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); }

assert_eq() {
    local label="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        ok "$label"
    else
        fail "$label: expected '$expected', got '$actual'"
    fi
}

assert_json_field() {
    local label="$1" json="$2" field="$3" expected="$4"
    local actual
    actual=$(echo "$json" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d${field})" 2>/dev/null || echo "PARSE_ERROR")
    if [ "$actual" = "$expected" ]; then
        ok "$label"
    else
        fail "$label: expected '$expected', got '$actual'"
    fi
}

mcp_call() {
    local binary="$1" method="$2" params="$3"
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"acceptance","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"%s","params":%s}\n' "$method" "$params" \
        | "$binary" mcp 2>/dev/null \
        | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    d = json.loads(line)
    if d.get('id') == 2:
        result = d.get('result', {})
        if 'content' in result:
            text = result['content'][0].get('text', '')
            try:
                parsed = json.loads(text)
                print(json.dumps(parsed))
            except:
                print(text)
        else:
            print(json.dumps(result))
        break
" 2>/dev/null
}

mcp_call_full_toolset() {
    local binary="$1" method="$2" params="$3"
    CODELATTICE_MCP_TOOLSET=full printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"acceptance","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"%s","params":%s}\n' "$method" "$params" \
        | CODELATTICE_MCP_TOOLSET=full "$binary" mcp 2>/dev/null \
        | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    d = json.loads(line)
    if d.get('id') == 2:
        result = d.get('result', {})
        if 'content' in result:
            text = result['content'][0].get('text', '')
            try:
                parsed = json.loads(text)
                print(json.dumps(parsed))
            except:
                print(text)
        else:
            print(json.dumps(result))
        break
" 2>/dev/null
}

section "Source Info"
HEAD_COMMIT=$(cd "$ROOT" && git rev-parse --short HEAD)
echo "  HEAD: $HEAD_COMMIT"

if [ "$DEV_ONLY" = false ] && [ "$INSTALLED_ONLY" = false ] || [ "$DEV_ONLY" = true ]; then
    section "Dev Binary"

    if [ ! -f "$DEV_BINARY" ]; then
        fail "Dev binary not found at $DEV_BINARY"
        echo "  Run: cargo build"
    else
        DEV_VERSION=$("$DEV_BINARY" --version 2>&1 | head -1 || echo "unknown")
        echo "  Version: $DEV_VERSION"

        section "Dev: Default AI Tools (expect 6)"
        TOOLS_JSON=$(mcp_call "$DEV_BINARY" "tools/list" '{}')
        if [ -z "$TOOLS_JSON" ]; then
            fail "tools/list returned empty"
        else
            TOOL_COUNT=$(echo "$TOOLS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('tools',[])))" 2>/dev/null || echo "0")
            assert_eq "Default toolset count" "6" "$TOOL_COUNT"

            HAS_CACHE=$(echo "$TOOLS_JSON" | python3 -c "import sys,json; names=[t['name'] for t in json.load(sys.stdin).get('tools',[])]; print('yes' if 'codelattice_cache' in names else 'no')" 2>/dev/null || echo "no")
            assert_eq "Includes codelattice_cache" "yes" "$HAS_CACHE"

            HAS_CLEANUP=$(echo "$TOOLS_JSON" | python3 -c "import sys,json; names=[t['name'] for t in json.load(sys.stdin).get('tools',[])]; print('yes' if 'codelattice_cleanup' in names else 'no')" 2>/dev/null || echo "no")
            assert_eq "Excludes codelattice_cleanup" "no" "$HAS_CLEANUP"
        fi

        section "Dev: Full Tools (expect 49)"
        FULL_JSON=$(mcp_call_full_toolset "$DEV_BINARY" "tools/list" '{}')
        if [ -z "$FULL_JSON" ]; then
            fail "full tools/list returned empty"
        else
            FULL_COUNT=$(echo "$FULL_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('tools',[])))" 2>/dev/null || echo "0")
            assert_eq "Full toolset count" "49" "$FULL_COUNT"
        fi

        section "Dev: call_chains smoke"
        CC_JSON=$(mcp_call "$DEV_BINARY" "tools/call" "{\"name\":\"codelattice_symbol\",\"arguments\":{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"mode\":\"call_chains\",\"query\":\"helper\",\"compact\":true}}")
        if [ -z "$CC_JSON" ]; then
            fail "call_chains returned empty"
        else
            assert_json_field "call_chains schemaVersion" "$CC_JSON" "['schemaVersion']" "codelattice.callChains.v1"
            CC_CHAINS=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); c=d.get('callChains',[]); print('has_chains' if c and isinstance(c[0]['chain'], list) else 'no_chains')" 2>/dev/null || echo "parse_error")
            assert_eq "call_chains has chain array" "has_chains" "$CC_CHAINS"
            CC_HAS_RO=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'readOrder' in d else 'no')" 2>/dev/null || echo "PARSE_ERROR")
            assert_eq "call_chains has readOrder" "yes" "$CC_HAS_RO"
            CC_HAS_FI=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'filesInvolved' in d else 'no')" 2>/dev/null || echo "PARSE_ERROR")
            assert_eq "call_chains has filesInvolved" "yes" "$CC_HAS_FI"
        fi

        section "Dev: ask v2 smoke"
        ASK_JSON=$(mcp_call "$DEV_BINARY" "tools/call" "{\"name\":\"codelattice_workflow\",\"arguments\":{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"mode\":\"ask\",\"question\":\"helper 的执行流程是什么\",\"compact\":true}}")
        if [ -z "$ASK_JSON" ]; then
            fail "ask returned empty"
        else
            assert_json_field "ask schemaVersion" "$ASK_JSON" "['schemaVersion']" "codelattice.ask.v2"
            assert_json_field "ask intent" "$ASK_JSON" "['intent']" "explain_flow"
            assert_json_field "ask targetQuery" "$ASK_JSON" "['targetQuery']" "helper"
            ASK_ORCH=$(echo "$ASK_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); steps=d.get('orchestration',{}).get('stepsAttempted',[]); print('has_call_chains' if 'call_chains:executed' in steps else 'missing')" 2>/dev/null || echo "parse_error")
            assert_eq "ask orchestration has call_chains:executed" "has_call_chains" "$ASK_ORCH"
        fi

        section "Dev: cache status smoke"
        CACHE_JSON=$(mcp_call "$DEV_BINARY" "tools/call" "{\"name\":\"codelattice_cache\",\"arguments\":{\"mode\":\"status\"}}")
        if [ -z "$CACHE_JSON" ]; then
            fail "cache status returned empty"
        else
            ok "cache status returned non-empty"
        fi
    fi
fi

if [ "$INSTALLED_ONLY" = true ] || ([ "$DEV_ONLY" = false ] && [ -x "$INSTALLED_WRAPPER" ]); then
    section "Installed Version"

    if [ ! -x "$INSTALLED_WRAPPER" ]; then
        fail "Installed wrapper not found: $INSTALLED_WRAPPER"
    else
        INST_VERSION=$("$INSTALLED_WRAPPER" --version 2>&1 | head -1 || echo "unknown")
        echo "  Installed version: $INST_VERSION"

        if [ -f "$INSTALLED_DIR/manifest.json" ]; then
            echo "  Manifest:"
            python3 -c "import json; d=json.load(open('$INSTALLED_DIR/manifest.json')); print(f'    sourceCommit: {d.get(\"sourceCommit\",\"N/A\")}'); print(f'    serverVersion: {d.get(\"serverVersion\",\"N/A\")}'); print(f'    toolCount: {d.get(\"toolCount\",\"N/A\")}')" 2>/dev/null || echo "    (manifest parse error)"
        else
            echo "  No manifest.json found"
        fi

        section "Installed: Self-test"
        if "$INSTALLED_WRAPPER" --self-test 2>/dev/null | grep -q "Self-test passed"; then
            ok "Installed self-test passed"
        else
            fail "Installed self-test failed or not supported"
        fi

        section "Installed: Default AI Tools (expect 6)"
        INST_TOOLS=$(mcp_call "$INSTALLED_WRAPPER" "tools/list" '{}')
        if [ -z "$INST_TOOLS" ]; then
            fail "Installed tools/list returned empty"
        else
            INST_COUNT=$(echo "$INST_TOOLS" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('tools',[])))" 2>/dev/null || echo "0")
            assert_eq "Installed default toolset" "6" "$INST_COUNT"
        fi
    fi
fi

if [ "$OPEN_NWE" = true ]; then
    OPEN_NWE_PATH="$ROOT/../open-nwe"
    section "Open-NWE Read-Only Smoke"
    if [ ! -d "$OPEN_NWE_PATH" ]; then
        fail "open-nwe not found at $OPEN_NWE_PATH"
    else
        echo "  Analyzing open-nwe (read-only)..."
        ONWE_RESULT=$("$DEV_BINARY" analyze --root "$OPEN_NWE_PATH" --language auto --format json 2>/dev/null | python3 -c "
import sys,json
d=json.load(sys.stdin)
g=d.get('graph',{})
nodes=g.get('nodes',[])
edges=g.get('edges',[])
files=[n for n in nodes if n.get('label')=='source-file' or n.get('kind')=='source-file']
print(f'files={len(files)}, nodes={len(nodes)}, edges={len(edges)}')
" 2>/dev/null || echo "analysis failed")
        echo "  $ONWE_RESULT"
        if echo "$ONWE_RESULT" | grep -q "analysis failed"; then
            fail "open-nwe analysis failed"
        else
            ok "open-nwe read-only analysis completed"
        fi
    fi
fi

if [ "$SYNC" = true ]; then
    section "Sync to Installed"

    if [ ! -d "$INSTALLED_DIR" ]; then
        fail "Installed dir not found: $INSTALLED_DIR"
    else
        echo "  Building..."
        cargo build --manifest-path "$ROOT/Cargo.toml" --bin codelattice 2>&1 | tail -1

        echo "  Copying binary..."
        cp "$DEV_BINARY" "$INSTALLED_BIN"
        ok "Binary copied to $INSTALLED_BIN"

        if [ -f "$INSTALLED_DIR/manifest.json" ]; then
            echo "  Updating manifest..."
            python3 -c "
import json, datetime
m = json.load(open('$INSTALLED_DIR/manifest.json'))
m['sourceCommit'] = '$HEAD_COMMIT'
m['installedAt'] = datetime.datetime.now().isoformat()
json.dump(m, open('$INSTALLED_DIR/manifest.json','w'), indent=2)
print('  manifest updated')
"
            ok "Manifest updated"
        fi

        echo ""
        echo "  ⚠️  IMPORTANT: Restart your MCP session (Claude/OpenCode/TRAE) to pick up the new binary."
    fi
fi

section "Summary"
echo "  HEAD: $HEAD_COMMIT"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "❌ Acceptance FAILED with $FAIL failures."
    exit 1
fi

echo "✅ Acceptance PASSED."
