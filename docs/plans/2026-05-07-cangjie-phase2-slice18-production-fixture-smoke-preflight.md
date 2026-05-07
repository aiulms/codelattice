# Slice 18 Preflight — Cangjie Production Fixture Smoke

**Date:** 2026-05-07  
**Status:** Preflight  
**Type:** Production Validation / Smoke Test  
**Slice ID:** Phase 2 Slice 18  

## Background

Slice 17 (Cangjie CLI surface MVP) 已完成 CLI 实现，feature-gate follow-up 已关闭。现在需要验证 Rust-native Cangjie CLI 在真实生产环境中的可用性。

**当前状态：**
- Rust-core CLI: `cangjie inspect` / `cangjie graph` 可用
- Integration tests: 233/233 pass (with feature), 45/45 pass (without feature)
- Feature-gate: graceful failure with clear error message
- Fixtures: 小型 test fixtures (cjpm-basic, imports-basic, etc.)

**生产环境验证需求：**
- 当前测试 fixture 规模较小（~10-20 files）
- 需要验证在真实 Cangjie 项目中的表现
- 发现潜在的 runtime bug / 性能问题 / 边界情况

## Production Fixture

**选择：** `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index`

**理由：**
- 真实的 Cangjie GUI 应用项目
- 包含多个子项目（runtime/cjgui, labs/cffi_smoke, labs/macos_bridge_smoke）
- 代码规模适中（~10+ .cj files）
- 只读访问，不修改 live repo

**项目结构：**
```
cangjie-GitNexus-Index/
  runtime/cjgui/          # 主项目
    cjpm.toml
    src/
      action_handoff.cj
      window_lifecycle.cj
      runtime_bootstrap.cj
      ...
  labs/cffi_smoke/        # FFI smoke test
    cjpm.toml
    src/main.cj
  labs/macos_bridge_smoke/ # macOS bridge smoke test
    cjpm.toml
    src/main.cj
```

## Goals

### Primary Goals

1. **验证 CLI 可用性**
   - 在真实项目上成功运行 `cangjie inspect`
   - 输出合法 JSON（可被 `jq` 解析）
   - 无 panic / 无挂死 / 无 OOM

2. **统计基础指标**
   - Nodes / edges 总数
   - Node type 分布（Repository/Package/SourceFile/Symbol）
   - Edge type 分布（ContainsPackage/OwnsSource/Defines/Imports/Uses）
   - 运行时间（粗粒度）

3. **验证图结构完整性**
   - 所有 edge source/target 必须在 nodes 中找到（endpoint integrity）
   - 无 dangling edges
   - Node id 唯一性

4. **验证输出确定性**
   - 连续运行两次，输出结构稳定
   - 相同输入 → 相同输出

### Secondary Goals

5. **发现潜在 bug**
   - 记录遇到的任何错误 / panic / hang
   - 如 bug 明显且 bounded，可在本 slice 内修复
   - 否则只记录 gap，不开新功能

6. **评估性能表现**
   - 运行时间是否合理（< 30s）
   - 内存使用是否正常

## Write Set

**允许修改：**
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/`：
  - `docs/plans/`：添加 preflight / execution card / closure review
  - `crates/cli/`：如有明显 bug，可修复 CLI 代码
  - `crates/cangjie/`：如有明显 bug，可修复 core 代码
  - `fixtures/`：添加小型 helper test（如需要）
  - `tests/`：添加 smoke test（可选）

**禁止修改：**
- `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/`：只读访问
- `/Users/jiangxuanyang/Desktop/cangjie/`：live repo，禁止修改
- GitNexus-RC runtime/schema/package/web
- GitNexus-RC-Tool

## Stop-lines

**严格禁止：**
- 不引入 LSP/MCP/HTTP/UI/embedding
- 不做 type inference / trait solving
- 不做 macro expansion
- 不修改 GitNexus-RC runtime/Tool/live repo
- 不把 production fixture 的分析产物当业务代码提交

**修复边界：**
- 只修复明显的 runtime bug（panic / crash / deadlock）
- 不开新功能 / 不扩展现有 API
- 不做算法优化 / 性能调优（除非严重影响可用性）

## Risk Assessment

### Low Risk
- ✅ 只读访问 production fixture
- ✅ 不修改 GitNexus-RC / Tool / live repo
- ✅ CLI 已通过 integration tests
- ✅ 图结构已有 endpoint integrity 检查

### Medium Risk
- ⚠️ 真实项目可能暴露未知 bug
- ⚠️ 文件规模可能较大（需监控性能）
- ⚠️ 可能有语法错误 / 编译错误（graceful degrade）

### Mitigation
- ✅ 已有 graceful degrade 机制（错误返回 exit 1）
- ✅ 不提交分析产物 JSON（只提交统计报告）
- ✅ 如遇 bug，先评估是否 bounded 修复

## Acceptance Criteria

### Must Have
- [ ] CLI 在 production fixture 上成功运行（exit 0）
- [ ] 输出合法 JSON（可被 `jq` 解析）
- [ ] 统计报告包含：nodes/edges 总数、node type 分布、edge type 分布
- [ ] Endpoint integrity 检查通过（无 dangling edges）
- [ ] 输出确定性验证通过（两次运行结构相同）
- [ ] 运行时间 < 30s
- [ ] 无 panic / crash / hang

### Should Have
- [ ] 记录运行时间
- [ ] 如遇 bug，记录详细错误信息和复现步骤
- [ ] 添加小型 smoke test（可选）

### Nice to Have
- [ ] 性能优化建议（如发现明显瓶颈）
- [ ] 下一刀优先级建议

## Exit Criteria

Slice 18 完成的标志：
1. ✅ Production fixture smoke 运行成功
2. ✅ 统计报告完整
3. ✅ Closure review 完成
4. ✅ `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` 全部 pass
5. ✅ Commit + push gitcode master
6. ✅ docs/plans/README.md 更新

## Next Openings

根据 Slice 18 暴露的缺口，选择下一个 bounded slice：
- **Option A**: 如发现明显 bug → Fix bug slice
- **Option B**: 如性能有问题 → Performance optimization slice
- **Option C**: 如功能缺口 → Feature enhancement slice
- **Option D**: 如一切正常 → 继续扩展现有能力（如 diagnostics 集成、更多 edge types）

**优先级：** 最小且最有生产价值的一刀

## Dependencies

- ✅ Slice 17 (Cangjie CLI surface MVP) 已完成
- ✅ Slice 17 feature-gate follow-up 已完成
- ✅ Production fixture 存在且只读可访问

## Timeline Estimate

- Preflight: 已完成
- Implementation: 1-2 小时
- Verification: 30 分钟
- Closure Review: 30 分钟
- **Total: ~2-3 小时**

---

**Decision:** Proceed to execution card (bounded scope, low risk).
