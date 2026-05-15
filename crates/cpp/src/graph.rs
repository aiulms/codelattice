//! Graph output for C++ project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extractors::call::CppCall;
use crate::extractors::include::CppInclude;
use crate::extractors::symbol::CppSymbol;
use crate::include_resolution::{CppIncludeResolver, CppResolvedIncludeKind};
use crate::project::CppProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CppNodeKind {
    Repository,
    SourceFile,
    HeaderFile,
    Namespace,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CppGraphNode {
    pub id: String,
    pub kind: CppNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CppEdgeKind {
    OwnsSource,
    Defines,
    Includes,
    Contains,
    Calls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CppGraphEdge {
    #[serde(rename = "type")]
    pub kind: CppEdgeKind,
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
pub struct CppGraphOutput {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub nodes: Vec<CppGraphNode>,
    pub edges: Vec<CppGraphEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Confidence tiers for CALLS edges
// ---------------------------------------------------------------------------

/// Confidence for direct same-file function call.
const CONFIDENCE_DIRECT_SAME_FILE: f64 = 0.90;
/// Confidence for qualified project function call.
const CONFIDENCE_QUALIFIED_PROJECT: f64 = 0.80;
/// Confidence for call matching header-declared function.
const CONFIDENCE_HEADER_DECLARED: f64 = 0.75;
/// Confidence for static method name match.
const CONFIDENCE_STATIC_METHOD_MATCH: f64 = 0.75;
/// Confidence for name-only cross-file candidate.
const CONFIDENCE_NAME_ONLY_CROSS_FILE: f64 = 0.60;
/// Confidence for receiver method name only.
const CONFIDENCE_RECEIVER_METHOD: f64 = 0.45;

// ---------------------------------------------------------------------------
// Build graph
// ---------------------------------------------------------------------------

/// Build the C++ graph from project model, per-file symbols, includes, and calls.
pub fn build_cpp_graph(
    project: &CppProject,
    symbols_by_file: &BTreeMap<PathBuf, Vec<CppSymbol>>,
    includes_by_file: &BTreeMap<PathBuf, Vec<CppInclude>>,
    calls_by_file: &BTreeMap<PathBuf, Vec<CppCall>>,
    include_resolver: Option<&CppIncludeResolver>,
) -> CppGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut id_counter: u64 = 0;
    let mut diagnostics = Vec::new();
    let root = &project.root;

    // 1. Repository node
    let repo_id = format!("repo:{}", root.display());
    nodes.push(CppGraphNode {
        id: repo_id.clone(),
        kind: CppNodeKind::Repository,
        label: root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        properties: serde_json::json!({
            "root": root.to_string_lossy(),
            "projectKind": format!("{:?}", project.kind),
        }),
    });

    // 2. Source and header file nodes
    let mut file_ids: BTreeMap<PathBuf, String> = BTreeMap::new();

    for (files, node_kind, edge_kind) in [
        (
            &project.source_files,
            CppNodeKind::SourceFile,
            CppEdgeKind::OwnsSource,
        ),
        (
            &project.header_files,
            CppNodeKind::HeaderFile,
            CppEdgeKind::OwnsSource,
        ),
    ] {
        for file in files {
            id_counter += 1;
            let rel = file.strip_prefix(root).unwrap_or(file);
            let file_id = format!("file:{id_counter}:{}", rel.display());
            nodes.push(CppGraphNode {
                id: file_id.clone(),
                kind: node_kind,
                label: rel.to_string_lossy().to_string(),
                properties: serde_json::json!({
                    "path": rel.to_string_lossy(),
                }),
            });
            edges.push(CppGraphEdge {
                kind: edge_kind,
                source: Some(repo_id.clone()),
                target: file_id.clone(),
                properties: None,
            });
            file_ids.insert(file.clone(), file_id);
        }
    }

    // 3. Namespace nodes
    let mut namespace_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut seen_namespaces = std::collections::HashSet::new();

    for (_file, symbols) in symbols_by_file {
        for sym in symbols {
            if sym.kind == crate::extractors::CppSymbolKind::Namespace
                && !seen_namespaces.contains(&sym.qualified_name)
            {
                seen_namespaces.insert(sym.qualified_name.clone());
                id_counter += 1;
                let ns_id = format!("ns:{id_counter}:{}", sym.name);
                nodes.push(CppGraphNode {
                    id: ns_id.clone(),
                    kind: CppNodeKind::Namespace,
                    label: sym.name.clone(),
                    properties: serde_json::json!({
                        "qualifiedName": sym.qualified_name,
                        "lineStart": sym.start_line,
                        "lineEnd": sym.end_line,
                    }),
                });

                // Namespace parent containment
                if let Some(ref parent) = sym.parent_name {
                    if let Some(parent_id) = namespace_ids.get(parent) {
                        edges.push(CppGraphEdge {
                            kind: CppEdgeKind::Contains,
                            source: Some(parent_id.clone()),
                            target: ns_id.clone(),
                            properties: None,
                        });
                    }
                }

                namespace_ids.insert(sym.qualified_name.clone(), ns_id);
            }
        }
    }

    // 4. Symbol nodes
    let mut symbol_ids: BTreeMap<String, String> = BTreeMap::new();
    // Track project function names for call resolution
    let mut project_function_names: Vec<String> = Vec::new();

    for (file, symbols) in symbols_by_file {
        let file_id = match file_ids.get(file) {
            Some(id) => id.clone(),
            None => continue,
        };

        for sym in symbols {
            if sym.kind == crate::extractors::CppSymbolKind::Namespace {
                continue; // Already handled
            }

            id_counter += 1;
            let sym_id = format!("sym:{id_counter}:{}", sym.qualified_name);
            let kind_str = sym.kind.to_string();

            // Track function/method names for call resolution
            match sym.kind {
                crate::extractors::CppSymbolKind::FunctionDefinition
                | crate::extractors::CppSymbolKind::FunctionDeclaration
                | crate::extractors::CppSymbolKind::MethodDefinition
                | crate::extractors::CppSymbolKind::MethodDeclaration
                | crate::extractors::CppSymbolKind::ConstructorDefinition
                | crate::extractors::CppSymbolKind::ConstructorDeclaration
                | crate::extractors::CppSymbolKind::DestructorDefinition
                | crate::extractors::CppSymbolKind::DestructorDeclaration => {
                    project_function_names.push(sym.qualified_name.clone());
                    project_function_names.push(sym.name.clone());
                }
                _ => {}
            }

            let mut props = serde_json::json!({
                "name": sym.name,
                "kind": kind_str,
                "lineStart": sym.start_line,
                "lineEnd": sym.end_line,
                "isDefinition": sym.is_definition,
            });

            if !sym.qualified_name.is_empty() {
                props["qualifiedName"] = serde_json::json!(sym.qualified_name);
            }
            if let Some(ref parent) = sym.parent_name {
                props["parentName"] = serde_json::json!(parent);
            }

            props["visibility"] = serde_json::json!(sym.visibility.to_string());
            props["storageClass"] = serde_json::json!(sym.storage_class.to_string());

            nodes.push(CppGraphNode {
                id: sym_id.clone(),
                kind: CppNodeKind::Symbol,
                label: sym.name.clone(),
                properties: props,
            });

            // DEFINES edge: file -> symbol
            edges.push(CppGraphEdge {
                kind: CppEdgeKind::Defines,
                source: Some(file_id.clone()),
                target: sym_id.clone(),
                properties: None,
            });

            // CONTAINS edge: namespace -> symbol
            if let Some(ref parent) = sym.parent_name {
                if let Some(ns_id) = namespace_ids.get(parent) {
                    edges.push(CppGraphEdge {
                        kind: CppEdgeKind::Contains,
                        source: Some(ns_id.clone()),
                        target: sym_id.clone(),
                        properties: None,
                    });
                }
            }

            symbol_ids.insert(sym.qualified_name.clone(), sym_id);
        }
    }

    // 5. Include edges
    for (file, includes) in includes_by_file {
        let file_id = match file_ids.get(file) {
            Some(id) => id.clone(),
            None => continue,
        };

        if let Some(resolver) = include_resolver {
            // Use compile_commands-aware resolution
            for inc in includes {
                let resolved = resolver.resolve(file, inc);

                match resolved.resolution_kind {
                    CppResolvedIncludeKind::SameDirectory
                    | CppResolvedIncludeKind::QuoteIncludeDir
                    | CppResolvedIncludeKind::ProjectIncludeDir
                    | CppResolvedIncludeKind::ForcedInclude => {
                        if let Some(ref target_file) = resolved.target_file {
                            if let Some(target_id) = file_ids.get(target_file).cloned() {
                                edges.push(CppGraphEdge {
                                    kind: CppEdgeKind::Includes,
                                    source: Some(file_id.clone()),
                                    target: target_id,
                                    properties: Some(serde_json::json!({
                                        "includePath": inc.path,
                                        "includeKind": format!("{:?}", inc.kind),
                                        "line": inc.line,
                                        "confidence": resolved.confidence.unwrap_or(0.0),
                                        "reason": resolved.reason,
                                    })),
                                });
                            }
                        }
                    }
                    CppResolvedIncludeKind::SystemExternal => {
                        // No edge for system/external includes
                    }
                    CppResolvedIncludeKind::Unresolved | CppResolvedIncludeKind::Ambiguous => {
                        diagnostics.push(serde_json::json!({
                            "kind": resolved.reason,
                            "sourceFile": file.to_string_lossy(),
                            "includePath": inc.path,
                            "line": inc.line,
                            "includeKind": format!("{:?}", inc.kind),
                        }));
                    }
                }
            }

            // Forced includes
            for forced in resolver.resolve_forced_includes(file) {
                if let Some(ref target_file) = forced.target_file {
                    if let Some(target_id) = file_ids.get(target_file).cloned() {
                        edges.push(CppGraphEdge {
                            kind: CppEdgeKind::Includes,
                            source: Some(file_id.clone()),
                            target: target_id,
                            properties: Some(serde_json::json!({
                                "confidence": forced.confidence.unwrap_or(0.0),
                                "reason": forced.reason,
                            })),
                        });
                    }
                }
            }
        } else {
            // Legacy behavior: simple filename-based matching (no compile_commands)
            // IMPORTANT: Do NOT create `unresolved:` synthetic targets
            for inc in includes {
                let target_file = find_include_target(file, &inc.path, project);
                let target_id = match target_file {
                    Some(ref tf) => file_ids.get(tf).cloned(),
                    None => None,
                };

                if let Some(tid) = target_id {
                    edges.push(CppGraphEdge {
                        kind: CppEdgeKind::Includes,
                        source: Some(file_id.clone()),
                        target: tid,
                        properties: Some(serde_json::json!({
                            "includePath": inc.path,
                            "includeKind": format!("{:?}", inc.kind),
                            "line": inc.line,
                        })),
                    });
                }
                // Unresolved includes: no edge, no synthetic target
            }
        }
    }

    // 6. Call edges (using pre-extracted calls)
    for (file, calls) in calls_by_file {
        let file_id = match file_ids.get(file) {
            Some(id) => id.clone(),
            None => continue,
        };

        for call in calls {
            // Try to resolve callee to a known symbol
            let callee_id = resolve_callee(&call.callee_name, &call.callee_qualified, &symbol_ids);

            if let Some(target_id) = callee_id {
                edges.push(CppGraphEdge {
                    kind: CppEdgeKind::Calls,
                    source: Some(file_id.clone()),
                    target: target_id,
                    properties: Some(serde_json::json!({
                        "confidence": call.confidence,
                        "reason": call.reason,
                        "line": call.line,
                        "calleeName": call.callee_name,
                    })),
                });
            }
            // Unresolved calls are not emitted as edges in Phase A
        }
    }

    CppGraphOutput {
        schema_version: "v0.2.0".to_string(),
        nodes,
        edges,
        diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_include_target(
    from_file: &PathBuf,
    include_path: &str,
    project: &CppProject,
) -> Option<PathBuf> {
    // Try relative to the including file's directory
    let from_dir = from_file.parent()?;
    let candidate = from_dir.join(include_path);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Try relative to project root
    let root_candidate = project.root.join(include_path);
    if root_candidate.is_file() {
        return Some(root_candidate);
    }

    // Try matching by filename in project header files
    let filename = std::path::Path::new(include_path)
        .file_name()
        .and_then(|n| n.to_str())?;

    for header in &project.header_files {
        if header
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == filename)
            .unwrap_or(false)
        {
            return Some(header.clone());
        }
    }

    None
}

fn resolve_callee(
    callee_name: &str,
    callee_qualified: &Option<String>,
    symbol_ids: &BTreeMap<String, String>,
) -> Option<String> {
    // 1. Try exact qualified name match
    if let Some(qname) = callee_qualified {
        if let Some(id) = symbol_ids.get(qname) {
            return Some(id.clone());
        }
    }

    // 2. Try matching callee_name against qualified names
    for (qname, id) in symbol_ids {
        if qname.ends_with(&format!("::{callee_name}")) || qname == callee_name {
            return Some(id.clone());
        }
    }

    None
}
