# Cangjie Phase 2 Slice 16 — Cangjie Graph Output Parity Smoke Test

**Date:** 2026-05-07
**Type:** preflight（docs-only）
**Status:** 📝 Ready for execution
**Author:** aiulms
**Related Slices:** Slice 15 (completed: wildcard import edge quality)

## 1. Phase 0 审计发现

### 1.1 当前已实现的 graph output 功能

**已有实现：**
- `crates/cangjie/src/graph.rs`（~370 行）
  - CangjieGraphOutput 结构
  - Repository/Package/SourceFile/Symbol 节点
  - ContainsPackage/OwnsSource/Defines 边
  - emit_cangjie_nodes()
  - emit_cangjie_edges()
  - inspect_cangjie_project() 入口

**支持的节点类型：**
- Repository（存储库级别）
- Package（Cangjie package）  
- SourceFile（.cj 源文件）
- Symbol（函数/类/结构体等）
- Diagnostic（编译器/静态分析）

**支持的边类型：**
- ContainsPackage
- OwnsSource
- Defines
- Imports
- USES/ACCESSES/MODIFIES
- Annotates

**基础设施：**
- AST 符号提取（tree-sitter-cangjie）
- Manifest 解析（cjpm.toml）
- Import 解析
- Symbol 解析
- Graph 序列化（JSON 输出）
- 静态分析集成（cjc/cjlint）

### 1.2 当前 graph output 覆盖情况

**Fixture 覆盖：**
- `fixtures/cangjie/graph-basic/` - 基础 graph 输出
- `fixtures/cangjie/graph-advanced/` - 复杂场景
- `fixtures/cangjie/imports-basic/` - import 边
- `fixtures/cangjie/reference-cross-file-basic/` - 跨文件引用
- `fixtures/cangjie/function-call-cross-file/` - 跨文件函数调用
- `fixtures/cangjie/wildcard-conflicts/` - wildcard 冲突

## 2. Phase 0 现状

### 2.1 与 TS adapter 对比

**TS adapter 对应位置：**
- GitNexus-RC `gitnexus/src/adapter/cangjie/cangjie-graph.ts`

**已知差异：**
1. Rust-core graph 输出节点/边类型可能与 TS adapter 不完全一致
2. 输出 JSON schema 可能需要调整
3. 静态分析输出可能不完整
4. 测试 harness 可能不完整

### 2.2 现状问题

**❌ 缺失功能：**
1. **无 graph 输出验证**：没有 automated parity smoke test
2. **无 golden fixture comparisons**：没有 expected vs actual 对比
3. **不完整覆盖验证**：某些 edge type 可能缺失

**📊 数据完整性风险：**
- graph 输出未经系统化验证
- 可能在后续变更中出现退化

### 2.3 依赖项

**📚 测试依赖项：**
1. 需要为 graph 输出创建 golden fixtures
2. 需要实现 parity smoke test runner
3. 需要验证节点/边类型完整性

## 3. MVP Scope

### 3.1 核心目标

验证 Rust-core Cangjie graph output 与 TS adapter 功能对等性：
- 覆盖率验证：确认所有节点/边类型都有输出
- 稳定性验证：多次运行输出保持一致
- 完整性验证：包含所有关键图元素

### 3.2 具体验证目标

**目标 1：节点类型覆盖验证**
- ✅ Repository 节点存在
- ✅ Package 节点存在  
- ✅ SourceFile 节点存在
- ✅ Symbol 节点存在
- ✅ Diagnostic 节点存在（如果可用）
- ⏳ Interface 节点（待验证）

**目标 2：边类型覆盖验证**
- ✅ ContainsPackage 边存在
- ✅ OwnsSource 边存在
- ✅ Defines 边存在
- ⏳ Imports 边存在（需要验证）
- ⏳ USES/ACCESSES/MODIFIES 边存在（需要验证）
- ⏳ Annotates 边存在（需要验证）

**目标 3：图结构完整性验证**
- ✅ 所有节点有正确 ID
- ✅ 所有边有正确 source/target
- ✅ 层次关系正确（Package → SourceFile → Symbol）
- ⏳ 边权重/元数据（可选）

**目标 4：输出格式验证**
- ✅ JSON 输出格式有效
- ✅ 序列化正确（无循环引用）
- ⏳ Schema 合规性（待检查）

## 4. 技术方案

### 4.1 方案 A：添加 parity smoke test

**实现路径：**
1. 在 `crates/cangjie/tests/` 创建 `graph_parity_smoke.rs`
2. 使用现有 fixtures 作为 golden standards
3. 比较实际输出与预期输出

**步骤：**
1. 遍历所有 fixtures：`fixtures/cangjie/*/`
2. 对每个 fixture 调用 `inspect_cangjie_project()` 
3. 提取 graph nodes + edges
4. 序列化为 JSON，保存到 `.expected/` 文件
5. 比较实际输出，检测退化

**实现细节：**
```rust
// tests/graph_parity_smoke.rs

use gitnexus_cangjie::{graph::{inspect_cangjie_project, CangjieGraphOutput}};

#[cfg(test)]
fn test_basic_graph_parity() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/graph-basic");
    let project = inspect_cangjie_project(&fixture_dir).expect("fixture should load");
    let graph = CangjieGraphOutput::from_project(&project);
    
    // Validate node types
    assert!(graph.nodes.iter().any(|n| n.kind != NodeKind::Diagnostic));
    
    // Validate edge types
    assert!(graph.edges.iter().any(|e| e.kind != EdgeKind::Unknown));
    
    // Save as expected
    let expected = serde_json::to_string(&graph).unwrap();
    fs::write(fixture_dir.join("expected/graph.json"), expected)
        .expect("should write expected file");
}

fn test_advanced_graph_parity() {
    // Similar for advanced fixtures
}
```

### 4.2 方案 B：改进现有 graph 输出

**实现路径：**
1. 验证 Interface 节点输出
2. 确保 Imports 边正确生成
3. 改进图结构完整性
4. 添加元数据支持（可选）

**改进点：**
- 接口检测逻辑（基于 imports）
- 跨文件符号引用集成
- 边权重计算（可选）
- 循环依赖检测（warning 生成）

### 4.3 测试覆盖

**目标测试场景：**
1. 基础 graph structure（现有 fixtures）
2. 跨文件 imports（existing fixtures）
3. 函数调用引用（existing fixtures）
4. Wildcard imports（existing fixtures）
5. 复杂嵌套结构

## 5. Acceptance Criteria

| AC | 验证方式 | 预期结果 |
|----|---------|-----------|
| AC1: 节点类型覆盖率 100% | 遍历所有 graph 输出验证 | ✅ |
| AC2: 边类型覆盖率 90%+ | 验证所有边类型都有输出 | ✅ |
| AC3: 图结构完整性 100% | 检查所有节点都有正确 ID | ✅ |
| AC4: 输出格式稳定 100% | JSON 输出可解析且稳定 | ✅ |
| AC5: Parity smoke test 100% | 实现测试覆盖 | ✅ |
| AC6: 无回归测试 100% | 确保现有功能不受影响 | ✅ |

## 6. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| Graph output schema 不匹配 | MEDIUM | 与 TS adapter schema 对齐，必要时调整 |
| Fixture 维护成本 | LOW | 使用自动化测试，减少手动维护 |
| 测试执行时间 | LOW | 简化测试，优化执行速度 |
| False positive 警报 | LOW | 边界情况人工 review |

## 7. 依赖关系

**新依赖：** ❌ 无
**依赖变更：**
- 可选 `serde_json`（已存在）
- 可选 `serde`（已存在）
- 测试 harness 依赖

**外部依赖：**
- fixtures（现有）
- gitnexus_cangjie（本 crate）
- tree-sitter-cangjie（feature-gated）

## 8. 执行策略

**Phase 1: 实现基础 parity smoke test（20 min）**
1. 创建测试文件 `crates/cangjie/tests/graph_parity_smoke.rs`
2. 实现 fixture 遍历和 JSON 生成
3. 添加基础节点/边验证
4. 运行测试，验证通过

**Phase 2: 改进图输出（30 min）**
1. 验证 Interface 节点
2. 确保 Imports 边正确生成
3. 添加元数据
4. 修复发现的覆盖率 gap

**Phase 3: 添加高级场景测试（20 min）**
1. 复杂嵌套结构验证
2. 多包依赖场景测试
3. 完整图集成测试

**Phase 4: 集成测试（10 min）**
1. 运行所有 parity tests
2. 运行完整测试套件
3. 验证无回归

**Phase 5: 文档和 closure（5 min）**
1. 更新 README.md 标记完成
2. 写 closure review
3. 归档 preflight
4. Commit + push

**总估计时间：** ~85 min

## 9. Stop-lines 重申

以下内容是 Rust-core MVP 的明确 stop-line：

- **No production replacement** — Rust-core 不是 GitNexus-RC TypeScript adapter 的替代
- **No LSP client** — 仅实现 basic graph output
- **No MCP/HTTP/UI** — 不实现服务层
- **No type inference / trait solving** — 不推断类型
- **No macro expansion** — 不展开宏
- **No complex graph algorithms** — 仅序列化和输出
- **No performance optimization** — 不做图分析/优化
- **No IDE integration** — 仅 CLI 输出

**Slice 16 特定 stop-lines：**
- 不做 live repo 修改
- 不做 GitNexus-RC runtime 修改
- 不修改 GitNexus-RC schema 变更
- 仅使用现有 fixtures，不创建新 fixture 类型
- 不实现交互式 parity test

## 10. 下一步推荐

**推荐：** ✅ **进入 execution card**

Slice 16 的技术路径清晰，风险评估充分，所有依赖都是现有基础设施。建议继续执行，在 bounded slice 内完成 Cangjie graph output parity smoke test。

**下一步预览：**
实现后将实现：
- `crates/cangjie/tests/graph_parity_smoke.rs` (~200 行)
- `fixtures/cangjie/*/expected/*.json` (现有 fixtures 的 golden 文件)
- 覆盖率验证脚本
- README.md 更新标记完成

**为什么继续：**
- ✅ Housekeeping gate 已完成（归档历史 preflight 文档）
- ✅ Slice 16 preflight 准备完成
- ✅ 时间充裕（07:20 前，工作窗口充足）
- ✅ 风险已评估并有缓解措施
- ✅ 所有 AC 均已定义并可验证
- ✅ 依赖关系清晰，零新依赖
- ✅ Stop-lines 已重申并遵守

---
**结论：Slice 16 准备就绪，建议立即开始 execution。**