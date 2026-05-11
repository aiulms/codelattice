//! GitNexus ArkTS language adapter.
//!
//! This crate provides ArkTS-specific semantic analysis on top of the
//! `gitnexus-typescript` shared base. ArkTS is a TypeScript subset used in
//! HarmonyOS development, featuring decorators (`@Component`, `@State`, etc.),
//! `struct`-based component definitions, and declarative UI via `build()`.
//!
//! The tree-sitter grammar reuses `tree-sitter-typescript` since ArkTS is a
//! strict subset with added decorators. The ArkTS semantic layer interprets
//! ERROR nodes produced by `struct`/`decorator` patterns and maps them to
//! component/state/UI concepts.
//!
//! ## Layout
//!
//! - `extractors/` — ArkTS-specific extractors (component, state, UI)
//! - `graph.rs` — ArkTS-aware graph construction (extends TypeScript graph)

pub mod extractors;
pub mod graph;

// Re-export base types from the TypeScript crate
pub use gitnexus_typescript::{
    is_ts_parser_available, list_source_files, TsImport, TsManifest, TsManifestError,
    TsPackageInfo, TsParseError, TsProject, TsProjectKind, TsReference, TsSymbol, TsSymbolKind,
};
