# expected.json Schema

> **日期：** 2026-05-01
> **类型：** skeleton
> **状态：** 草案（待补全）

---

## 目的

定义 Rust-core ProjectModel 的 `expected.json` 输出 schema。

本文件是 skeleton，需要从 GitNexus-RC 的 expected.json 文件逆向工程。

---

## 当前状态

本文件尚未完成。预期内容：

- JSON schema for expected output
- Required vs optional fields
- 示例

---

## 待补全

1. **Schema 定义** — `package`、`workspace`、`target`、`ownership`、`absence` 等字段
2. **Required fields** — 每个断言类型必需的字段
3. **示例** — 真实的 expected.json 示例
4. **版本策略** — schema 演进方式

---

## 来源

GitNexus-RC 中 14 个 expected.json 文件和 [Golden Fixture 规格](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-golden-fixture-spec.md)。
