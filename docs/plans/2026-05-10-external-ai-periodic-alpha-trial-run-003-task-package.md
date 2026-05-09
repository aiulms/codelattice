# External AI Independent Periodic Alpha Trial Run #003 — Task Package

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** 完整任务包（自包含，不依赖聊天上下文）
> **执行者：** 外部 AI（非本轮 session 的 AI）
> **关联：** [Alpha Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Evidence Board](2026-05-10-beta-readiness-evidence-board.md)

---

## 一、背景

CodeLattice 是一个本地代码图谱分析核心，面向 Rust 和 Cangjie 项目。当前处于 Alpha Production Trial 阶段。

- **已完成 trial：** Run #001（2026-05-09）和 Run #002（2026-05-10）均 PASS
- **Run #003 的目的：** 独立复现 runbook 流程，验证非原始开发者 AI 也能按文档操作成功
- **不是功能开发。** 不要修改任何 runtime 代码、配置、依赖。

---

## 二、工作区

| 路径 | 用途 | 规则 |
|------|------|------|
| `/Users/jiangxuanyang/Desktop/codelattice` | CodeLattice 项目（分析目标 + 文档产出地） | 只改 `docs/plans/`，不改代码 |
| `/Users/jiangxuanyang/Desktop/GitNexus-RC` | 下游消费方（不修改） | 只读验证 |
| `/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool` | Tool CLI（不修改） | 只读使用 |
| `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index` | Cangjie index checkout（Cangjie 分析目标） | 不修改业务源码 |
| `/Users/jiangxuanyang/Desktop/cangjie` | Cangjie live repo | **禁止修改** |
| `/Users/jiangxuanyang/Desktop/open-nwe` | open-nwe live repo | **禁止修改** |

---

## 三、必须读取的文档

执行前先读取以下文档：

1. `/Users/jiangxuanyang/Desktop/codelattice/AGENTS.md` — 项目治理和 stop-line
2. `/Users/jiangxuanyang/Desktop/codelattice/docs/plans/2026-05-09-alpha-production-trial-runbook.md` — 操作手册
3. `/Users/jiangxuanyang/Desktop/codelattice/docs/plans/2026-05-09-alpha-trial-maintenance-and-failure-playbook.md` — 失败分类和第一响应
4. `/Users/jiangxuanyang/Desktop/codelattice/docs/plans/2026-05-09-periodic-alpha-trial-log-template.md` — trial log 模板
5. `/Users/jiangxuanyang/Desktop/codelattice/docs/plans/2026-05-10-beta-readiness-evidence-board.md` — 证据看板
6. `/Users/jiangxuanyang/Desktop/codelattice/docs/plans/2026-05-10-periodic-alpha-trial-run-003-placeholder.md` — Run #003 占位文件

---

## 四、标准命令

以下命令使用绝对路径，不要用 `npx gitnexus`。

### 4.1 Rust Real-Project Bridge JSON Generation

```bash
cd /Users/jiangxuanyang/Desktop/codelattice
RUST_BRIDGE_JSON="$(mktemp /tmp/codelattice-rust-trial-XXXXXX.json)"
echo "RUST_BRIDGE_JSON=$RUST_BRIDGE_JSON"

cargo run -- analyze \
  --root /Users/jiangxuanyang/Desktop/codelattice \
  --language rust \
  --format gitnexus-rc \
  --strict \
  > "$RUST_BRIDGE_JSON" 2>/tmp/codelattice-rust-stderr.txt
echo "EXIT_CODE=$?"
```

### 4.2 JSON Purity Check

```bash
python3 -c "
import json, os
f = '$RUST_BRIDGE_JSON'
size = os.path.getsize(f)
with open(f, 'rb') as fh: first = fh.read(1)
d = json.load(open(f))
print(f'Size: {size} bytes')
print(f'First byte: {first} (hex: {first.hex()})')
print(f'schemaVersion: {d[\"schemaVersion\"]}')
print(f'packages: {len(d[\"packages\"])}')
print(f'sourceFiles: {len(d[\"sourceFiles\"])}')
print(f'symbols: {len(d[\"symbols\"])}')
print(f'diagnostics: {len(d[\"diagnostics\"])}')
e = d['edges']
total = sum(len(v) for v in e.values())
print(f'edges total: {total}')
# Dangling check
from collections import Counter
node_ids = [d['repository']['id']]
for p in d['packages']: node_ids.append(p['id'])
for sf in d['sourceFiles']: node_ids.append(sf['id'])
for s in d['symbols']: node_ids.append(s['id'])
node_set = set(node_ids)
dangling = sum(1 for grp in e.values() for edge in grp if edge['sourceId'] not in node_set or edge['targetId'] not in node_set)
dup_nodes = {k:v for k,v in Counter(node_ids).items() if v > 1}
edge_triples = [(edge['sourceId'], edge['targetId'], edge['kind']) for grp in e.values() for edge in grp]
dup_edges = {k:v for k,v in Counter(edge_triples).items() if v > 1}
print(f'dangling: {dangling}')
print(f'dup nodes: {len(dup_nodes)}')
print(f'dup edges: {len(dup_edges)}')
print(f'stats consistent: {d[\"stats\"][\"nodeCount\"] == len(node_ids) and d[\"stats\"][\"edgeCount\"] == total}')
"
```

### 4.3 Tool Import

```bash
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze \
  --force \
  --experimental-rust-core-bridge-graph "$RUST_BRIDGE_JSON" \
  --skip-agents-md \
  --name codelattice
```

### 4.4 Tool Status / detect-changes

```bash
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js status
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
```

### 4.5 Cangjie Real-Project Bridge JSON Generation

```bash
cd /Users/jiangxuanyang/Desktop/codelattice
CANGJIE_BRIDGE_JSON="$(mktemp /tmp/codelattice-cjgui-trial-XXXXXX.json)"
echo "CANGJIE_BRIDGE_JSON=$CANGJIE_BRIDGE_JSON"

cargo run --features tree-sitter-cangjie -- analyze \
  --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
  --language cangjie \
  --format gitnexus-rc \
  --strict \
  > "$CANGJIE_BRIDGE_JSON" 2>/tmp/codelattice-cjgui-stderr.txt
echo "EXIT_CODE=$?"
```

### 4.6 Cangjie JSON Purity Check

同 4.2 的 python3 脚本，替换文件路径为 `$CANGJIE_BRIDGE_JSON`。

### 4.7 Cangjie Tool Import

```bash
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze \
  --force \
  --experimental-rust-core-bridge-graph "$CANGJIE_BRIDGE_JSON" \
  --skip-agents-md
```

### 4.8 Cangjie Tool Verification

```bash
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js status
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo cjgui --scope all
/opt/homebrew/bin/node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js context main --repo cjgui
```

---

## 五、成功标准

全部以下条件必须满足：

- [ ] Rust bridge JSON：stdout 从第 1 字节是合法 JSON
- [ ] Rust bridge JSON：0 dangling endpoints
- [ ] Rust bridge JSON：0 duplicate node IDs、0 duplicate edge triples
- [ ] Rust bridge JSON：stats 字段与实际值一致
- [ ] Rust Tool import：`indexed successfully`
- [ ] Cangjie bridge JSON：stdout 从第 1 字节是合法 JSON
- [ ] Cangjie bridge JSON：0 dangling、0 duplicate
- [ ] Cangjie Tool import：`indexed successfully`
- [ ] `detect-changes --repo codelattice`：正常运行
- [ ] `detect-changes --repo cjgui`：No changes detected
- [ ] Tool registry 包含 `codelattice`
- [ ] cangjie-GitNexus-Index clean（无业务源码修改）
- [ ] /Users/jiangxuanyang/Desktop/cangjie 未被修改
- [ ] /Users/jiangxuanyang/Desktop/open-nwe 未被修改

---

## 六、失败分类和第一响应

参照 [Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)：

| 失败类型 | 症状 | 第一响应 |
|----------|------|---------|
| stdout purity regression | JSON.parse 失败 | 不用 sed 修复；记录 stderr；报告 |
| endpoint integrity regression | dangling > 0 | 不修改 validator；报告 |
| Tool ingestion failure | Tool import 报错 | 检查 bridge JSON 大小和 schemaVersion；报告 |
| rename/index identity regression | registry 不含 codelattice | 重新 `analyze --name codelattice`；报告 |
| smoke script cleanup regression | `.claude/` 残留 | `rm -rf`；报告 |

---

## 七、必须填写的 Trial Log 字段

完成 trial 后，替换 [Run #003 Placeholder](2026-05-10-periodic-alpha-trial-run-003-placeholder.md) 的内容，填写以下字段（全部用真实数据）：

- Date
- Executor（你的 AI session 标识）
- CodeLattice HEAD commit
- Rust trial：command, JSON size, stats, quality checks, Tool result
- Cangjie trial：command, JSON size, stats, quality checks, Tool result
- detect-changes / status 结果
- Cleanup performed
- Failure classification
- Final status

---

## 八、最终汇报模板

完成后用中文汇报，至少包含：

1. 四个 repo 状态
2. Rust trial 结果（JSON size, stats, Tool output）
3. Cangjie trial 结果
4. stdout purity 结果
5. dangling / duplicate / stats consistency
6. detect-changes 结果
7. 是否修改了 runtime / Tool / live repo
8. 是否切了默认工具
9. 当前 Alpha/Beta 判断

---

## 九、禁止事项

- ❌ 不要修改 CodeLattice runtime 代码
- ❌ 不要修改 GitNexus-RC / Tool
- ❌ 不要修改 cangjie / open-nwe live repo
- ❌ 不要切默认工具
- ❌ 不要使用 `npx gitnexus`
- ❌ 不要伪造数据
- ❌ 不要提交 `.claude/`、`.sisyphus/`、临时 JSON、编译产物
- ❌ 不要把 generatedAt 值用于 deterministic strict compare
- ❌ 不要用 sed 作为 stdout purity 正式修复方案

---

## 十、完成后操作

1. 删除临时 bridge JSON 和 stderr 文件
2. 确保 codelattice 工作区 clean（仅 docs 改动）
3. 确保无 agent 目录残留
4. 提交 trial log 和 evidence board 更新：
   ```
   git add docs/plans/2026-05-10-periodic-alpha-trial-run-003-placeholder.md docs/plans/2026-05-10-beta-readiness-evidence-board.md
   git commit -m "docs(trial): record periodic alpha trial run 003"
   git push gitcode master
   ```
5. 更新 [Evidence Board](2026-05-10-beta-readiness-evidence-board.md) 的 Run #003 行
