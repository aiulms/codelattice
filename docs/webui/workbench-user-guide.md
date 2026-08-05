# CodeLattice Project Understanding Workbench — 用户指南（P0）

> 对应 execution card：`docs/plans/2026-08-05-project-understanding-p0.md`（v4）
> 范围：本地开发构建；商业发行、签名、公证、自动更新需独立 release gate。

## 1. 首次打开

```bash
# 1) 构建引擎二进制（含 Python/TypeScript/C/C++ 支持按需启用）
cargo build --bin codelattice --features tree-sitter-python,tree-sitter-c,tree-sitter-cpp,tree-sitter-arkts

# 2) 启动桌面 Workbench（macOS）
cd apps/desktop
npm install
npx tauri dev
```

打开后默认加载仓库 `fixtures/webui-snapshots/` 下最近的 snapshot。若用
`CODELATTICE_SNAP_DIR=/path/to/dir` 启动可指定其它 snapshot 目录。

## 2. 界面

```
┌────────────────────────────────────────────────────────────┐
│ 项目@版本 · 模型池 · 收起 Chat │                            │
├─────────┬──────────────┬──────────────┬────────────────────┤
│ 结构树   │ 图谱画布      │ 事实检查器   │ Chat（可折叠）      │
│         │ 节点/边/链路  │ 上下游/证据  │ 回答与证据 chips    │
├─────────┴──────────────┴──────────────┴────────────────────┤
│ 状态栏：snapshotId · 静态边界 · Desktop Worker             │
└────────────────────────────────────────────────────────────┘
```

- **未选择**：顶部显示确定性仪表盘（节点/边/CALLS 统计、截断标记、限制）。不触发模型。
- **选择节点**：Inspector 显示直接调用方/被调方、源码引用、覆盖率上下文（project scope）。
- **选择边**：Inspector 显示 source → target、关系类型、直接上下游、依赖扩散。
- **解释当前选择**：显式调用模型，回答按 claim 分级显示。
- **加入对话**：把当前选择 pin 到 Chat 上下文（聊 B 时选 A，Inspector 切到 A 而 Chat 仍 pinned B）。

## 3. 事实与解释的区分

| 显示 | 含义 |
|---|---|
| 事实（引擎字段） | snapshot 静态分析直接产出：节点、边、置信度、位置、覆盖边界 |
| 有依据的解释 | 模型断言且引用证据包中真实存在的 rel:/src:/limit: id |
| 假设 | 模型断言但无引用、引用不完整，或声明的标识不在证据词表 |
| 未知 | 模型明确无法回答 |

模型输出永不写回事实图谱或 snapshot；清除理解缓存不影响任何事实功能。

## 4. 模型池

1. 点头部“模型池”。
2. 添加 Ollama：`id=qwen-local, provider=ollama, baseUrl=http://127.0.0.1:11434/v1, model=qwen3:14b`。
3. 添加远程（OpenAI-compatible）：输入 baseUrl / model / API Key；Key 经
   `workbench_secret_set` 写入系统钥匙串，配置文件中只保存 `keychain:codelattice/<id>`。
4. 测试连接（不发送任何证据）、设默认、删除（删除模型时清理其理解缓存）。

**隐私边界**：远程模型默认只接收结构化图证据（节点/边/关系/统计），路径按
snapshot 的 redact 规则脱敏；`get_source_excerpt`（源码片段）与
`get_change_impact`（影响分析）必须显式确认/携带 what-if，P0 Chat 默认不发送
源码正文。本地 Ollama 可显示“项目证据不离开本机”。

## 5. Chat

- 项目级问题（“这个项目的主流程是什么？”）直接提问；Gateway 会迭代调用只读
  工具（search_nodes / get_node_context / get_call_chain / …）取证后再回答。
- 选择级追问：先在 Inspector 点“加入对话”，再围绕该节点/边提问。
- 每轮工具调用写入会话 trace；回答中的 claim 分级显示，evidence chip 点击会
  发出显式导航（更新图谱选择与 Inspector），而不是隐式覆盖对话上下文。
- 取证预算默认：L1 3×8KiB、L2 5×16KiB、L3 4×32KiB/50 行、会话 12 次调用、
  16K evidence tokens；超限停止并提示，不静默丢证据。
- 生成中可“停止生成”。

## 6. 缓存与清理

- 理解缓存键 = evidenceHash + 模型 + prompt 版本 + locale + 级别；snapshot 更新后
  只失效 evidence 变化的条目。
- 删除模型配置会清理关联缓存；整体删除理解缓存不影响事实层（可从 snapshot
  与模型重新生成）。

## 7. Desktop Analyzer 与 Agent MCP

- Desktop 分析任务：Tauri Core 启停的独立 worker（`nice -n 10` 低优先级、单任务、
  可取消）；snapshot 通过 temp + atomic rename 发布，失败不留半写文件。
- 外部 Agent MCP 使用自己的长生命周期进程与固定 snapshot 版本；桌面发布新版本
  不会强制刷新正在进行的 MCP 请求。
- Workbench 默认保留当前 snapshot 与上一版本引用（pinned 优先），额外版本按
  未 pinned 优先清理（只清理发布目录，不触碰 fixtures）。

## 8. 已知边界（P0）

- Windows/WebView2 未覆盖（无环境）。
- G6 布局坐标/动画为人工 smoke（自动测试只覆盖状态、数据流、DTO）。
- occurrenceKey 仅在事实层提供稳定 call-site designation 时产出；P0 为
  relation-level（跨版本定位是 best-effort，byte offset/排序序号不宣称稳定身份）。
- 商业签名、公证、自动更新、公开安装器不在 P0 范围。
