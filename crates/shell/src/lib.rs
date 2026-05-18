//! CodeLattice Shell language adapter.
//!
//! This adapter is intentionally parser-light: it statically scans shell scripts
//! for functions, command invocations, sourced files, environment variables, and
//! risky script patterns. It never executes scripts and is not a shellcheck
//! replacement.

pub mod graph;
pub mod project;

pub use graph::{
    build_shell_graph, extract_shell_file, ShellCommand, ShellDiagnostic, ShellEdgeKind,
    ShellEnvAccess, ShellFileAnalysis, ShellGraphOutput, ShellNodeKind, ShellSourceRef,
    ShellSymbol, ShellSymbolKind,
};
pub use project::{
    find_shell_project_root, list_shell_source_files, ShellProject, ShellProjectKind,
};
