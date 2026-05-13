# LanguageAdapter Trait

> **日期：** 2026-05-01 (updated 2026-05-13)
> **类型：** skeleton
> **状态：** 草案（待补全）

---

## 目的

定义 Rust-core LanguageAdapter trait，作为语言特定 adapter 的通用契约。

本文件是 skeleton，需要根据 Rust-core 实际实现填充。

---

## 当前已实现的语言

### Rust (stable)
- 项目检测：`Cargo.toml`
- 符号提取：函数、方法、类型、trait、枚举、宏、init
- 调用解析：同模块、跨文件、import 绑定、部分关联函数
- 质量门：完整
- MCP 工具：全部 21 个

### Cangjie / 仓颉 (stable)
- 项目检测：`cjpm.toml`
- 符号提取：函数、类、接口、枚举等
- 调用解析：同模块、跨文件
- 质量门：完整
- MCP 工具：全部 21 个

### ArkTS / HarmonyOS (alpha / production trial)
- 项目检测：`oh-package.json5`
- 符号提取：基础 TS 符号 + @Component、@State/@Local/@Prop、build()、UI 调用
- 调用解析：import 边（模块级），不做符号级跨文件绑定
- 图输出：repository → package → sourceFile → symbol/component/buildMethod
- Bridge 格式：完整（sourcePath, kind differentiation, packageId）
- 质量门：duplicate_nodes, dangling_source, deterministic
- MCP 工具：analyze, project_overview, summary, symbol_search, symbol_context
- 已验证真实项目：CoolMallArkTS (2675 nodes, 122 components), harmony-utils (3549 nodes)
- **Feature flag**: `tree-sitter-arkts`
- **已知限制**:
  - `struct` 被 tree-sitter-typescript 解析为 ERROR，通过模式匹配恢复
  - 不支持 @Builder, @Extend 高级装饰器
  - 不解析 ArkUI 声明式语法树（仅提取 UI 调用名称）
  - TypeScript 保留为 experimental，不阻塞 ArkTS

### TypeScript (experimental)
- 项目检测：`tsconfig.json` / `package.json`
- 基础符号和 import 提取
- 不独立发布，作为 ArkTS 共享基础层

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
