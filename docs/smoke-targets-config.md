# Smoke Targets Config — Read-only Local Trial Profile

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Active
> **用途：** 描述本机 read-only smoke 目标列表，供 CLI 或 test 手动执行

---

## 设计原则

- **不写 live repo：** 所有目标只读
- **缺失路径 graceful skip：** 路径不存在时跳过并记录原因
- **可由 test 或 CLI 手动执行**
- **输出 summary report**

---

## Smoke 目标列表

### Tier 1 — Always Available（repo 内 fixtures）

这些目标始终可用，不依赖外部 repo。

| # | 路径 | 语言 | 类型 | 预期结果 |
|---|------|------|------|---------|
| 1 | `fixtures/rust/portable-smoke/` | Rust | repo fixture | 16 nodes, 25 edges, 0 dup, 0 dangling |
| 2 | `fixtures/rust/imports-cross-crate/` | Rust | repo fixture | 14 nodes, 22 edges, external symbols |
| 3 | `fixtures/rust/multi-module/` | Rust | repo fixture | 10 nodes, 12 edges, cross-file CALLS |
| 4 | `fixtures/rust/module-hierarchy/` | Rust | repo fixture | 13 nodes, 15 edges, crate::/super::/import |
| 5 | `fixtures/rust/inline-module/` | Rust | repo fixture | 12 nodes, 18 edges, HAS_PARENT |
| 6 | `fixtures/rust/self-path/` | Rust | repo fixture | self:: calls, module hierarchy |
| 7 | `fixtures/rust/enum-variant/` | Rust | repo fixture | 7 variant symbols, HAS_PARENT |
| 8 | `fixtures/rust/workspace-member/` | Rust | repo fixture | workspace + 2 crates, cross-crate CALLS |
| 9 | `fixtures/cangjie/portable-smoke/` | Cangjie | repo fixture | 27 nodes, 36 edges, 0 synthetic |
| 10 | `fixtures/cangjie/imports-basic/` | Cangjie | repo fixture | Named/grouped/wildcard imports |
| 11 | `fixtures/cangjie/constructor-basic/` | Cangjie | repo fixture | Multi-init class, Init #arity |
| 12 | `fixtures/cangjie/reference-cross-file-basic/` | Cangjie | repo fixture | Cross-file Uses edges |

### Tier 2 — Machine-Local (需要本地 repo 存在)

这些目标依赖本机文件系统上的外部 repo。

| # | 路径 | 语言 | 缺失行为 |
|---|------|------|---------|
| 13 | `../gitnexus-rust-core/`（自身） | Rust | 几乎总可用 |
| 14 | `../cangjie-GitNexus-Index/runtime/cjgui/` | Cangjie | skip if absent |
| 15 | `../cangjie/runtime/cjgui/` | Cangjie | skip if absent |
| 16 | `../CangjieSkills/` 的 web_framework test | Cangjie | skip if absent |
| 17 | `../CangjieSkills/` 的 json_parser test | Cangjie | skip if absent |

---

## CLI Smoke 命令

### Rust

```bash
# analyze
cargo run -p gitnexus-rust-core-cli --bin codelattice -- analyze --root fixtures/rust/portable-smoke --format json

# quality
cargo run -p gitnexus-rust-core-cli --bin codelattice -- quality --root fixtures/rust/portable-smoke --language rust

# summary
cargo run -p gitnexus-rust-core-cli --bin codelattice -- summary --root fixtures/rust/portable-smoke --language rust

# 自身 smoke
cargo run -p gitnexus-rust-core-cli --bin codelattice -- analyze --root . --language auto
```

### Cangjie

```bash
# 需要 --features tree-sitter-cangjie
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bin codelattice -- \
  analyze --root fixtures/cangjie/portable-smoke --format json

cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bin codelattice -- \
  quality --root fixtures/cangjie/portable-smoke --language cangjie
```

---

## Quick Verification Script

```bash
#!/bin/bash
# 快速验证所有 Tier 1 smoke targets
set -e
cd "$(dirname "$0")/.."

echo "=== Rust fixture smoke ==="
for f in portable-smoke imports-cross-crate multi-module module-hierarchy inline-module self-path enum-variant workspace-member; do
  echo "  $f:"
  cargo run -p gitnexus-rust-core-cli --bin codelattice -- summary --root "fixtures/rust/$f" --language rust --format json 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'    nodes={d[\"graphSummary\"][\"nodeCount\"]} edges={d[\"graphSummary\"][\"edgeCount\"]} passed={d[\"qualitySummary\"][\"passed\"]}/{d[\"qualitySummary\"][\"total\"]}')"
done

echo ""
echo "=== Cangjie fixture smoke ==="
for f in portable-smoke imports-basic constructor-basic reference-cross-file-basic; do
  echo "  $f:"
  cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bin codelattice -- summary --root "fixtures/cangjie/$f" --language cangjie --format json 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'    nodes={d[\"graphSummary\"][\"nodeCount\"]} edges={d[\"graphSummary\"][\"edgeCount\"]} passed={d[\"qualitySummary\"][\"passed\"]}/{d[\"qualitySummary\"][\"total\"]}')"
done

echo ""
echo "=== GitNexus Rust-core self smoke ==="
cargo run -p gitnexus-rust-core-cli --bin codelattice -- summary --root . --language rust --format json 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'  nodes={d[\"graphSummary\"][\"nodeCount\"]} edges={d[\"graphSummary\"][\"edgeCount\"]} symbols={d[\"graphSummary\"][\"symbolCount\"]}')"

echo ""
echo "=== Done ==="
```

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.0.0 | 初始版本：16 个 smoke targets（12 Tier 1 + 5 Tier 2），CLI smoke 命令，Quick Verification Script |
