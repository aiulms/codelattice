# LanguageAdapter Trait

> **日期：** 2026-05-01
> **类型：** skeleton
> **状态：** 草案（待补全）

---

## 目的

定义 Rust-core LanguageAdapter trait，作为语言特定 adapter 的通用契约。

本文件是 skeleton，需要根据 Rust-core 实际实现填充。

---

## 当前状态

本文件尚未完成。预期内容：

- `LanguageAdapter` trait 定义
- 方法签名：scan、resolve、diagnose
- 错误类型

---

## 待补全

1. **Trait 定义** — `pub trait LanguageAdapter`
2. **方法签名** — `scan`、`resolve`、`diagnose`
3. **Error 类型** — adapter-specific 错误
4. **Default implementations** — 如果有

---

## 来源

- [Rust-core rebuild preflight](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-04-28-rust-core-rebuild-preflight.md)
- [LanguageProvider interface in GitNexus-RC](../lang-resolution/language-provider.ts)（参考）
