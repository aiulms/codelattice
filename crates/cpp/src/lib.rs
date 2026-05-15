//! GitNexus C++ language adapter.
//!
//! This crate provides AST parsing and symbol/include/call extraction
//! for C++ (.cpp, .cc, .cxx, .hpp, .hh, .hxx, etc.) source files
//! using `tree-sitter-cpp` as the underlying grammar.
//!
//! ## Layout
//!
//! - `extractors/` — tree-sitter-based symbol, include, call extraction
//! - `graph.rs` — graph output types compatible with the project-model schema
//! - `project.rs` — project root detection and source file discovery
//!
//! The tree-sitter parser is gated behind the `tree-sitter-cpp` feature.
//!
//! ## Phase A Limitations
//!
//! - No full preprocessing (no macro expansion, no `#ifdef` evaluation)
//! - No template instantiation
//! - No full overload resolution
//! - No virtual dispatch resolution
//! - No function pointer call resolution
//! - No build system execution
//! - No compile_commands.json include path resolution
//! - Not a replacement for clang / clangd / IDE

pub mod extractors;
pub mod graph;
pub mod project;

// Re-export key types for convenience
#[cfg(feature = "tree-sitter-cpp")]
pub use extractors::{extract_cpp_calls, extract_cpp_includes, extract_cpp_symbols};
pub use extractors::{
    is_cpp_parser_available, CppCall, CppInclude, CppIncludeKind, CppSymbol, CppSymbolKind,
    CppVisibility,
};
pub use graph::{build_cpp_graph, CppEdgeKind, CppGraphOutput, CppNodeKind};
pub use project::{find_cpp_project_root, list_cpp_source_files, CppProject, CppProjectKind};
