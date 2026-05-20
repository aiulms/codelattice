//! GitNexus JavaScript/JSX language adapter.
//!
//! 为 JavaScript (.js, .mjs, .cjs) 和 JSX (.jsx) 源文件提供 AST 解析、
//! 符号抽取、import/require 提取和图谱生成。
//!
//! ## Parser 后端
//!
//! JavaScript adapter 复用 `gitnexus-typescript` 的 tree-sitter-typescript grammar。
//! TypeScript grammar 向下兼容纯 JavaScript 语法（函数声明、箭头函数、class、ESM 等），
//! 因此不需要独立的 tree-sitter-javascript grammar。
//!
//! ## 与 TypeScript adapter 的区别
//!
//! - 只分析 `.js/.jsx/.mjs/.cjs` 文件，不碰 `.ts/.tsx`
//! - 支持 CommonJS `require()`/`module.exports`/`exports.foo`
//! - 支持 dynamic `import()` 检测（作为 diagnostic）
//! - 支持 `package.json` 的 `main`/`module`/`exports`/`bin`/`browser` 入口识别
//!
//! ## Layout
//!
//! - `extractors/` — 基于 tree-sitter 的符号、import、reference 提取
//! - `graph.rs` — 图谱输出（nodes + edges + diagnostics）
//! - `manifest.rs` — package.json 解析、framework hint 检测
//! - `module_resolution.rs` — 相对路径和 package.json 解析
//! - `project.rs` — 项目根检测和源文件发现

pub mod extractors;
pub mod graph;
pub mod manifest;
pub mod module_resolution;
pub mod project;

pub use extractors::{
    is_js_parser_available, JsImport, JsImportKind, JsParseError, JsReference, JsReferenceKind,
    JsSymbol, JsSymbolKind,
};
pub use manifest::{
    detect_framework_hints, parse_package_json, JsEntryPoint, JsEntryPointKind, JsFrameworkHint,
    JsManifest, JsManifestError,
};
pub use module_resolution::{JsModuleResolver, JsResolutionKind, ResolvedJsImport};
pub use project::{
    detect_project_kind, find_javascript_project_root, is_js_source_file, list_source_files,
    JsProject, JsProjectKind,
};
