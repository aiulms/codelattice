# Alpha Trial Bridge Endpoint + Stdout Purity Closure Review

> **日期：** 2026-05-09
> **类型：** Closure Review
> **关联 Preflight：** docs/plans/2026-05-09-production-trial-readiness-and-roadmap-pivot-preflight.md
> **结论：** ✅ Alpha Production Trial Ready

---

## 一、修复范围

| 修复项 | 文件 | 说明 |
|--------|------|------|
| Rust workspace dangling edges | `crates/cli/src/rust_bridge.rs` | workspace→package, diagnostic→symbol 映射 |
| Cangjie stdout purity | `crates/cangjie/vendor/tree-sitter-cangjie/src/scanner.c` | fprintf→stderr（stdout 硬污染根因） |
| Debug log 收敛 | `crates/cli/src/main.rs` | `#[cfg(debug_assertions)]` guard（避免调试日志在混流/脚本消费中制造噪声） |
| Test 适配 | `crates/cli/tests/productization_commands.rs` | bridge vs json symbolCount 改为 >= |
| .gitignore | `.gitignore` | 新增 .gitnexus |

---

## 二、Rust Workspace Bridge Dangling Edge

### Root Cause

Rust graph 输出包含 workspace、diagnostic 等 label 的节点。Bridge 格式转换 `partition_rust_nodes()` 原先将这些 label 全部跳过（不计入 symbol 也不计入 package）。但 graph 中存在引用这些节点 ID 的边：

- `CONTAINS_WORKSPACE` / `CONTAINS_PACKAGE`：source=target → workspace / package
- `ANNOTATES`：source → diagnostic

这些边进入 GitNexus-RC Tool 的 bridge adapter 时，validator V6 检查所有 edge 的 sourceId/targetId 是否在 node-like ID 集合中。workspace 和 diagnostic 节点未被输出为任何 node-like entry → dangling。

### 修复策略

在 bridge 端做显式映射，不放宽 Tool adapter validator：

- **workspace 节点** → 输出为 `packages[]` 条目（name="workspace"），作为 `CONTAINS_WORKSPACE` / `CONTAINS_PACKAGE` 的端点
- **diagnostic 节点** → 输出为 `symbols[]` 条目（kind="Diagnostic"），作为 `ANNOTATES` edge 的 source 端点
- **module 节点** → 保持跳过（当前无 edge 引用 module 节点 ID）

### 过滤原则

如果未来某类 metadata edge 不应进入下游 consumption，应在 bridge 端过滤（不输出该边），并记录 reason。不允许产生 sourceId/targetId 指向不存在 node 的边。

### 修复后结果

- Rust-core 自身 bridge JSON：1700 nodes, 2634 edges, **0 dangling**
- Tool `--experimental-rust-core-bridge-graph` 导入：**成功**（4711 nodes / 7000 edges）

---

## 三、Cangjie Stdout Purity

### Root Cause

stdout 硬污染来自 tree-sitter-cangjie scanner.c 中 `fprintf(stdout, msg)`。该调用在 `DEBUG_SCANNER` 启用时直接向 stdout 写入调试信息，污染 JSON 输出前缀。

此外，CLI main.rs 中的 `eprintln!("分析中...")` 虽然本身走 stderr（不是 stdout 污染源），但在不当混流（`2>&1`）或消费脚本捕获全部输出时可能制造噪声，因此也做了收敛处理。

### 修复

- scanner.c：`fprintf(stdout, msg)` → `fprintf(stderr, "%s", msg)`，消除 stdout 硬污染根因，同时修复格式化参数安全性
- main.rs：在 `eprintln!` 外加 `#[cfg(debug_assertions)]`，release build 不输出调试日志（额外收敛，不是 stdout 污染主因）

### 修复后结果

- `cargo run --features tree-sitter-cangjie -- analyze ... --format gitnexus-rc` stdout 从第 1 字节就是合法 JSON
- **不再需要 sed 清理**
- 编译 warning（unused parameter/logger）仅出现在 stderr，不影响 stdout

---

## 四、Real Project Alpha Smoke

### Rust Workspace Trial

```
Rust-core 自身 bridge JSON:
  cargo run -- analyze --root ... --language rust --format gitnexus-rc --strict
  → Valid JSON, 0 dangling edges, exit 0

Tool 导入:
  node ... analyze --force --experimental-rust-core-bridge-graph /tmp/rust-core-bridge.json
  → Repository indexed successfully (4711 nodes, 7000 edges)
```

### Cangjie Real-Project Trial

```
cjgui bridge JSON:
  cargo run --features tree-sitter-cangjie -- analyze --root .../cjgui --language cangjie --format gitnexus-rc --strict
  → Valid JSON from byte 1, no sed needed

Tool 导入:
  node ... analyze --force --experimental-rust-core-bridge-graph /tmp/cjgui-bridge.json --name cjgui-bridge
  → Repository indexed successfully (4851 nodes, 7000 edges)

Tool detect-changes:
  → Changes: 5 files, 8 symbols, 13 flows (expected — reflects current dirty state)
```

---

## 五、generatedAt 字段策略

`generatedAt` 只保证字段存在和 ISO 8601 格式稳定。**值不稳定**（每次生成不同），不能参与 strict deterministic 比较。deterministic 测试和 `verify-bridge.sh` 已排除此字段。任何消费方不应依赖 `generatedAt` 值做严格相等比较。

---

## 六、回归验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo fmt --check` | ✅ clean |
| `git diff --check` | ✅ clean |
| `cargo test` | ✅ 全部通过 |
| `cargo test --features tree-sitter-cangjie` | ✅ 全部通过 |
| `cargo test --test bridge_roundtrip` | ✅ 13/13 (Rust only) |
| `cargo test --features tree-sitter-cangjie --test bridge_roundtrip` | ✅ 26/26 (13 Rust + 13 Cangjie) |
| `cargo test --test productization_commands` | ✅ 11/11 |
| `cargo test --features tree-sitter-cangjie --test productization_commands` | ✅ 19/19 |
| `scripts/verify-bridge.sh --rust-only` | ✅ 2/2 PASS |
| `scripts/verify-bridge.sh` | ✅ 4/4 PASS |
| `scripts/smoke.sh --quick` | ✅ 8/8 PASS, 1 SKIP |
| GitNexus-RC `npx tsc --noEmit` | ✅ clean |
| GitNexus-RC-web `npx tsc -b --noEmit` | ✅ clean |
| Tool `detect-changes --repo gitnexus-rc` | ✅ No changes |

---

## 七、Web/LLM/MCP 触碰

**未触碰。** 本次修复仅涉及 Rust-core CLI 和 bridge JSON 生成端。未修改 GitNexus-RC runtime/adapter/schema，未启动 Web 服务，未调用 LLM API，未操作 MCP server。

---

## 八、半成品清理

| 项目 | 处理 |
|------|------|
| `.claude/` | 已删除（agent 私有目录） |
| `CLAUDE.md` | 已删除（Tool 自动生成的 header artifact） |
| `AGENTS.md` gitnexus block | 已还原（git checkout），保留原始治理文档 |
| `cangjie-GitNexus-Index` AGENTS.md/CLAUDE.md | 已还原（git checkout） |
| `.gitignore` +.gitnexus | **保留** — Tool 索引产生 .gitnexus 目录，应忽略 |
| scanner.c fprintf 修复 | **保留** — stdout purity 必要 |
| main.rs cfg guard | **保留** — stdout purity 必要 |
| rust_bridge.rs endpoint mapping | **保留** — dangling edge 修复必要 |
| productization_commands test 适配 | **保留** — 反映 bridge endpoint 映射后的正确行为 |

---

## 九、Alpha Production Trial Ready 评估

| 标准 | 状态 |
|------|------|
| 真实 Rust 项目稳定运行 | ✅ Rust-core 自身 4711 nodes / 7000 edges |
| 真实 Cangjie 项目稳定运行 | ✅ cjgui 4851 nodes / 7000 edges |
| JSON 字段稳定 | ✅ schema 版本化 |
| 无 bad data（dangling/duplicate/non-deterministic） | ✅ quality gates 全 pass |
| 无法确定的语义 → no-edge / low-confidence | ✅ 停止线内 |
| AI 可消费 summary/quality/graph/diagnostics | ✅ 三命令 + bridge |
| 固定 smoke targets 和验收命令 | ✅ verify-bridge + smoke.sh |
| stdout purity（Cangjie） | ✅ 不需要 sed |
| 跨仓库 Tool 导入 | ✅ 无 dangling |

**结论：✅ Alpha Production Trial Ready**
