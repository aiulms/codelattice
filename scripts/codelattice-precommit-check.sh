#!/usr/bin/env bash
# CodeLattice-native pre-commit governance check.
#
# This script intentionally uses CodeLattice's own CLI/MCP capabilities for the
# daily change review path. Legacy GitNexus-Tool checks are fallback/comparison
# only and are not required for normal CodeLattice development.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="$REPO_ROOT/target/debug/codelattice"
TMP_DIR="$(mktemp -d /tmp/codelattice-precommit-XXXXXX)"
REPORT="${CODELATTICE_PRECOMMIT_REPORT:-/tmp/codelattice-precommit-detect-changes.json}"

LANGUAGE="rust"
SCOPE="all"
RUN_FULL_TEST=0
FAIL_ON_HIGH_RISK=0

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

usage() {
  cat <<'USAGE'
Usage: scripts/codelattice-precommit-check.sh [options]

Options:
  --language <lang>       Language for native detect-changes (default: rust)
  --scope <scope>         Change scope: all/staged/unstaged/head (default: all)
  --full                  Also run full `cargo test`
  --fail-on-high-risk     Exit non-zero when native detect-changes reports high/critical
  -h, --help              Show this help

This script runs:
  1. cargo fmt --check
  2. git diff --check
  3. cargo test --test productization_commands
  4. cargo test --test mcp_server
  5. scripts/codelattice-detect-changes-smoke.sh
  6. codelattice detect-changes --scope <scope>

It does not call GitNexus-Tool by default.
Set CODELATTICE_PRECOMMIT_REPORT to override the output report path.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --language)
      LANGUAGE="${2:-}"
      shift 2
      ;;
    --scope)
      SCOPE="${2:-}"
      shift 2
      ;;
    --full)
      RUN_FULL_TEST=1
      shift
      ;;
    --fail-on-high-risk)
      FAIL_ON_HIGH_RISK=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$LANGUAGE" || -z "$SCOPE" ]]; then
  echo "language and scope must be non-empty" >&2
  exit 2
fi

step() {
  echo
  echo "==> $*"
}

step "CodeLattice native precommit check"
echo "Repo:     $REPO_ROOT"
echo "Language: $LANGUAGE"
echo "Scope:    $SCOPE"

cd "$REPO_ROOT"

step "cargo fmt --check"
cargo fmt --check

step "git diff --check"
git diff --check

step "cargo test --test productization_commands"
cargo test --test productization_commands

step "cargo test --test mcp_server"
cargo test --test mcp_server

step "CodeLattice detect-changes smoke"
bash "$REPO_ROOT/scripts/codelattice-detect-changes-smoke.sh"

if [[ "$RUN_FULL_TEST" -eq 1 ]]; then
  step "cargo test"
  cargo test
fi

step "Build debug codelattice binary"
cargo build -p gitnexus-rust-core-cli --bin codelattice --quiet

step "Native detect-changes"
"$BIN" detect-changes \
  --root "$REPO_ROOT" \
  --language "$LANGUAGE" \
  --scope "$SCOPE" \
  --format json \
  >"$REPORT"

CHANGE_REVIEW="/tmp/codelattice-change-review.json"
cp "$REPORT" "$CHANGE_REVIEW"

python3 - "$REPORT" "$FAIL_ON_HIGH_RISK" <<'PY'
import json
import sys

path = sys.argv[1]
fail_on_high = sys.argv[2] == "1"
data = json.load(open(path))
summary = data.get("summary", {})
risk = summary.get("riskLevel", "unknown")
ws = data.get("workspaceContext", {}) or {}
affected_projects = data.get("affectedProjects", []) or []
unsupported_hits = data.get("unsupportedBoundaryHits", []) or []
cross_risk = data.get("crossProjectRisk")
file_owners = data.get("fileOwners", []) or []
followups = data.get("recommendedFollowups", []) or []

print(f"schema: {data.get('schemaVersion')}")
print(f"risk: {risk}")
print(f"tracked files: {summary.get('changedFileCount')}")
print(f"untracked files: {summary.get('untrackedFileCount')}")
print(f"total files: {summary.get('totalFileChangeCount')}")
print(f"changed symbols: {summary.get('changedSymbolCount')}")
print(f"unknown hunks: {summary.get('unknownHunkCount')}")
affected_process_count = summary.get("affectedProcessCount")
print(f"affectedProcessCount: {affected_process_count if affected_process_count is not None else 'null'}")

# workspace 摘要
print()
print("── workspace ──")
print(f"workspace: {ws.get('workspaceRoot', 'N/A')}")
print(f"isWorkspace: {ws.get('isWorkspace', False)}")
print(f"projects: {ws.get('projectCount', 0)} ({ws.get('supportedProjectCount', 0)} supported, {ws.get('unsupportedProjectCount', 0)} unsupported)")
print(f"graph available: {ws.get('workspaceGraphAvailable', False)}")
print(f"affected projects: {len(affected_projects)}")
print(f"workspace edges: {len(data.get('affectedWorkspaceEdges', []) or [])}")
print(f"unsupported boundary hits: {len(unsupported_hits)}")
print(f"cross-project risk: {cross_risk or 'N/A'}")

# 中文风险摘要
print()
print("── 变更智能摘要 ──")
print(f"风险等级: {risk}")
print(f"变更文件数: {summary.get('changedFileCount', 0)}")
print(f"变更符号数: {summary.get('changedSymbolCount', 0)}")
print(f"受影响项目数: {len(affected_projects)}")
print(f"跨项目风险: {cross_risk or '无'}")
print(f"不支持语言边界: {len(unsupported_hits)} 个")

if unsupported_hits:
    for hit in unsupported_hits[:3]:
        print(f"  ⚠ {hit.get('label', '?')} ({hit.get('language', '?')}) — {hit.get('reason', '')}")

if followups:
    print("推荐跟进:")
    for f in followups[:5]:
        print(f"  → {f}")

if data.get("schemaVersion") != "codelattice.detectChanges.v1":
    raise SystemExit("unexpected detect-changes schema")

if data.get("generatedFrom", {}).get("nativeCodeLattice") is not True:
    raise SystemExit("detect-changes report is not native CodeLattice output")

if risk in {"high", "critical"}:
    print(f"\n⚠️  风险等级为 {risk}，请在提交前审查变更内容")
    if fail_on_high:
        raise SystemExit(1)
PY

echo
echo "Native report: $REPORT"
echo "Change review: $CHANGE_REVIEW"
echo "All CodeLattice native precommit checks completed."
