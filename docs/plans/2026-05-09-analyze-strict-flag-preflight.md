# Analyze --strict Flag Preflight

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Preflight
> **依赖：** Productization Phase (d016b5d..8bdcddf)

---

## 一、动机

当前产品化 CLI 中，`cangjie inspect/graph` 已有 `--strict` flag（检查 synthetic>0 并 exit non-zero），但统一入口 `analyze` 没有。这导致：

1. CI/CD pipeline 无法通过 exit code 判断分析质量
2. AI workflow 调用 `analyze` 后需额外解析 JSON 才能判断质量门状态
3. 与 Cangjie 子命令的 `--strict` 行为不一致

本 slice 目标：为 `analyze` 命令新增 `--strict` flag，与现有 quality gates 集成。

## 二、设计方案

### 2.1 CLI 参数

```bash
gitnexus-rust-core analyze --root <path> --language auto --strict
```

- `--strict`：默认 false，布尔 flag（`--strict` 或 `--strict=true`）
- 当 `--strict` 为 true 时，分析完成后检查所有 quality gates
- 任一 gate 失败 → stderr 输出失败摘要 → exit code 1
- 全部 pass → 正常输出 JSON → exit code 0

### 2.2 行为逻辑

```
analyze --strict:
  1. 运行分析 → 获得 graph JSON + nodes + edges
  2. 计算 quality gates（与现有逻辑相同）
  3. 如果 is_bridge: 输出 bridge JSON
     否则: 输出 LanguageAnalysisResult JSON
  4. 检查 quality gates:
     - 全部 pass → exit 0
     - 有失败 → stderr 打印 "质量门失败: gate1, gate2, ..." → exit 1
```

### 2.3 注意事项

- `deterministic` gate 在单次 CLI 运行中始终 pass（"not verified from single CLI run"），不会触发 strict 失败
- `--strict` 不影响 JSON 输出内容，只影响 exit code
- `--strict` 与 `--format gitnexus-rc` 兼容（bridge 格式也输出后检查 gates）

## 三、Write Set

| 文件 | 操作 |
|------|------|
| `crates/cli/src/main.rs` | 修改：Analyze 命令新增 --strict flag + strict 逻辑 |
| `crates/cli/tests/productization_commands.rs` | 修改：新增 --strict 成功/失败路径测试 |
| `docs/plans/README.md` | 修改：新增本 slice 记录 |
| `docs/plans/2026-05-09-productization-phase-closure-review.md` | 修改：更新 residual gaps |

## 四、Stop-line 验证

| Stop-line | 状态 |
|-----------|------|
| 不修改 GitNexus-RC | ✅ |
| 不修改 GitNexus-RC-Tool | ✅ |
| 不修改 live repo | ✅ |
| 不新增依赖 | ✅ |
| 不做 destructive git | ✅ |
| 不做 WebUI/MCP/HTTP | ✅ |
| 不做 production replacement | ✅ |

## 五、风险

| 风险 | 级别 | 缓解 |
|------|------|------|
| strict + bridge 格式兼容 | LOW | bridge 格式也走相同 quality gates 路径 |
| deterministic gate 误触发 | LOW | 该 gate 始终 pass |
