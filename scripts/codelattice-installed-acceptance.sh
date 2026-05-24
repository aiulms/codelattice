#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEV_BINARY="$ROOT/target/debug/codelattice"
DEV_COMPAT_BINARY="$ROOT/target/debug/gitnexus-rust-core-cli"
INSTALLED_DIR="/Users/jiangxuanyang/Desktop/CodeLattice-Tool"
INSTALLED_WRAPPER="$INSTALLED_DIR/codelattice-mcp.sh"
INSTALLED_BIN="$INSTALLED_DIR/bin/codelattice"
INSTALLED_LEGACY_ALIAS_BIN="$INSTALLED_DIR/bin/codelattice-cli"
INSTALLED_COMPAT_BIN="$INSTALLED_DIR/bin/gitnexus-rust-core-cli"
FIXTURE="$ROOT/fixtures/call-resolution/c1-same-module"
ALL_LANGUAGE_FEATURES="tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python"

SYNC=false
DEV_ONLY=false
INSTALLED_ONLY=false
OPEN_NWE=false
REQUIRE_FRESH_INSTALLED=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --sync) SYNC=true; shift ;;
        --dev-only) DEV_ONLY=true; shift ;;
        --installed-only) INSTALLED_ONLY=true; shift ;;
        --open-nwe) OPEN_NWE=true; shift ;;
        --require-fresh-installed) REQUIRE_FRESH_INSTALLED=true; shift ;;
        --help) echo "Usage: $0 [--sync] [--dev-only] [--installed-only] [--open-nwe] [--require-fresh-installed]"; exit 0 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

PASS=0
FAIL=0
WARN=0

section() { echo ""; echo "=== $1 ==="; }

ok() { echo "  ✅ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); }
warn() { echo "  ⚠️  $1"; WARN=$((WARN + 1)); }

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

sync_installed_profile_json() {
    local attempt
    local out

    for attempt in 1 2 3; do
        out=$(printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"acceptance-sync","version":"1.0"}}}' \
            | env CODELATTICE_MCP_TOOLSET=full "$INSTALLED_BIN" mcp 2>/dev/null \
            | python3 -c 'import json,sys
for line in sys.stdin:
    text=line.strip()
    if not text:
        continue
    try:
        doc=json.loads(text)
    except Exception:
        continue
    if doc.get("id") == 1:
        print(json.dumps(doc, separators=(",", ":")))
        break' 2>/dev/null || true)
        if [ -n "$out" ]; then
            printf '%s\n' "$out"
            return 0
        fi

        # macOS 偶尔会对刚替换的二进制出现一次性拒绝启动；重拷贝同一份
        # runtime 后再试，避免 sync 留下 stale manifest/profile。
        rm -f "$INSTALLED_COMPAT_BIN"
        cp "$DEV_COMPAT_BINARY" "$INSTALLED_COMPAT_BIN"
        chmod +x "$INSTALLED_COMPAT_BIN"
        sleep 0.2
    done

    printf '{}\n'
    return 1
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
            CC_BYTES=$(printf '%s' "$CC_JSON" | wc -c | tr -d ' ')
            if [ "$CC_BYTES" -le 12000 ]; then
                ok "call_chains compact payload <= 12KB"
            else
                fail "call_chains compact payload too large: ${CC_BYTES} bytes"
            fi
            CC_CHAINS=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); c=d.get('callChains',[]); print('has_chains' if c and isinstance(c[0]['chain'], list) else 'no_chains')" 2>/dev/null || echo "parse_error")
            assert_eq "call_chains has chain array" "has_chains" "$CC_CHAINS"
            CC_HAS_RO=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'readOrder' in d else 'no')" 2>/dev/null || echo "PARSE_ERROR")
            assert_eq "call_chains has readOrder" "yes" "$CC_HAS_RO"
            CC_HAS_FI=$(echo "$CC_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'filesInvolved' in d else 'no')" 2>/dev/null || echo "PARSE_ERROR")
            assert_eq "call_chains has filesInvolved" "yes" "$CC_HAS_FI"
        fi

        section "Dev: compact root diagnosis hygiene"
        HYGIENE_JSON=$(mcp_call "$DEV_BINARY" "tools/call" "{\"name\":\"codelattice_project\",\"arguments\":{\"root\":\"$ROOT/fixtures/workspace\",\"language\":\"auto\",\"mode\":\"overview\",\"compact\":true}}")
        if [ -z "$HYGIENE_JSON" ]; then
            fail "compact project hygiene returned empty"
        else
            HYGIENE_BYTES=$(printf '%s' "$HYGIENE_JSON" | wc -c | tr -d ' ')
            if [ "$HYGIENE_BYTES" -le 12000 ]; then
                ok "compact project payload <= 12KB"
            else
                fail "compact project payload too large: ${HYGIENE_BYTES} bytes"
            fi
            HYGIENE_HAS_FULL=$(echo "$HYGIENE_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'sourceOnlyEntries' in d.get('rootDiagnosis',{}) else 'no')" 2>/dev/null || echo "parse_error")
            assert_eq "compact rootDiagnosis omits sourceOnlyEntries" "no" "$HYGIENE_HAS_FULL"
            HYGIENE_HAS_PREVIEW=$(echo "$HYGIENE_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); preview=d.get('rootDiagnosis',{}).get('sourceOnlyEntryPreview',[]); print('yes' if isinstance(preview, list) and len(preview) <= 5 else 'no')" 2>/dev/null || echo "parse_error")
            assert_eq "compact rootDiagnosis has bounded preview" "yes" "$HYGIENE_HAS_PREVIEW"
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

if [ "$INSTALLED_ONLY" = true ] || ([ "$DEV_ONLY" = false ] && [ "$SYNC" = false ] && [ -x "$INSTALLED_WRAPPER" ]); then
    section "Installed Version"

    if [ ! -x "$INSTALLED_WRAPPER" ]; then
        fail "Installed wrapper not found: $INSTALLED_WRAPPER"
    else
        INST_VERSION=$("$INSTALLED_WRAPPER" --version 2>&1 | head -1 || echo "unknown")
        echo "  Installed version: $INST_VERSION"

        if [ -f "$INSTALLED_DIR/manifest.json" ]; then
            echo "  Manifest:"
            python3 -c "import json; d=json.load(open('$INSTALLED_DIR/manifest.json')); p=d.get('profile', {}); print(f'    sourceCommit: {d.get(\"sourceCommit\",\"N/A\")}'); print(f'    serverVersion: {d.get(\"serverVersion\", p.get(\"serverVersion\", \"N/A\"))}'); print(f'    toolCount: {d.get(\"toolCount\", p.get(\"toolCount\", \"N/A\"))}')" 2>/dev/null || echo "    (manifest parse error)"
            INSTALLED_SOURCE_COMMIT=$(python3 -c "import json; d=json.load(open('$INSTALLED_DIR/manifest.json')); print(d.get('sourceCommit',''))" 2>/dev/null || echo "")
            if [ -n "$INSTALLED_SOURCE_COMMIT" ] && [ "$INSTALLED_SOURCE_COMMIT" != "$HEAD_COMMIT" ]; then
                warn "Installed binary is stale: installed=$INSTALLED_SOURCE_COMMIT, source=$HEAD_COMMIT. Run scripts/codelattice-installed-acceptance.sh --sync to update CodeLattice-Tool."
                if [ "$REQUIRE_FRESH_INSTALLED" = true ]; then
                    fail "Installed sourceCommit does not match current HEAD while --require-fresh-installed is set"
                fi
            elif [ -n "$INSTALLED_SOURCE_COMMIT" ]; then
                ok "Installed sourceCommit matches current HEAD"
            fi
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
        cargo build --manifest-path "$ROOT/Cargo.toml" -p gitnexus-rust-core-cli --features "$ALL_LANGUAGE_FEATURES" --bins 2>&1 | tail -1

        echo "  Copying binary..."
        cp "$DEV_COMPAT_BINARY" "$INSTALLED_COMPAT_BIN"
        chmod +x "$INSTALLED_COMPAT_BIN"
        rm -f "$INSTALLED_BIN"
        rm -f "$INSTALLED_LEGACY_ALIAS_BIN"
        ln -s gitnexus-rust-core-cli "$INSTALLED_BIN"
        ln -s gitnexus-rust-core-cli "$INSTALLED_LEGACY_ALIAS_BIN"
        ok "Runtime copied to $INSTALLED_COMPAT_BIN with codelattice/codelattice-cli links"

        SYNC_PROFILE_JSON=$(sync_installed_profile_json || true)
        BINARY_SHA256=$(shasum -a 256 "$INSTALLED_COMPAT_BIN" | awk '{print $1}')

        if [ -f "$INSTALLED_DIR/manifest.json" ]; then
            echo "  Updating manifest..."
            PROFILE_JSON="$SYNC_PROFILE_JSON" BINARY_SHA256="$BINARY_SHA256" HEAD_COMMIT="$HEAD_COMMIT" python3 -c "
import datetime, json, os
manifest_path = '$INSTALLED_DIR/manifest.json'
m = json.load(open(manifest_path))
profile = json.loads(os.environ.get('PROFILE_JSON') or '{}')
server = profile.get('result', {}).get('serverInfo', {})
m['sourceCommit'] = os.environ['HEAD_COMMIT']
m['installedAt'] = datetime.datetime.now().isoformat()
m['binary'] = 'bin/codelattice'
m['legacyAliasBinary'] = 'bin/codelattice-cli'
m['compatBinary'] = 'bin/gitnexus-rust-core-cli'
m['binarySha256'] = os.environ['BINARY_SHA256']
m.setdefault('paths', {})['binary'] = 'bin/codelattice'
m['paths']['legacyAliasBinary'] = 'bin/codelattice-cli'
m['paths']['compatBinary'] = 'bin/gitnexus-rust-core-cli'
m['profile'] = {
    'serverVersion': server.get('version', 'unknown'),
    'cangjieSupport': bool(server.get('cangjieSupport', False)),
    'arktsSupport': bool(server.get('arktsSupport', False)),
    'typescriptSupport': bool(server.get('typescriptSupport', False)),
    'javascriptSupport': bool(server.get('javascriptSupport', False)),
    'cSupport': bool(server.get('cSupport', False)),
    'cppSupport': bool(server.get('cppSupport', False)),
    'pythonSupport': bool(server.get('pythonSupport', False)),
    'shellSupport': bool(server.get('shellSupport', False)),
    'toolCount': int(server.get('toolCount', 0) or 0),
}
json.dump(m, open(manifest_path,'w'), indent=2)
print('  manifest updated')
"
            ok "Manifest updated"
        fi

        echo "  Verifying synced installed runtime..."
        if "$INSTALLED_WRAPPER" --self-test >/dev/null 2>&1; then
            ok "Synced installed self-test passed"
        else
            rm -f "$INSTALLED_COMPAT_BIN"
            cp "$DEV_COMPAT_BINARY" "$INSTALLED_COMPAT_BIN"
            chmod +x "$INSTALLED_COMPAT_BIN"
            if "$INSTALLED_WRAPPER" --self-test >/dev/null 2>&1; then
                ok "Synced installed self-test passed after retry"
            else
                fail "Synced installed self-test failed"
            fi
        fi
        SYNC_TOOLS_JSON=$(mcp_call "$INSTALLED_BIN" "tools/list" '{}')
        if [ -z "$SYNC_TOOLS_JSON" ]; then
            fail "Synced installed tools/list returned empty"
        else
            SYNC_TOOL_COUNT=$(echo "$SYNC_TOOLS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d.get('tools',[])))" 2>/dev/null || echo "0")
            assert_eq "Synced installed default toolset count" "6" "$SYNC_TOOL_COUNT"
        fi

        echo ""
        echo "  ⚠️  IMPORTANT: Restart your MCP session (Claude/OpenCode/TRAE) to pick up the new binary."
    fi
fi

section "Summary"
echo "  HEAD: $HEAD_COMMIT"
echo "  Passed: $PASS"
echo "  Warnings: $WARN"
echo "  Failed: $FAIL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "❌ Acceptance FAILED with $FAIL failures."
    exit 1
fi

echo "✅ Acceptance PASSED."
