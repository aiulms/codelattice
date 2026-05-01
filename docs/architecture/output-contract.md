# Output Contract

> **日期：** 2026-05-01
> **类型：** skeleton
> **状态：** 草案（待补全）

---

## 目的

定义 Rust-core ProjectModel CLI 的输出格式和契约。

本文件是 skeleton，需要根据 CLI/output contract preflight 填充。

---

## 当前状态

本文件尚未完成。预期内容：

- JSON 输出格式
- NDJSON 行格式
- Exit codes
- Error output

---

## 待补全

1. **JSON 结构** — PackageModel、WorkspaceModel、TargetModel 的序列化格式
2. **NDJSON 格式** — 如果用流式输出
3. **Exit codes** — 成功/错误/partial
4. **Error output** — 错误消息格式
5. **CLI flags** — `--format`、`--output`、`--verbose` 等

---

## 来源

- [Rust-core ProjectModel CLI/output contract preflight](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-cli-output-contract-preflight.md)
- [Output comparison harness 设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-output-comparison-harness-design.md)
