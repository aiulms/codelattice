#!/usr/bin/env bash
# webui-snapshot.sh — Generate CodeLattice WebUI Snapshot (V1) — Phase A Enriched
#
# P3 起本脚本只是 `codelattice analyze --format webui-snapshot` 的瘦包装：
# 全部段落（summary/quality/explore/cleanup/releaseReview/insights/
# workflowPresets/graph/moduleGraph/limitations）与 --redact-root 脱敏
# 都由 CLI 转换器 crates/cli/src/webui_snapshot.rs 单一事实源产出，
# 旧的 Python 聚合脚本已退役删除。
#
# 兼容性：--full / --include-explore / --include-review / --include-workflows
# 保留接受（CLI 恒输出全量段，等价于全开）；--compact / --no-enrichment 已移除。

set -euo pipefail

ROOT=""
LANGUAGE="auto"
OUTPUT=""
REDACT_ROOT=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<EOF
Usage: $(basename "$0") --root <path> --language <lang> --output <path> [options]

Generate a CodeLatticeWebSnapshotV1 JSON (Phase A enriched) for WebUI consumption.
The snapshot is produced by the codelattice CLI converter (single source of truth).

Required:
  --root <path>        Project root directory
  --language <lang>    rust|cangjie|arkts|typescript|javascript|c|cpp|python|shell|auto
  --output <path>      Output JSON file path, or '-' for stdout

Options:
  --redact-root        Redact absolute paths to <redacted-root> in output
  --full               Accepted for compatibility (enrichment is always on)
  --include-explore    Accepted for compatibility (always on)
  --include-review     Accepted for compatibility (always on)
  --include-workflows  Accepted for compatibility (always on)
  -h, --help           Show this help
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --language) LANGUAGE="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --redact-root) REDACT_ROOT=true; shift ;;
    --full|--include-explore|--include-review|--include-workflows) shift ;;
    -h|--help) usage ;;
    *) echo "Error: Unknown argument: $1 (note: --compact/--no-enrichment were removed with the Python snapshot-gen retirement)" >&2; exit 1 ;;
  esac
done

if [[ -z "$ROOT" ]]; then echo "Error: --root is required" >&2; exit 1; fi
if [[ ! "$ROOT" = /* ]]; then ROOT="$(cd "$ROOT" 2>/dev/null && pwd)" || { echo "Error: $ROOT not found" >&2; exit 1; }; fi
if [[ ! -d "$ROOT" ]]; then echo "Error: root not a directory: $ROOT" >&2; exit 1; fi
if [[ -z "$OUTPUT" ]]; then echo "Error: --output is required" >&2; exit 1; fi
if [[ "$OUTPUT" != "-" ]] && [[ ! -d "$(dirname "$OUTPUT")" ]]; then echo "Error: output dir not found: $(dirname "$OUTPUT")" >&2; exit 1; fi

# ── Discover codelattice binary ──────────────────────────────────────────────

CODELATTICE=""
for candidate in "${CODELATTICE_BIN:-}" "${WORKSPACE_ROOT}/target/debug/codelattice" "${WORKSPACE_ROOT}/target/release/codelattice" "$(command -v codelattice 2>/dev/null)"; do
  if [[ -n "${candidate:-}" && -x "${candidate}" ]]; then CODELATTICE="$candidate"; break; fi
done
if [[ -z "$CODELATTICE" ]]; then
  echo "Error: Cannot find codelattice binary. Run 'cargo build --release --bins' first." >&2; exit 1
fi

# ── Validate language ────────────────────────────────────────────────────────

VALID_LANGUAGES=("rust" "cangjie" "arkts" "typescript" "javascript" "c" "cpp" "python" "shell" "auto")
LANG_VALID=0; for vl in "${VALID_LANGUAGES[@]}"; do [[ "$LANGUAGE" == "$vl" ]] && LANG_VALID=1 && break; done
if [[ $LANG_VALID -eq 0 ]]; then echo "Error: unsupported language: $LANGUAGE" >&2; exit 1; fi

# ── Generate snapshot via CLI converter ─────────────────────────────────────

generate_snapshot() {
  local redact_args=()
  $REDACT_ROOT && redact_args+=(--redact-root)
  "$CODELATTICE" analyze \
    --root "$ROOT" \
    --language "$LANGUAGE" \
    --format webui-snapshot \
    --profile full \
    "${redact_args[@]+"${redact_args[@]}"}"
}

# ── Output ───────────────────────────────────────────────────────────────────

if [[ "$OUTPUT" == "-" ]]; then
  generate_snapshot
else
  generate_snapshot > "$OUTPUT"
  echo "[snapshot] Generated: $OUTPUT ($(wc -c < "$OUTPUT" | tr -d ' ') bytes)" >&2
fi
