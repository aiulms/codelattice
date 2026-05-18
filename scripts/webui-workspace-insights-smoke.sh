#!/usr/bin/env bash
set -euo pipefail
# CodeLattice WebUI Workspace Insights Smoke Test (Phase F-polish)
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 28765)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
TD=$(mktemp -d /tmp/cls-wsi.XXXXXX)
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD" "$WF"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "CodeLattice Workspace Insights Smoke (Phase F-polish)"

# ── Fixture ──────────────────────────────────────────────────────────
WF=$(mktemp -d /tmp/cls-wsi-fixture.XXXXXX)
mkdir -p "$WF/rust-hello/src" "$WF/shell-scripts" "$WF/unsupported-csharp" "$WF/unsupported-go" "$WF/.git"
cat > "$WF/rust-hello/Cargo.toml" <<'EOF'
[package]
name = "hello-wsi"
version = "0.1.0"
edition = "2021"
EOF
echo 'fn main() { let a=1; println!("{}",a); }' > "$WF/rust-hello/src/main.rs"
echo '#!/bin/bash' > "$WF/shell-scripts/setup.sh"
cat > "$WF/unsupported-csharp/demo.csproj" <<'EOF'
<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType></PropertyGroup></Project>
EOF
echo 'class Program {}' > "$WF/unsupported-csharp/Program.cs"
echo 'module example' > "$WF/unsupported-go/go.mod"

# ── Start Runner ─────────────────────────────────────────────────────
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null || { fail "not started"; exit 1; }
pass "started"
BASE="http://127.0.0.1:$PORT"

# ── Inventory ─────────────────────────────────────────────────────────
curl -s "$BASE/api/workspace/inventory?root=$WF"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "inventory"||fail "inventory"

# ── Analyze ──────────────────────────────────────────────────────────
AN=$(curl -s -X POST "$BASE/api/workspace/analyze" -H "Content-Type: application/json" -d "{\"root\":\"$WF\",\"mode\":\"recommended\",\"redactRoot\":true}")
WID=$(echo "$AN"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['workspaceId'])" 2>/dev/null)
[[ -n "$WID" ]] && pass "analyze" || { fail "analyze"; echo "  ${AN:0:200}"; }

# ── Insights (GET) ────────────────────────────────────────────────────
[[ -n "$WID" ]] && {
  INS=$(curl -s "$BASE/api/workspace/insights?runId=$WID")
  echo "$INS" | python3 -c "
import json,sys; d=json.load(sys.stdin);
assert d['success'], 'insights GET failed';
ins=d['data'];
sm=ins['summary'];
assert 0 <= sm['overallHealthScore'] <= 100, 'healthScore out of range: '+str(sm['overallHealthScore']);
assert sm['overallRiskLevel'] in ('low','medium','high','unknown'), 'bad riskLevel: '+sm['overallRiskLevel'];
assert len(ins.get('projectScores',[])) >= 1, 'should have project scores';
r1=ins.get('readFirst',[]); r2=ins.get('reviewFirst',[]); r3=ins.get('cleanupFirst',[]);
assert len(r1)+len(r2)+len(r3) >= 1, 'need at least one recommendation list';
cp=ins.get('crossProjectSignals',{});
uc=cp.get('unsupportedLanguageClusters',[]);
assert len(uc)>=1, 'should have unsupported clusters';
gf=ins['generatedFrom'];
assert gf['scriptsExecuted'] is False, 'scriptsExecuted must be false';
cauts=ins.get('cautions',[]); assert len(cauts)>=2, 'should have cautions';
print('OK: health='+str(sm['overallHealthScore'])+'/'+sm['overallRiskLevel']+
  ' scores='+str(len(ins['projectScores']))+
  ' read='+str(len(r1))+' review='+str(len(r2))+' cleanup='+str(len(r3))+
  ' clusters='+str(len(uc)))
" 2>/dev/null && pass "insights GET" || { fail "insights GET"; echo "  ${INS:0:300}"; }

  # ── Insights (POST) ───────────────────────────────────────────────
  INS2=$(curl -s -X POST "$BASE/api/workspace/insights" -H "Content-Type: application/json" -d "{\"workspaceRunId\":\"$WID\"}")
  echo "$INS2" | python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];print('OK: POST health='+str(d['data']['summary']['overallHealthScore']))" 2>/dev/null && pass "insights POST" || fail "insights POST"
}

# ── Error: nonexistent run ───────────────────────────────────────────
curl -s "$BASE/api/workspace/insights?runId=nonexistent" | python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False and d.get('status')==404" 2>/dev/null && pass "insights 404" || fail "insights 404"

# ── Shutdown ──────────────────────────────────────────────────────────
kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"

echo ""
echo "=== Results: $P passed, $F failed, $((P+F)) total ==="
[[ $F -gt 0 ]] && { echo "INSIGHTS SMOKE FAILED"; exit 1; }
echo "INSIGHTS SMOKE PASSED"
