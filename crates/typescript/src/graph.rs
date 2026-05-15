//! Graph output for TypeScript project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extractors::imports::TsImport;
use crate::extractors::references::TsReference;
use crate::extractors::symbol::TsSymbol;
use crate::module_resolution::TsModuleResolver;
use crate::project::TsProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TsNodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphNode {
    pub id: String,
    pub kind: TsNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsEdgeKind {
    ContainsPackage,
    OwnsSource,
    Defines,
    Imports,
    Calls,
    TypeUse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphEdge {
    #[serde(rename = "type")]
    pub kind: TsEdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Graph output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphOutput {
    pub nodes: Vec<TsGraphNode>,
    pub edges: Vec<TsGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

/// Build a complete graph from a TsProject and extracted per-file data.
pub fn build_ts_graph(
    project: &TsProject,
    symbols: &BTreeMap<PathBuf, Vec<TsSymbol>>,
    imports: &BTreeMap<PathBuf, Vec<TsImport>>,
    references: &BTreeMap<PathBuf, Vec<TsReference>>,
    module_resolver: Option<&TsModuleResolver>,
) -> TsGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();

    // Repository node
    let repo_id = format!("repo:{}", project.root.display());
    nodes.push(TsGraphNode {
        id: repo_id.clone(),
        kind: TsNodeKind::Repository,
        label: "repository".to_string(),
        properties: serde_json::json!({
            "name": project
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("root")
                .to_string(),
            "language": format!("{:?}", project.kind),
        }),
    });

    // Package node (if manifest available)
    let pkg_id = if let Some(ref manifest) = project.manifest {
        let pkg_id = format!("pkg:{}", manifest.name);
        let manifest_path = match manifest.kind {
            crate::manifest::TsManifestKind::OhPackageJson5 => project
                .root
                .join("oh-package.json5")
                .to_string_lossy()
                .to_string(),
            crate::manifest::TsManifestKind::PackageJson => project
                .root
                .join("package.json")
                .to_string_lossy()
                .to_string(),
            crate::manifest::TsManifestKind::TsconfigJson => project
                .root
                .join("tsconfig.json")
                .to_string_lossy()
                .to_string(),
        };
        nodes.push(TsGraphNode {
            id: pkg_id.clone(),
            kind: TsNodeKind::Package,
            label: "package".to_string(),
            properties: serde_json::json!({
                "name": manifest.name.clone(),
                "manifestPath": manifest_path,
            }),
        });
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::ContainsPackage,
            source: Some(repo_id.clone()),
            target: pkg_id.clone(),
            properties: None,
        });
        Some(pkg_id)
    } else {
        None
    };

    // Collect all file node IDs for dangling edge prevention
    let mut file_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Also build a map from canonical absolute path → file_id for resolver matching.
    // The resolver produces absolute paths, but file_ids may be relative (when project.root is relative).
    let mut canonical_to_file_id: std::collections::BTreeMap<PathBuf, String> =
        std::collections::BTreeMap::new();

    // Source file nodes
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        file_ids.insert(file_id.clone());
        // Try to canonicalize for resolver matching
        if let Ok(canonical) = std::fs::canonicalize(file) {
            canonical_to_file_id.insert(canonical, file_id.clone());
        } else {
            // Fallback: store as-is (absolute path)
            canonical_to_file_id.insert(file.clone(), file_id.clone());
        }
        nodes.push(TsGraphNode {
            id: file_id.clone(),
            kind: TsNodeKind::SourceFile,
            label: "source-file".to_string(),
            properties: serde_json::json!({
                "sourcePath": rel.to_string_lossy().to_string(),
                "packageId": pkg_id,
            }),
        });
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: file_id.clone(),
            properties: None,
        });

        // Symbol nodes for this file
        if let Some(syms) = symbols.get(file) {
            for sym in syms {
                let sym_id = format!(
                    "sym:{}:{}:{}:{}",
                    rel.display(),
                    sym.kind,
                    sym.name,
                    sym.start_line
                );
                nodes.push(TsGraphNode {
                    id: sym_id.clone(),
                    kind: TsNodeKind::Symbol,
                    label: "symbol".to_string(),
                    properties: serde_json::json!({
                        "name": sym.name,
                        "symbolKind": sym.kind.to_string(),
                        "sourcePath": rel.display().to_string(),
                        "fileId": file_id,
                        "lineStart": sym.start_line,
                        "lineEnd": sym.end_line,
                        "ownerName": sym.owner_name,
                    }),
                });
                edges.push(TsGraphEdge {
                    kind: TsEdgeKind::Defines,
                    source: Some(file_id.clone()),
                    target: sym_id,
                    properties: None,
                });
            }
        }
    }

    // Build import alias map (local name -> resolved target file ID) for call resolution
    let mut _import_alias_map: BTreeMap<(String, String), String> = BTreeMap::new();

    // Import edges — use resolver if available
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        if let Some(imps) = imports.get(file) {
            for imp in imps {
                if let Some(resolver) = module_resolver {
                    let resolved = resolver.resolve_import(file, &imp.module_path);
                    match resolved.resolution_kind {
                        crate::module_resolution::TsResolutionKind::External => {
                            // No edge — diagnostic only
                            diagnostics.push(serde_json::json!({
                                "kind": "typescript-external-package-not-indexed",
                                "source": file_id,
                                "specifier": imp.module_path,
                                "line": imp.line,
                                "reason": resolved.reason,
                            }));
                        }
                        crate::module_resolution::TsResolutionKind::Unresolved => {
                            // No edge — diagnostic only
                            diagnostics.push(serde_json::json!({
                                "kind": "typescript-import-unresolved",
                                "source": file_id,
                                "specifier": imp.module_path,
                                "line": imp.line,
                                "reason": resolved.reason,
                            }));
                        }
                        _ => {
                            if let Some(ref target_file) = resolved.target_file {
                                // Resolve the target file to the correct file_id via canonical path
                                let target_id = {
                                    // Try canonical match first
                                    if let Ok(canonical) = std::fs::canonicalize(target_file) {
                                        canonical_to_file_id.get(&canonical).cloned()
                                    } else {
                                        None
                                    }
                                }
                                .unwrap_or_else(|| format!("file:{}", target_file.display()));

                                // Only create edge if target is an existing node
                                if file_ids.contains(&target_id) {
                                    let mut props = serde_json::json!({
                                        "names": imp.imported_names,
                                        "line": imp.line,
                                    });
                                    if let Some(confidence) = resolved.confidence {
                                        props["confidence"] = serde_json::json!(confidence);
                                    }
                                    props["reason"] = serde_json::json!(resolved.reason);
                                    edges.push(TsGraphEdge {
                                        kind: TsEdgeKind::Imports,
                                        source: Some(file_id.clone()),
                                        target: target_id,
                                        properties: Some(props),
                                    });

                                    // Track aliases for call resolution
                                    for name in &imp.imported_names {
                                        _import_alias_map.insert(
                                            (file_id.clone(), name.clone()),
                                            format!("file:{}", target_file.display()),
                                        );
                                    }
                                } else {
                                    // Resolved but not a known source file — diagnostic
                                    diagnostics.push(serde_json::json!({
                                        "kind": "typescript-import-unresolved",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": "resolved-target-not-in-graph",
                                    }));
                                }
                            }
                        }
                    }
                } else {
                    // Backward-compatible: no resolver, use module: specifier
                    edges.push(TsGraphEdge {
                        kind: TsEdgeKind::Imports,
                        source: Some(file_id.clone()),
                        target: format!("module:{}", imp.module_path),
                        properties: Some(serde_json::json!({
                            "names": imp.imported_names,
                            "line": imp.line,
                        })),
                    });
                }
            }
        }
    }

    // Reference edges
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        if let Some(refs) = references.get(file) {
            for rf in refs {
                let edge_kind = match rf.kind {
                    crate::extractors::references::TsReferenceKind::Call => TsEdgeKind::Calls,
                    crate::extractors::references::TsReferenceKind::TypeUse => TsEdgeKind::TypeUse,
                    _ => TsEdgeKind::Calls,
                };
                edges.push(TsGraphEdge {
                    kind: edge_kind,
                    source: Some(file_id.clone()),
                    target: format!("ref:{:?}:{}", rf.kind, rf.name),
                    properties: Some(serde_json::json!({
                        "line": rf.line,
                        "fullText": rf.full_text,
                    })),
                });
            }
        }
    }

    TsGraphOutput {
        nodes,
        edges,
        diagnostics,
    }
}
