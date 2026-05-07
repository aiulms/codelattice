# Slice 21 Closure Review — Cangjie Constructor Symbol Extraction

**Date:** 2026-05-07  
**Status:** Closure Review  
**Type:** Feature / Semantic Upgrade  
**Slice ID:** Phase 2 Slice 21  

---

## Landed Reality

### ✅ 成功完成的目标

1. **语义核查结论：能安全推进，采用方案 C3（补充 + Fallback 共存）**
   - Constructor symbol extraction 不会替代 synthetic nodes，而是补充
   - Init symbol nodes 替代 Constructor 类 synthetic nodes
   - Method/Function 类 synthetic nodes 保留作为 fallback

2. **CangjieSymbolKind::Init 枚举变体**
   - 新增 Init kind + Display trait
   - CangjieSymbol 新增 owner_name: Option<String> 字段
   - Init symbol 命名格式：`Owner.init`（如 `Foo.init`）

3. **Init symbol extraction**
   - tree-sitter query 新增 `(init "init" @init_name)` 模式
   - 从 classDefinition/structDefinition body 中提取 init 定义
   - 提取 owner_name 通过向上遍历 AST 到 class/struct definition
   - 4 个 unit tests 验证

4. **Constructor source ID → Init symbol node ID 映射**
   - `emit_cangjie_reference_edges` 返回 `(edges, resolved_source_ids)`
   - `resolve_source_id()` 函数：Constructor source_id → init symbol node_id
   - 支持 #arity 后缀的匹配

5. **Synthetic nodes coexistence policy**
   - `emit_synthetic_source_nodes` 接受 `resolved_source_ids` 参数
   - 已被 init symbol 覆盖的 source ID 不再发 synthetic node
   - Constructor 类 synthetic nodes 降为 0

6. **Fixture/test expansion**
   - 新增 `fixtures/cangjie/constructor-basic/`：class init / struct init / 多 init / 无 init
   - 新增 `fixtures/cangjie/constructor-cross-file/`：跨文件 constructor
   - 新增 `tests/constructor_extraction.rs`：6 个 integration tests
   - 增强 `tests/endpoint_integrity.rs`：11 个 property tests（含确定性、去重、覆盖）

### 📊 Before/After 数据对比

| Metric | Before (Slice 20) | After (Slice 21) | Change |
|--------|------------------|------------------|--------|
| CangjieSymbolKind variants | 7 | 8 | +1 (Init) |
| CangjieSymbol fields | 4 | 5 | +1 (owner_name) |
| Constructor synthetic nodes | ~646 (production) | 0 (fixture) | -100% on fixture |
| Synthetic node emission | 无条件 | 条件（跳过已覆盖） | 逻辑升级 |
| Lib tests (with feature) | 105 | 109 | +4 (init extraction) |
| Endpoint integrity tests | 4 | 11 | +7 (property tests) |
| Constructor tests | 0 | 6 | +6 |
| Total integration tests (with feature) | ~7 | ~17 | +10 |

### ✅ 验证结果

- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ 95/95 passed
- `cargo test --features tree-sitter-cangjie`: ✅ all passed (109 lib + 17 integration)
- Endpoint integrity: ✅ 0 dangling source, 0 dangling target (all fixtures)
- Constructor synthetic nodes: ✅ 0 on constructor-basic fixture
- Graph output determinism: ✅ verified

---

## Residual Risks

### Low Risk
- ✅ Synthetic fallback 保留：Method/Function 类 source IDs 仍使用 synthetic nodes
- ✅ 不改变 reference source ID 格式：只在 graph emission 层做映射
- ✅ 不改 GitNexus-RC / Tool / live repo

### Medium Risk
- ⚠️ tree-sitter grammar 对 init 的解析可能有 edge cases（如泛型 init、prop init）
- **缓解：** synthetic fallback 兜底；known limitation 记录

---

## Next Openings

### Priority 1: Cangjie graph quality hardening on production fixture
- 对 production fixture 验证 Constructor synthetic nodes 下降数量
- 更新 multi-project smoke baseline

### Priority 2: Cangjie import/reference quality
- alias/wildcard/import resolver edge cases
- grouped import + alias + cross-file reference regression

### Priority 3: Method symbol extraction (future, post stop-line)
- 当 method dispatch 支持时，Method 类 synthetic nodes 可被替代
- 当前是 stop-line

---

## Exit Criteria Review

- ✅ `cargo fmt --check` pass
- ✅ `cargo test` pass
- ✅ `cargo test --features tree-sitter-cangjie` pass
- ✅ Endpoint integrity 0 dangling (all fixtures)
- ✅ Constructor synthetic nodes 降为 0 (fixture)
- ✅ Init symbol extraction 正确
- ✅ Closure review 完成
- ✅ docs/plans 更新
- ⏳ Commit + push（进行中）

**Slice 21 状态：** ✅ **完全成功（Fully Successful）**
