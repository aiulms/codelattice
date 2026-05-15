//! GitNexus C language adapter.
//!
//! This crate provides AST parsing and symbol/include/call extraction
//! for C (.c, .h) source files using `tree-sitter-c` as the underlying grammar.
//!
//! ## Layout
//!
//! - `extractors/` — tree-sitter-based symbol, include extraction
//! - `graph.rs` — graph output types compatible with the project-model schema
//! - `project.rs` — project root detection and source file discovery
//!
//! The tree-sitter parser is gated behind the `tree-sitter-c` feature.
//!
//! ## Phase A Limitations
//!
//! - No full preprocessing (no macro expansion, no `#ifdef` evaluation)
//! - No function pointer call resolution
//! - No C++ support (separate adapter)
//! - No build system execution
//! - Not a replacement for clang / clangd

pub mod compile_commands;
pub mod extractors;
pub mod graph;
pub mod include_resolution;
pub mod project;

// Re-export key types for convenience
pub use compile_commands::{load_compile_commands, CompileCommandDb, CompileCommandEntry};
#[cfg(feature = "tree-sitter-c")]
pub use extractors::{extract_c_includes, extract_c_symbols};
pub use extractors::{
    is_c_parser_available, CInclude, CIncludeKind, CSymbol, CSymbolKind, CVisibility,
};
pub use graph::{build_c_graph, CEdgeKind, CGraphOutput, CNodeKind};
pub use include_resolution::{CIncludeResolver, CResolvedInclude, CResolvedIncludeKind};
pub use project::{find_c_project_root, list_c_source_files, CProject, CProjectKind};
