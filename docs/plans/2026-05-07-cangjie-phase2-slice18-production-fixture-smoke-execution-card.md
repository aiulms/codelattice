# Slice 18 Execution Card — Cangjie Production Fixture Smoke

**Date:** 2026-05-07  
**Status:** Execution  
**Type:** Production Validation / Smoke Test  
**Slice ID:** Phase 2 Slice 18  
**Preflight:** `2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-preflight.md`

## Frozen Scope

### Write Set

**必须修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/README.md`：更新 Slice 18 状态
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/docs/plans/2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-closure-review.md`：新增 closure review

**可选修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/crates/cli/`：如遇明显 bug
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/crates/cangjie/`：如遇明显 bug
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/tests/`：如需添加 smoke test

**禁止修改：**
- `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/`：只读访问
- `/Users/jiangxuanyang/Desktop/cangjie/`：live repo
- GitNexus-RC / Tool / runtime / schema / package / web

### Stop-lines

**严格执行：**
- ❌ 不引入 LSP/MCP/HTTP/UI/embedding
- ❌ 不做 type inference / trait solving
- ❌ 不做 macro expansion
- ❌ 不修改 GitNexus-RC / Tool / live repo
- ❌ 不提交 production fixture 分析产物 JSON
- ❌ 不开新功能 / 不扩展现有 API
- ❌ 不做算法优化 / 性能调优（除非严重影响可用性）

**修复边界：**
- ✅ 只修复明显的 runtime bug（panic / crash / deadlock）
- ✅ Bug 修复必须在 bounded scope 内（< 50 行变更）
- ✅ 如 bug 修复需较大改动，只记录 gap，不开新 slice

### Acceptance Criteria

**Must Have（所有项必须完成）：**
1. CLI 在 production fixture 上成功运行（exit 0）
2. 输出合法 JSON（可被 `jq` 解析）
3. 统计报告包含：
   - Nodes / edges 总数
   - Node type 分布（Repository/Package/SourceFile/Symbol）
   - Edge type 分布（ContainsPackage/OwnsSource/Defines/Imports/Uses）
4. Endpoint integrity 检查通过（无 dangling edges）
5. 输出确定性验证通过（两次运行结构相同）
6. 运行时间 < 30s
7. 无 panic / crash / hang

**Should Have：**
- 记录运行时间
- 如遇 bug，记录详细错误信息和复现步骤

**Nice to Have：**
- 性能优化建议
- 下一刀优先级建议

## Implementation Steps

### Step 1: 准备工作（5 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
git status  # 确认工作树干净
git log --oneline -1  # 确认 HEAD
```

**Expected:**
- 工作树干净
- HEAD = `984a0ac` (Slice 17 feature-gate follow-up closure)

### Step 2: 运行 Production Fixture Smoke（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
time cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index \
  2>/dev/null | jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}'
```

**Expected:**
- Exit code = 0
- 输出合法 JSON
- 运行时间 < 30s

**Failure Mode:**
- Exit code ≠ 0 → 记录 stderr，分析错误原因
- Panic → 记录 backtrace，评估是否修复
- Hang → 终止进程，分析卡死原因
- Timeout > 30s → 记录性能问题

### Step 3: 统计节点/边类型分布（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index \
  2>/dev/null | jq '{
  nodeTypes: (.nodes | map(.kind) | group_by(.) | map({kind: .[0], count: length})),
  edgeTypes: (.edges | map(.kind) | group_by(.) | map({kind: .[0], count: length}))
}'
```

**Expected:**
- Node types: Repository, Package, SourceFile, Symbol
- Edge types: ContainsPackage, OwnsSource, Defines, Imports, Uses

### Step 4: Endpoint Integrity 检查（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index \
  2>/dev/null | jq '{
  nodeIds: ([.nodes[].id] | sort | unique | length),
  edgeSourceIds: ([.edges[].source_id] | sort | unique | length),
  edgeTargetIds: ([.edges[].target_id] | sort | unique | length),
  danglingSources: [.edges[] | select(.source_id as $id | .nodes[] | .id == $id | not)],
  danglingTargets: [.edges[] | select(.target_id as $id | .nodes[] | .id == $id | not)]
}'
```

**Expected:**
- `danglingSources` = `[]`
- `danglingTargets` = `[]`
- 所有 edge source/target 都在 nodes 中找到

### Step 5: 输出确定性验证（10 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
# 第一次运行
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index \
  2>/dev/null > /tmp/run1.json

# 第二次运行
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index \
  2>/dev/null > /tmp/run2.json

# 比较结构
diff <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/run1.json) \
     <(jq '. | {nodeCount: (.nodes | length), edgeCount: (.edges | length)}' /tmp/run2.json)
```

**Expected:**
- `diff` 无输出（两次运行结构相同）

### Step 6: Bug 修复（如需要）（30 分钟）

**Action:**
- 如 Step 2-5 发现明显 bug（panic / crash / dangling edges），在 bounded scope 内修复
- Bug 修复必须：< 50 行变更，只改逻辑不改 API
- 如 bug 修复需较大改动，只记录 gap，不开新功能

**Example:**
```bash
# 如需修复 bug
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
git add -A
git commit -m "fix(cangjie): <bug description>"
git push gitcode master
```

### Step 7: 验证测试（5 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
```

**Expected:**
- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ 45/45 pass
- `cargo test --features tree-sitter-cangjie`: ✅ 233/233 pass

### Step 8: 编写 Closure Review（15 分钟）

**Action:**
- 创建 `docs/plans/2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-closure-review.md`
- 记录：
  - Landed reality（实际运行结果）
  - 统计数据（nodes/edges count, type distribution）
  - 发现的 bug / gap（如有）
  - 残留风险
  - Next opening 建议

### Step 9: 更新文档并提交（5 分钟）

**Action:**
```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
git add docs/plans/
git commit -m "test(cangjie): add production fixture smoke"
git push gitcode master
```

**Expected:**
- Commit 成功
- Push 成功

## Exit Criteria

Slice 18 完成的标志：
- ✅ Production fixture smoke 运行成功（exit 0）
- ✅ 统计报告完整（nodes/edges count, type distribution）
- ✅ Endpoint integrity 检查通过
- ✅ 输出确定性验证通过
- ✅ 运行时间 < 30s
- ✅ 无 panic / crash / hang
- ✅ Closure review 完成
- ✅ `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` 全部 pass
- ✅ Commit + push gitcode master
- ✅ docs/plans/README.md 更新

## Next Openings

根据 Slice 18 结果，选择下一个 bounded slice：
- **Option A**: 如发现 bug → Fix bug slice（bounded scope）
- **Option B**: 如性能问题 → Performance optimization slice
- **Option C**: 如功能缺口 → Feature enhancement slice
- **Option D**: 如一切正常 → 继续扩展现有能力

**优先级原则：**
1. 最小 slice（< 4 小时）
2. 最有生产价值
3. 低风险（不违反 stop-lines）

## Timeline

- Step 1: 准备工作（5 分钟）
- Step 2: Production fixture smoke（10 分钟）
- Step 3: 统计节点/边类型分布（10 分钟）
- Step 4: Endpoint integrity 检查（10 分钟）
- Step 5: 输出确定性验证（10 分钟）
- Step 6: Bug 修复（如需要）（30 分钟）
- Step 7: 验证测试（5 分钟）
- Step 8: Closure review（15 分钟）
- Step 9: 更新文档并提交（5 分钟）

**Total: ~1.5-2 小时**（不含 bug 修复）

---

**Decision:** Begin implementation following frozen scope and stop-lines.
