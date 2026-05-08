# Cangjie Production Acceptance 固化 — Closure Review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 4 — Cangjie Maintenance + Quality Gate 固化
**Commit：** `e81fe19`

---

## 总结

本轮不新增功能，聚焦于将已完成的 Cangjie Production Acceptance Stages 1-3 固化为可长期使用、可回归、可交接的生产化质量门。通过基线全量回归验证 + 过期文档同步，确保所有声明与实际落地状态一致。

## 变更内容

### 基线验证

| 验证项 | 结果 |
|--------|------|
| `cargo fmt --check` | clean |
| `git diff --check` | clean |
| `cargo test` (no-feature) | 全部通过（~200 tests） |
| `cargo test --features tree-sitter-cangjie` | 全部通过（~330 tests） |
| `cangjie_inspect` (18 tests) | 18/18 pass |
| `graph_contract` (24 tests, 4 fixtures) | 24/24 pass |
| `multi_project_smoke` fixture (4 targets) | 4/4 pass（synth=0, dup=0, dang=(0,0), det=true） |
| `multi_project_smoke` production (4 targets) | 4/4 pass（synth=0, dup=0, dang=(0,0), det=true） |

### Production Smoke 生产级验证

| Target | Nodes | Edges | Synthetic | Duplicate | Dangling | Deterministic |
|--------|-------|-------|-----------|-----------|----------|---------------|
| cjgui-GitNexus-Index | 903 | 3252 | 0 | 0 | (0,0) | true |
| cjgui-cangjie | 2287 | 6199 | 0 | 0 | (0,0) | true |
| web_framework | 151 | 180 | 0 | 0 | (0,0) | true |
| json_parser | 150 | 158 | 0 | 0 | (0,0) | true |
| **Total** | **3491** | **9789** | **0** | **0** | **(0,0)** | **true** |

所有 synthetic nodes = 0（Constructor=0, Method=0, Function=0），全部 production targets 达到生产质量门。

### 文档同步

| 文件 | 变更 |
|------|------|
| `README.md` | 更新 Last updated → 2026-05-08；resolution rate → 62.4%；test counts → 200+/330+；HEAD → 496941c；新增 enum constructor call form；Verification 节更新为完整质量门命令 |
| `AGENTS.md` | 更新 CALLS resolution rate → 62.4%（2183/3500） |
| `docs/plans/README.md` | 替换过期"Slice 7 recommended next"为当前状态总结 + 实际 openings（Priority 2 Rust CALLS + Priority 4 Cangjie maintenance） |

## Cangjie 当前 Readiness 判断

**状态：READY for local trial use as development-quality graph tool**

全部 8 项质量门通过：

| Gate | Status |
|------|--------|
| Duplicate node IDs = 0 | ✅ 4/4 production + 4/4 fixture |
| Duplicate edge triples = 0 | ✅ 4/4 production + 4/4 fixture |
| Dangling source = 0 | ✅ 4/4 production + 4/4 fixture |
| Dangling target = 0 | ✅ 4/4 production + 4/4 fixture |
| Deterministic output | ✅ 4/4 production + 4/4 fixture |
| Synthetic nodes = 0 | ✅ Constructor/Method/Function all 0 |
| Init symbols have #arity | ✅ graph_contract + multi_project_smoke |
| Strict CLI tests | ✅ cangjie_inspect 18/18 |

## Rust 当前 Readiness 判断

- Resolution rate: 62.4%（2183/3500）
- 0 dangling CALLS edges（external symbol node completion）
- Graph contract: 8/8 pass（0 dup, 0 dangling, deterministic）
- Enum constructor resolution: 305/305 resolved
- 主缺口：1255 unresolved method-calls（stop-line: no type inference）

## Stop-lines 合规

- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未新增依赖
- ✅ 未做 destructive git 操作
- ✅ 未新增 extractor 功能
- ✅ 未做 type inference / trait solving / macro expansion

## 下一轮 Opening

**Priority 2 续 — Rust CALLS resolution quality：改善 receiver type annotation 扫描**

当前 1255 unresolved method-calls 中的部分可通过扩展 `scan_variable_type_annotation` 的扫描范围来解决（跨语句 let chain、match scrutinee 类型传播等），不需要 full type inference。同时可修复 ~38 个 free-function 调用（来自闭包/嵌套函数中的 same-file 解析失败）。

**或 Priority 5 — Bridge preparation docs-only**（如果用户认为两线质量门都已稳定）
