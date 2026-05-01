# 命令权威

> **日期：** 2026-05-01
> **类型：** 命令权威
> **状态：** 镜像 GitNexus-RC AGENTS.md

---

## 目的

本文档镜像 GitNexus-RC 的命令权威规则，确保 GitNexus-RC 和 gitnexus-rust-core 之间的工具使用一致。

---

## 规则

### MCP Tools 优先

可用时，优先使用 GitNexus MCP tools：

- `gitnexus_detect_changes()`
- `gitnexus_impact()`
- `gitnexus_context()`
- `gitnexus_query()`

### MCP 不可用时用 Tool CLI

当 MCP tools 不可用时，使用 Tool CLI 绝对路径：

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js <command>
```

可用命令：
- `analyze [path]` — 分析仓库
- `status` — 检查状态
- `list` — 列出已索引 repos
- `detect-changes --repo <name> --scope <scope>` — 检测变更
- `impact <target>` — 运行 impact analysis
- `context <target>` — 获取 symbol context

### 禁止使用 npx gitnexus

**禁止使用 `npx gitnexus`** 做生产分析。`npx` 解析到旧版 npm 发布的 `gitnexus@1.6.1`，该版本：
- 缺少 `detect-changes`
- 缺少 Cangjie 支持
- 缺少多 repo 功能

---

## 此 Workspace 状态

gitnexus-rust-core **默认未索引**。它没有 `.gitnexus/` 目录，也未注册到 GitNexus registry。

查 GitNexus 关于此 workspace：
1. 此 workspace 在 GitNexus-RC 外部
2. 它不会自动被索引
3. 使用 GitNexus-RC Tool CLI 查询 GitNexus-RC repo

---

## 快速参考

| 任务 | 命令 |
|------|---------|
| 分析 GitNexus-RC | `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/GitNexus-RC` |
| 检查状态 | `node .../index.js status` |
| 列出 repos | `node .../index.js list` |
| 检测变更 | `node .../index.js detect-changes --repo gitnexus-rc --scope all` |
| Impact analysis | `node .../index.js impact <target>` |

---

## 来源

GitNexus-RC AGENTS.md § "GitNexus Command Authority" 和 GUARDRAILS.md § "GitNexus Command Authority"。
