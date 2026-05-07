# Slice 19 Preflight — Cangjie Reference Source Endpoint Integrity Repair

**Date:** 2026-05-07  
**Status:** Preflight  
**Type:** Bug Fix / Endpoint Integrity Repair  
**Slice ID:** Phase 2 Slice 19  

## Background

Slice 18 production fixture smoke 发现严重的 graph endpoint integrity 问题：

**问题数据：**
- nodes = 715
- edges = 3,401
- dangling target edges = 0 ✅
- dangling source edges = 2,687 ❌
- unique dangling source ids = 646 ❌
- dangling source edge kind = 全部为 `uses`
- dangling source id 格式：`Constructor:/absolute/path/to/file:ClassName.init#arity`

## Root Cause Analysis

### 关键纠偏

**不是 target symbol extraction 问题：**
- Target endpoints 都存在（dangling target edges = 0）
- 问题出在 **source endpoints**

**Root Cause：ID 策略不一致**

1. **Reference extraction 生成 source ID：**
   - 位置：`crates/cangjie/src/extractors/references.rs::build_source_id()`
   - 格式：`Constructor:<absolute-file-path>:<Owner>.init#<arity>`
   - 示例：`Constructor:/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui/src/action_handoff.cj:CjguiInternalActionHandoffAcceptance.init#5`

2. **Symbol extraction 生成 node ID：**
   - 位置：`crates/cangjie/src/graph.rs::symbol_node_id()`
   - 格式：`sym:<relative-file-path>:<Kind>:<name>`
   - 示例：`sym:src/action_handoff.cj:Struct:CjguiInternalActionHandoffAcceptance`

3. **Graph 中缺失的内容：**
   - 没有 Constructor / Method / Function source scope nodes
   - Reference source IDs 无法在 graph nodes 中找到匹配项

### Impact 范围

- **影响模块：** Cangjie graph source node / source id alignment
- **影响功能：** 所有构造函数调用的 USES edges（19% 的 edges 损坏）
- **不影响：** Target endpoints（dangling target edges = 0）

## Goals

### Primary Goals

1. **修复 reference source endpoint integrity**
   - 使所有 reference source IDs 都能在 graph nodes 中找到匹配项
   - 消除 646 个 unique dangling source IDs
   - 消除 2,687 个 dangling source edges

2. **保持语义诚实**
   - 不通过删除 edges 来掩盖问题
   - 不把 source 降级成 SourceFile node（除非作为最后 fallback）
   - 正确表达构造函数调用的语义

3. **可测试性**
   - 添加 endpoint integrity regression test
   - 验证 constructor body 内的 reference sources 能命中 graph nodes

### Secondary Goals

4. **扩展性**
   - 如果发现 Method / Function source IDs 也有同类风险，一并修复
   - 抽出共享 ID builder，但不要扩大到 method dispatch

## Write Set

**必须修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/`：
  - `docs/plans/`：添加 preflight / execution card / closure review
  - `crates/cangjie/src/extractors/`：修改 reference extraction 和/或 symbol extraction
  - `crates/cangjie/src/graph.rs`：修改 graph emission
  - `tests/`：添加 endpoint integrity regression test

**可选修改：**
- `fixtures/`：添加小型 constructor fixture（如需要）

**禁止修改：**
- `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/`：只读访问
- `/Users/jiangxuanyang/Desktop/cangjie/`：live repo，禁止修改
- GitNexus-RC runtime/schema/package/web
- GitNexus-RC-Tool

## Stop-lines

**严格执行：**
- ❌ 不做 type inference / trait solving
- ❌ 不做 method dispatch（保持 low-confidence heuristic only）
- ❌ 不做 macro expansion
- ❌ 不引入 LSP/MCP/HTTP/UI/embedding
- ❌ 不修改 GitNexus-RC / Tool / live repo

**修复边界：**
- ✅ 只修复 source endpoint integrity（不扩展 target 解析）
- ✅ 保持在 bounded API 范围内（不大幅重构）
- ✅ 优先使用最小、语义诚实、可测试的方案

## Implementation Options

### 方案 A（推荐）：扩展 Symbol Extraction

**思路：**
- 扩展 Cangjie symbol extraction，识别构造函数（init 方法）作为独立 symbols
- 为构造函数创建 Symbol nodes，使其与 reference source IDs 对齐
- 统一 ID 生成策略（使用共享 builder）

**实现步骤：**
1. 识别 `init` 方法节点（tree-sitter query）
2. 生成构造函数 Symbol nodes
3. 统一 ID 生成策略（`Constructor:` 前缀或改为 `sym:` 前缀）
4. 在 graph emission 中包含构造函数 nodes

**优点：**
- ✅ 语义准确（构造函数作为独立的 callable symbols）
- ✅ 为未来 method dispatch 功能打基础
- ✅ 符合 graph schema 的语义模型

**缺点：**
- ⚠️ 需要扩展 symbol extraction API（~200-300 行代码）
- ⚠️ 可能影响现有 symbol extraction 逻辑

**预估工作量：** ~300-400 行代码，~4-6 小时

### 方案 B（备选）：Synthetic Source Nodes

**思路：**
- 不改变 symbol extraction 主模型
- 在 graph emission 阶段为 reference source IDs emit synthetic nodes
- 节点标记 `synthetic = true`，`kind = Constructor|Method|Function`

**实现步骤：**
1. 收集所有 reference source IDs
2. 为每个 unique source ID emit synthetic node
3. 标记 synthetic = true

**优点：**
- ✅ 实现简单（~100-150 行代码）
- ✅ 不影响现有 symbol extraction 逻辑
- ✅ 风险可控

**缺点：**
- ⚠️ 语义降级（synthetic nodes 不是真实的 symbols）
- ⚠️ 未来可能需要重构为真实 symbols

**预估工作量：** ~100-150 行代码，~2-3 小时

### 禁止方案

❌ **删除 constructor/reference edges：** 掩盖问题，不解决 root cause

❌ **Source 降级成 SourceFile node：** 语义不准确，丢失调用点信息

❌ **新增 target-only constructor node：** 不解决 dangling source 问题

## Diagnostic Requirements

### 1. Endpoint Audit Helper

先写一个小型 helper/test，复现当前问题：

```rust
// 对 production fixture 或小型 fixture 输出：
- danglingSourceEdges: 总数
- danglingSourceUnique: 唯一 ID 数量
- danglingTargetEdges: 总数
- danglingByKind: 按边类型分类
- source id sample: 前 5 个 dangling source ID 示例
```

### 2. Regression Test

添加 regression test，至少覆盖：
- Constructor body 内的 type/use/call reference sources 能命中 graph nodes
- Target endpoints 仍能命中 graph nodes
- 验证 no dangling sources after fix

### 3. 禁止事项

- ❌ 不要把 production fixture JSON 提交到仓库
- ✅ 只提交统计报告和测试代码

## Acceptance Criteria

### Must Have
- [ ] `cargo fmt --check` pass
- [ ] `cargo test` pass（192/192）
- [ ] `cargo test --features tree-sitter-cangjie` pass（259/259）
- [ ] 小型 fixture endpoint integrity test pass
- [ ] Production fixture smoke pass：
  - root: `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui`
  - danglingTargetEdges = 0（保持）
  - danglingSourceEdges 显著下降；理想为 0
  - 如果不能为 0，必须列出 remaining source id pattern 和原因
- [ ] 输出确定性保持：连续两次 nodes/edges count 稳定
- [ ] 不提交 production smoke 输出 JSON

### Should Have
- [ ] 记录 before/after endpoint integrity 数据对比
- [ ] 记录采用的方案（A/B）
- [ ] 记录修改的文件列表

### Nice to Have
- [ ] 如果 Method / Function source IDs 也有同类风险，一并修复
- [ ] 抽出共享 ID builder

## Risk Assessment

### Medium Risk
- ⚠️ 方案 A 需要扩展 symbol extraction API，可能影响现有逻辑
- ⚠️ ID 策略统一可能影响其他引用类型

### Low Risk
- ✅ 只修改 Cangjie 相关代码，不影响 Rust project model
- ✅ 只读访问 production fixture
- ✅ 不修改 GitNexus-RC / Tool / live repo

### Mitigation
- ✅ 预先写 endpoint audit helper，量化问题范围
- ✅ 优先使用方案 B（风险更低），如方案 A 风险过大
- ✅ 添加 regression test，防止回退

## Exit Criteria

Slice 19 完成的标志：
1. ✅ Production fixture smoke 通过（danglingSourceEdges 显著下降）
2. ✅ 小型 fixture endpoint integrity test 通过
3. ✅ `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` 全部 pass
4. ✅ Closure review 完成
5. ✅ Commit + push gitcode master
6. ✅ docs/plans/README.md 更新

## Next Openings

根据 Slice 19 结果，选择下一个 bounded slice：
- **Option A:** 如修复成功 → 继续扩展现有能力
- **Option B:** 如发现新问题 → 修复新问题
- **Option C:** 如一切正常 → 继续其他 Cangjie 功能

## Dependencies

- ✅ Slice 18 (Cangjie production fixture smoke) 已完成
- ✅ 问题明确：646 个 dangling source edges
- ✅ Root cause 已确认：ID 策略不一致

## Timeline Estimate

- Preflight: 已完成
- Endpoint audit helper: 30 分钟
- Implementation (方案 B): ~2-3 小时
- Regression tests: 1 小时
- Verification: 30 分钟
- Closure review: 30 分钟
- **Total: ~4-5 小时**（如采用方案 B）

---

**Decision:** Proceed to execution card（推荐方案 B：Synthetic Source Nodes，风险更低）。
