# CodeLattice WebUI Contract Tests (P0-F0 characterization)

P0-F0 契约字符化测试套件：冻结旧 `snapshot-viewer` / Web Runner 的可观察契约，
作为新桌面 Workbench（`apps/desktop`）实现前的行为基线。

## 运行

```bash
npm install          # 仅安装 playwright-core（browser smoke 需要）
npm test             # 全部（node + browser）
npm run test:node    # 仅 node 测试（snapshot / graph-events / runner-dto）
npm run test:browser # 仅浏览器 smoke（需要 macOS Google Chrome）
```

## 覆盖范围

| 文件 | 契约 |
|---|---|
| `tests/snapshot-contract.test.mjs` | `webui.snapshot.v1` 结构、截断标记、关键计数、再生成确定性、无 dangling edge |
| `tests/graph-events.test.mjs` | `graph-g6.js` selection 高亮语义、node 事件 payload、edge 事件缺口 |
| `tests/runner-dto.test.mjs` | runner REST ok/err DTO、snapshot list/get、错误码、未知路由行为 |
| `tests/viewer-smoke.test.mjs` | 真实 Chrome 打开旧 Viewer、注入 snapshot、视图切换、0 uncaught error |

## 明确不自动化的范围（P0-F0 边界）

- G6 布局坐标、动画时序、Canvas 像素级视觉
- CSS 像素等价、字体渲染、旧页面全部交互组合

以上用固定 fixture + 截图 + 人工 smoke checklist 覆盖。

## 已知 divergence（characterization 冻结）

1. `limitations` 实际是对象（`{…, notes[]}`），契约文档写的是数组。
2. `generatedFrom` 不发出 coverageVerified 等标志（absent ≠ false 的显式声明）。
3. graph edge 无稳定业务 ID（平行边用布局序号 `#i` 去重）→ P0-A 引入 relationKey。
4. 未知 `/api/*` 路由返回 HTML 404（SimpleHTTPRequestHandler 回退），非 JSON 错误信封。
5. runner 无任何 graph 查询端点 → P0-A Track B 引入按需证据查询。
