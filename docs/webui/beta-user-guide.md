# CodeLattice WebUI — Beta User Guide

## 快速开始

```bash
cd /path/to/codelattice
bash scripts/webui-runner.sh --open
```

浏览器打开 `http://127.0.0.1:8765`。

## 基础使用

### 1. 创建 Project Profile
点击 **Project Profiles → + New Profile**，输入名称和项目路径，选择语言。

### 2. 生成 Snapshot
选择 profile 后点击 **⚡ Generate**，runner 调用 `webui-snapshot.sh` 生成 enriched snapshot。

### 3. Snapshot Library
展开 **Snapshot Library**，可搜索/过滤/排序、Load/Diff/Timeline/Download/Delete。

### 4. 浏览 Dashboard
查看 source files、symbols、edges、quality gates passed/failed。

### 5. Explore 符号
搜索/过滤符号、查看 detail、source files 列表。

### 6. Graph 视图
查看 node/edge list、search by kind、click for detail。

### 7. Diff 对比
Load Compare Snapshot → 对比 summary counts、added/removed symbols。

### 8. Timeline 趋势
加载 2+ snapshots → SVG 趋势图 + metric 表格。

### 9. Guided Review
选择场景（onboarding/before_edit/release_check 等）→ 按 steps 勾选 → 导出 report。

### 10. Report 导出
选择模板 → Generate → Copy/Download .md。

## 清理

```bash
rm -rf .codelattice-webui/
```

## Static File Mode
如果不启动 runner，直接打开 `webui/snapshot-viewer/index.html`，可通过 file input 加载 JSON snapshot。
