# WebUI Phase D — Local Runner + Snapshot Library + Live-lite

> **日期:** 2026-05-17
> **状态:** Preflight
> **关联:** [phase-d-closure.md](./2026-05-17-webui-phase-d-closure.md)

## 1. 目标

把 WebUI 从"手动加载 snapshot 文件"推进到"本地启动、生成 snapshot、管理历史、复用报告"。

本阶段不是完整桌面应用，也不是复杂 Live MCP WebSocket。

## 2. 核心组件

1. **Local Runner** (`webui-runner.sh` + `webui-runner.py`)
   - Python stdlib HTTP server (127.0.0.1 only)
   - 静态服务 `webui/snapshot-viewer/`
   - API: health, generate-snapshot, snapshots list, snapshot detail

2. **Runner Frontend Integration** (viewer UI upgrade)
   - Runner mode detection (API health check)
   - Project root input + language select + Generate button
   - Snapshot Library panel

3. **Snapshot Library**
   - Managed directory: `.codelattice-webui/snapshots/`
   - index.json metadata
   - Load/Compare/Timeline from library

## 3. 安全边界

- 只绑定 127.0.0.1
- 只读分析目标项目
- 不写目标项目
- 输出到 runner 管理目录
- 不自动打开 forbidden repos
- 路径由用户显式输入

## 4. Stop-lines

- 不声称 runtime proof
- 不执行项目代码/测试/build
- 不做完整 Live MCP
- 不做桌面壳
- 不引入外部依赖

## 5. Acceptance Criteria

- `scripts/webui-runner.sh --open` 可启动
- `/api/health` 返回 ok
- `/api/generate-snapshot` 生成 Rust fixture snapshot
- Snapshot Library 列出历史
- Runner smoke 通过
- Phase C 功能不回归
