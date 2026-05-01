# Graph Schema v0

> **日期：** 2026-05-01
> **类型：** skeleton
> **状态：** 草案（待补全）

---

## 目的

定义 Rust-core ProjectModel 输出的 graph node 和 edge types。

本文件是 skeleton，需要根据 Rust-core 实际实现填充。

---

## 当前状态

本文件尚未完成。预期内容：

- Node types：PackageNode、TargetNode、SourceNode、WorkspaceNode
- Edge types：包含关系、ownership、resolution
- Schema 版本策略

---

## 待补全

1. **Node types 列表** — 基于 Rust data model 补充
2. **Edge types 列表** — 基于 ProjectModel 职责补充
3. **Required vs optional fields** — 每种 node/edge 的必需/可选字段
4. **Schema 版本策略** — 如何处理 schema 演进

---

## 来源

- [Rust-core ProjectModel 模块设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-module-design.md)
- [GitNexus-RC graph schema](../graph/schema-v3.md)（参考）
