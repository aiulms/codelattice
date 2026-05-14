#!/usr/bin/env bash
# typescript-real-project-smoke.sh — Read-only TypeScript analysis smoke test.
#
# Generates a synthetic multi-file TypeScript project in /tmp and runs
# CodeLattice analyze on it. Does NOT require npm/tsc/any external tool.
#
# Usage: bash scripts/typescript-real-project-smoke.sh [--project <path>]
#
# --project: Analyze an existing TS project instead of the synthetic one.
#            No npm/tsc is run; only read-only static analysis.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse args
PROJECT=""
if [[ "${1:-}" == "--project" ]]; then
    PROJECT="${2:-}"
    if [[ ! -d "$PROJECT" ]]; then
        echo "FAIL: --project path does not exist: $PROJECT"
        exit 1
    fi
fi

# Find binary
BIN=""
for candidate in \
    "$REPO_ROOT/target/debug/codelattice" \
    "$REPO_ROOT/target/release/codelattice"; do
    if [[ -x "$candidate" ]]; then
        BIN="$candidate"
        break
    fi
done

if [[ -z "$BIN" ]]; then
    echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli --features tree-sitter-typescript --bins"
    exit 1
fi

# Check if TS feature is compiled
TS_CHECK=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"/tmp","language":"typescript"}}}' \
    | "$BIN" mcp 2>/dev/null | head -1 || true)
if echo "$TS_CHECK" | grep -q "typescript_disabled\|not compiled"; then
    echo "SKIP: TypeScript feature not compiled. Rebuild with --features tree-sitter-typescript"
    exit 0
fi

PASS=0
FAIL=0

if [[ -n "$PROJECT" ]]; then
    ANALYZE_ROOT="$PROJECT"
    echo "=== TypeScript Real Project Smoke ==="
    echo "Project: $PROJECT"
else
    # Generate synthetic project
    SYNTH="/tmp/codelattice-ts-smoke-$$"
    mkdir -p "$SYNTH/src/models" "$SYNTH/src/services" "$SYNTH/src/utils"
    cat > "$SYNTH/tsconfig.json" << 'TSEOF'
{ "compilerOptions": { "target": "ES2020", "module": "ESNext", "strict": true }, "include": ["src"] }
TSEOF
    cat > "$SYNTH/package.json" << 'TSEOF'
{ "name": "synthetic-smoke", "version": "1.0.0", "private": true }
TSEOF
    cat > "$SYNTH/src/utils/math.ts" << 'TSEOF'
export function add(a: number, b: number): number { return a + b; }
export function multiply(a: number, b: number): number { return a * b; }
export function subtract(a: number, b: number): number { return a - b; }
TSEOF
    cat > "$SYNTH/src/utils/format.ts" << 'TSEOF'
export function formatCurrency(amount: number): string { return `$${amount.toFixed(2)}`; }
export function formatDate(iso: string): string { return iso.split('T')[0]; }
TSEOF
    cat > "$SYNTH/src/models/user.ts" << 'TSEOF'
export interface User { id: string; name: string; email: string; }
export type UserStatus = "active" | "inactive" | "suspended";
export class UserRecord implements User { constructor(public id: string, public name: string, public email: string) {} getStatus(): UserStatus { return "active"; } }
TSEOF
    cat > "$SYNTH/src/models/product.ts" << 'TSEOF'
export interface Product { id: string; name: string; price: number; }
export class ProductCatalog { private items: Product[] = []; add(product: Product): void { this.items.push(product); } find(id: string): Product | undefined { return this.items.find(p => p.id === id); } }
TSEOF
    cat > "$SYNTH/src/services/order.ts" << 'TSEOF'
import { add, multiply } from "../utils/math";
import { formatCurrency } from "../utils/format";
import type { User } from "../models/user";
import type { Product } from "../models/product";
export interface Order { id: string; user: User; products: Product[]; }
export class OrderService { calculateTotal(order: Order): number { return order.products.reduce((sum, p) => add(sum, p.price), 0); } formatTotal(order: Order): string { return formatCurrency(this.calculateTotal(order)); } }
TSEOF
    cat > "$SYNTH/src/index.ts" << 'TSEOF'
import { UserRecord } from "./models/user";
import { ProductCatalog } from "./models/product";
import { OrderService } from "./services/order";
export function main(): void {
  const user = new UserRecord("1", "Alice", "alice@example.com");
  const catalog = new ProductCatalog();
  const orderService = new OrderService();
  console.log(user.name, catalog.find("p1"), orderService.formatTotal({ id: "o1", user, products: [] }));
}
TSEOF
    ANALYZE_ROOT="$SYNTH"
    echo "=== TypeScript Synthetic Project Smoke ==="
    echo "Root: $SYNTH"
    trap 'rm -rf "$SYNTH"' EXIT
fi

echo "Binary: $BIN"
echo ""

# Run analysis
RESULT=$("$BIN" analyze --root "$ANALYZE_ROOT" --language typescript --format json 2>/dev/null) || {
    echo "FAIL: analyze command failed"
    exit 1
}

# Parse results
SYMBOL_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['symbolCount'])" 2>/dev/null || echo "0")
NODE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['nodeCount'])" 2>/dev/null || echo "0")
EDGE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['summary']['edgeCount'])" 2>/dev/null || echo "0")
FILE_COUNT=$(echo "$RESULT" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['summary'].get('sourceFileCount', len([n for n in d['graph']['nodes'] if n.get('kind') in ('sourceFile','source-file')])))" 2>/dev/null || echo "0")

echo "Results:"
echo "  sourceFileCount: $FILE_COUNT"
echo "  symbolCount:     $SYMBOL_COUNT"
echo "  nodeCount:       $NODE_COUNT"
echo "  edgeCount:       $EDGE_COUNT"
echo ""

# Assertions
if [[ "$FILE_COUNT" -gt 0 ]]; then ((PASS++)); else ((FAIL++)); echo "FAIL: sourceFileCount > 0"; fi
if [[ "$SYMBOL_COUNT" -gt 0 ]]; then ((PASS++)); else ((FAIL++)); echo "FAIL: symbolCount > 0"; fi
if [[ "$EDGE_COUNT" -gt 0 ]]; then ((PASS++)); else ((FAIL++)); echo "FAIL: edgeCount > 0"; fi
if [[ "$NODE_COUNT" -gt 0 ]]; then ((PASS++)); else ((FAIL++)); echo "FAIL: nodeCount > 0"; fi

echo ""
echo "=== Results: PASS=$PASS FAIL=$FAIL ==="
[[ $FAIL -eq 0 ]] && echo "All checks passed." || echo "Some checks failed."
exit $FAIL
