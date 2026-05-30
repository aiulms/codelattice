# TypeScript In-Process Job Runtime — Pack B 设计文档

> **状态**: 设计草案 · **日期**: 2026-05-30
> **依赖**: Pack A (job wait=true) 已落地

## 1. 问题陈述

CodeLattice 的 TypeScript 适配器（`crates/typescript/src/`）当前通过 CLI subprocess 执行分析。
对大型 TS 项目，fallback 到 CLI subprocess 的冷启动代价极高：

- **337s CLI fallback**（TypeScript 大项目实测）：spawn node → npx tsc → 扫描 → 序列化 → IPC
- 相比之下，Rust 原生路径约 3-8s
- 主因：TS 的 tsc 初始化 + sourceFile 解析本身是 O(n)，但 node spawn + IPC 序列化增加了 10-20x 开销

## 2. 目标

将 TypeScript 分析从 "CLI subprocess fallback" 迁移为 "in-process engine adapter"，
使其与 Rust adapter 享有相同的 job runtime（submit → wait → result）。

**非目标**：
- 不做 full tsc type inference（只做 AST 级别的 symbol/call 提取）
- 不替代 tsc --noEmit 的类型检查
- 不引入 node binding / WASM（纯 Rust 实现 TS 解析）

## 3. 五阶段计划

### Phase 1: TS Source File Parser（Rust 原生）

| 项 | 说明 |
|----|------|
| **输入** | `.ts` / `.tsx` / `.js` / `.jsx` 源文件 |
| **工具** | `tree-sitter-typescript` crate 或手写简易 parser |
| **输出** | `SymbolNode[]`（function/class/interface/enum/type/module） |
| **约束** | 不依赖 node/npm，纯 Rust |
| **预估** | 3-5 天 |

### Phase 2: TS Call Resolution

| 项 | 说明 |
|----|------|
| **输入** | Phase 1 的 symbol 表 + import 声明 |
| **策略** | import binding → module resolve → call target 匹配 |
| **置信度** | 同模块: 0.85, 跨模块有 import: 0.70, 动态/推断: 0.40 |
| **预估** | 5-7 天 |

### Phase 3: TS Engine Adapter

| 项 | 说明 |
|----|------|
| **接口** | `impl AnalysisEngine for TypeScriptEngineAdapter` |
| **行为** | 与 Rust adapter 共享 job runtime（submit → execute → result） |
| **cache** | 复用 `AnalysisArtifact` cache 层 |
| **预估** | 2-3 天 |

### Phase 4: In-Process Migration

| 项 | 说明 |
|----|------|
| **改动** | `engine_bridge.rs` 中 TS 路径：CLI fallback → in-process |
| **回退** | 保留 CLI fallback 作为 degraded 模式 |
| **验证** | 大型 TS 项目（1000+ files）分析时间 < 30s |
| **预估** | 2-3 天 |

### Phase 5: Performance Validation

| 项 | 说明 |
|----|------|
| **benchmark** | 10/100/1000/5000 file TS projects |
| **对比** | in-process vs CLI fallback 时间比 |
| **目标** | in-process ≤ CLI fallback / 5 |
| **预估** | 1-2 天 |

## 4. 风险与缓解

| 风险 | 缓解 |
|------|------|
| tree-sitter-typescript 维护风险 | 使用稳定版 pin；备选手写简易递归下降 parser |
| TS JSX/装饰器/泛型解析不全 | Phase 1 只做 symbol 提取，不做类型；复杂语法可 skip |
| 大文件内存占用 | streaming parse，不持有全 AST |
| 与现有 CLI fallback 结果不一致 | Phase 4 并行运行双路径，对比结果 |

## 5. 依赖关系

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
                                   ↑
                            Pack A job wait (已完成)
```

## 6. 估算

| 阶段 | 天数 | 人力 |
|------|------|------|
| Phase 1 | 3-5 | 1 Rust engineer |
| Phase 2 | 5-7 | 1 Rust engineer |
| Phase 3 | 2-3 | 1 Rust engineer |
| Phase 4 | 2-3 | 1 Rust engineer |
| Phase 5 | 1-2 | 1 Rust engineer |
| **总计** | **13-20** | |

## 7. 开放问题

1. tree-sitter-typescript vs 手写 parser 的 tradeoff 决策点
2. TS project references / monorepo (workspace) 的 module resolve 策略
3. `.d.ts` 声明文件是否纳入分析范围
4. 现有 fixture 覆盖度评估（需新增 TS fixture）
