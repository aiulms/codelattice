# WebUI Phase G — Live MCP Job Mode

> **日期:** 2026-05-17 | **状态:** Preflight

## 1. 目标

在 beta workbench 基础上加入 Live MCP Mode：通过 local runner 按需调用 CodeLattice MCP 工具，执行 live analysis jobs，结果回填 WebUI。

## 2. 核心设计

- **传输**: JSON-RPC stdio（MCP 原生协议）
- **Job 模式**: HTTP polling（job create → poll → result）
- **不引入**: WebSocket / 外部依赖 / 桌面壳
- **安全**: 127.0.0.1, subprocess.run, 无 shell=True

## 3. Job Lifecycle

queued → running → succeeded / failed / cancelled

## 4. Workflows (6+)

project_overview, symbol_search, impact_preview, project_insights, dead_code_candidates, release_check, custom_tool

## 5. Acceptance

- Live MCP API (status/tools/jobs crud)
- 6 workflow jobs 可执行
- Frontend Live MCP tab
- Job result 可导入 report
- Smoke + contract tests
- Phase F beta sanity 不回归
