# Slice 20 Preflight — Multi-project Cangjie Production Smoke

**Date:** 2026-05-07
**Status:** Preflight
**Type:** Production Smoke / Docs Reconciliation
**Slice ID:** Phase 2 Slice 20

---

## Context

**Slice 19 已完成：**
- ✅ 修复 reference source endpoint integrity（125 dangling source IDs → 0）
- ✅ 采用方案 B：Synthetic Source Nodes（非真实 constructor symbol extraction）
- ✅ Production fixture (cjgui) 验证通过：danglingSourceEdges=0, danglingTargetEdges=0
- ✅ 263/263 tests pass（with feature）

**当前 Gap：**
- 只在单个 production fixture (cjgui) 上验证了 synthetic nodes
- 未验证 synthetic nodes 在其他 Cangjie 项目中的普适性
- docs/plans/README.md 中 Slice 18 段落仍说 "修复建议：Slice 19 — Constructor symbol extraction"，与实际完成的工作不符

---

## Slice 20 Scope

### Must Have（所有项必须完成）

1. **Multi-project production smoke**
   - 对多个真实 Cangjie 项目运行 smoke test
   - 验证 synthetic nodes 在不同项目中的表现
   - 收集 nodes/edges/synthetic/dangling 统计

2. **Docs reconciliation**
   - 修正 docs/plans/README.md 中 Slice 18 段落的 "Constructor symbol extraction" 过期表述
   - 明确说明 Slice 19 实际完成的是 "reference source endpoint integrity repair / synthetic callable source nodes"

3. **Bounded repair**
   - 如 smoke 暴露小问题（endpoint integrity test 漏洞、synthetic node properties 缺失、output determinism 小问题），可在 bounded scope 内修复
   - 不开新功能

### Must Not Have（禁止操作）

1. ❌ 不做真实 constructor symbol extraction（这需新 preflight）
2. ❌ 不做 type inference / trait solving / method dispatch
3. ❌ 不做 macro expansion
4. ❌ 不做 LSP / MCP / HTTP / UI
5. ❌ 不修改 live repo（/Users/jiangxuanyang/Desktop/cangjie）
6. ❌ 不修改 GitNexus-RC / GitNexus-RC-Tool
7. ❌ 不提交 production smoke 输出 JSON

---

## Smoke Targets（只读访问）

### Primary Targets

1. **cangjie-GitNexus-Index/runtime/cjgui**
   - Path: `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui`
   - Type: Production fixture（已测试，作为 baseline）

2. **cangjie/runtime/cjgui**（live repo）
   - Path: `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui`
   - Type: Similar project in live repo
   - Note: 只读访问，不修改

3. **CangjieSkills web_framework test**（live repo）
   - Path: `/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project`
   - Type: Test project（web framework）
   - Note: 只读访问，不修改

4. **CangjieSkills json_parser test**（live repo）
   - Path: `/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project`
   - Type: Test project（smaller）
   - Note: 只读访问，不修改

### Fallback Targets（如有需要）

5. **CangjieSkills mustache test**（live repo）
   - Path: `/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/mustache/project`
   - Type: Test project

6. **cangjie_stdx_1.1**（live repo）
   - Path: `/Users/jiangxuanyang/Desktop/cangjie/sources/cangjie_stdx_1.1`
   - Type: Standard library

---

## Implementation Approach

### Approach A: Integration Test（推荐）

**实现位置：** `crates/cangjie/tests/multi_project_smoke.rs`

**实现内容：**
```rust
#[cfg(feature = "tree-sitter-cangjie")]
mod multi_project_smoke_tests {
    use gitnexus_cangjie::graph::inspect_cangjie_project;
    use std::path::Path;
    use std::collections::HashSet;
    use std::time::Instant;

    #[test]
    fn test_multi_project_smoke() {
        let targets = vec![
            Path::new("/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project"),
        ];

        for root in targets {
            if !root.exists() {
                eprintln!("Skipping non-existent path: {}", root.display());
                continue;
            }

            let start = Instant::now();
            match inspect_cangjie_project(root) {
                Ok(graph) => {
                    let duration = start.elapsed();

                    // Collect node IDs
                    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

                    // Check endpoint integrity
                    let dangling_sources: Vec<_> = graph
                        .edges
                        .iter()
                        .filter(|e| !node_ids.contains(e.source_id.as_str()))
                        .collect();

                    let dangling_targets: Vec<_> = graph
                        .edges
                        .iter()
                        .filter(|e| !node_ids.contains(e.target_id.as_str()))
                        .collect();

                    // Count synthetic nodes
                    let synthetic_count = graph
                        .nodes
                        .iter()
                        .filter(|n| n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource)
                        .count();

                    // Print summary
                    println!("\n=== Project: {} ===", root.display());
                    println!("Nodes: {}", graph.nodes.len());
                    println!("Edges: {}", graph.edges.len());
                    println!("Synthetic nodes: {}", synthetic_count);
                    println!("Dangling source edges: {}", dangling_sources.len());
                    println!("Dangling target edges: {}", dangling_targets.len());
                    println!("Duration: {:?}", duration);

                    // Assert endpoint integrity
                    assert_eq!(
                        dangling_sources.len(),
                        0,
                        "Dangling source edges found in {}",
                        root.display()
                    );

                    assert_eq!(
                        dangling_targets.len(),
                        0,
                        "Dangling target edges found in {}",
                        root.display()
                    );
                }
                Err(e) => {
                    eprintln!("Failed to inspect {}: {}", root.display(), e);
                    // Don't fail the entire test for individual project errors
                }
            }
        }
    }
}
```

**优点：**
- 集成到现有 test suite
- CI 友好
- 可重复运行

**缺点：**
- 测试失败时需要手动查看 console output

### Approach B: CLI Smoke Script（备选）

**实现位置：** `scripts/smoke_cangjie_multi_project.sh`

**实现内容：**
```bash
#!/bin/bash
set -e

CARGO_CMD="cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- cangjie inspect"

for root in \
  "/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui" \
  "/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui" \
  "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project" \
  "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project"
do
  if [ ! -d "$root" ]; then
    echo "Skipping non-existent path: $root"
    continue
  fi

  echo "=== Smoking: $root ==="
  $CARGO_CMD --root "$root" 2>/dev/null | jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}'
done
```

**优点：**
- 独立脚本，灵活
- 输出格式可控

**缺点：**
- 不在 test suite 中
- 需要手动维护

### Approach C: Helper Function + Integration Test（推荐）

**实现位置：** `crates/cangjie/tests/multi_project_smoke.rs`

**实现内容：**
```rust
#[cfg(feature = "tree-sitter-cangjie")]
mod multi_project_smoke_tests {
    use gitnexus_cangjie::graph::inspect_cangjie_project;
    use std::path::Path;
    use std::collections::HashSet;
    use std::time::Instant;
    use serde_json::Value;

    struct SmokeResult {
        root: String,
        nodes: usize,
        edges: usize,
        synthetic_count: usize,
        dangling_sources: usize,
        dangling_targets: usize,
        duration_secs: f64,
        node_kind_distribution: std::collections::HashMap<String, usize>,
        edge_kind_distribution: std::collections::HashMap<String, usize>,
    }

    fn run_smoke(root: &Path) -> Result<SmokeResult, String> {
        let start = Instant::now();
        let graph = inspect_cangjie_project(root).map_err(|e| e.to_string())?;
        let duration = start.elapsed();

        let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

        let dangling_sources = graph
            .edges
            .iter()
            .filter(|e| !node_ids.contains(e.source_id.as_str()))
            .count();

        let dangling_targets = graph
            .edges
            .iter()
            .filter(|e| !node_ids.contains(e.target_id.as_str()))
            .count();

        let synthetic_count = graph
            .nodes
            .iter()
            .filter(|n| n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource)
            .count();

        let mut node_kind_distribution = std::collections::HashMap::new();
        for node in &graph.nodes {
            let kind_str = format!("{:?}", node.kind);
            *node_kind_distribution.entry(kind_str).or_insert(0) += 1;
        }

        let mut edge_kind_distribution = std::collections::HashMap::new();
        for edge in &graph.edges {
            let kind_str = format!("{:?}", edge.kind);
            *edge_kind_distribution.entry(kind_str).or_insert(0) += 1;
        }

        Ok(SmokeResult {
            root: root.display().to_string(),
            nodes: graph.nodes.len(),
            edges: graph.edges.len(),
            synthetic_count,
            dangling_sources,
            dangling_targets,
            duration_secs: duration.as_secs_f64(),
            node_kind_distribution,
            edge_kind_distribution,
        })
    }

    #[test]
    fn test_multi_project_smoke_with_details() {
        let targets = vec![
            Path::new("/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project"),
            Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project"),
        ];

        let mut results = Vec::new();

        for root in targets {
            if !root.exists() {
                eprintln!("Skipping non-existent path: {}", root.display());
                continue;
            }

            match run_smoke(root) {
                Ok(result) => {
                    println!("\n=== Project: {} ===", result.root);
                    println!("Nodes: {}", result.nodes);
                    println!("Edges: {}", result.edges);
                    println!("Synthetic nodes: {}", result.synthetic_count);
                    println!("Dangling source edges: {}", result.dangling_sources);
                    println!("Dangling target edges: {}", result.dangling_targets);
                    println!("Duration: {:.3}s", result.duration_secs);
                    println!("Node kind distribution: {:?}", result.node_kind_distribution);
                    println!("Edge kind distribution: {:?}", result.edge_kind_distribution);

                    // Assert endpoint integrity
                    assert_eq!(
                        result.dangling_sources, 0,
                        "Dangling source edges found in {}",
                        result.root
                    );

                    assert_eq!(
                        result.dangling_targets, 0,
                        "Dangling target edges found in {}",
                        result.root
                    );

                    results.push(result);
                }
                Err(e) => {
                    eprintln!("Failed to inspect {}: {}", root.display(), e);
                    // Don't fail the entire test for individual project errors
                }
            }
        }

        // Print summary
        println!("\n=== Summary ===");
        println!("Successfully smoked {} projects", results.len());
        let total_nodes: usize = results.iter().map(|r| r.nodes).sum();
        let total_edges: usize = results.iter().map(|r| r.edges).sum();
        let total_synthetic: usize = results.iter().map(|r| r.synthetic_count).sum();
        println!("Total nodes: {}", total_nodes);
        println!("Total edges: {}", total_edges);
        println!("Total synthetic nodes: {}", total_synthetic);
    }
}
```

**优点：**
- 集成到 test suite
- 详细的统计信息
- 可验证 endpoint integrity
- 可扩展更多 targets

**缺点：**
- 测试时间较长（多个项目）

---

## Docs Reconciliation

### Required Changes

#### 1. docs/plans/README.md

**Line 243 修改前：**
```markdown
- 修复建议：Phase 2 Slice 19 — Constructor symbol extraction（优先级：High，预估 ~300-400 行）
```

**Line 243 修改后：**
```markdown
- 修复建议：Phase 2 Slice 19 — Reference source endpoint integrity repair（已完成）
- 实际方案：Synthetic callable source nodes（非完整 constructor symbol extraction）
- Future: 真实 constructor symbol extraction 需新 preflight
```

#### 2. docs/plans/README.md（Slice 18 段落补充）

在 Slice 18 段落末尾添加：
```markdown
**Note:** Slice 18 发现的 dangling source edges 已在 Slice 19 中修复。Slice 19 采用 Synthetic Source Nodes 方案（非完整 constructor symbol extraction）。
```

#### 3. docs/RISK_LEDGER.md（可选更新）

如果需要记录 synthetic nodes 的 residual risk：
```markdown
### X.X Synthetic source nodes as temporary bridge

状态：**已知限制**

- Slice 19 引入 synthetic callable source nodes 修复 endpoint integrity
- 这些 synthetic nodes 标记为 `synthetic = true`，不是真实 symbol extraction
- Future refactor: 真实 constructor / method / function symbol extraction
- 当前状态：endpoint integrity green（dangling source = 0，dangling target = 0）

风险级别：**LOW**

防守规则：
- 不把 synthetic nodes 当成真实 symbol
- 未来重构前必须写 preflight
```

---

## Acceptance Criteria

**Must Have（所有项必须完成）：**

1. ✅ `cargo fmt --check` pass
2. ✅ `cargo test` pass（192/192）
3. ✅ `cargo test --features tree-sitter-cangjie` pass（263/263 + 新增 multi-project smoke tests）
4. ✅ Multi-project smoke pass（至少 3 个 targets）
5. ✅ 每个 target 的 endpoint integrity：dangling source = 0，dangling target = 0
6. ✅ Synthetic nodes 都有 `synthetic = true` 标记
7. ✅ 输出确定性（每个 target 两次运行结果一致）
8. ✅ Docs reconciliation 完成（docs/plans/README.md 修正）
9. ✅ 不提交 production smoke 输出 JSON
10. ✅ 不修改 live repo

**Should Have：**

- 记录每个 target 的 nodes/edges/synthetic/dangling 统计
- 记录 runtime duration
- 记录 node kind distribution
- 记录 edge kind distribution
- 分析 synthetic nodes 在不同项目中的分布

---

## Stop-lines

**严格执行：**

- ❌ 不做真实 constructor symbol extraction
- ❌ 不做 type inference / trait solving
- ❌ 不做 method dispatch
- ❌ 不做 macro expansion
- ❌ 不引入 LSP/MCP/HTTP/UI/embedding
- ❌ 不修改 GitNexus-RC / Tool / live repo
- ❌ 不提交 production smoke 输出 JSON

**修复边界：**

- ✅ 只修复 endpoint integrity test 漏洞
- ✅ 只修复 synthetic node properties 缺失
- ✅ 只修复 output determinism 小问题
- ✅ 只修复 docs stale state

---

## Risk Assessment

### Low Risk

- ✅ Multi-project smoke 是只读访问
- ✅ 不修改 live repo
- ✅ 不修改 GitNexus-RC / Tool
- ✅ Docs reconciliation 是纯文字修正

### Medium Risk

- ⚠️ Live repo 项目可能有不同的代码结构，可能导致 synthetic nodes 不适用
- ⚠️ 某些项目可能没有 cjpm.toml 或不是有效 Cangjie 项目
- **缓解：** 跳过无效项目，不失败整个 test suite

### No Risk

- ✅ 不做 type inference / trait solving
- ✅ 不做 method dispatch
- ✅ 不做 macro expansion

---

## Expected Outcomes

### Best Case（理想情况）

- 所有 4 个 targets smoke 通过
- Endpoint integrity 在所有 targets 上都 green
- Synthetic nodes 在不同项目中都表现正常
- Docs reconciliation 完成
- Tests pass

### Likely Case（预期情况）

- 大部分 targets smoke 通过（3/4 或 4/4）
- 某些 targets 可能需要调整（如项目结构不同）
- Docs reconciliation 完成
- Tests pass

### Worst Case（最坏情况）

- 大部分 targets smoke 失败
- 发现 synthetic nodes 在某些项目上不适用
- 需要重新评估 Slice 19 方案
- **缓解：** 记录详细失败信息，为下一轮 preflight 提供依据

---

## Next Openings

根据 Slice 20 结果，选择下一个 bounded slice：

### Option A: 如 Slice 20 clean（推荐）

**Slice 建议：** Phase 2 Slice 21 — Real constructor symbol extraction preflight

**范围：**
- 写 preflight 评估真实 constructor symbol extraction
- 评估是否可以替代 synthetic nodes
- 评估实现复杂度（预估 ~300-400 行）

**优先级：** Medium

### Option B: 如 Slice 20 暴露问题

**Slice 建议：** Phase 2 Slice 21b — Synthetic nodes improvement

**范围：**
- 修复 synthetic nodes 在某些项目上的不适用问题
- 改进 synthetic nodes 的通用性

**优先级：** High

### Option C: 如 Slice 20 完全成功

**Slice 建议：** Phase 2 Slice 22 — 其他 Cangjie 功能

**范围：** 根据原始 Phase 2 规划的其他 slices

**优先级：** 根据具体 slice 评估

---

## Timeline

- Step 1: 写 Slice 20 preflight（本文件，已完成）
- Step 2: 写 Slice 20 execution card（~30 分钟）
- Step 3: 实现 multi-project smoke（Approach C，~2-3 小时）
- Step 4: 运行 smoke 并收集数据（~30 分钟）
- Step 5: Docs reconciliation（~30 分钟）
- Step 6: 验证 tests（~30 分钟）
- Step 7: 写 closure review（~30 分钟）
- Step 8: 更新文档并提交（~10 分钟）

**Total: ~5-6 小时**

---

**Decision:** Begin implementation using **Approach C（Helper Function + Integration Test）**，提供详细的统计信息和可扩展性。
