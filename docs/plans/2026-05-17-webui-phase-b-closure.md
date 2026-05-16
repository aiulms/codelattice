# WebUI Phase B — Closure Review

> **日期:** 2026-05-17
> **阶段:** Phase B (Graph Visualization + Snapshot Diff + Smoke Hardening)
> **状态:** ✅ 完成

## 1. Scope Compliance

| # | Check | Status |
|---|-------|--------|
| 1 | 只修改 CodeLattice repo | ✅ |
| 2 | 不修改 GitNexus-RC / Tool / CodeLattice-Tool | ✅ |
| 3 | 不引入 npm/yarn/前端框架 | ✅ |
| 4 | 不做后端/MCP/Live Mode | ✅ |
| 5 | 不做桌面壳 | ✅ |
| 6 | Graph 用原生 DOM（无 D3/cytoscape） | ✅ |
| 7 | 不声称 runtime/deletion proof | ✅ |

## 2. 交付物

| 文件 | 变更 |
|------|------|
| `scripts/codelattice-snapshot-gen.py` | +build_graph_section (nodes/edges/summary)；+全局路径泄露最后防线 |
| `webui/snapshot-viewer/index.html` | +Graph tab + Diff tab |
| `webui/snapshot-viewer/app.js` | +renderGraph/+selectGraphNode/+loadDiffSnapshot/+computeAndRenderDiff/+stableSymbolKey/+deltaBadge |
| `webui/snapshot-viewer/styles.css` | +.graph-layout/.detail-table/.diff-controls |
| `scripts/webui-viewer-smoke.sh` | 重写：消除 pipe subshell 问题，5 语言 matrix 强制验证 |
| `scripts/webui-snapshot-smoke.sh` | 重写：独立 Python validator + 强制 5 语言 |
| `fixtures/webui-snapshots/*.json` | 全部 re-gen：graph section + 零路径泄露 |

## 3. Verification

| 验证项 | 结果 |
|--------|------|
| cargo fmt --check | ✅ PASS |
| git diff --check | ✅ PASS |
| cargo test --test mcp_server | ✅ 114 passed |
| codelattice-mcp.sh --self-test | ✅ 37 tools |
| mcp-dogfood.sh | ✅ PASS |
| viewer-smoke --skip-browser | ✅ 26/26, Matrix 5/5 |
| 5 语言 JSON valid | ✅ ALL VALID |
| Path leak check | ✅ Zero leaks |
| detect-changes | ✅ LOW (WebUI-only) |
| Index refresh | ✅ 7,595/14,152 |

### Fixture Matrix

| 语言 | Nodes | Edges | Calls | Symbols | Source Files |
|------|-------|-------|-------|---------|-------------|
| Rust | 15 | 25 | 5 | 9 | 2 |
| TypeScript | 26 | 29 | - | 20 | 4 |
| C | 10 | 14 | - | 22 | 3 |
| C++ | 10 | 15 | - | 33 | 3 |
| Python | 10 | 11 | 1 | 23 | 5 |

## 4. Smoke Hardening 修复

- **snapshot-smoke**: 消除 `pipe | while read` 子 shell 丢失计数器问题，改用独立 Python 文件做验证
- **viewer-smoke**: 消除 subshell 问题，`TD` 变量初始化，Matrix 改为强制 5/5 验证
- **路径泄露**: 增加 string-level 全局替换（`Desktop/codelattice`→`project/codelattice`）作为最后防线

## 5. Known Limitations

- Graph 用 DOM 列表渲染（非 Canvas/SVG 力导向图）
- Diff 基于可识别的 explore 数据（C/C++/Python 源文件数 = 0 但符号列表有数据）
- 无跨语言 symbol diff
- 无跨版本 snapshot diff 时间线

## 6. 下一步建议

1. **Live MCP Mode** — WebSocket/streaming 实时查询
2. **Canvas/SVG Graph** — 力导向布局图
3. **Desktop Shell** — Tauri/Electron/HarmonyOS PC
4. **Snapshot Timeline** — 多版本 diff 时间线
5. **ArkTS/Cangjie** — 多语言 matrix 扩展