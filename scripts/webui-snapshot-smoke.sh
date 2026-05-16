#!/bin/bash
set -e
SD="$(cd "$(dirname "$0")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
SS="$SD/webui-snapshot.sh"
GP="$SD/codelattice-snapshot-gen.py"
KT=""
F=""
LF=""
for a in "$@"; do
  case "$a" in
    --keep-temp) KT=1 ;;
    --full) F="--full" ;;
    --language) shift; LF="$1" ;;
    *) echo "Usage: $0 [--keep-temp] [--full] [--language <lang>]"; exit 1 ;;
  esac
done
P=0; F=0
TD="$(mktemp -d /tmp/cls.XXXXXX)"
trap '[ -z "$KT" ] && rm -rf "$TD"' EXIT
pass() { P=$((P+1)); echo "  [PASS] $1"; }
fail() { F=$((F+1)); echo "  [FAIL] $1"; }
section() { echo ""; echo "=== $1 ==="; }
echo "CodeLattice Snapshot Smoke (Phase A)"
section Prerequisites
[ -x "$SS" ] || fail "no snapshot script" && exit 1
pass "script exists"
CB=""
for c in "$WS/target/release/codelattice" "$WS/target/debug/codelattice"; do
  [ -x "$c" ] && CB="$c" && break
done
[ -z "$CB" ] && fail "no codelattice binary" && exit 1
pass "binary: $CB"
command -v python3 >/dev/null 2>&1 || fail "no python3" && exit 1
pass "python3 OK"
[ -f "$GP" ] && pass "gen.py exists" || fail "gen.py missing"
SL=""
for L in rust typescript c cpp python; do
  [ -n "$LF" ] && [ "$LF" != "$L" ] && continue
  section "Generate $L"
  FX="$WS/fixtures/$L/portable-smoke"
  O="$TD/$L.json"
  [ -d "$FX" ] || fail "no fixture: $FX" || continue
  bash "$SS" --root "$FX" --language "$L" --output "$O" --redact-root ${F:---full} >/dev/null 2>&1 || fail "generate failed" || continue
  [ -f "$O" ] || fail "no output" || continue
  SZ=$(wc -c < "$O" | tr -d " ")
  pass "$L ($SZ bytes)"
  SL="$SL $L:$O"
done
section Validate
for E in $SL; do
  LN="${E%%:*}"
  FL="${E#*:}"
  section "Validate $LN"
  python3 -c "
import json,sys
with open('$FL') as D:
    S=json.load(D)
def C(n,c):
    print(('PASS' if c else 'FAIL')+':'+n)
C('v1',S.get('schemaVersion')=='webui.snapshot.v1')
G=S.get('generatedFrom',{})
C('static',G.get('staticAnalysis') is True)
C('!runtime',G.get('runtimeVerified') is False)
E=S.get('explore',{});SD=S.get('summary',{})
H=SD.get('sourceFileCount',0)>0 or len(E.get('symbols',[]))>0 or len(E.get('sourceFiles',[]))>0
C('data',H)
C('quality',bool(S.get('quality')))
LM=S.get('limitations',{})
C('limits',bool(LM) and (len(LM) if isinstance(LM,list) else bool(LM.get('notes'))))
C('explore',len(E.get('symbols',[]))>0 or len(E.get('sourceFiles',[]))>0)
WP=S.get('workflowPresets',{})
C('workflows>=10',len(WP.get('presets',[]))>=10)
R=json.dumps(S)
C('no_leak','/Users/' not in R)
" | while IFS=: read -R S L; do
    T=$((T+1))
    if [ "$S" = PASS ]; then P=$((P+1)); echo "  PASS: $L"; else F=$((F+1)); echo "  FAIL: $L"; fi
done
T=$((P+F))
echo ""
echo "=== Results: $P passed, $F failed, $T total ==="
if [ $F -gt 0 ]; then echo SMOKE FAILED; exit 1; fi
echo SMOKE PASSED
