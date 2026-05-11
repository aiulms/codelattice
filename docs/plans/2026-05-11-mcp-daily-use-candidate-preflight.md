# MCP Daily-use Candidate Pack — Preflight

> **日期：** 2026-05-11
> **版本：** v0.5.0
> **目标：** 把 MCP 推进到 "本机 AI 客户端日常试用候选版"

## 目标

1. Cache 长期运行正确性（mtime invalidation + LRU）
2. 高频图查询工具返回 source snippet
3. 新增 daily workflow 工具（production_assist, compare_runs）
4. Real client readiness（doctor + dry-run）
5. 文档稳定化

## Scope

- Stage 1: Cache Correctness (mtime + LRU)
- Stage 2: Source Snippet Expansion (calls_from/to, impact, query, rename)
- Stage 3: Daily Workflow Tools (production_assist, compare_runs)
- Stage 4: Real Client Readiness (--doctor, enhanced --self-test, real-client-dry-run.sh)
- Stage 5: Documentation
- Stage 6: Verification
- Stage 7: Tool Index Refresh
- Stage 8: Commit / Push

## 不做的事

- 不切默认工具
- 不修改 GitNexus-RC runtime/schema/WebUI
- 不自动改用户真实 AI 客户端配置
- 不做真实 rename/refactor apply
- 不新增依赖
