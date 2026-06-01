#!/usr/bin/env bash
# Smoke test for the CodeLattice-native `detect-changes` CLI command.
#
# This creates a temporary git-backed Rust fixture, modifies one function, and
# verifies that `codelattice detect-changes` returns a static-only JSON report.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="$REPO_ROOT/target/debug/codelattice"
TMP="$(mktemp -d /tmp/codelattice-detect-changes-smoke-XXXXXX)"
NONGIT_TMP=""
PASS=0
FAIL=0

cleanup() {
  rm -rf "$TMP"
  if [[ -n "${NONGIT_TMP:-}" ]]; then
    rm -rf "$NONGIT_TMP"
  fi
}
trap cleanup EXIT

check() {
  local label="$1"
  local cmd="$2"
  if eval "$cmd"; then
    PASS=$((PASS + 1))
    echo "PASS: $label"
  else
    FAIL=$((FAIL + 1))
    echo "FAIL: $label"
  fi
}

echo "--- CodeLattice detect-changes smoke ---"
echo "Repo: $REPO_ROOT"
echo "Temp: $TMP"

echo "1. Build debug codelattice binary"
cargo build -p gitnexus-rust-core-cli --bin codelattice --quiet

mkdir -p "$TMP/src"
cat >"$TMP/Cargo.toml" <<'EOF'
[package]
name = "detect-changes-smoke"
version = "0.1.0"
edition = "2021"
EOF
cat >"$TMP/src/lib.rs" <<'EOF'
pub fn helper() -> i32 {
    41
}

pub fn entry() -> i32 {
    helper()
}
EOF

git -C "$TMP" init >/dev/null
git -C "$TMP" config user.email "smoke@example.com"
git -C "$TMP" config user.name "Smoke"
git -C "$TMP" add .
git -C "$TMP" commit -m baseline >/dev/null

cat >"$TMP/src/lib.rs" <<'EOF'
pub fn helper() -> i32 {
    99
}

pub fn entry() -> i32 {
    helper()
}
EOF
echo 'pub fn new_helper() {}' >"$TMP/src/new_module.rs"
mkdir -p "$TMP/.arts" "$TMP/.sisyphus/run-continuation"
echo '{"private":true}' >"$TMP/.arts/settings.json"
echo '{"session":"private"}' >"$TMP/.sisyphus/run-continuation/session.json"

REPORT="$TMP/report.json"
"$BIN" detect-changes \
  --root "$TMP" \
  --language rust \
  --scope all \
  --format json \
  >"$REPORT"

check "JSON parse" "python3 -m json.tool '$REPORT' >/dev/null"
check "schema version" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d['schemaVersion'] == 'codelattice.detectChanges.v1'
PY"
check "diff mode maps all to head" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d['diffMode'] == 'head'
PY"
check "changed file count > 0" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d['summary']['changedFileCount'] > 0
PY"
check "changed symbols include helper" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert any(s.get('name') == 'helper' for s in d.get('changedSymbols', []))
PY"
check "untracked file count > 0" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d['summary']['untrackedFileCount'] > 0
assert 'src/new_module.rs' in d.get('untrackedFiles', [])
assert not any(p.startswith('.arts/') or p.startswith('.sisyphus/') for p in d.get('untrackedFiles', []))
PY"
check "static-only generatedFrom" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
g = d['generatedFrom']
assert g['staticAnalysis'] is True
assert g['runtimeVerified'] is False
assert g['noWrites'] is True
assert g['nativeCodeLattice'] is True
PY"
check "underlying tools recorded" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
tools = d.get('underlyingTools', [])
assert 'codelattice_changed_symbols' in tools
assert 'codelattice_production_assist' in tools
PY"
check "workspace fields present" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert 'workspaceContext' in d, 'missing workspaceContext'
assert 'fileOwners' in d, 'missing fileOwners'
assert 'affectedProjects' in d, 'missing affectedProjects'
assert 'affectedWorkspaceEdges' in d, 'missing affectedWorkspaceEdges'
assert 'unsupportedBoundaryHits' in d, 'missing unsupportedBoundaryHits'
assert 'crossProjectRisk' in d, 'missing crossProjectRisk'
assert 'recommendedFollowups' in d, 'missing recommendedFollowups'
PY"
check "workspaceContext has required fields" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
ws = d.get('workspaceContext')
if ws:
    assert 'isWorkspace' in ws, 'missing isWorkspace'
    assert 'workspaceGraphAvailable' in ws, 'missing workspaceGraphAvailable'
    assert 'projectCount' in ws, 'missing projectCount'
PY"
check "generatedFrom has workspaceGraphEnabled" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
g = d['generatedFrom']
assert 'workspaceGraphEnabled' in g, 'missing workspaceGraphEnabled'
PY"
check "underlying tools include workspace" "python3 - '$REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
tools = d.get('underlyingTools', [])
assert 'codelattice_workspace_graph' in tools, 'missing workspace_graph'
assert 'codelattice_cross_project_impact' in tools, 'missing cross_project_impact'
PY"

echo "2. Non-git repo graceful failure"
NONGIT_TMP="$(mktemp -d /tmp/codelattice-detect-nongit-XXXXXX)"
NONGIT="$NONGIT_TMP/nongit"
mkdir -p "$NONGIT/src"
cat >"$NONGIT/Cargo.toml" <<'EOF'
[package]
name = "nongit"
version = "0.1.0"
edition = "2021"
EOF
echo 'pub fn helper() {}' >"$NONGIT/src/lib.rs"
if "$BIN" detect-changes --root "$NONGIT" --language rust >/tmp/codelattice-detect-nongit.out 2>/tmp/codelattice-detect-nongit.err; then
  echo "FAIL: non-git repo should fail"
  FAIL=$((FAIL + 1))
else
  if grep -q "changed_symbols" /tmp/codelattice-detect-nongit.err; then
    echo "PASS: non-git repo returns changed_symbols error"
    PASS=$((PASS + 1))
  else
    echo "FAIL: non-git error did not mention changed_symbols"
    FAIL=$((FAIL + 1))
  fi
fi

echo "3. Workspace config change has graph impact"
WS="$TMP/workspace"
mkdir -p "$WS"
cp -R "$REPO_ROOT/fixtures/workspace/multi-project/." "$WS/"
git -C "$WS" init >/dev/null
git -C "$WS" config user.email "smoke@example.com"
git -C "$WS" config user.name "Smoke"
git -C "$WS" add .
git -C "$WS" commit -m baseline >/dev/null
printf '\n# smoke tweak\n' >>"$WS/.github/workflows/ci.yml"
WS_REPORT="$TMP/workspace-report.json"
"$BIN" detect-changes \
  --root "$WS" \
  --language rust \
  --scope all \
  --format json \
  >"$WS_REPORT"
check "workspace config owner resolves to graph node" "python3 - '$WS_REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
owners = d.get('fileOwners', [])
assert any(o.get('ownerKind') == 'config' and o.get('ownerNodeId', '').startswith('workflow:') for o in owners), owners
PY"
check "workspace config change affects projects" "python3 - '$WS_REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert len(d.get('affectedProjects', [])) > 0
assert any(e.get('kind') == 'config_refs' for e in d.get('affectedWorkspaceEdges', []))
PY"

echo "4. Workspace impact precision filters fixture-only adjacency noise"
PREC="$TMP/precision-workspace"
mkdir -p "$PREC/app/src" "$PREC/fixtures/corpus-one/src" "$PREC/fixtures/corpus-two/src"
cat >"$PREC/app/Cargo.toml" <<'EOF'
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
corpus-one = { path = "../fixtures/corpus-one" }
EOF
cat >"$PREC/app/src/lib.rs" <<'EOF'
pub fn app_entry() -> i32 {
    1
}
EOF
cat >"$PREC/fixtures/corpus-one/Cargo.toml" <<'EOF'
[package]
name = "corpus-one"
version = "0.1.0"
edition = "2021"
EOF
echo 'pub fn fixture_one() {}' >"$PREC/fixtures/corpus-one/src/lib.rs"
cat >"$PREC/fixtures/corpus-two/Cargo.toml" <<'EOF'
[package]
name = "corpus-two"
version = "0.1.0"
edition = "2021"
EOF
echo 'pub fn fixture_two() {}' >"$PREC/fixtures/corpus-two/src/lib.rs"
git -C "$PREC" init >/dev/null
git -C "$PREC" config user.email "smoke@example.com"
git -C "$PREC" config user.name "Smoke"
git -C "$PREC" add .
git -C "$PREC" commit -m baseline >/dev/null
cat >"$PREC/app/src/lib.rs" <<'EOF'
pub fn app_entry() -> i32 {
    2
}
EOF
PREC_REPORT="$TMP/precision-report.json"
"$BIN" detect-changes \
  --root "$PREC" \
  --language rust \
  --scope all \
  --format json \
  >"$PREC_REPORT"
check "daily precision suppresses fixture-only projects" "python3 - '$PREC_REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
summary = d.get('workspaceImpactSummary') or {}
policy = summary.get('policy') or {}
assert policy.get('includeFixtures') is False
assert policy.get('strictWorkspace') is False
assert summary.get('suppressedProjectCount', 0) >= 1, summary
assert summary.get('fixtureOnlyCount', 0) >= 1, summary
labels = {p.get('label') for p in d.get('affectedProjects', [])}
assert 'corpus-one' not in labels and 'corpus-two' not in labels, labels
assert d.get('crossProjectRisk') in ('low', 'medium'), d.get('crossProjectRisk')
PY"
PREC_FULL_REPORT="$TMP/precision-report-full.json"
"$BIN" detect-changes \
  --root "$PREC" \
  --language rust \
  --scope all \
  --format json \
  --include-fixtures \
  >"$PREC_FULL_REPORT"
check "include-fixtures exposes suppressed fixture projects" "python3 - '$PREC_FULL_REPORT' <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
summary = d.get('workspaceImpactSummary') or {}
policy = summary.get('policy') or {}
assert policy.get('includeFixtures') is True
labels = {p.get('label') for p in d.get('affectedProjects', [])}
assert 'corpus-one' in labels, labels
assert summary.get('suppressedProjectCount', 0) == 0, summary
PY"

echo ""
echo "PASS: $PASS"
echo "FAIL: $FAIL"

if [[ "$FAIL" -ne 0 ]]; then
  exit 1
fi

echo "ALL PASS"
