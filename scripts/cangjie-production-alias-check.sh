#!/usr/bin/env bash
# cangjie-production-alias-check.sh
# Read-only stable window check for the live Cangjie repo.
# Determines whether the live repo is in a stable enough state for production smoke.
#
# Usage:
#   bash scripts/cangjie-production-alias-check.sh --status
#   bash scripts/cangjie-production-alias-check.sh --smoke
#   bash scripts/cangjie-production-alias-check.sh --full
#
# Modes:
#   --status   Show HEAD, dirty count, stable window assessment
#   --smoke    Run MCP smoke on live root (cangjie-live-codelattice-smoke.sh --mcp)
#   --full     Run full production pipeline (smoke + tool-ingest)
#
# Stable window rules:
#   dirty <= 10: green  — safe for full production smoke + default switch consideration
#   dirty 11-50: yellow — readonly analyze/mcp OK, do not switch defaults
#   dirty > 50:  red    — large diff, recommend waiting before production smoke
#
# Hard rules:
#   - NEVER modify cangjie source code
#   - NEVER write AGENTS.md/CLAUDE.md to live repo
#   - All temp output to /tmp/codelattice-cangjie-live-*

set -euo pipefail

LIVE_REPO="/Users/jiangxuanyang/Desktop/cangjie"
LIVE_ROOT="$LIVE_REPO/runtime/cjgui"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMOKE_SCRIPT="$SCRIPT_DIR/cangjie-live-codelattice-smoke.sh"
TOOL_CLI="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"
REGISTRY_NAME="cangjie-live-codelattice"

MODE="${1:---status}"

# ── Colors (no-ops if not a terminal) ─────────────────────────
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    NC='\033[0m'
else
    GREEN='' YELLOW='' RED='' NC=''
fi

# ── Gather live repo state ─────────────────────────────────────

echo "=== Cangjie Production Alias Check ==="
echo "Mode: $MODE"
echo ""

if [ ! -d "$LIVE_REPO/.git" ]; then
    echo "FATAL: Not a git repo: $LIVE_REPO"
    exit 1
fi

HEAD=$(git -C "$LIVE_REPO" rev-parse --short HEAD 2>/dev/null || echo "unknown")
DIRTY=$(git -C "$LIVE_REPO" status --short 2>/dev/null | wc -l | tr -d ' ')
MODIFIED=$(git -C "$LIVE_REPO" diff --name-only 2>/dev/null | wc -l | tr -d ' ')
UNTRACKED=$(git -C "$LIVE_REPO" ls-files --others --exclude-standard 2>/dev/null | wc -l | tr -d ' ')
BRANCH=$(git -C "$LIVE_REPO" branch --show-current 2>/dev/null || echo "detached")

# Determine window level
if [ "$DIRTY" -le 10 ]; then
    LEVEL="green"
    LEVEL_LABEL="${GREEN}GREEN${NC}"
    RECOMMENDATION="Stable window: safe for full production smoke + default switch consideration"
elif [ "$DIRTY" -le 50 ]; then
    LEVEL="yellow"
    LEVEL_LABEL="${YELLOW}YELLOW${NC}"
    RECOMMENDATION="Moderate diff: readonly analyze/mcp OK, do not switch defaults"
else
    LEVEL="red"
    LEVEL_LABEL="${RED}RED${NC}"
    RECOMMENDATION="Large diff: recommend waiting before production smoke"
fi

# ── Status output ──────────────────────────────────────────────

echo "Live repo: $LIVE_REPO"
echo "Branch:    $BRANCH"
echo "HEAD:      $HEAD"
echo "Modified:  $MODIFIED files"
echo "Untracked: $UNTRACKED files"
echo "Dirty:     $DIRTY total"
echo ""
echo "Stable window: $LEVEL_LABEL (dirty=$DIRTY)"
echo "  $RECOMMENDATION"
echo ""

# Registry status
echo "=== Registry Status ==="
if [ -f "$TOOL_CLI" ]; then
    REG_ENTRY=$(node "$TOOL_CLI" list 2>&1 | grep -A3 "$REGISTRY_NAME" || echo "  (not found)")
    echo "$REG_ENTRY"
else
    echo "  Tool CLI not found: $TOOL_CLI"
fi
echo ""

# ── Mode execution ─────────────────────────────────────────────

if [ "$MODE" = "--status" ]; then
    echo "Status only. No smoke tests run."
    exit 0
fi

if [ "$MODE" = "--smoke" ] || [ "$MODE" = "--full" ]; then
    # Warn if red/yellow
    if [ "$LEVEL" = "red" ]; then
        echo "⚠️  RED window: $DIRTY dirty files. Proceeding with readonly smoke..."
        echo "   (Full production pipeline not recommended until dirty count drops.)"
        echo ""
        if [ "$MODE" = "--full" ]; then
            echo "Downgrading --full to --smoke due to RED window."
            MODE="--smoke"
        fi
    elif [ "$LEVEL" = "yellow" ]; then
        echo "⚠️  YELLOW window: $DIRTY dirty files. Readonly smoke proceeding..."
        echo ""
        if [ "$MODE" = "--full" ]; then
            echo "Note: --full will proceed but defaults should NOT be switched at this time."
        fi
    fi

    # Run smoke
    if [ -f "$SMOKE_SCRIPT" ]; then
        echo "=== Running MCP Smoke ==="
        bash "$SMOKE_SCRIPT" --mcp 2>&1
    else
        echo "FATAL: Smoke script not found: $SMOKE_SCRIPT"
        exit 1
    fi

    # Full mode also runs tool-ingest
    if [ "$MODE" = "--full" ]; then
        echo ""
        echo "=== Running Full Pipeline ==="
        if [ -f "$SMOKE_SCRIPT" ]; then
            bash "$SMOKE_SCRIPT" --analyze 2>&1
            bash "$SMOKE_SCRIPT" --tool-ingest 2>&1
        fi
    fi
fi

echo ""
echo "=== Production Alias Recommendation ==="
echo "  Recommended entry: cangjie-live-codelattice"
echo "  Fixture entry:     cjgui-index (path: /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index)"
echo "  Deprecated:        cjgui (ambiguous — two entries share this name)"
echo ""
echo "Done."
