//! Module resolution for JavaScript imports and requires.
//!
//! Resolves relative imports and package.json-based entry points.
//! External packages are marked as external (not indexed).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Extension variants to try when resolving modules.
const JS_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx"];

/// Index file names to try.
const INDEX_FILES: &[&str] = &[
    "index.js",
    "index.mjs",
    "index.cjs",
    "index.jsx",
    "index.ts",
    "index.tsx",
];

/// Resolution result kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsResolutionKind {
    /// Successfully resolved to a local file.
    Resolved,
    /// Resolved to an external package (node_modules).
    External,
    /// Could not resolve the import.
    Unresolved,
}

/// Resolved import information.
#[derive(Debug, Clone)]
pub struct ResolvedJsImport {
    pub kind: JsResolutionKind,
    pub target_file: Option<PathBuf>,
    pub confidence: Option<f64>,
    pub reason: String,
}

/// Module resolver for JavaScript imports.
#[derive(Debug, Clone)]
pub struct JsModuleResolver {
    root: PathBuf,
    package_json_dir: Option<PathBuf>,
}

impl JsModuleResolver {
    /// Create a new resolver for the given project root.
    pub fn new(root: PathBuf) -> Self {
        let package_json_dir = root.join("package.json");
        let package_json_dir = if package_json_dir.is_file() {
            Some(root.clone())
        } else {
            None
        };
        Self {
            root,
            package_json_dir,
        }
    }

    /// Resolve an import specifier from a given source file.
    pub fn resolve_import(&self, source_file: &Path, specifier: &str) -> ResolvedJsImport {
        if !specifier.starts_with('.') {
            return self.resolve_external_package(specifier);
        }
        self.resolve_relative_import(source_file, specifier)
    }

    fn resolve_relative_import(&self, source_file: &Path, specifier: &str) -> ResolvedJsImport {
        let source_dir = source_file.parent().unwrap_or(&self.root);
        let resolved = source_dir.join(specifier);

        if let Ok(canonical) = std::fs::canonicalize(&resolved) {
            return ResolvedJsImport {
                kind: JsResolutionKind::Resolved,
                target_file: Some(canonical),
                confidence: Some(0.90),
                reason: "relative-path-resolved".to_string(),
            };
        }

        for ext in JS_EXTENSIONS {
            let with_ext = resolved.with_extension(ext);
            if with_ext.is_file() {
                if let Ok(canonical) = std::fs::canonicalize(&with_ext) {
                    return ResolvedJsImport {
                        kind: JsResolutionKind::Resolved,
                        target_file: Some(canonical),
                        confidence: Some(0.85),
                        reason: format!("relative-path-resolved-with-extension-{}", ext),
                    };
                }
            }
        }

        if resolved.is_dir() {
            for index_file in INDEX_FILES {
                let index_path = resolved.join(index_file);
                if index_path.is_file() {
                    if let Ok(canonical) = std::fs::canonicalize(&index_path) {
                        return ResolvedJsImport {
                            kind: JsResolutionKind::Resolved,
                            target_file: Some(canonical),
                            confidence: Some(0.80),
                            reason: "index-file-resolved".to_string(),
                        };
                    }
                }
            }
        }

        ResolvedJsImport {
            kind: JsResolutionKind::Unresolved,
            target_file: None,
            confidence: Some(0.0),
            reason: "relative-import-not-found".to_string(),
        }
    }

    fn resolve_external_package(&self, specifier: &str) -> ResolvedJsImport {
        ResolvedJsImport {
            kind: JsResolutionKind::External,
            target_file: None,
            confidence: Some(0.50),
            reason: "external-package-not-indexed".to_string(),
        }
    }
}
