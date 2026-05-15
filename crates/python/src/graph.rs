//! Graph output for Python project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::extractors::call::PythonCall;
use crate::extractors::import::PythonImport;
use crate::extractors::symbol::PythonSymbol;
use crate::module_resolution::PythonModuleIndex;
use crate::project::PythonProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PythonNodeKind {
    Repository,
    SourceFile,
    Package,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonGraphNode {
    pub id: String,
    pub kind: PythonNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PythonEdgeKind {
    OwnsSource,
    Defines,
    Imports,
    Contains,
    Calls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonGraphEdge {
    #[serde(rename = "type")]
    pub kind: PythonEdgeKind,
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
pub struct PythonGraphOutput {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub nodes: Vec<PythonGraphNode>,
    pub edges: Vec<PythonGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Build graph
// ---------------------------------------------------------------------------

/// Build a Python graph from extracted data.
pub fn build_python_graph(
    project: &PythonProject,
    symbols_by_file: &BTreeMap<PathBuf, Vec<PythonSymbol>>,
    imports_by_file: &BTreeMap<PathBuf, Vec<PythonImport>>,
    calls_by_file: &BTreeMap<PathBuf, Vec<PythonCall>>,
    module_index: Option<&PythonModuleIndex>,
) -> PythonGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let root_path = project.root.to_string_lossy();

    // 1. Repository node
    let repo_id = format!("py:repo:{root_path}");
    nodes.push(PythonGraphNode {
        id: repo_id.clone(),
        kind: PythonNodeKind::Repository,
        label: project
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        properties: serde_json::json!({
            "projectKind": format!("{:?}", project.kind),
        }),
    });

    // 2. Source file nodes + OWNS_SOURCE edges
    let mut file_id_map: BTreeMap<PathBuf, String> = BTreeMap::new();
    for file in &project.source_files {
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        let rel_str = rel.to_string_lossy();
        let id = format!("py:src:{rel_str}");
        file_id_map.insert(file.clone(), id.clone());
        nodes.push(PythonGraphNode {
            id: id.clone(),
            kind: PythonNodeKind::SourceFile,
            label: rel_str.to_string(),
            properties: serde_json::json!({
                "extension": file.extension().and_then(|e| e.to_str()).unwrap_or(""),
            }),
        });
        edges.push(PythonGraphEdge {
            kind: PythonEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: id,
            properties: None,
        });
    }
    for file in &project.stub_files {
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        let rel_str = rel.to_string_lossy();
        let id = format!("py:src:{rel_str}");
        file_id_map.insert(file.clone(), id.clone());
        nodes.push(PythonGraphNode {
            id: id.clone(),
            kind: PythonNodeKind::SourceFile,
            label: rel_str.to_string(),
            properties: serde_json::json!({
                "extension": file.extension().and_then(|e| e.to_str()).unwrap_or(""),
            }),
        });
        edges.push(PythonGraphEdge {
            kind: PythonEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: id,
            properties: None,
        });
    }

    // 3. Symbol nodes + DEFINES edges
    let mut symbol_id_set: Vec<String> = Vec::new();
    for (file, symbols) in symbols_by_file {
        let file_id = file_id_map.get(file).cloned().unwrap_or_default();
        for sym in symbols {
            nodes.push(PythonGraphNode {
                id: sym.id.clone(),
                kind: PythonNodeKind::Symbol,
                label: sym.name.clone(),
                properties: serde_json::json!({
                    "qualifiedName": sym.qualified_name,
                    "symbolKind": sym.kind.to_string(),
                    "visibility": sym.visibility.to_string(),
                    "isAsync": sym.is_async,
                    "isTest": sym.is_test,
                    "decorators": sym.decorators,
                    "lineStart": sym.line_start,
                    "lineEnd": sym.line_end,
                }),
            });
            symbol_id_set.push(sym.id.clone());

            edges.push(PythonGraphEdge {
                kind: PythonEdgeKind::Defines,
                source: Some(file_id.clone()),
                target: sym.id.clone(),
                properties: None,
            });

            // CONTAINS edge: class -> method/constructor
            if sym.kind == crate::extractors::symbol::PythonSymbolKind::Method
                || sym.kind == crate::extractors::symbol::PythonSymbolKind::Constructor
            {
                // Find the parent class symbol
                let parent_class = sym.qualified_name.rsplit_once('.').map(|(p, _)| p);
                if let Some(parent_name) = parent_class {
                    let parent_id = symbols_by_file
                        .values()
                        .flat_map(|s| s.iter())
                        .find(|s| {
                            s.qualified_name == parent_name
                                && s.kind == crate::extractors::symbol::PythonSymbolKind::Class
                        })
                        .map(|s| s.id.clone());
                    if let Some(pid) = parent_id {
                        edges.push(PythonGraphEdge {
                            kind: PythonEdgeKind::Contains,
                            source: Some(pid),
                            target: sym.id.clone(),
                            properties: None,
                        });
                    }
                }
            }
        }
    }

    // 4. Import edges (with module resolution)
    let mut diagnostics: Vec<serde_json::Value> = Vec::new();
    let mut import_alias_map: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();

    for (file, imports) in imports_by_file {
        let file_id = file_id_map.get(file).cloned().unwrap_or_default();
        for imp in imports {
            // Skip dynamic imports entirely
            if imp.module_path.starts_with("importlib")
                || imp.imported_name.as_deref() == Some("import_module")
            {
                diagnostics.push(serde_json::json!({
                    "severity": "info",
                    "code": "python-dynamic-import-not-resolved",
                    "message": format!("dynamic import not resolved: {}", imp.module_path),
                    "line": imp.line,
                }));
                continue;
            }

            // Handle star imports
            if imp.kind == crate::extractors::import::PythonImportKind::StarImport {
                diagnostics.push(serde_json::json!({
                    "severity": "info",
                    "code": "python-star-import-not-expanded",
                    "message": format!("star import not expanded: from {} import *", imp.module_path),
                    "line": imp.line,
                }));
                continue;
            }

            // Try module resolution
            if let Some(idx) = module_index {
                match idx.resolve_import(
                    &imp.module_path,
                    imp.imported_name.as_deref(),
                    imp.alias.as_deref(),
                    imp.level,
                    file,
                ) {
                    Ok(resolved) => {
                        // Record alias for CALLS resolution
                        let alias_key = imp.alias.clone().or_else(|| imp.imported_name.clone());
                        if let Some(key) = alias_key {
                            import_alias_map.insert(
                                key,
                                (
                                    resolved.target_module.clone(),
                                    resolved.target_symbol.clone(),
                                ),
                            );
                        }

                        // Create edge to target file
                        if let Some(ref target_file) = resolved.target_file {
                            let target_id =
                                file_id_map.get(target_file).cloned().unwrap_or_else(|| {
                                    format!("py:src:{}", target_file.to_string_lossy())
                                });

                            edges.push(PythonGraphEdge {
                                kind: PythonEdgeKind::Imports,
                                source: Some(file_id.clone()),
                                target: target_id,
                                properties: Some(serde_json::json!({
                                    "importKind": format!("{:?}", imp.kind),
                                    "importedName": imp.imported_name,
                                    "alias": imp.alias,
                                    "level": imp.level,
                                    "confidence": resolved.confidence,
                                    "reason": resolved.reason,
                                    "resolvedModule": resolved.target_module,
                                    "resolvedSymbol": resolved.target_symbol,
                                    "line": imp.line,
                                })),
                            });

                            // If a specific symbol was imported, create edge to symbol node
                            if let Some(ref sym_name) = resolved.target_symbol {
                                if sym_name != "*" {
                                    let sym_id = find_symbol_id_in_file(
                                        target_file,
                                        sym_name,
                                        symbols_by_file,
                                    );
                                    if let Some(sid) = sym_id {
                                        edges.push(PythonGraphEdge {
                                            kind: PythonEdgeKind::Imports,
                                            source: Some(file_id.clone()),
                                            target: sid,
                                            properties: Some(serde_json::json!({
                                                "importKind": format!("{:?}", imp.kind),
                                                "confidence": resolved.confidence,
                                                "reason": resolved.reason,
                                                "importedSymbol": sym_name,
                                            })),
                                        });
                                    }
                                }
                            }
                        } else {
                            // Module resolved but target file not in graph
                            diagnostics.push(serde_json::json!({
                                "severity": "info",
                                "code": "python-import-module-not-found",
                                "message": format!("module {} not found in project", resolved.target_module),
                                "line": imp.line,
                            }));
                        }
                    }
                    Err(diag) => {
                        diagnostics.push(serde_json::json!({
                            "severity": "info",
                            "code": diag.reason_code,
                            "message": diag.message,
                            "line": imp.line,
                        }));
                    }
                }
            } else {
                // No module index available - fall back to synthetic nodes (backward compat)
                let module_id = format!("py:mod:{}", imp.module_path);
                let confidence = match imp.kind {
                    crate::extractors::import::PythonImportKind::StarImport => 0.20,
                    crate::extractors::import::PythonImportKind::RelativeImport => 0.70,
                    _ => 0.85,
                };
                let reason = match imp.kind {
                    crate::extractors::import::PythonImportKind::StarImport => {
                        "star-import-ambiguous".to_string()
                    }
                    crate::extractors::import::PythonImportKind::RelativeImport => {
                        "relative-import".to_string()
                    }
                    _ => "explicit-import".to_string(),
                };

                edges.push(PythonGraphEdge {
                    kind: PythonEdgeKind::Imports,
                    source: Some(file_id.clone()),
                    target: module_id,
                    properties: Some(serde_json::json!({
                        "importKind": format!("{:?}", imp.kind),
                        "importedName": imp.imported_name,
                        "alias": imp.alias,
                        "level": imp.level,
                        "confidence": confidence,
                        "reason": reason,
                    })),
                });
            }
        }
    }

    // 5. CALLS edges
    for (file, calls) in calls_by_file {
        let file_id = file_id_map.get(file).cloned().unwrap_or_default();
        for call in calls {
            // Try to resolve callee to a known symbol, using alias map if available.
            let callee_target = resolve_callee(
                &call.callee_name,
                &call.callee_qualified,
                symbols_by_file,
                &import_alias_map,
            );

            let confidence = call.confidence;
            let reason = call.reason.clone();

            if let Some(target_id) = callee_target {
                edges.push(PythonGraphEdge {
                    kind: PythonEdgeKind::Calls,
                    source: Some(file_id.clone()),
                    target: target_id,
                    properties: Some(serde_json::json!({
                        "confidence": confidence,
                        "reason": reason,
                        "line": call.line,
                    })),
                });
            } else {
                // Unresolved call -- create a placeholder symbol node
                let placeholder_id = format!(
                    "py:call-target:{}:{}:{}",
                    file_id, call.callee_name, call.line
                );
                nodes.push(PythonGraphNode {
                    id: placeholder_id.clone(),
                    kind: PythonNodeKind::Symbol,
                    label: call.callee_name.clone(),
                    properties: serde_json::json!({
                        "qualifiedName": call.callee_qualified,
                        "symbolKind": "unresolved",
                        "line": call.line,
                    }),
                });
                edges.push(PythonGraphEdge {
                    kind: PythonEdgeKind::Calls,
                    source: Some(file_id.clone()),
                    target: placeholder_id,
                    properties: Some(serde_json::json!({
                        "confidence": confidence,
                        "reason": reason,
                        "line": call.line,
                    })),
                });
            }
        }
    }

    PythonGraphOutput {
        schema_version: "v0.2".to_string(),
        nodes,
        edges,
        diagnostics,
    }
}

/// Try to resolve a callee name to a known symbol ID.
fn resolve_callee(
    callee_name: &str,
    callee_qualified: &Option<String>,
    symbols_by_file: &BTreeMap<PathBuf, Vec<PythonSymbol>>,
    import_alias_map: &BTreeMap<String, (String, Option<String>)>,
) -> Option<String> {
    // First try exact qualified name match
    if let Some(qualified) = callee_qualified {
        for symbols in symbols_by_file.values() {
            for sym in symbols {
                if &sym.qualified_name == qualified {
                    return Some(sym.id.clone());
                }
            }
        }
    }

    // Try import alias resolution: if callee_name is an imported alias,
    // look up the target symbol across files.
    if let Some((_target_module, target_symbol)) = import_alias_map.get(callee_name) {
        if let Some(sym_name) = target_symbol {
            for symbols in symbols_by_file.values() {
                for sym in symbols {
                    if sym.name == *sym_name {
                        return Some(sym.id.clone());
                    }
                }
            }
        }
    }

    // Then try name match (for function calls)
    for symbols in symbols_by_file.values() {
        for sym in symbols {
            if sym.name == callee_name {
                return Some(sym.id.clone());
            }
        }
    }

    None
}

/// Find a symbol node ID by name in a specific file.
fn find_symbol_id_in_file(
    file: &Path,
    symbol_name: &str,
    symbols_by_file: &BTreeMap<PathBuf, Vec<PythonSymbol>>,
) -> Option<String> {
    symbols_by_file.get(file).and_then(|syms| {
        syms.iter()
            .find(|s| s.name == symbol_name)
            .map(|s| s.id.clone())
    })
}
