# Production Trial Acceptance Checklist

> **日期：** 2026-05-09
> **状态：** Active
> **来源：** [Production Trial Readiness and Roadmap Pivot Preflight](2026-05-09-production-trial-readiness-and-roadmap-pivot-preflight.md)
> **用途：** 固化 alpha production trial 的最低验收标准，作为收尾实现和 closure review 的检查清单。
> **Stop-line：** 本文只定义验收标准，不启动新功能实现，不改 runtime/CLI/schema。

---

## 一、验证命令（每次提交前必须通过）

### 1.1 格式化与基础检查

- [ ] `cargo fmt --check` — 代码格式一致
- [ ] `git diff --check` — 无空白冲突

### 1.2 测试套件

- [ ] `cargo test` — 全量 no-feature 测试通过
- [ ] `cargo test --features tree-sitter-cangjie` — 全量 Cangjie feature 测试通过
- [ ] `cargo test --test bridge_roundtrip` — bridge 格式回归（13 tests）
- [ ] `cargo test --features tree-sitter-cangjie --test bridge_roundtrip` — 双语言 bridge 回归（26 tests）
- [ ] `cargo test --test productization_commands` — 产品化命令回归（11 tests）
- [ ] `cargo test --features tree-sitter-cangjie --test productization_commands` — 双语言产品化回归（19 tests）

### 1.3 合同测试

- [ ] `cargo test --test project_model_graph_contract` — Rust graph contract（58/58 on 8 fixtures）
- [ ] `cargo test --features tree-sitter-cangjie --test graph_contract` — Cangjie graph contract（24/24 on 4 fixtures）
- [ ] `cargo test --features tree-sitter-cangjie --test cangjie_inspect` — Cangjie inspect（18/18）
- [ ] `cargo test --features tree-sitter-cangjie --test multi_project_smoke` — 多项目 smoke

### 1.4 手动 smoke

- [ ] Rust bridge smoke: `analyze --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc --strict` → exit 0
- [ ] Cangjie bridge smoke: `analyze --root fixtures/cangjie/portable-smoke --language cangjie --format gitnexus-rc --strict` → exit 0
- [ ] `scripts/verify-bridge.sh --rust-only` → all PASS
- [ ] `scripts/verify-bridge.sh` → all PASS（需 Cangjie feature）
- [ ] `scripts/smoke.sh --quick` → all PASS 或 documented graceful skip

---

## 二、命令行为冻结

以下命令行为必须在 alpha trial 周期内保持稳定：

| 命令 | 冻结点 | 验证方式 |
|------|--------|---------|
| `analyze --format json` | JSON envelope 结构、exit code（0/1） | `productization_commands` tests |
| `analyze --format gitnexus-rc` | bridge JSON 结构、分组边、端点归一化 | `bridge_roundtrip` tests |
| `analyze --strict` | 质量门失败时 exit non-zero | `productization_commands` tests |
| `quality` | gate 列表、JSON 输出、exit code 0/1/2 | CLI smoke |
| `summary` | 轻量统计摘要 JSON | CLI smoke |
| `--language auto` | 自动检测规则（Cargo.toml → Rust, cjpm.toml → Cangjie）| `productization_commands` tests |

### 暂不冻结

- experimental / internal 子命令
- `--format` 的旧命名（保留为兼容 alias，但 README 默认示例不再首推）

---

## 三、输出字段冻结

### 3.1 Analyze JSON envelope

| 字段 | 稳定性 | 说明 |
|------|--------|------|
| `language` | **Stable** | `"rust"` 或 `"cangjie"` |
| `root` | **Stable** | 项目根目录绝对路径 |
| `schemaVersion` | **Stable** | 当前 `"0.3.0"` |
| `generatedAt` | **Stable** | ISO 8601 时间戳 |
| `summary` | **Stable** | 统计摘要 |
| `qualityGates` | **Stable** | 质量门结果数组 |
| `graph` | **Stable** | 图数据（nodes/edges/stats） |

**建议补充（下一轮）：** `schemaName`、`toolVersion`、`warnings`、`capabilities`

### 3.2 Bridge JSON（`--format gitnexus-rc`）

| 字段路径 | 稳定性 | 说明 |
|---------|--------|------|
| `schemaVersion` | **Stable** | |
| `generatedAt` | **Stable** | |
| `language` | **Stable** | |
| `root` | **Stable** | |
| `repository.id` / `repository.path` | **Stable** | |
| `packages[].id` / `name` / `manifestPath` | **Stable** | |
| `sourceFiles[].id` / `path` | **Stable** | |
| `symbols[].id` / `name` / `kind` | **Stable** | kind 已具体化（非通用 `"symbol"`） |
| `edges.*[].sourceId` / `targetId` / `kind` | **Stable** | 端点已归一化 |
| `edges.*[].confidence` / `reason` | **Stable** | Rust 有值，Cangjie 为 null |
| `stats.*` | **Stable** | 与数组实际计数一致 |
| `diagnostics[]` | **Stable** | 数组结构稳定 |

**Node ID 格式声明：** bridge JSON 的 raw node ID（`repo:`/`package:`/`symbol:`/`file:` 前缀）不保证跨版本稳定。消费侧不应依赖 raw ID 做跨系统互查，应通过 adapter 转换。

---

## 四、质量门冻结

以下 quality gates 必须在每次 smoke 中验证：

| 质量门 | 阈值 | 验证方式 |
|--------|------|---------|
| Duplicate node IDs | 0 | `graph_contract` / `multi_project_smoke` |
| Duplicate edge triples | 0 | `graph_contract` / `multi_project_smoke` |
| Dangling source references | 0 | `graph_contract` / `verify-bridge.sh` |
| Dangling target references | 0 | `graph_contract` / `verify-bridge.sh` |
| Deterministic output | 两次运行结果一致 | `verify-bridge.sh` / `graph_contract` |
| Symbol kind 具体化 | 无通用 `"symbol"` | `bridge_roundtrip` / `verify-bridge.sh` |
| Stats consistency | stats 字段与数组计数一致 | `bridge_roundtrip` / `verify-bridge.sh` |
| Endpoint field normalization | edge 用 `sourceId`/`targetId`（非 `source`/`target`） | `bridge_roundtrip` |
| CALLS endpoint integrity | CALLS edge 的 source/target 节点必须存在 | `graph_contract` |
| Synthetic nodes (Cangjie) | 0（fixture 级别） | `multi_project_smoke` |
| Init symbols `#arity` suffix (Cangjie) | 所有 Init 符号匹配 | `graph_contract` |

---

## 五、Smoke Targets 固化

### 5.1 Tier 1：仓库内 fixture（always available, must pass）

**Rust fixtures:**
- [ ] `fixtures/rust/portable-smoke/` — 全符号类型 + 全边类型
- [ ] `fixtures/rust/module-hierarchy/` — crate::/super::/import 绑定
- [ ] `fixtures/rust/inline-module/` — inline module + HAS_PARENT
- [ ] `fixtures/rust/self-path/` — self:: 路径
- [ ] `fixtures/rust/enum-variant/` — enum variant 符号 + 调用
- [ ] `fixtures/rust/workspace-member/` — workspace + 跨 crate 调用
- [ ] `fixtures/rust/imports-cross-crate/` — 外部符号 + ACCESSES
- [ ] `fixtures/rust/multi-module/` — 跨文件 crate:: 路径

**Cangjie fixtures:**
- [ ] `fixtures/cangjie/portable-smoke/` — 全符号类型 + 全边类型
- [ ] `fixtures/cangjie/imports-basic/` — named/grouped/wildcard/alias imports
- [ ] `fixtures/cangjie/constructor-basic/` — Init 符号 + #arity
- [ ] `fixtures/cangjie/reference-cross-file-basic/` — 跨文件 Uses + Imports

### 5.2 Tier 2：本机真实项目只读 smoke（optional, graceful skip）

- [ ] `gitnexus-rust-core` 自身 Rust self-smoke
- [ ] Cangjie live repo 只读目标（4 production targets）
- [ ] 缺失时 graceful skip（不报错，输出 SKIP）

要求：只读、不 clean/build 用户项目、输出统计记录到 closure review。

### 5.3 Tier 3：人工验收样本（alpha release 前）

- [ ] 选 1 个中等 Rust 项目，完整 analyze + quality
- [ ] 选 1 个中等 Cangjie 项目，完整 analyze + quality
- [ ] 将输出喂给 AI 做一次实际任务（理解模块/定位影响范围/生成变更风险报告）

---

## 六、AI 消费最小接口

以下接口必须稳定，供 AI / script 消费：

### 6.1 summary

```bash
cargo run -p gitnexus-rust-core-cli -- summary --root <project> --language <lang> --format json
```

输出：项目规模（nodes/edges/symbols/files/packages）、语言、quality gate 总览。

### 6.2 quality

```bash
cargo run -p gitnexus-rust-core-cli -- quality --root <project> --language <lang>
```

输出：gate 列表、pass/fail、fail 原因。Exit code 0/1/2。

### 6.3 analyze（完整 graph）

```bash
cargo run -p gitnexus-rust-core-cli -- analyze --root <project> --language <lang> --format json --strict
```

输出：完整 graph JSON（nodes/edges/diagnostics/stats + confidence/reason）。

### 6.4 analyze（bridge format）

```bash
cargo run -p gitnexus-rust-core-cli -- analyze --root <project> --language <lang> --format gitnexus-rc --strict
```

输出：分组 bridge JSON（calls/defines/uses/accesses/designations/imports/contains/owns/annotates/other），供下游 adapter/AI workflow 消费。

---

## 七、已知限制文档化

以下限制必须在 README.md 中明确说明：

### 7.1 Rust 已知限制

- [ ] 不做完整类型推断
- [ ] 不做 trait solving
- [ ] 不做 proc-macro / build.rs 执行
- [ ] 不做 macro expansion
- [ ] 不做完整 cfg evaluator
- [ ] 不做任意第三方 crate API 深度解析
- [ ] method dispatch 为低置信度启发式（~1204 unresolved）

### 7.2 Cangjie 已知限制

- [ ] 不做完整 method dispatch
- [ ] 不做完整 interface / extend solving
- [ ] 不做 macro / metaprogramming 深解析
- [ ] 不做跨仓全局依赖图
- [ ] 不修改 live repo

### 7.3 产品层不承诺

- [ ] UI / WebUI
- [ ] MCP server
- [ ] 云端服务
- [ ] 多语言大覆盖
- [ ] 完整 IDE 插件
- [ ] v1.0 兼容性承诺

---

## 八、不进入短期收尾的工作

以下工作明确进入长期路线，不阻塞 alpha production trial：

- [ ] 正式改名（crate/binary 全量重命名）
- [ ] `--format` 中性命名方案（保留现有 flag 为兼容 alias）
- [ ] MCP server 实现
- [ ] UI/Web 实现
- [ ] 新语言支持
- [ ] 深层 trait/type/macro 求解
- [ ] 完整 method dispatch
- [ ] Release CI/CD
- [ ] crates.io 发布

---

## 九、标 v0.1 / alpha production trial 的前置条件

### 必须满足（阻塞性）

- [ ] `cargo fmt --check` pass
- [ ] `cargo test` pass
- [ ] `cargo test --features tree-sitter-cangjie` pass
- [ ] `scripts/smoke.sh --quick` pass
- [ ] 完整 smoke 通过或仅有 documented graceful skip
- [ ] README 公开定位清楚
- [ ] LICENSE 存在
- [ ] Quality gates 文档化（QUALITY.md）
- [ ] Rust / Cangjie capability matrix 文档化（README.md）
- [ ] Known limitations 文档化（README.md / QUALITY.md）

### 建议满足（非阻塞）

- [ ] 至少 1 个真实 Rust 项目 smoke 记录
- [ ] 至少 1 个真实 Cangjie 项目 smoke 记录
- [ ] AI-friendly summary/report 初版
- [ ] `PROVENANCE.md` 或类似说明（降低公开误解）

---

## 十、与原有 Stage 0-5 框架的关系

本 checklist 不覆盖 Stages 0-5 中已完成的工作（bridge format 产品化、consumer contract 固化、adapter readiness test pack、local trial packaging、adapter preflight）。这些工作已验证完成，相关文档和测试已落地。

本 checklist 面向的是：在 Stages 0-5 基础上，收束为一个可对外声明的 alpha production trial，而不是继续扩产品面。

---

## 十一、下一步

1. 按本 checklist 逐项打勾，确认当前状态。
2. 如有未满足项，开最小 execution card 修复。
3. 全部满足后，标记 `alpha production trial ready`。
4. 后续工作（MCP/UI/新语言/正式改名）进入长期路线，不阻塞 alpha trial。
