//! GitNexus TypeScript/TSX/ArkTS language adapter.
//!
//! This crate provides shared AST parsing and base symbol/call/import
//! extraction for TypeScript (.ts), TSX (.tsx), and ArkTS (.ets) source
//! files using `tree-sitter-typescript` as the underlying grammar.
//!
//! ## Layout
//!
//! - `extractors/` — tree-sitter-based symbol, import, and reference extraction
//! - `graph.rs` — graph output types compatible with the project-model schema
//! - `manifest.rs` — oh-package.json5 / tsconfig.json / package.json parsing
//! - `project.rs` — project root detection and source file discovery
//!
//! The tree-sitter parser is gated behind the `tree-sitter-typescript` feature.

pub mod extractors;
pub mod graph;
pub mod manifest;
pub mod module_resolution;
pub mod project;
pub mod tsconfig;

// Re-export key types for convenience
pub use extractors::{
    is_ts_parser_available, TsImport, TsParseError, TsReference, TsReferenceKind, TsSymbol,
    TsSymbolKind,
};
pub use manifest::{
    load_ts_manifest, parse_oh_package_json5, parse_package_json, parse_tsconfig_json, TsManifest,
    TsManifestError,
};
pub use module_resolution::{ResolvedTsImport, TsModuleResolver, TsResolutionKind};
pub use project::{
    find_project_root, find_typescript_project_root, list_source_files, TsPackageInfo, TsProject,
    TsProjectKind,
};
pub use tsconfig::{discover_tsconfigs, load_tsconfig, TsConfigInfo};
