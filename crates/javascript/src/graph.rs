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
use crate::manifest::detect_framework_hints;
use crate::module_resolution::JsModuleResolver;
use crate::project::JsProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JsNodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
    EntryPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsGraphNode {
    pub id: String,
    pub kind: JsNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

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
    EntryPointFor,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsGraphOutput {
    pub nodes: Vec<JsGraphNode>,
    pub edges: Vec<JsGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub framework_hints: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_surface: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub summary: serde_json::Value,
}

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
        let mut pkg_props = serde_json::json!({
            "name": manifest.name.clone(),
            "manifestPath": "package.json",
        });
        if let Some(ref main) = manifest.main {
            pkg_props["main"] = serde_json::json!(main);
        }
        if let Some(ref module) = manifest.module {
            pkg_props["module"] = serde_json::json!(module);
        }
        if let Some(ref browser) = manifest.browser {
            pkg_props["browser"] = serde_json::json!(browser);
        }
        if !manifest.entry_points.is_empty() {
            pkg_props["entryPoints"] = serde_json::json!(manifest.entry_points);
        }
        nodes.push(JsGraphNode {
            id: pkg_id.clone(),
            kind: JsNodeKind::Package,
            label: "package".to_string(),
            properties: pkg_props,
        });
        edges.push(JsGraphEdge {
            kind: JsEdgeKind::ContainsPackage,
            source: Some(repo_id.clone()),
            target: pkg_id.clone(),
            properties: None,
        });

        for ep in &manifest.entry_points {
            let ep_path = project.root.join(&ep.path);
            if ep_path.exists() {
                let ep_id = format!("entry:{}:{}", ep.field, ep.path);
                nodes.push(JsGraphNode {
                    id: ep_id.clone(),
                    kind: JsNodeKind::EntryPoint,
                    label: "entry-point".to_string(),
                    properties: serde_json::json!({
                        "field": ep.field,
                        "path": ep.path,
                        "kind": format!("{:?}", ep.kind).to_lowercase(),
                        "heuristic": true,
                        "runtimeVerified": false,
                    }),
                });
                edges.push(JsGraphEdge {
                    kind: JsEdgeKind::EntryPointFor,
                    source: Some(ep_id),
                    target: pkg_id.clone(),
                    properties: Some(serde_json::json!({
                        "confidence": 0.80,
                        "reason": "package-json-field",
                    })),
                });
            } else {
                diagnostics.push(serde_json::json!({
                    "kind": "javascript-entry-point-not-found",
                    "severity": "warning",
                    "field": ep.field,
                    "path": ep.path,
                    "reason": "package-json-field-target-missing",
                }));
            }
        }

        Some(pkg_id)
    } else {
        None
    };

    let mut file_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut canonical_to_file_id: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut symbol_ids_by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut symbol_ids_by_file_name: BTreeMap<(PathBuf, String), Vec<String>> = BTreeMap::new();
    let mut symbol_ranges_by_file: BTreeMap<PathBuf, Vec<(usize, usize, String)>> = BTreeMap::new();
    let mut exported_symbols: Vec<serde_json::Value> = Vec::new();

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
                    if let Some(ref package_id) = pkg_id {
                        edges.push(JsGraphEdge {
                            kind: JsEdgeKind::Exports,
                            source: Some(sym_id.clone()),
                            target: package_id.clone(),
                            properties: None,
                        });
                    }
                    let is_index = rel
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("index."))
                        .unwrap_or(false);
                    exported_symbols.push(serde_json::json!({
                        "name": sym.name,
                        "symbolKind": format!("{:?}", sym.kind).to_lowercase(),
                        "sourcePath": rel.display().to_string(),
                        "isDefault": sym.is_default_export,
                        "isBarrelExport": is_index,
                        "externalUsageVerified": false,
                        "staticOnly": true,
                    }));
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
                        if let Some(ref package_id) = pkg_id {
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

    let framework_hints = if let Some(ref manifest) = project.manifest {
        detect_framework_hints(manifest)
            .into_iter()
            .map(|h| {
                serde_json::json!({
                    "framework": h.framework,
                    "confidence": h.confidence,
                    "reason": h.reason,
                    "heuristic": true,
                    "runtimeVerified": false,
                })
            })
            .collect()
    } else {
        vec![]
    };

    let source_file_count = project.source_files.len();
    let symbol_count: usize = symbols.values().map(|v| v.len()).sum();
    let import_edge_count = edges
        .iter()
        .filter(|e| matches!(e.kind, JsEdgeKind::Imports | JsEdgeKind::Requires))
        .count();
    let export_edge_count = edges
        .iter()
        .filter(|e| matches!(e.kind, JsEdgeKind::Exports))
        .count();
    let call_edge_count = edges
        .iter()
        .filter(|e| matches!(e.kind, JsEdgeKind::Calls))
        .count();
    let dynamic_import_count = diagnostics
        .iter()
        .filter(|d| d["kind"].as_str() == Some("javascript-dynamic-import"))
        .count();
    let unresolved_count = diagnostics
        .iter()
        .filter(|d| {
            d["kind"].as_str() == Some("javascript-import-unresolved")
                || d["kind"].as_str() == Some("javascript-require-unresolved")
        })
        .count();
    let external_import_count = diagnostics
        .iter()
        .filter(|d| {
            d["kind"].as_str() == Some("javascript-external-import")
                || d["kind"].as_str() == Some("javascript-external-require")
        })
        .count();
    let entry_point_count = nodes
        .iter()
        .filter(|n| matches!(n.kind, JsNodeKind::EntryPoint))
        .count();

    let summary = serde_json::json!({
        "language": "javascript",
        "sourceFileCount": source_file_count,
        "symbolCount": symbol_count,
        "edgeCount": edges.len(),
        "callEdgeCount": call_edge_count,
        "importEdgeCount": import_edge_count,
        "exportEdgeCount": export_edge_count,
        "diagnosticCount": diagnostics.len(),
        "unresolvedImportCount": unresolved_count,
        "dynamicImportCount": dynamic_import_count,
        "externalImportCount": external_import_count,
        "publicSurfaceCandidateCount": exported_symbols.len(),
        "frameworkHintCount": framework_hints.len(),
        "entryPointCount": entry_point_count,
        "staticOnly": true,
    });

    JsGraphOutput {
        nodes,
        edges,
        diagnostics,
        framework_hints,
        public_surface: exported_symbols,
        summary,
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
