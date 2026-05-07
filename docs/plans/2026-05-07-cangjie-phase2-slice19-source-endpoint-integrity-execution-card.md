# Slice 19 Execution Card — Cangjie Reference Source Endpoint Integrity Repair

**Date:** 2026-05-07  
**Status:** Execution  
**Type:** Bug Fix / Endpoint Integrity Repair  
**Slice ID:** Phase 2 Slice 19  
**Preflight:** `2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-preflight.md`

## Frozen Scope

### Write Set

**必须修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/README.md`：更新 Slice 19 状态
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-closure-review.md`：新增 closure review
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/crates/cangjie/src/`：修复 source endpoint integrity
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/tests/`：添加 endpoint integrity test

**可选修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/fixtures/`：添加小型 constructor fixture

**禁止修改：**
- `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/`：只读访问
- `/Users/jiangxuanyang/Desktop/cangjie/`：live repo
- GitNexus-RC / Tool / runtime / schema / package / web

### Stop-lines

**严格执行：**
- ❌ 不做 type inference / trait solving
- ❌ 不做 method dispatch（保持 low-confidence heuristic only）
- ❌ 不做 macro expansion
- ❌ 不引入 LSP/MCP/HTTP/UI/embedding
- ❌ 不修改 GitNexus-RC / Tool / live repo

**修复边界：**
- ✅ 只修复 source endpoint integrity（不扩展 target 解析）
- ✅ 保持在 bounded API 范围内（< 200 行代码变更）
- ✅ 优先使用方案 B（Synthetic Source Nodes）

### Acceptance Criteria

**Must Have（所有项必须完成）：**
1. `cargo fmt --check` pass
2. `cargo test` pass（192/192）
3. `cargo test --features tree-sitter-cangjie` pass（259/259）
4. 小型 fixture endpoint integrity test pass
5. Production fixture smoke pass：
   - root: `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui`
   - danglingTargetEdges = 0（保持）
   - danglingSourceEdges 显著下降；理想为 0
   - 如果不能为 0，必须列出 remaining source id pattern 和原因
6. 输出确定性保持：连续两次 nodes/edges count 稳定
7. 不提交 production smoke 输出 JSON

**Should Have：**
- 记录 before/after endpoint integrity 数据对比
- 记录采用的方案（A/B）
- 记录修改的文件列表

## Implementation Steps

### Step 1: 诊断当前问题（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core

# 运行 production fixture smoke，收集详细数据
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_after_slice18.json

# 分析 dangling sources
echo "=== Current State (Slice 18) ===" && \
echo "Node count:" && jq '.nodes | length' /tmp/cjgui_after_slice18.json && \
echo "Edge count:" && jq '.edges | length' /tmp/cjgui_after_slice18.json && \
echo "Dangling source IDs:" && \
jq -r '.edges[].sourceId' /tmp/cjgui_after_slice18.json | \
  grep "^Constructor:" | \
  sort | uniq | wc -l && \
echo "Dangling source edges:" && \
jq -r '.edges[] | select(.sourceId | startswith("Constructor:"))' /tmp/cjgui_after_slice18.json | \
  wc -l && \
echo "Sample dangling source IDs:" && \
jq -r '.edges[].sourceId' /tmp/cjgui_after_slice18.json | \
  grep "^Constructor:" | \
  sort | uniq | head -5
```

**Expected:**
- Dangling source IDs: 646
- Dangling source edges: 2,687

### Step 2: 审视现有代码（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core

# 查看 reference extraction 如何生成 source IDs
grep -n "build_source_id" crates/cangjie/src/extractors/references.rs

# 查看 graph emission 如何生成 node IDs
grep -n "symbol_node_id" crates/cangjie/src/graph.rs

# 查看当前 emitted node types
grep -n "NodeKind::" crates/cangjie/src/graph.rs
```

**Expected:**
- 确认 `build_source_id()` 使用 `Constructor:` 前缀
- 确认 `symbol_node_id()` 使用 `sym:` 前缀
- 确认当前没有 Constructor node kind

### Step 3: 实现方案 B - Synthetic Source Nodes（2-3 小时）

**Action:**
1. 在 `crates/cangjie/src/graph.rs` 中添加 synthetic node emission逻辑
2. 收集所有 reference source IDs
3. 为每个 unique source ID emit synthetic node
4. 标记 `synthetic = true`，`kind = Constructor`

**实现建议：**
```rust
// 在 emit_cangjie_nodes() 中添加 synthetic nodes
fn emit_synthetic_source_nodes(
    references: &[CangjieReference],
    nodes: &mut Vec<Node>,
    node_ids: &mut HashSet<String>,
) {
    let unique_source_ids: HashSet<_> = references
        .iter()
        .map(|r| r.source_id.clone())
        .collect();
    
    for source_id in unique_source_ids {
        // 检查是否已经在 nodes 中（例如 SourceFile nodes）
        if !node_ids.contains(&source_id) {
            // 解析 source_id 格式，确定 kind
            let kind = if source_id.starts_with("Constructor:") {
                "Constructor".to_string()
            } else if source_id.starts_with("Method:") {
                "Method".to_string()
            } else if source_id.starts_with("Function:") {
                "Function".to_string()
            } else {
                continue; // 跳过其他类型
            };
            
            nodes.push(Node {
                id: source_id.clone(),
                label: extract_label(&source_id),
                kind,
                synthetic: true, // 标记为 synthetic
                // ... 其他字段
            });
            node_ids.insert(source_id);
        }
    }
}
```

**预期变更：** ~100-150 行代码

### Step 4: 添加 Endpoint Integrity Test（1 小时）

**Action:**
在 `crates/cangjie/tests/` 中添加 `endpoint_integrity.rs`：

```rust
#[cfg(feature = "tree-sitter-cangjie")]
mod endpoint_integrity_tests {
    use gitnexus_cangjie::graph::inspect_cangjie_project;
    use std::path::PathBuf;
    
    #[test]
    fn test_no_dangling_source_ids() {
        let fixture_dir = PathBuf::from("fixtures/cangjie/cjpm-basic");
        if !fixture_dir.exists() {
            return;
        }
        
        let graph = inspect_cangjie_project(&fixture_dir).unwrap();
        
        // 收集所有 node IDs
        let node_ids: HashSet<_> = graph.nodes.iter().map(|n| &n.id).collect();
        
        // 检查所有 edge source IDs 都在 nodes 中
        for edge in &graph.edges {
            assert!(
                node_ids.contains(&edge.source_id),
                "Edge source ID '{}' not found in nodes (target: '{}')",
                edge.source_id,
                edge.target_id
            );
        }
    }
    
    #[test]
    fn test_no_dangling_target_ids() {
        let fixture_dir = PathBuf::from("fixtures/cangjie/cjpm-basic");
        if !fixture_dir.exists() {
            return;
        }
        
        let graph = inspect_cangjie_project(&fixture_dir).unwrap();
        
        // 收集所有 node IDs
        let node_ids: HashSet<_> = graph.nodes.iter().map(|n| &n.id).collect();
        
        // 检查所有 edge target IDs 都在 nodes 中
        for edge in &graph.edges {
            assert!(
                node_ids.contains(&edge.target_id),
                "Edge target ID '{}' not found in nodes (source: '{}')",
                edge.target_id,
                edge.source_id
            );
        }
    }
}
```

### Step 5: 验证测试（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
```

**Expected:**
- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ 192/192 pass
- `cargo test --features tree-sitter-cangjie`: ✅ 259/259 pass（包括新增的 endpoint integrity test）

### Step 6: Production Fixture Smoke 验证（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core

# 运行 production fixture smoke
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_after_slice19.json

# 分析 after 状态
echo "=== After Slice 19 ===" && \
echo "Node count:" && jq '.nodes | length' /tmp/cjgui_after_slice19.json && \
echo "Edge count:" && jq '.edges | length' /tmp/cjgui_after_slice19.json && \
echo "Dangling source IDs:" && \
jq -r '.edges[].sourceId' /tmp/cjgui_after_slice19.json | \
  grep "^Constructor:" | \
  sort | uniq | wc -l && \
echo "Dangling source edges:" && \
jq -r '.edges[] | select(.sourceId | startswith("Constructor:"))' /tmp/cjgui_after_slice19.json | \
  wc -l && \
echo "Dangling target IDs:" && \
jq -r '.edges[].targetId' /tmp/cjgui_after_slice19.json | \
  sort | uniq | wc -l
```

**Expected:**
- Node count: 715 + 646 = 1,361（新增 646 个 synthetic nodes）
- Edge count: 3,401（保持不变）
- Dangling source IDs: 0（目标）
- Dangling source edges: 0（目标）
- Dangling target IDs: 0（保持）

### Step 7: 输出确定性验证（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core

# 第一次运行
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/run1.json

# 第二次运行
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/run2.json

# 比较结构
diff <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/run1.json) \
     <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/run2.json)
```

**Expected:**
- `diff` 无输出（两次运行结构相同）

### Step 8: 编写 Closure Review（30 分钟）

**Action:**
- 创建 `docs/plans/2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-closure-review.md`
- 记录：
  - Landed reality（实际修复结果）
  - Before/after endpoint integrity 数据对比
  - 采用的方案（B）
  - 修改的文件列表
  - 残留风险
  - Next opening 建议

### Step 9: 更新文档并提交（5 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
git add -A
git commit -m "fix(cangjie): repair reference source endpoint integrity"
git push gitcode master
```

**Expected:**
- Commit 成功
- Push 成功

## Exit Criteria

Slice 19 完成的标志：
- ✅ Production fixture smoke 通过（danglingSourceEdges = 0）
- ✅ 小型 fixture endpoint integrity test 通过
- ✅ `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` 全部 pass
- ✅ Closure review 完成
- ✅ Commit + push gitcode master
- ✅ docs/plans/README.md 更新

## Expected Before/After Data

| Metric | Before (Slice 18) | After (Slice 19) |
|--------|------------------|------------------|
| Nodes | 715 | 1,361 (+646 synthetic) |
| Edges | 3,401 | 3,401 (保持不变) |
| Dangling source IDs | 646 | 0 (目标) |
| Dangling source edges | 2,687 | 0 (目标) |
| Dangling target IDs | 0 | 0 (保持) |

## Next Openings

根据 Slice 19 结果，选择下一个 bounded slice：
- **Option A:** 如修复成功 → 继续扩展现有能力
- **Option B:** 如发现新问题 → 修复新问题
- **Option C:** 如一切正常 → 继续其他 Cangjie 功能

## Timeline

- Step 1: 诊断当前问题（30 分钟）
- Step 2: 审视现有代码（30 分钟）
- Step 3: 实现方案 B（2-3 小时）
- Step 4: 添加 endpoint integrity test（1 小时）
- Step 5: 验证测试（30 分钟）
- Step 6: Production fixture smoke 验证（30 分钟）
- Step 7: 输出确定性验证（10 分钟）
- Step 8: Closure review（30 分钟）
- Step 9: 更新文档并提交（5 分钟）

**Total: ~5-6 小时**

---

**Decision:** Begin implementation using **方案 B（Synthetic Source Nodes）**，风险更低，实现更快。
