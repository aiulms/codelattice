# CodeLattice WebUI — Beta User Guide

## 快速开始

```bash
cd /path/to/codelattice
bash scripts/webui-runner.sh --open
```

浏览器打开 `http://127.0.0.1:8765`。

## 基础使用

### 1. 打开项目
首页点击 **选择文件夹**，在 macOS 文件夹选择器中选中项目目录；如果系统选择器不可用，可使用页面里的 **网页内浏览文件夹** 或手动粘贴绝对路径。

### 2. 一键分析
选择语言或保持 `auto`，点击 **分析项目**。runner 会自动创建/更新 Project Profile，调用 `webui-snapshot.sh` 生成 enriched snapshot，并加载到 Dashboard。

### 3. Project Profiles
后续也可以在 **Project Profiles** 中选择已有项目，点击 **Gen** 重新分析。

### 4. Snapshot Library
展开 **Snapshot Library**，可搜索/过滤/排序、Load/Diff/Timeline/Download/Delete。

### 5. 浏览 Dashboard
查看 source files、symbols、edges、quality gates passed/failed。

### 6. Explore 符号
搜索/过滤符号、查看 detail、source files 列表。

### 7. Graph 视图
查看 node/edge list、search by kind、click for detail。

### 8. Diff 对比
Load Compare Snapshot → 对比 summary counts、added/removed symbols。

### 9. Timeline 趋势
加载 2+ snapshots → SVG 趋势图 + metric 表格。

### 10. Guided Review
选择场景（onboarding/before_edit/release_check 等）→ 按 steps 勾选 → 导出 report。

### 11. Report 导出
选择模板 → Generate → Copy/Download .md。

## 清理

```bash
rm -rf .codelattice-webui/
```

## Static File Mode
如果不启动 runner，直接打开 `webui/snapshot-viewer/index.html`，可通过 file input 加载 JSON snapshot。
