# Alpha Trial Maintenance and Failure Playbook

> **日期：** 2026-05-09
> **版本：** 1.0.0
> **状态：** Active
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)

---

## 一、日常验证

### 1.1 每次 Rust-core 变更后

```bash
# 最小验证集
cargo fmt --check
cargo test --test bridge_roundtrip
cargo test --features tree-sitter-cangjie --test bridge_roundtrip
scripts/verify-bridge.sh --rust-only
```

预期耗时：< 2 分钟。

### 1.2 涉及 Cangjie 变更时

```bash
cargo test --features tree-sitter-cangjie --test bridge_roundtrip
scripts/verify-bridge.sh
```

### 1.3 涉及 productization 命令变更时

```bash
cargo test --test productization_commands
cargo test --features tree-sitter-cangjie --test productization_commands
```

---

## 二、周期 Smoke 推荐

### 2.1 Weekly / Release-Candidate

```bash
# 完整内部验证
cargo fmt --check
git diff --check
cargo test --features tree-sitter-cangjie
scripts/verify-bridge.sh
scripts/smoke.sh --quick

# 端到端 Tool 导入验证
scripts/alpha-trial-smoke.sh
```

### 2.2 真实项目 Trial（只读）

**Rust 自身项目：**
```bash
cargo run -- analyze \
  --root /Users/jiangxuanyang/Desktop/gitnexus-rust-core \
  --language rust --format gitnexus-rc --strict \
  > /tmp/rust-core-bridge.json

python3 -c "import json; json.load(open('/tmp/rust-core-bridge.json'))"
# 验证后清理
rm -f /tmp/rust-core-bridge.json
```

**Cangjie index checkout：**
```bash
cargo run --features tree-sitter-cangjie -- analyze \
  --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  --language cangjie --format gitnexus-rc --strict \
  > /tmp/cjgui-bridge.json

python3 -c "import json; json.load(open('/tmp/cjgui-bridge.json'))"
# 验证后清理
rm -f /tmp/cjgui-bridge.json
```

---

## 三、失败分类与第一响应

### 3.1 stdout purity failure

**症状：** `python3 -c "import json; json.load(open(...))")` 失败，stdout 第 1 字节不是 `{`

**复查命令：**
```bash
# 检查是否 stderr/stdout 混流
cargo run -- analyze ... > /tmp/test.json 2>/tmp/test-stderr.txt
head -c 20 /tmp/test.json | xxd | head -2
```

**第一响应：**
1. 确认是否 stderr/stdout 正确分离（不要用 `2>&1`）
2. 检查 `scanner.c` 是否有新的 `fprintf(stdout, ...)` 调用
3. 检查 `main.rs` 是否有新的 `println!` 在 bridge 输出路径中
4. 如果是编译 warning 混入 stdout → 检查 C 代码和 build profile

**不应做的修复：**
- ❌ 不用 sed 修 JSON 作为正式方案
- ❌ 不在 bridge 输出路径加 println!

### 3.2 dangling endpoint failure

**症状：** Tool adapter 报 `Dangling source: "..."` 或 `Dangling target: "..."`

**复查命令：**
```bash
# 生成 bridge JSON 后用 python 检查
python3 -c "
import json
d = json.load(open('/tmp/test-bridge.json'))
node_ids = set()
node_ids.add(d['repository']['id'])
for p in d['packages']: node_ids.add(p['id'])
for sf in d['sourceFiles']: node_ids.add(sf['id'])
for s in d['symbols']: node_ids.add(s['id'])
dangling = []
for group, edges in d['edges'].items():
    for e in edges:
        if e['sourceId'] not in node_ids: dangling.append(('src', group, e['kind'], e['sourceId']))
        if e['targetId'] not in node_ids: dangling.append(('tgt', group, e['kind'], e['targetId']))
if dangling:
    print(f'DANGLING: {len(dangling)}')
    for d in dangling[:10]: print(f'  {d}')
else:
    print('OK: 0 dangling')
"
```

**第一响应：**
1. 确认是否有新的 node label 类型未被 `partition_rust_nodes()` 处理
2. 确认 workspace / diagnostic 节点映射是否仍然生效
3. 检查新增 edge type 是否引用了未映射的 node

**不应做的修复：**
- ❌ 不放宽 Tool adapter validator
- ❌ 不跳过 dangling 检测

### 3.3 duplicate node ID / edge triple

**症状：** quality gate `duplicate_nodes` 或 `duplicate_edges` 失败

**复查命令：**
```bash
cargo run -- quality --root <path> --language rust
```

**第一响应：**
1. 检查是否有同一 symbol 被两个不同 extractor 提取
2. 检查 dedupe 逻辑是否被新变更破坏
3. 检查 fixture 是否有预期外重复

### 3.4 deterministic drift

**症状：** `verify-bridge.sh` 确定性测试失败

**复查命令：**
```bash
# 手动跑两次并 diff（排除 generatedAt）
cargo run -- analyze --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc > /tmp/a.json
cargo run -- analyze --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc > /tmp/b.json
python3 -c "
import json
a = json.load(open('/tmp/a.json'))
b = json.load(open('/tmp/b.json'))
del a['generatedAt'], b['generatedAt']
if a == b: print('DETERMINISTIC: OK')
else: print('DETERMINISTIC: FAILED')
"
```

**第一响应：**
1. 确认 `generatedAt` 已从比较中排除
2. 检查是否有 HashMap 遍历顺序不确定的输出
3. 检查是否有依赖系统时间的字段

**不应做的修复：**
- ❌ 不把 `generatedAt` 纳入 strict deterministic compare
- ❌ 不用固定时间戳 hack

### 3.5 Tool adapter validation failure

**症状：** Tool 报 `Unknown edge kind`、`Missing top-level field`、`legacy field "source" without "sourceId"`

**复查命令：**
```bash
# 检查 bridge JSON 结构
python3 -c "
import json
d = json.load(open('/tmp/test-bridge.json'))
required = ['schemaVersion','generatedAt','language','root','repository','packages','sourceFiles','symbols','edges','diagnostics','stats']
for f in required:
    if f not in d: print(f'MISSING: {f}')
print('Structure check done')
"
```

**第一响应：**
1. 检查 `bridge_format.rs` 是否有字段 rename 或删除
2. 检查新增 edge kind 是否在 Tool adapter 白名单中
3. 检查端点字段名是否正确归一化为 `sourceId`/`targetId`

### 3.6 index header artifact dirty

**症状：** cangjie-GitNexus-Index 的 `AGENTS.md` / `CLAUDE.md` 被 Tool 修改

**第一响应：**
```bash
cd /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index
git checkout -- AGENTS.md CLAUDE.md
```

确认未修改业务源码。Header artifact 是 Tool 运行副作用，不应提交。

### 3.7 command authority violation

**症状：** 使用了 `npx gitnexus` 或切了默认工具

**第一响应：**
1. 立即停止当前操作
2. 确认未产生持久影响
3. 使用 Tool CLI 绝对路径重试

---

## 四、明确不应做的修复

| 不应做 | 原因 |
|--------|------|
| 放宽 Tool adapter validator | Validator 是 fail-closed 设计，问题在 bridge 生成端 |
| 把 `generatedAt` 纳入 deterministic strict compare | 值不稳定，每次生成不同 |
| 用 sed 修 JSON 作为正式方案 | 掩盖 stdout purity 问题 |
| 写 live repo | 只读操作，任何修改需要授权 |
| 切默认工具 | Rust-core 是 explicit opt-in |
| 在 bridge 输出路径加 println! | 污染 stdout |

---

## 五、试用期记录格式

每次 trial 操作应记录以下信息：

```
日期：2026-05-09
Target：gitnexus-rust-core（Rust 自身）
Language：Rust
Command：cargo run -- analyze --root ... --format gitnexus-rc --strict
Result：SUCCESS
Nodes/Edges/Symbols：1700 / 2634 / 1635
Failures：0
Cleanup：已删除临时 bridge JSON
```

```
日期：2026-05-09
Target：cangjie-GitNexus-Index/runtime/cjgui
Language：Cangjie
Command：cargo run --features tree-sitter-cangjie -- analyze --root ... --format gitnexus-rc --strict
Result：SUCCESS
Nodes/Edges/Symbols：~880 / ~3252 / 887
Failures：0
Cleanup：已还原 AGENTS.md/CLAUDE.md header artifact
```

---

## 六、退出 Alpha / 升级 Beta 候选条件

以下条件全部持续满足时可考虑从 Alpha 升级为 Beta：

1. **多次真实项目 smoke 稳定**：Rust + Cangjie 各 ≥ 3 次成功真实项目 trial
2. **无 stdout purity 回归**：连续 ≥ 2 周无 stdout 污染问题
3. **无 dangling endpoint 回归**：连续 ≥ 2 周 0 dangling
4. **Tool ingestion 稳定**：无 adapter validation failure
5. **Docs/scripts 维护**：runbook / playbook / smoke 脚本保持最新
6. **确定性输出**：排除 generatedAt 后输出完全 deterministic
7. **无命令权限违反**：无 npx gitnexus、无 live repo 写入、无默认工具切换

升级 Beta 需要：
- 明确的 beta scope 定义
- 更多真实项目覆盖（≥ 3 个 Rust + ≥ 2 个 Cangjie）
- 至少一个外部执行 AI 成功按 runbook 操作

---

## 七、相关文档

- [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)
- [Public Identity and Legacy Command Cleanup Plan](2026-05-09-public-identity-and-legacy-command-cleanup-plan.md)
- [Bridge Endpoint + Stdout Purity Closure Review](2026-05-09-alpha-trial-bridge-endpoint-stdout-purity-closure-review.md)
- [Production Trial Acceptance Checklist](2026-05-09-production-trial-acceptance-checklist.md)
