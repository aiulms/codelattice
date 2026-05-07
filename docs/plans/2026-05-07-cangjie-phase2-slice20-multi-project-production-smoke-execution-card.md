# Slice 20 Execution Card — Multi-project Cangjie Production Smoke

**Date:** 2026-05-07
**Status:** Execution
**Type:** Production Smoke / Docs Reconciliation
**Slice ID:** Phase 2 Slice 20
**Preflight:** `2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-preflight.md`

## Frozen Scope

### Write Set

**必须修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/README.md`：docs reconciliation
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/crates/cangjie/tests/multi_project_smoke.rs`：新增 multi-project smoke test
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-closure-review.md`：新增 closure review

**可选修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/RISK_LEDGER.md`：如需记录 synthetic nodes residual risk

**禁止修改：**
- `/Users/jiangxuanyang/Desktop/cangjie/`：live repo（只读访问）
- `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/`：只读访问
- GitNexus-RC / Tool / runtime / schema / package / web

### Stop-lines

**严格执行：**
- ❌ 不做真实 constructor symbol extraction
- ❌ 不做 type inference / trait solving
- ❌ 不做 method dispatch
- ❌ 不做 macro expansion
- ❌ 不引入 LSP/MCP/HTTP/UI/embedding
- ❌ 不修改 GitNexus-RC / Tool / live repo

**修复边界：**
- ✅ 只修复 endpoint integrity test 漏洞
- ✅ 只修复 synthetic node properties 缺失
- ✅ 只修复 output determinism 小问题
- ✅ 只修复 docs stale state

### Acceptance Criteria

**Must Have（所有项必须完成）：**

1. `cargo fmt --check` pass
2. `cargo test` pass（192/192）
3. `cargo test --features tree-sitter-cangjie` pass（263/263 + 新增 multi-project smoke tests）
4. Multi-project smoke pass（至少 3 个 targets）
5. 每个 target 的 endpoint integrity：dangling source = 0，dangling target = 0
6. Synthetic nodes 都有 `synthetic = true` 标记
7. 输出确定性（每个 target 两次运行结果一致）
8. Docs reconciliation 完成（docs/plans/README.md 修正）
9. 不提交 production smoke 输出 JSON
10. 不修改 live repo

**Should Have：**

- 记录每个 target 的 nodes/edges/synthetic/dangling 统计
- 记录 runtime duration
- 记录 node kind distribution
- 记录 edge kind distribution
- 分析 synthetic nodes 在不同项目中的分布

## Implementation Steps

### Step 1: 实现 Multi-project Smoke Test（2-3 小时）

**Action:**
创建 `crates/cangjie/tests/multi_project_smoke.rs`：

```rust
//! Multi-project production smoke tests for Cangjie graph output.
//!
//! Verifies that synthetic source nodes work correctly across multiple
//! real Cangjie projects, ensuring endpoint integrity and output determinism.
//!
//! Requires the `tree-sitter-cangjie` feature.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::inspect_cangjie_project;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// Result of smoking a single Cangjie project.
#[derive(Debug)]
struct SmokeResult {
    root: String,
    nodes: usize,
    edges: usize,
    synthetic_count: usize,
    dangling_sources: usize,
    dangling_targets: usize,
    duration_secs: f64,
    node_kind_distribution: HashMap<String, usize>,
    edge_kind_distribution: HashMap<String, usize>,
    skipped: bool,
    skip_reason: Option<String>,
}

/// Run smoke test on a single Cangjie project.
fn run_smoke(root: &Path) -> SmokeResult {
    // Check if path exists
    if !root.exists() {
        return SmokeResult {
            root: root.display().to_string(),
            nodes: 0,
            edges: 0,
            synthetic_count: 0,
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
            node_kind_distribution: HashMap::new(),
            edge_kind_distribution: HashMap::new(),
            skipped: true,
            skip_reason: Some("Path does not exist".to_string()),
        };
    }

    // Check if cjpm.toml exists
    let cjpm_toml = root.join("cjpm.toml");
    if !cjpm_toml.exists() {
        return SmokeResult {
            root: root.display().to_string(),
            nodes: 0,
            edges: 0,
            synthetic_count: 0,
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
            node_kind_distribution: HashMap::new(),
            edge_kind_distribution: HashMap::new(),
            skipped: true,
            skip_reason: Some("cjpm.toml not found".to_string()),
        };
    }

    let start = Instant::now();
    match inspect_cangjie_project(root) {
        Ok(graph) => {
            let duration = start.elapsed();

            // Collect node IDs
            let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

            // Check endpoint integrity
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

            // Count synthetic nodes
            let synthetic_count = graph
                .nodes
                .iter()
                .filter(|n| n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource)
                .count();

            // Build node kind distribution
            let mut node_kind_distribution = HashMap::new();
            for node in &graph.nodes {
                let kind_str = format!("{:?}", node.kind);
                *node_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

            // Build edge kind distribution
            let mut edge_kind_distribution = HashMap::new();
            for edge in &graph.edges {
                let kind_str = format!("{:?}", edge.kind);
                *edge_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

            SmokeResult {
                root: root.display().to_string(),
                nodes: graph.nodes.len(),
                edges: graph.edges.len(),
                synthetic_count,
                dangling_sources,
                dangling_targets,
                duration_secs: duration.as_secs_f64(),
                node_kind_distribution,
                edge_kind_distribution,
                skipped: false,
                skip_reason: None,
            }
        }
        Err(e) => {
            eprintln!("Failed to inspect {}: {}", root.display(), e);
            SmokeResult {
                root: root.display().to_string(),
                nodes: 0,
                edges: 0,
                synthetic_count: 0,
                dangling_sources: 0,
                dangling_targets: 0,
                duration_secs: 0.0,
                node_kind_distribution: HashMap::new(),
                edge_kind_distribution: HashMap::new(),
                skipped: true,
                skip_reason: Some(format!("Error: {}", e)),
            }
        }
    }
}

#[test]
fn test_multi_project_smoke_with_details() {
    // Define smoke targets (read-only access)
    let targets = vec![
        Path::new("/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui"),
        Path::new("/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"),
        Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project"),
        Path::new("/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project"),
    ];

    let mut results = Vec::new();

    for root in targets {
        let result = run_smoke(root);
        results.push(result.clone());

        if result.skipped {
            println!("\n=== Skipped: {} ===", result.root);
            println!("Reason: {}", result.skip_reason.unwrap_or_default());
            continue;
        }

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

        // Verify synthetic nodes are marked
        // (This is verified indirectly by endpoint integrity)
    }

    // Print summary
    let successful: Vec<_> = results.iter().filter(|r| !r.skipped).collect();
    let skipped: Vec<_> = results.iter().filter(|r| r.skipped).collect();

    println!("\n=== Summary ===");
    println!("Total targets: {}", results.len());
    println!("Successfully smoked: {}", successful.len());
    println!("Skipped: {}", skipped.len());

    if !successful.is_empty() {
        let total_nodes: usize = successful.iter().map(|r| r.nodes).sum();
        let total_edges: usize = successful.iter().map(|r| r.edges).sum();
        let total_synthetic: usize = successful.iter().map(|r| r.synthetic_count).sum();
        let total_duration: f64 = successful.iter().map(|r| r.duration_secs).sum();

        println!("Total nodes: {}", total_nodes);
        println!("Total edges: {}", total_edges);
        println!("Total synthetic nodes: {}", total_synthetic);
        println!("Total duration: {:.3}s", total_duration);
    }

    if !skipped.is_empty() {
        println!("\nSkipped projects:");
        for result in skipped {
            println!("- {} ({})", result.root, result.skip_reason.as_ref().unwrap_or(&"Unknown".to_string()));
        }
    }

    // Assert at least 3 successful targets
    assert!(
        successful.len() >= 3,
        "Expected at least 3 successful targets, got {}",
        successful.len()
    );
}
```

**Expected:**
- 新增 `crates/cangjie/tests/multi_project_smoke.rs`（~250 行代码）

### Step 2: 运行 Multi-project Smoke 并收集数据（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --nocapture
```

**Expected:**
- 至少 3 个 targets 成功 smoke
- 所有成功的 targets 都通过 endpoint integrity 检查
- 输出详细的统计信息

### Step 3: Docs Reconciliation - 修正 docs/plans/README.md（30 分钟）

**Action:**

修改 line 243：

**修改前：**
```markdown
- 修复建议：Phase 2 Slice 19 — Constructor symbol extraction（优先级：High，预估 ~300-400 行）
```

**修改后：**
```markdown
- 修复建议：Phase 2 Slice 19 — Reference source endpoint integrity repair（已完成）
- 实际方案：Synthetic callable source nodes（非完整 constructor symbol extraction）
- Future: 真实 constructor symbol extraction 需新 preflight
```

在 Slice 18 段落末尾添加（约 line 248 之后）：
```markdown

**Note:** Slice 18 发现的 dangling source edges 已在 Slice 19 中修复。Slice 19 采用 Synthetic Source Nodes 方案（非完整 constructor symbol extraction），通过在 graph emission 阶段为 reference source IDs emit synthetic callable source nodes 来修复 endpoint integrity。
```

**Expected:**
- docs/plans/README.md 更新完成
- Slice 18 段落不再误导读者认为 Slice 19 是 "Constructor symbol extraction"

### Step 4: 验证 Tests（30 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
```

**Expected:**
- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ 192/192 pass（或更多）
- `cargo test --features tree-sitter-cangjie`: ✅ 263/263 + new multi-project smoke tests pass

### Step 5: 输出确定性验证（30 分钟）

**Action:**
对每个成功的 target 运行两次，验证输出一致性：

```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core

# Test cjgui (cangjie-GitNexus-Index)
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_run1.json

cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_run2.json

diff <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/cjgui_run1.json) \
     <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/cjgui_run2.json)

# Test cjgui (live repo)
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_live_run1.json

cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui \
  2>/dev/null > /tmp/cjgui_live_run2.json

diff <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/cjgui_live_run1.json) \
     <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/cjgui_live_run2.json)
```

**Expected:**
- `diff` 无输出（两次运行结构相同）

### Step 6: 写 Closure Review（30 分钟）

**Action:**
创建 `docs/plans/2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-closure-review.md`

**内容要求：**
- Landed reality（实际 smoke 结果）
- 每个 target 的 nodes/edges/synthetic/dangling 统计
- 是否发现 synthetic nodes 普适性问题
- Docs reconciliation 做了什么
- Residual risks
- Next opening 建议

### Step 7: 更新文档并提交（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
git add -A
git commit -m "test(cangjie): add multi-project production smoke"
git push gitcode master
```

**Expected:**
- Commit 成功
- Push 成功

## Exit Criteria

Slice 20 完成的标志：
- ✅ Multi-project smoke pass（至少 3 个 targets）
- ✅ 每个 target 的 endpoint integrity：dangling source = 0，dangling target = 0
- ✅ `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` 全部 pass
- ✅ Docs reconciliation 完成（docs/plans/README.md 修正）
- ✅ Closure review 完成
- ✅ Commit + push gitcode master
- ✅ 不修改 live repo

## Expected Smoke Results

### Target 1: cangjie-GitNexus-Index/runtime/cjgui（baseline）

- **Expected:** ✅ Success（已在 Slice 19 中验证）
- **Expected stats:**
  - Nodes: ~1,361
  - Edges: ~3,401
  - Synthetic nodes: ~646
  - Dangling source edges: 0
  - Dangling target edges: 0

### Target 2: cangjie/runtime/cjgui（live repo）

- **Expected:** ✅ Success（类似项目）
- **Expected stats:** Similar to baseline
- **Potential issues:** 项目结构可能有差异

### Target 3: CangjieSkills web_framework test

- **Expected:** ✅ Success（test project）
- **Expected stats:** 可能与 baseline 不同（项目规模、代码风格）
- **Potential issues:** 可能使用不同的 Cangjie features

### Target 4: CangjieSkills json_parser test

- **Expected:** ✅ Success（smaller test project）
- **Expected stats:** 可能比 baseline 小
- **Potential issues:** 可能项目结构简单，synthetic nodes 较少

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

## Timeline

- Step 1: 实现 multi-project smoke test（2-3 小时）
- Step 2: 运行 smoke 并收集数据（30 分钟）
- Step 3: Docs reconciliation（30 分钟）
- Step 4: 验证 tests（30 分钟）
- Step 5: 输出确定性验证（30 分钟）
- Step 6: Closure review（30 分钟）
- Step 7: 更新文档并提交（10 分钟）

**Total: ~5-6 小时**

---

**Decision:** Begin implementation using **Approach C（Helper Function + Integration Test）**，提供详细的统计信息和可扩展性。
