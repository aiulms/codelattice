# CodeLattice Workbench — Tauri 2 desktop shell (P0)

## 结构

```
apps/desktop/
├── package.json          # Vite + React + TS；vitest 测试
├── vite.config.ts        # dev server 固定 1420（Tauri 期望）
├── index.html
├── src/
│   ├── types.ts          # 共享 DTO（selection / evidence / answer / transport）
│   ├── state/            # GraphSelectionStore + ConversationStore（双状态）
│   ├── graph/            # G6 adapter（edge 事件 + relationKey 身份）+ highlight 纯函数
│   ├── data/             # snapshot-reader（静态事实）+ evidence-client（按需证据）
│   ├── transport/        # DesktopTransport：Tauri / HTTP / Fake 三实现
│   ├── panels/           # dashboard / inspector / chat / graph-pane / model-pool
│   ├── data/stream-consumer.ts  # 流式事件 → 消息状态纯归约（B1/B2 共用）
│   └── smoke/            # WKWebView selftest（G1 gate）
└── src-tauri/            # 薄 Tauri Core：commands、capability、analyzer supervisor、
                          #   models.json 配置、Keychain SecretStore、snapshot store
```

## 开发

```bash
npm install
npm test            # vitest（双 store / highlight / adapter / reader / stream-consumer）
npm run build       # tsc + vite build
npx tauri dev       # 本地开发（macOS WKWebView）
```

依赖：仓库 `target/debug/codelattice`（`cargo build --bin codelattice`）。
Rust 侧：

```bash
cargo test -p understanding-gateway --features http   # Gateway（provider/validator/cache/…）
cd apps/desktop/src-tauri && cargo test                # Tauri 壳（snapshot store 等）
```

## 功能（P0 验收对应）

- **事实默认可用**：打开项目即显示确定性仪表盘；选择节点/边后 Inspector 显示
  直接 callers/callees、coverageContext、证据与静态边界，全部来自真实 snapshot，
  不依赖模型（P1/P10）。
- **解释显式触发**：Inspector 的“解释当前选择”才调用模型（P2）。
- **模型池**：头部“模型池”按钮管理 Ollama / OpenAI-compatible 模型；Key 写入
  系统钥匙串，前端只持有 secretRef（G4）。
- **Chat**：项目级提问与选择级追问共用只读 Tool Dispatcher；模型迭代调用
  search_nodes / get_node_context / get_call_chain 等工具取证；会话 trace 可展开
  （P0-B2）。claim 按“有依据解释 / 假设 / 未知”分级，evidence chip 点击发出
  显式导航请求（P6/P5）。
- **取消**：Chat 生成中可“停止生成”。

## 模型配置（~/.codelattice/models.json）

只保存非敏感配置与 secret reference（§7.1）。示例：

```json
{
  "default": "qwen-local",
  "models": [
    {"id": "qwen-local", "provider": "ollama",
     "baseUrl": "http://127.0.0.1:11434/v1", "model": "qwen3:14b"}
  ]
}
```

远程模型在 UI 添加时输入 API Key，经 `workbench_secret_set` 写入 macOS 钥匙串
（`keychain:codelattice/<id>`），配置文件中只有 ref。远程模型默认只发送结构化
图证据，路径按 snapshot 的 redact 规则脱敏；源码片段（get_source_excerpt）与
影响分析（get_change_impact）必须显式确认/携带 what-if（§7.2）。

## Selftest（G1 gate：真实 WKWebView 的 G6 smoke）

```bash
CODELATTICE_SELFTEST=1 CODELATTICE_SMOKE_OUT=$PWD/target/selftest-report.json npx tauri dev
# 等待 target/selftest-report.json 出现，然后 Ctrl-C 退出
```

selftest 在真实 WebView 内执行：真实 snapshot → G6 mount → node click →
edge click → resize → 重复 mount/unmount ×3 → 结果写盘。

## 基线脚本

```bash
node scripts/webui-f1-baseline.mjs        # F1 内存基线（空壳 / snapshot / analyze / 20 轮）
python3 scripts/webui-p0c-isolation.py     # P0-C：Agent MCP + Desktop Analyzer 并发隔离
python3 scripts/query-store-spike.py ...   # G3：查询数据源三候选基准
```

## Selftest（G1 gate：真实 WKWebView 的 G6 smoke）

```bash
CODELATTICE_SELFTEST=1 CODELATTICE_SMOKE_OUT=$PWD/target/selftest-report.json npx tauri dev
# 等待 target/selftest-report.json 出现，然后 Ctrl-C 退出
```

selftest 在真实 WebView 内执行：真实 snapshot → G6 mount → node click →
edge click → resize → 重复 mount/unmount ×3 → 结果写盘。

## 边界（P0 stop-line 遵守）

- 不把业务堆入 `src-tauri/main.rs`：Gateway 在 `crates/understanding-gateway`。
- 不写回旧 `webui/snapshot-viewer/app.js` / `runner.js` / `webui-runner.py`。
- Desktop Analyzer 只由 Tauri Core 启停，不复用 Agent MCP job registry；
  snapshot 通过 temp + atomic rename 发布，只清理发布目录，不触碰 fixtures。
- Key 不进入前端状态 / localStorage / 日志（SecretStore 在 Rust 侧）。
- relationKey = sha256(source + kind + target)（§6.1）；occurrenceKey 在事实层
  无 call-site designation 时不产出（relation-level，G2 冻结）。
