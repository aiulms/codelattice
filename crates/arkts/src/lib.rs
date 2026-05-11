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
pub mod project;

// Re-export base types from the TypeScript crate (always available)
pub use gitnexus_typescript::{
    graph::build_ts_graph, is_ts_parser_available, list_source_files, load_ts_manifest, TsImport,
    TsManifest, TsManifestError, TsPackageInfo, TsParseError, TsProject, TsProjectKind,
    TsReference, TsSymbol, TsSymbolKind,
};

// Re-export tree-sitter-dependent items when the feature is enabled
#[cfg(feature = "tree-sitter-arkts")]
pub use gitnexus_typescript::extractors::{
    extract_ts_imports, extract_ts_references, extract_ts_symbols, try_init_ts_parser, TsLanguage,
};

// Re-export ArkTS-specific types
pub use extractors::component::ArkTsComponent;

#[cfg(feature = "tree-sitter-arkts")]
pub use extractors::component::extract_arkts_components;
