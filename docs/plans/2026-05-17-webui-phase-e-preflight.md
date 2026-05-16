# WebUI Phase E — Project Workbench + Guided Review + Runner Hardening

> **日期:** 2026-05-17 | **状态:** Preflight

## 1. 目标

把 WebUI 从"本地 runner + snapshot library"推进到"可日常使用的本地项目分析工作台"。

## 2. 核心组件

1. **Project Profiles** — 保存常用项目、语言、snapshot 历史
2. **Guided Review** — 6 场景 UI 引导流程
3. **Report Templates** — 场景化 Markdown 模板
4. **Snapshot Library 增强** — 搜索/过滤/排序
5. **Runner Hardening** — API 统一结构、错误处理、路径安全
6. **Workbench Trial** — fixture profiles 闭环验证

## 3. Stop-lines

- 不执行项目代码
- 不把 checklist 当验证证明
- 不把 report 当 release approval
- 不把 dead code candidate 当删除证明
- 不写目标项目

## 4. Acceptance Criteria

- Profiles CRUD API + UI
- Guided Review 6 场景
- Report Templates 5+
- Library search/filter/sort
- Runner/Viewer smoke 通过
- Workbench trial 通过
