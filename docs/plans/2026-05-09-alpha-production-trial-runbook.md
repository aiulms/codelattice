# Alpha Production Trial Runbook

> **日期：** 2026-05-09
> **版本：** 1.0.1
> **状态：** Active
> **适用范围：** Rust workspace/package 项目 + Cangjie 项目 + Tool bridge ingestion（experimental flag）

---

## 一、Readiness 结论

**Alpha Production Trial Ready — explicit opt-in.**

- Rust-core 是独立 Rust-native 分析核心，不替代 GitNexus-RC TypeScript adapter。
- 仅支持 Rust + Cangjie 两门语言。
- 通过 `--format gitnexus-rc` bridge JSON + `--experimental-rust-core-bridge-graph` Tool 导入进行 alpha trial。
- 不切换默认生产引擎、WebUI 默认引擎、MCP 默认引擎。

---

## 二、适用范围

| 场景 | 支持状态 |
|------|----------|
| Rust workspace / package 项目 bridge JSON 生成 | ✅ |
| Cangjie 项目 bridge JSON 生成 | ✅ |
| Tool bridge ingestion（experimental flag） | ✅ |
| Quality gates / --strict | ✅ |
| Deterministic output（排除 generatedAt） | ✅ |
| Bridge endpoint integrity（0 dangling） | ✅ |

## 三、不适用范围

| 场景 | 说明 |
|------|------|
| 默认生产引擎切换 | Rust-core 不替代 TS adapter |
| WebUI 默认切换 | 不涉及 |
| MCP 默认切换 | 不涉及 |
| open-nwe / cangjie live repo 直接写入 | 禁止修改业务源码 |
| 多语言扩张 | 第一阶段仅 Rust + Cangjie |
| 完整 type inference / trait solving | Rust stop-line |
| 完整 Cangjie method dispatch / interface solving | Cangjie stop-line |
| 宏展开 / proc-macro / build.rs | Rust stop-line |
| Full cfg evaluator | Rust stop-line |
| External crate API symbol resolution | Rust stop-line |

---

## 四、标准命令

### 4.1 Rust Bridge JSON 生成

```bash
# 在 Rust-core 工作区执行
cargo run -- analyze \
  --root <目标项目路径> \
  --language rust \
  --format gitnexus-rc \
  --strict \
  > /tmp/<项目名>-bridge.json
```

**验证 stdout 纯净：**
```bash
# stdout 必须从第 1 字节就是合法 JSON
python3 -c "import json,sys; json.load(open('/tmp/<项目名>-bridge.json'))" && echo "JSON OK"
```

### 4.2 Cangjie Bridge JSON 生成

```bash
# 需要 tree-sitter-cangjie feature
cargo run --features tree-sitter-cangjie -- analyze \
  --root <目标项目路径> \
  --language cangjie \
  --format gitnexus-rc \
  --strict \
  > /tmp/<项目名>-bridge.json
```

### 4.3 Tool CLI Bridge 导入

```bash
# 使用 Tool CLI 绝对路径（不要用 npx gitnexus）
TOOL="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"
node "$TOOL" analyze \
  --force \
  --experimental-rust-core-bridge-graph /tmp/<项目名>-bridge.json \
  --name <项目别名>

# 如果 name 冲突：
node "$TOOL" analyze \
  --force \
  --allow-duplicate-name \
  --experimental-rust-core-bridge-graph /tmp/<项目名>-bridge.json \
  --name <项目别名>-trial
```

### 4.4 Status / Detect-Changes / Context 复查

```bash
TOOL="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"

# 索引状态
node "$TOOL" status

# 变更检测
node "$TOOL" detect-changes --repo <项目别名> --scope all
```

### 4.5 回归验证

```bash
# Rust-core 工作区内执行
cargo fmt --check
cargo test --features tree-sitter-cangjie
scripts/verify-bridge.sh
scripts/smoke.sh --quick

# 跨 repo 端到端验证
scripts/alpha-trial-smoke.sh
```

---

## 五、成功判定

| 检查项 | 判定标准 |
|--------|----------|
| stdout JSON 纯净 | `JSON.parse` 从 byte 0 成功 |
| Tool adapter validation 通过 | 无 ERROR 级别报告 |
| 0 dangling source/target | 所有 edge 端点在 node-like ID 集合中 |
| 0 duplicate node IDs / edge triples | quality gate `duplicate_nodes` + `duplicate_edges` pass |
| deterministic output | `verify-bridge.sh` 确定性测试 pass（排除 `generatedAt`） |
| quality gates pass | `--strict` exit 0 |
| stats 一致性 | `stats.symbolCount` / `sourceFileCount` / `packageCount` 与实际数组长度一致 |

---

## 六、失败判定

| 失败模式 | 说明 |
|----------|------|
| JSON stdout 前缀污染 | stdout 第 1 字节不是 `{` |
| bridge validation failure | Tool 报 Dangling source/target 或 unknown edge kind |
| dangling endpoints | edge sourceId/targetId 不在 node 集合中 |
| Tool 写入业务源码 | Tool 修改了目标项目的源码文件（非 header artifact） |
| generatedAt 用于 strict deterministic compare | 值不稳定，不能参与严格相等比较 |
| quality gates fail | `--strict` exit non-zero |
| non-deterministic output | 排除 `generatedAt` 后两次输出不一致 |

---

## 七、回滚 / 清理

### 7.1 删除临时 bridge JSON

```bash
rm -f /tmp/rust-core-bridge.json /tmp/cjgui-bridge.json /tmp/*-bridge.json
```

### 7.2 还原 index checkout 的 header artifact

Tool 运行后可能修改 `AGENTS.md` / `CLAUDE.md`（header artifact）：

```bash
cd /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index
git checkout -- AGENTS.md CLAUDE.md
```

### 7.3 不提交 Tool 运行副作用

- 不提交 `.claude/`、`CLAUDE.md`、AGENTS.md 中的 gitnexus block
- 不提交 `.gitnexus/` 目录

### 7.4 不清理用户业务改动

- 如果 Tool 检测到业务代码变更（来自 alpha trial 之外的原因），不做任何还原
- 仅还原 Tool 自身产生的 header artifact

---

## 八、风险边界

### Rust Stop-line

- 无 rust-analyzer
- 无宏展开（`foo!()` 不展开）
- 无 full cfg evaluator（cfg-gated `mod` 标记 `unknown`）
- 无 trait solving / type inference / generic inference
- 不执行 `cargo metadata`、proc-macro、build.rs
- External crate 支持仅限 std/core/alloc direct path 和 imported stdlib/prelude type

### Cangjie Stop-line

- 不切默认生产引擎
- 不做 live repo 写入
- 不做完整 method dispatch / interface solving
- 不执行 cjc 编译（diagnostics 为 opt-in subprocess，非默认）

---

## 九、执行 AI 最小 Checklist

每次执行 alpha trial 操作时，执行 AI 应确认：

- [ ] **工作区状态**：Rust-core master clean，无 agent 私有文件
- [ ] **Bridge JSON 生成**：stdout JSON.parse from byte 0 成功
- [ ] **Tool 导入**：`--experimental-rust-core-bridge-graph` 无 validation failure
- [ ] **Endpoint integrity**：0 dangling source/target
- [ ] **Quality gates**：`--strict` exit 0
- [ ] **Deterministic**：`verify-bridge.sh` pass
- [ ] **Header artifact 还原**：index checkout 的 AGENTS.md / CLAUDE.md 已还原
- [ ] **无业务源码修改**：未触碰 open-nwe / cangjie live repo
- [ ] **无 agent 私有文件提交**：.claude / CLAUDE.md / .sisyphus 未进入 commit
- [ ] **generatedAt 未用于 strict compare**：仅检查字段存在和格式，不比较值
- [ ] **使用 Tool CLI 绝对路径**：未使用 `npx gitnexus`
- [ ] **未切默认工具**：Rust-core 仍为 explicit opt-in

---

## 十、相关文档

- [Production Trial Readiness Preflight](2026-05-09-production-trial-readiness-and-roadmap-pivot-preflight.md)
- [Bridge Endpoint + Stdout Purity Closure Review](2026-05-09-alpha-trial-bridge-endpoint-stdout-purity-closure-review.md)
- [Production Trial Acceptance Checklist](2026-05-09-production-trial-acceptance-checklist.md)
- [Public Identity and Legacy Command Cleanup Plan](2026-05-09-public-identity-and-legacy-command-cleanup-plan.md)
- [Rust-core AGENTS.md](../../AGENTS.md)
