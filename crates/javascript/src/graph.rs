//! Graph output for JavaScript project analysis.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extractors::imports::JsImport;
use crate::extractors::references::{JsReference, JsReferenceKind};
use crate::extractors::symbol::JsSymbol;
use crate::module_resolution::JsModuleResolver;
use crate::project::JsProject;

/// Graph node kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JsNodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
}

/// Graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsGraphNode {
    pub id: String,
    pub kind: JsNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

/// Graph edge kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JsEdgeKind {
    ContainsPackage,
    OwnsSource,
    Defines,
    Imports,
    Requires,
    Calls,
    Exports,
}

/// Graph edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsGraphEdge {
    #[serde(rename = "type")]
    pub kind: JsEdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

/// Graph output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsGraphOutput {
    pub nodes: Vec<JsGraphNode>,
    pub edges: Vec<JsGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

/// Build a complete graph from a JsProject and extracted per-file data.
pub fn build_js_graph(
    project: &JsProject,
    symbols: &BTreeMap<PathBuf, Vec<JsSymbol>>,
    imports: &BTreeMap<PathBuf, Vec<JsImport>>,
    references: &BTreeMap<PathBuf, Vec<JsReference>>,
    module_resolver: Option<&JsModuleResolver>,
) -> JsGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();

    let repo_id = format!("repo:{}", project.root.display());
    nodes.push(JsGraphNode {
        id: repo_id.clone(),
        kind: JsNodeKind::Repository,
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

    let pkg_id = if let Some(ref manifest) = project.manifest {
        let pkg_id = format!("pkg:{}", manifest.name);
        nodes.push(JsGraphNode {
            id: pkg_id.clone(),
            kind: JsNodeKind::Package,
            label: "package".to_string(),
            properties: serde_json::json!({
                "name": manifest.name.clone(),
                "manifestPath": "package.json",
            }),
        });
        edges.push(JsGraphEdge {
            kind: JsEdgeKind::ContainsPackage,
            source: Some(repo_id.clone()),
            target: pkg_id.clone(),
            properties: None,
        });
        Some(pkg_id)
    } else {
        None
    };

    let mut file_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut canonical_to_file_id: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut symbol_ids_by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut symbol_ids_by_file_name: BTreeMap<(PathBuf, String), Vec<String>> = BTreeMap::new();
    let mut symbol_ranges_by_file: BTreeMap<PathBuf, Vec<(usize, usize, String)>> = BTreeMap::new();

    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        file_ids.insert(file_id.clone());
        if let Ok(canonical) = std::fs::canonicalize(file) {
            canonical_to_file_id.insert(canonical, file_id.clone());
        } else {
            canonical_to_file_id.insert(file.clone(), file_id.clone());
        }
        nodes.push(JsGraphNode {
            id: file_id.clone(),
            kind: JsNodeKind::SourceFile,
            label: "source-file".to_string(),
            properties: serde_json::json!({
                "sourcePath": rel.to_string_lossy().to_string(),
                "packageId": pkg_id,
            }),
        });
        edges.push(JsGraphEdge {
            kind: JsEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: file_id.clone(),
            properties: None,
        });

        if let Some(syms) = symbols.get(file) {
            for sym in syms {
                let sym_id = format!(
                    "sym:{}:{}:{}:{}",
                    rel.display(),
                    format!("{:?}", sym.kind).to_lowercase(),
                    sym.name,
                    sym.start_line
                );
                nodes.push(JsGraphNode {
                    id: sym_id.clone(),
                    kind: JsNodeKind::Symbol,
                    label: "symbol".to_string(),
                    properties: serde_json::json!({
                        "name": sym.name,
                        "symbolKind": format!("{:?}", sym.kind).to_lowercase(),
                        "sourcePath": rel.display().to_string(),
                        "fileId": file_id,
                        "lineStart": sym.start_line,
                        "lineEnd": sym.end_line,
                        "ownerName": sym.owner_name,
                        "isAsync": sym.is_async,
                        "isExport": sym.is_export,
                        "isDefaultExport": sym.is_default_export,
                    }),
                });
                symbol_ids_by_name
                    .entry(sym.name.clone())
                    .or_default()
                    .push(sym_id.clone());
                symbol_ids_by_file_name
                    .entry((file.clone(), sym.name.clone()))
                    .or_default()
                    .push(sym_id.clone());
                symbol_ranges_by_file
                    .entry(file.clone())
                    .or_default()
                    .push((sym.start_line, sym.end_line, sym_id.clone()));
                edges.push(JsGraphEdge {
                    kind: JsEdgeKind::Defines,
                    source: Some(file_id.clone()),
                    target: sym_id.clone(),
                    properties: None,
                });

                if sym.is_export {
                    let Some(package_id) = &pkg_id else {
                        diagnostics.push(serde_json::json!({
                            "kind": "javascript-export-without-package",
                            "severity": "info",
                            "source": sym_id,
                            "reason": "package-manifest-missing",
                        }));
                        continue;
                    };
                    edges.push(JsGraphEdge {
                        kind: JsEdgeKind::Exports,
                        source: Some(sym_id),
                        target: package_id.clone(),
                        properties: None,
                    });
                }
            }
        }
    }

    let mut emitted_call_edges: std::collections::BTreeSet<(String, String, usize)> =
        std::collections::BTreeSet::new();
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        if let Some(imps) = imports.get(file) {
            for imp in imps {
                match imp.kind {
                    crate::extractors::imports::JsImportKind::EsmDynamicImport => {
                        diagnostics.push(serde_json::json!({
                            "kind": "javascript-dynamic-import",
                            "severity": "info",
                            "message": format!("dynamic import() at line {} cannot be statically resolved", imp.line),
                            "source": file_id,
                            "specifier": imp.module_path,
                            "line": imp.line,
                            "reason": "dynamic-import-not-statically-resolvable",
                        }));
                    }
                    crate::extractors::imports::JsImportKind::CommonJsRequire => {
                        if let Some(resolver) = module_resolver {
                            let resolved = resolver.resolve_import(file, &imp.module_path);
                            match resolved.kind {
                                crate::module_resolution::JsResolutionKind::External => {
                                    diagnostics.push(serde_json::json!({
                                        "kind": "javascript-external-require",
                                        "severity": "info",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": resolved.reason,
                                    }));
                                }
                                crate::module_resolution::JsResolutionKind::Unresolved => {
                                    diagnostics.push(serde_json::json!({
                                        "kind": "javascript-require-unresolved",
                                        "severity": "warning",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": resolved.reason,
                                    }));
                                }
                                crate::module_resolution::JsResolutionKind::Resolved => {
                                    if let Some(target) = resolved.target_file {
                                        let target_id = canonical_to_file_id
                                            .get(&target)
                                            .cloned()
                                            .unwrap_or_else(|| {
                                                format!("file:{}", target.display())
                                            });
                                        if file_ids.contains(&target_id) {
                                            edges.push(JsGraphEdge {
                                                kind: JsEdgeKind::Requires,
                                                source: Some(file_id.clone()),
                                                target: target_id,
                                                properties: Some(serde_json::json!({
                                                    "line": imp.line,
                                                    "confidence": resolved.confidence,
                                                    "reason": resolved.reason,
                                                })),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::extractors::imports::JsImportKind::CommonJsModuleExports
                    | crate::extractors::imports::JsImportKind::CommonJsExportsAccess => {
                        let Some(package_id) = &pkg_id else {
                            diagnostics.push(serde_json::json!({
                                "kind": "javascript-commonjs-export-without-package",
                                "severity": "info",
                                "source": file_id,
                                "line": imp.line,
                                "reason": "package-manifest-missing",
                            }));
                            continue;
                        };
                        edges.push(JsGraphEdge {
                            kind: JsEdgeKind::Exports,
                            source: Some(file_id.clone()),
                            target: package_id.clone(),
                            properties: Some(serde_json::json!({
                                "line": imp.line,
                                "kind": "commonjs",
                            })),
                        });
                    }
                    crate::extractors::imports::JsImportKind::EsmImport => {
                        if let Some(resolver) = module_resolver {
                            let resolved = resolver.resolve_import(file, &imp.module_path);
                            match resolved.kind {
                                crate::module_resolution::JsResolutionKind::External => {
                                    diagnostics.push(serde_json::json!({
                                        "kind": "javascript-external-import",
                                        "severity": "info",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": resolved.reason,
                                    }));
                                }
                                crate::module_resolution::JsResolutionKind::Unresolved => {
                                    diagnostics.push(serde_json::json!({
                                        "kind": "javascript-import-unresolved",
                                        "severity": "warning",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": resolved.reason,
                                    }));
                                }
                                crate::module_resolution::JsResolutionKind::Resolved => {
                                    if let Some(target) = resolved.target_file {
                                        let target_id = canonical_to_file_id
                                            .get(&target)
                                            .cloned()
                                            .unwrap_or_else(|| {
                                                format!("file:{}", target.display())
                                            });
                                        if file_ids.contains(&target_id) {
                                            edges.push(JsGraphEdge {
                                                kind: JsEdgeKind::Imports,
                                                source: Some(file_id.clone()),
                                                target: target_id,
                                                properties: Some(serde_json::json!({
                                                    "names": imp.imported_names,
                                                    "line": imp.line,
                                                    "confidence": resolved.confidence,
                                                    "reason": resolved.reason,
                                                })),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(refs) = references.get(file) {
            // JS Phase A 不做类型推断；CALLS 只用同文件/项目唯一名称做保守启发式连边。
            for reference in refs {
                if reference.kind != JsReferenceKind::Call {
                    continue;
                }
                let Some(source_id) =
                    source_symbol_for_call(file, reference.line, &symbol_ranges_by_file)
                else {
                    continue;
                };
                let target = resolve_call_target(
                    file,
                    &reference.name,
                    &symbol_ids_by_file_name,
                    &symbol_ids_by_name,
                );
                match target {
                    CallTargetResolution::Resolved {
                        target_id,
                        confidence,
                        reason,
                    } => {
                        if emitted_call_edges.insert((
                            source_id.clone(),
                            target_id.clone(),
                            reference.line,
                        )) {
                            edges.push(JsGraphEdge {
                                kind: JsEdgeKind::Calls,
                                source: Some(source_id),
                                target: target_id,
                                properties: Some(serde_json::json!({
                                    "line": reference.line,
                                    "callee": reference.name,
                                    "confidence": confidence,
                                    "reason": reason,
                                })),
                            });
                        }
                    }
                    CallTargetResolution::Ambiguous { candidates } => {
                        diagnostics.push(serde_json::json!({
                            "kind": "javascript-call-ambiguous",
                            "severity": "info",
                            "source": file_id,
                            "callee": reference.name,
                            "line": reference.line,
                            "candidateCount": candidates,
                            "reason": "multiple-symbols-match-callee",
                        }));
                    }
                    CallTargetResolution::Unresolved => {}
                }
            }
        }
    }

    JsGraphOutput {
        nodes,
        edges,
        diagnostics,
    }
}

enum CallTargetResolution {
    Resolved {
        target_id: String,
        confidence: f64,
        reason: &'static str,
    },
    Ambiguous {
        candidates: usize,
    },
    Unresolved,
}

fn source_symbol_for_call(
    file: &PathBuf,
    line: usize,
    symbol_ranges_by_file: &BTreeMap<PathBuf, Vec<(usize, usize, String)>>,
) -> Option<String> {
    symbol_ranges_by_file.get(file).and_then(|ranges| {
        ranges
            .iter()
            .filter(|(start, end, _)| *start <= line && line <= *end)
            .min_by_key(|(start, end, _)| end.saturating_sub(*start))
            .map(|(_, _, id)| id.clone())
    })
}

fn resolve_call_target(
    file: &PathBuf,
    callee: &str,
    symbol_ids_by_file_name: &BTreeMap<(PathBuf, String), Vec<String>>,
    symbol_ids_by_name: &BTreeMap<String, Vec<String>>,
) -> CallTargetResolution {
    let mut candidate_names = vec![callee.to_string()];
    if let Some(last) = callee.rsplit('.').next() {
        if last != callee {
            candidate_names.push(last.to_string());
        }
    }

    for name in &candidate_names {
        if let Some(ids) = symbol_ids_by_file_name.get(&(file.clone(), name.clone())) {
            if ids.len() == 1 {
                return CallTargetResolution::Resolved {
                    target_id: ids[0].clone(),
                    confidence: 0.85,
                    reason: "same-file-callee-name",
                };
            }
            if !ids.is_empty() {
                return CallTargetResolution::Ambiguous {
                    candidates: ids.len(),
                };
            }
        }
    }

    for name in &candidate_names {
        if let Some(ids) = symbol_ids_by_name.get(name) {
            if ids.len() == 1 {
                return CallTargetResolution::Resolved {
                    target_id: ids[0].clone(),
                    confidence: 0.55,
                    reason: "project-unique-callee-name",
                };
            }
            if !ids.is_empty() {
                return CallTargetResolution::Ambiguous {
                    candidates: ids.len(),
                };
            }
        }
    }

    CallTargetResolution::Unresolved
}
