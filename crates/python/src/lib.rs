//! GitNexus Python language adapter.
//!
//! This crate provides AST parsing and symbol/import/call extraction
//! for Python (.py, .pyi) source files using `tree-sitter-python`.
//!
//! ## Layout
//!
//! - `extractors/` — tree-sitter-based symbol, import, call extraction
//! - `graph.rs` — graph output types compatible with the project-model schema
//! - `project.rs` — project root detection and source file discovery
//!
//! The tree-sitter parser is gated behind the `tree-sitter-python` feature.
//!
//! ## Phase A Limitations
//!
//! - No runtime execution — no import-time side effects
//! - No dependency installation — no pip/poetry/uv/pdm
//! - No virtual environment reading — no site-packages traversal
//! - No dynamic type inference — no mypy/pyright/pylance
//! - No eval/getattr/importlib dynamic call resolution
//! - No star-import expansion — recorded as diagnostic only
//! - Not a replacement for pyright / pylance / mypy / IDE

pub mod extractors;
pub mod graph;
pub mod module_resolution;
pub mod project;

// Re-export key types for convenience
#[cfg(feature = "tree-sitter-python")]
pub use extractors::{extract_python_calls, extract_python_imports, extract_python_symbols};
pub use extractors::{
    is_python_parser_available, PythonCall, PythonImport, PythonImportKind, PythonSymbol,
    PythonSymbolKind, PythonVisibility,
};
pub use graph::{build_python_graph, PythonEdgeKind, PythonGraphOutput, PythonNodeKind};
pub use module_resolution::{ImportDiagnostic, PythonModuleIndex, ReExportInfo, ResolvedImport};
pub use project::{
    find_python_project_root, list_python_source_files, PythonProject, PythonProjectKind,
};
