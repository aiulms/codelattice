# WebUI Snapshot Viewer — Preflight

> **日期：** 2026-05-16
> **状态：** Pre-flight（实现前规划）
> **关联：** [closure](./2026-05-16-webui-snapshot-viewer-closure.md)

---

## 一、目标

基于已完成的 `CodeLatticeWebSnapshotV1` snapshot contract，开发第一版**只读本地静态 WebUI viewer**。

这不是完整 WebUI，也不是桌面应用壳。它是一个纯 HTML/CSS/JS 静态页面，只读取 `webui-snapshot.sh` 生成的 JSON 文件并渲染为人类可浏览的视图。

## 二、Scope Lock

### 2.1 本轮 In Scope

| 能力 | 描述 | 优先级 |
|------|------|--------|
| 静态 HTML/CSS/JS 页面 | 单个 `index.html` + `styles.css` + `app.js`，无 npm | P0 |
| Snapshot 文件加载 | `<input type="file">` 选择本地 JSON，或 URL query 参数 | P0 |
| Dashboard 视图 | summary cards + quality gates + limitations + generatedFrom flags | P0 |
| Explore 视图 | 符号列表 + 搜索过滤 + 符号详情面板 | P0 |
| Impact 视图 | on-demand empty state 或已有数据显示 | P1 |
| Cleanup / Release 视图 | summary card 或 not_collected 状态展示 | P1 |
| 全局 caution banner | Static analysis only，所有 generatedFrom false 标注 | P0 |
| 错误状态处理 | JSON parse error / schemaVersion mismatch / section 缺失 | P0 |
| 响应式基础布局 | 移动端可基本浏览，桌面端信息密度高 | P1 |

### 2.2 本轮 Out of Scope

| 能力 | 原因 |
|------|------|
| MCP server 调用 | viewer 只读 snapshot JSON，不调用任何服务 |
| 后端服务 / API server | 纯静态页面，不需要后端 |
| Tauri / Electron / HarmonyOS app shell | 不做桌面应用壳 |
| React / Vue / Svelte / Angular | 不引入前端框架或 npm |
| 实时分析 / 在线查询 | snapshot 是预生成的静态数据 |
| 用户交互编辑 | 只读展示，不修改任何内容 |
| 跨版本 diff / compare | 未来功能 |
| 认证 / 权限 | 本地工具，不上传数据 |
| CI/CD 集成面板 | 不做实时监控 |

### 2.3 第一版 MVP 视图优先级

| 视图 | MVP 实现 | 说明 |
|------|----------|------|
| **Dashboard** | ✅ 完整实现 | summary cards, quality gates, limitations, generatedFrom |
| **Explore** | ✅ 完整实现 | symbol list, search/filter, detail panel |
| **Impact** | ⚡ lightweight panel | 有数据显示；无数据时显示 on-demand empty state |
| **Cleanup** | 📋 summary card | 展示 not_collected 状态或 summary 统计 |
| **Release Review** | 📋 summary card | 展示 not_collected 状态或 checklist 概要 |

## 三、硬边界

1. **不修改** GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool
2. **不修改** AI client 配置（Codex/opencode/Claude）
3. **不修改** 真实项目源码
4. **不直接调用** MCP server
5. **不执行** 目标项目代码 / 测试 / package manager
6. **不做** 桌面应用壳
7. **不做** 后端服务
8. **不引入** npm/pnpm/yarn
9. **所有 caution 必须可见** — static analysis only, runtimeVerified=false 等

## 四、技术方案

### 4.1 目录结构

```
webui/snapshot-viewer/
├── index.html      # 主 HTML 页面
├── styles.css      # 样式表（本地工具风格）
├── app.js          # 应用逻辑（模块化函数拆分）
└── README.md       # 使用文档
```

### 4.2 JS 函数拆分

| 函数名 | 职责 |
|--------|------|
| `loadSnapshot(source)` | 从文件/URL/query 加载 JSON |
| `validateSnapshot(data)` | 校验 schemaVersion、必要字段存在性 |
| `normalizeSnapshot(data)` | 补全缺失字段为默认值，统一 null/undefined 处理 |
| `renderDashboard(data)` | 渲染 Dashboard 视图 |
| `renderExplore(data)` | 渲染 Explore 视图（符号搜索+列表） |
| `renderImpact(data)` | 渲染 Impact 视图（on-demand panel） |
| `renderCleanupRelease(data)` | 渲染 Cleanup + Release 视图 |
| `renderError(message, detail)` | 渲染错误状态 |

### 4.3 数据加载方式

1. **URL query 参数**: `?snapshot=../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json`
2. **File input**: `<input type="file" accept=".json">`
3. **内置 demo**: 无文件时提示用户加载

### 4.4 视觉风格

- 本地开发工具审美（类似 GitHub CLI / rust-analyzer 输出风格）
- 信息密度高、可扫描
- 卡片圆角 ≤ 8px
- 不使用 hero banner / 装饰渐变球
- 移动端单列、桌面端多列

## 五、风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| fixture snapshot 中 explore/cleanup 为 not_collected | Explore 视图为空 | graceful fallback：显示 "not collected" 提示 + 加载说明 |
| 浏览器 CORS 限制 file:// 协议的 fetch | query 参数方式可能失败 | 优先用 File input；query 方式标注需要 HTTP server |
| 不同浏览器 CSS 兼容性 | 样式偏差 | 使用基础 CSS 属性，避免实验特性 |
| 大型 snapshot JSON 解析慢 | UI 卡顿 | MVP 不优化；未来考虑 streaming parse |

## 六、验证计划

1. `scripts/webui-viewer-smoke.sh` — 静态检查（HTML/CSS/JS 存在、函数完整性、fixture JSON 可解析）
2. 手动验证：打开 `index.html` 并加载 Rust/TS fixture snapshot
3. 所有 Stage 6 verification 通过后才 commit

## 七、交付物清单

- [ ] `docs/plans/2026-05-16-webui-snapshot-viewer-preflight.md` — 本文档
- [ ] `docs/plans/2026-05-16-webui-snapshot-viewer-closure.md` — closure review
- [ ] `webui/snapshot-viewer/index.html` — 主页面
- [ ] `webui/snapshot-viewer/styles.css` — 样式
- [ ] `webui/snapshot-viewer/app.js` — 应用逻辑
- [ ] `webui/snapshot-viewer/README.md` — 使用文档
- [ ] `scripts/webui-viewer-smoke.sh` — smoke 测试脚本
- [ ] README.md 更新（WebUI Viewer 小节）
- [ ] CHANGELOG.md 更新
- [ ] docs/webui/README.md 更新（Viewer MVP 链接）
- [ ] docs/webui/webui-mvp.md 更新（MVP 状态标注）
