#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
SS="$SD/webui-snapshot.sh"
KT=""
FLAG=""
LF=""
for a in "$@"; do
  case "$a" in --keep-temp) KT=1 ;; --full) FLAG="--full" ;; --language) shift; LF="$1" ;; *) echo "Usage: $0 [--keep-temp] [--full] [--language <lang>]"; exit 1 ;; esac
done
P=0; FA=0; TD="$(mktemp -d /tmp/cls_snap.XXXXXX)"
trap '[[ -z "$KT" ]] && rm -rf "$TD"' EXIT
echo "CodeLattice Snapshot Smoke (Phase B)"
echo ""
echo "=== Prerequisites ==="
if [[ ! -x "$SS" ]]; then
  echo "  [FAIL] webui-snapshot.sh missing or not executable"
  exit 1
fi
echo "  [OK] script"
CB=""
for c in "$WS/target/release/codelattice" "$WS/target/debug/codelattice"; do [[ -x "$c" ]] && CB="$c" && break; done
[[ -z "$CB" ]] && echo "  [FAIL] no codelattice binary" && exit 1 || echo "  [OK] binary: $CB"
command -v python3 >/dev/null 2>&1 || { echo "  [FAIL] no python3"; exit 1; }; echo "  [OK] python3"
GP="$SD/codelattice-snapshot-gen.py"; [[ -f "$GP" ]] && echo "  [OK] gen.py" || { echo "  [FAIL] gen.py missing"; exit 1; }
REQ=(rust typescript javascript c cpp python shell)
SL=""
for L in "${REQ[@]}"; do
  [[ -n "$LF" && "$LF" != "$L" ]] && continue
  echo ""; echo "=== Generate $L ==="
  FX="$WS/fixtures/$L/portable-smoke"
  O="$TD/$L.json"
  [[ -d "$FX" ]] || { FA=$((FA+1)); P=$((P+1)); echo "  [SKIP] no fixture: $FX"; continue; }
  if bash "$SS" --root "$FX" --language "$L" --output "$O" --redact-root ${FLAG:---full} >/dev/null 2>&1; then
    SZ=$(wc -c < "$O" | tr -d ' '); echo "  [OK] $L ($SZ bytes)"; SL="$SL $L:$O"
  else
    FA=$((FA+1)); echo "  [FAIL] $L generate failed"
  fi
done
if [[ -z "$LF" || "$LF" == "arkts-auto" ]]; then
  echo ""; echo "=== Generate arkts-auto ==="
  FX="$WS/fixtures/arkts/portable-smoke"
  O="$TD/arkts_auto.json"
  if [[ -d "$FX" ]] && bash "$SS" --root "$FX" --language auto --output "$O" --redact-root ${FLAG:---full} >/dev/null 2>&1; then
    SZ=$(wc -c < "$O" | tr -d ' '); echo "  [OK] arkts-auto ($SZ bytes)"; SL="$SL arkts_auto:$O"
  else
    FA=$((FA+1)); echo "  [FAIL] arkts-auto generate failed"
  fi
fi
echo ""; echo "=== Validate ==="
PYV="$TD/validate.py"
cat > "$PYV" << 'PYEOF'
import sys, json, os
REQ = ["rust","typescript","javascript","c","cpp","python","shell","arkts_auto"]
results = {"pass":0, "fail":0}
for lang in REQ:
    fpath = os.environ.get(f"SNAP_{lang}","")
    if not fpath or not os.path.isfile(fpath):
        print(f"[SKIP] {lang}: file missing"); continue
    try:
        with open(fpath) as f: d=json.load(f)
    except: print(f"[FAIL] {lang}: JSON parse error"); results["fail"]+=1; continue
    sv=d.get("schemaVersion","")
    if sv!="webui.snapshot.v1": print(f"[FAIL] {lang}: schemaVersion={sv}"); results["fail"]+=1; continue
    gf=d.get("generatedFrom",{})
    if gf.get("staticAnalysis") is not True: print(f"[FAIL] {lang}: !staticAnalysis"); results["fail"]+=1; continue
    if gf.get("runtimeVerified") is not False: print(f"[FAIL] {lang}: runtimeVerified"); results["fail"]+=1; continue
    sd=d.get("summary",{}); e=d.get("explore",{})
    sfc=sd.get("sourceFileCount",0); sc=sd.get("symbolCount",0)
    slang=sd.get("language","")
    if not slang or slang in ("unknown","auto"):
        print(f"[FAIL] {lang}: language={slang}"); results["fail"]+=1; continue
    es=len(e.get("symbols",[])); efs=len(e.get("sourceFiles",[]))
    if max(sfc,es,efs)<=0: print(f"[FAIL] {lang}: no data"); results["fail"]+=1; continue
    if not d.get("quality"): print(f"[FAIL] {lang}: no quality"); results["fail"]+=1; continue
    lm=d.get("limitations",{})
    if not lm or (isinstance(lm,list) and len(lm)==0): print(f"[FAIL] {lang}: no limitations"); results["fail"]+=1; continue
    wp=d.get("workflowPresets",{}); wpn=len(wp.get("presets",[]))
    if wpn<10: print(f"[FAIL] {lang}: workflows={wpn}"); results["fail"]+=1; continue
    raw=json.dumps(d)
    if "/Users/" in raw or "/Desktop/codelattice" in raw: print(f"[FAIL] {lang}: path leak"); results["fail"]+=1; continue
    # graph check (Phase B)
    g=d.get("graph",{}); gn=len(g.get("nodes",[])); ge=len(g.get("edges",[]))
    if lang=="arkts_auto" and (gn<=0 or ge<=0):
        print(f"[FAIL] {lang}: graph nodes={gn} edges={ge}"); results["fail"]+=1; continue
    if gn>0 and ge>=0: gstatus="graph_ok"
    else: gstatus="graph_empty"
    print(f"[PASS] {lang}: sfc={sfc}, sc={sc}, es={es}, efs={efs}, wpn={wpn}, {gstatus}")
    results["pass"]+=1
print("")
print(f"RESULTS: {results['pass']} pass, {results['fail']} fail")
exit(1 if results["fail"]>0 else 0)
PYEOF
for L in "${REQ[@]}"; do
  for E in $SL; do LN="${E%%:*}"; F="${E#*:}"; [[ "$LN" == "$L" ]] && export "SNAP_$L=$F" && break; done
done
for E in $SL; do LN="${E%%:*}"; F="${E#*:}"; [[ "$LN" == "arkts_auto" ]] && export SNAP_arkts_auto="$F" && break; done
python3 "$PYV"
RC=$?
T=$((P+FA))
echo ""; echo "=== Gen: $P generated, $FA failed, $T total ==="
exit $RC
