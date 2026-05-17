# CodeLattice Snapshot Viewer

> **版本：** Workbench + G6 Graph Engine
> **状态：** 可用 — 本地 Runner / 静态 snapshot 双模式，AntV G6 vendored 图谱引擎
> **位置：** `webui/snapshot-viewer/`

## 是什么

CodeLattice Snapshot Viewer 是一个**只读本地 Web 页面**，用于可视化浏览 `CodeLatticeWebSnapshotV1` JSON snapshot。

它不是桌面应用，不执行项目代码。静态模式下，它只是一个 HTML 页面，加载 snapshot JSON 并渲染为人类可读的视图；Runner 模式下，它可以在本机生成 snapshot、管理快照库，并通过受控 API 发起 Live MCP job。

## 快速开始

### 1. 生成 Snapshot

```bash
bash scripts/webui-snapshot.sh \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --output /tmp/codelattice-snapshot.json
```

### 2. 打开 Viewer

**方式 A — File Input（推荐）：**

直接用浏览器打开 `webui/snapshot-viewer/index.html`，点击 **Load Snapshot** 按钮选择生成的 JSON 文件。

```bash
# macOS
open webui/snapshot-viewer/index.html

# Linux
xdg-open webui/snapshot-viewer/index.html

# 或者用任意 HTTP server
python3 -m http.server 8080 --directory webui/snapshot-viewer
```

**方式 B — URL Query Parameter（需要 HTTP server）：**

```bash
cd webui/snapshot-viewer && python3 -m http.server 8080
# 打开 http://localhost:8080/?snapshot=../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json
```

> **注意：** URL query 参数方式需要通过 HTTP server 访问，因为浏览器安全策略禁止 `file://` 协议的 fetch。

**方式 C — Drag & Drop：**

打开 index.html 后，将 `.json` 文件拖放到页面上即可加载。

## 支持的加载方式

| 方式 | 说明 | 限制 |
|------|------|------|
| File Input 按钮 | 选择本地 JSON 文件 | 无 |
| Drag & Drop | 拖放 JSON 文件到页面 | 无 |
| URL Query 参数 | `?snapshot=path/to/file.json` | 需要 HTTP server（CORS） |

## 视图说明

### Dashboard
- 项目统计卡片：源文件数、符号数、调用边数、包数
- Quality Gates 列表（pass/fail）
- Limitations & Cautions 列表
- GeneratedFrom 标志栏（staticAnalysis / runtimeVerified 等）

### Explore
- 符号搜索框（按 name / file 过滤）
- Kind 过滤下拉菜单
- 符号列表 + 详情面板（source snippet, edges, confidence samples）

> **注意：** 当前 CLI 生成的 snapshot 中 explore 数据默认为 `not_collected`（需要 MCP 工具）。viewer 会优雅地显示 "not collected" 提示。

### Impact
- On-demand empty state（impact 分析需要指定 target symbol）
- 如果 snapshot 中有 impact entries，会展示 risk level、metrics、direct callers

### Cleanup
- 四个 summary card: Dead Code Candidates / Reachability / External API Surface / Framework Entries
- 每个 card 显示 collected 统计或 not_collected 状态
- 全局 caution banner：NOT deletion-proof

### Release Review
- 三个 summary card: Breaking Change / Consistency / Config & Examples
- 显示 checklist 或 not_collected 状态
- 全局 caution banner：Static review only

### Graph

- 默认使用 `vendor/g6/g6.min.js` 中 vendored 的 AntV G6 5.1.1
- 支持拖拽、缩放、点击选中、双击下探
- 支持 G6 高级图谱 / SVG 兼容图谱切换
- 支持代码星云、模块星团、调用流向、蓝图架构、工程探索五种布局模板
- 支持海报模式，用于截图传播

## Caution 渲染

所有视图中都包含以下 caution 标注：

1. **全局顶部 Banner:** "Static Analysis Only" — 不是编译器验证、运行时测试或覆盖证明
2. **GeneratedFrom Bar:** 5 个标志位，false 的标红显示
3. **Cleanup View:** "NOT deletion-proof" 警告
4. **Release Review View:** "Static review only" 警告
5. **Heuristic 字段 badge:** preview/heuristic 标签

## 技术细节

| 属性 | 值 |
|------|-----|
| 前端框架 | 无（纯原生 HTML/CSS/JS） |
| 构建工具 | 无 |
| 包管理器 | 无 |
| 依赖项 | AntV G6 5.1.1 vendored browser bundle（MIT），无 npm/pnpm/yarn 工程 |
| 浏览器支持 | 现代浏览器（Chrome/Firefox/Safari/Edge） |
| JS 函数数量 | ~28 个核心函数 |
| CSS 变量 | 完整主题变量系统 |
| 响应式 | 移动端单列、桌面端多列 |

## 文件结构

```
webui/snapshot-viewer/
├── index.html      # 主页面 (~8 KB)
├── styles.css      # 样式表 (~6 KB)
├── app.js          # 应用逻辑 (~18 KB, ~540 行)
├── graph-g6.js     # AntV G6 adapter
├── vendor/g6/      # vendored G6 bundle + license
└── README.md       # 本文档
```

## Smoke 测试

```bash
bash scripts/webui-viewer-smoke.sh        # 标准（34 checks）
bash scripts/webui-viewer-smoke.sh --strict  # 严格模式
```

## 当前限制

1. **Explore 数据为空**: CLI snapshot 不包含符号列表；需 MCP 工具生成
2. **Impact 数据为空**: 需要指定 target symbol
3. **URL query 加载**: 受 CORS 限制，file:// 下不可用
4. **无暗色模式**: 第一版只有亮色主题
5. **无打印样式**: 未来迭代可增加
6. **无搜索索引**: 大型 snapshot 的客户端搜索可能较慢

## 与未来 WebUI 的关系

本 Viewer 是 WebUI 的 **MVP 第一版**。未来可能的方向：
- Tauri/Electron shell 包装
- MCP streaming 实时更新
- 暗色模式 / 主题切换
- 跨版本 snapshot diff
- 多项目 workspace 视图
