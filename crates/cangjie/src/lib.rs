pub mod diagnostics;
pub mod extractors;
pub mod graph;
pub mod manifest;
pub mod project;

// Re-export key types for convenience
pub use diagnostics::{CangjieDiagnostic, DiagnosticSeverity};
pub use extractors::{CangjieReference, CangjieSymbol, CangjieSymbolKind, ReferenceKind};
pub use manifest::{
    active_members, load_cjpm_manifest, parse_cjpm_lock, parse_cjpm_toml, resolve_path_dependency,
    resolve_workspace_manifest, CangjieDependency, CangjieManifest, CangjieManifestError,
    CangjiePackage, CangjieWorkspace, CjpmLock, CjpmLockEntry, WorkspaceManifest,
};
pub use project::{
    build_project_model, find_project_root, list_source_files, CangjiePackageInfo, CangjieProject,
};
