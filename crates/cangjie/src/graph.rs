//! Graph output for Cangjie project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.
//!
//! The symbol-to-graph path requires tree-sitter-cangjie and is gated behind
//! the `tree-sitter-cangjie` feature.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::project::{CangjiePackageInfo, CangjieProject};
use crate::CangjieSymbol;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// Kind of a graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
}

/// A node in the Cangjie graph.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

/// Kind of a graph edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    ContainsPackage,
    OwnsSource,
    Defines,
}

/// An edge in the Cangjie graph.
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub kind: EdgeKind,
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "targetId")]
    pub target_id: String,
}

// ---------------------------------------------------------------------------
// Top-level output
// ---------------------------------------------------------------------------

/// Cangjie graph output — a set of nodes and edges.
///
/// JSON structure is compatible with project-model `GraphOutput`.
#[derive(Debug, Clone, Serialize)]
pub struct CangjieGraphOutput {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

// ---------------------------------------------------------------------------
// Node ID builders
// ---------------------------------------------------------------------------

fn repo_node_id() -> String {
    "repo:cangjie".to_string()
}

fn package_node_id(pkg: &CangjiePackageInfo) -> String {
    if pkg.module_dir.is_empty() {
        format!("pkg:{}", pkg.name)
    } else {
        format!("pkg:{}/{}", pkg.module_dir, pkg.name)
    }
}

fn source_file_node_id(file_path: &Path, project_root: &Path) -> String {
    let rel = file_path.strip_prefix(project_root).unwrap_or(file_path);
    format!("file:{}", rel.to_string_lossy())
}

fn symbol_node_id(file_path: &Path, project_root: &Path, symbol: &CangjieSymbol) -> String {
    let rel = file_path.strip_prefix(project_root).unwrap_or(file_path);
    let kind_str = format!("{:?}", symbol.kind);
    format!("sym:{}:{}:{}", rel.to_string_lossy(), kind_str, symbol.name)
}

// ---------------------------------------------------------------------------
// Graph emitter
// ---------------------------------------------------------------------------

/// Build graph output from a Cangjie project model and per-file symbol
/// extraction results.
///
/// Produces:
/// - Repository node
/// - Package nodes + ContainsPackage edges
/// - SourceFile nodes + OwnsSource edges
/// - Symbol nodes + Defines edges
///
/// Nodes and edges are emitted in deterministic order (sorted by id / kind+source+target).
pub fn emit_cangjie_graph(
    project: &CangjieProject,
    symbols_by_file: &BTreeMap<PathBuf, Vec<CangjieSymbol>>,
) -> CangjieGraphOutput {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Repository node
    let repo_id = repo_node_id();
    nodes.push(GraphNode {
        id: repo_id.clone(),
        kind: NodeKind::Repository,
        label: "cangjie-repo".to_string(),
        properties: serde_json::json!({}),
    });

    for pkg in &project.packages {
        let pkg_id = package_node_id(pkg);
        nodes.push(GraphNode {
            id: pkg_id.clone(),
            kind: NodeKind::Package,
            label: pkg.name.clone(),
            properties: serde_json::json!({
                "name": pkg.name,
                "moduleDir": pkg.module_dir,
                "srcDir": pkg.src_dir,
                "version": pkg.version,
                "cjcVersion": pkg.cjc_version,
                "outputType": pkg.output_type,
            }),
        });

        edges.push(GraphEdge {
            kind: EdgeKind::ContainsPackage,
            source_id: repo_id.clone(),
            target_id: pkg_id,
        });
    }

    // Source files and symbols — we iterate project.source_files (the deduped
    // list from build_project_model) and cross-reference with symbols_by_file.
    let mut file_nodes: Vec<&PathBuf> = project.source_files.iter().collect();
    // Sort for determinism
    file_nodes.sort_by_key(|p| p.to_string_lossy().to_string());

    for file_path in &file_nodes {
        let file_id = source_file_node_id(file_path, &project.root);

        nodes.push(GraphNode {
            id: file_id.clone(),
            kind: NodeKind::SourceFile,
            label: file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            properties: serde_json::json!({
                "path": file_id.strip_prefix("file:").unwrap_or(&file_id),
            }),
        });

        // OwnsSource: link to the owning package
        let pkg_id = infer_owning_package(file_path, &project.root, &project.packages);
        edges.push(GraphEdge {
            kind: EdgeKind::OwnsSource,
            source_id: pkg_id,
            target_id: file_id.clone(),
        });

        // Symbols for this file
        if let Some(symbols) = symbols_by_file.get(*file_path) {
            for sym in symbols {
                let sym_id = symbol_node_id(file_path, &project.root, sym);
                nodes.push(GraphNode {
                    id: sym_id.clone(),
                    kind: NodeKind::Symbol,
                    label: sym.name.clone(),
                    properties: serde_json::json!({
                        "kind": format!("{:?}", sym.kind),
                        "startLine": sym.start_line,
                        "endLine": sym.end_line,
                    }),
                });

                edges.push(GraphEdge {
                    kind: EdgeKind::Defines,
                    source_id: file_id.clone(),
                    target_id: sym_id,
                });
            }
        }
    }

    // Deterministic sort
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.target_id.cmp(&b.target_id))
    });

    CangjieGraphOutput { nodes, edges }
}

/// Infer which package owns a source file (longest matching module_dir prefix).
fn infer_owning_package(
    file_path: &Path,
    project_root: &Path,
    packages: &[CangjiePackageInfo],
) -> String {
    let rel = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy();

    // Find the package whose module_dir is the longest prefix of the relative path
    let mut best: Option<&CangjiePackageInfo> = None;
    let mut best_len = 0;

    for pkg in packages {
        let prefix = if pkg.module_dir.is_empty() {
            String::new()
        } else {
            let mut p = pkg.module_dir.clone();
            if !p.ends_with('/') {
                p.push('/');
            }
            p
        };

        if rel.starts_with(&prefix) && prefix.len() >= best_len {
            best = Some(pkg);
            best_len = prefix.len();
        }
    }

    match best {
        Some(pkg) => package_node_id(pkg),
        None => package_node_id(&packages[0]), // fallback to first package
    }
}

// ---------------------------------------------------------------------------
// Convenience: one-shot inspect
// ---------------------------------------------------------------------------

/// Build project model, extract symbols from all source files, and emit
/// graph output in a single call.
///
/// Requires the `tree-sitter-cangjie` feature.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn inspect_cangjie_project(
    root: &Path,
) -> Result<CangjieGraphOutput, crate::CangjieManifestError> {
    use crate::extractors::symbol::extract_cangjie_symbols;

    let project = crate::project::build_project_model(root)?;

    let mut symbols_by_file: BTreeMap<PathBuf, Vec<CangjieSymbol>> = BTreeMap::new();
    for file_path in &project.source_files {
        if let Ok(source) = std::fs::read_to_string(file_path) {
            if let Ok(symbols) = extract_cangjie_symbols(&source) {
                symbols_by_file.insert(file_path.clone(), symbols);
            }
        }
    }

    Ok(emit_cangjie_graph(&project, &symbols_by_file))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CangjieSymbolKind;

    fn make_project() -> CangjieProject {
        CangjieProject {
            root: PathBuf::from("/fake/project"),
            manifest: crate::CangjieManifest {
                package: Some(crate::CangjiePackage {
                    name: Some("test".to_string()),
                    version: Some("1.0.0".to_string()),
                    src_dir: "src".to_string(),
                    cjc_version: None,
                    output_type: None,
                }),
                workspace: None,
                dependencies: vec![],
            },
            packages: vec![CangjiePackageInfo {
                name: "test".to_string(),
                module_dir: String::new(),
                src_dir: "src".to_string(),
                version: Some("1.0.0".to_string()),
                cjc_version: None,
                output_type: None,
            }],
            source_files: vec![
                PathBuf::from("/fake/project/src/main.cj"),
                PathBuf::from("/fake/project/src/lib.cj"),
            ],
        }
    }

    #[test]
    fn empty_project_produces_repo_and_package_nodes() {
        let project = make_project();
        let symbols = BTreeMap::new();
        let output = emit_cangjie_graph(&project, &symbols);

        // Repository + 1 package + 2 source files
        assert_eq!(output.nodes.len(), 4);

        let repo = output.nodes.iter().find(|n| n.kind == NodeKind::Repository);
        assert!(repo.is_some());
        assert_eq!(repo.unwrap().id, "repo:cangjie");

        let pkg = output.nodes.iter().find(|n| n.kind == NodeKind::Package);
        assert!(pkg.is_some());
        assert_eq!(pkg.unwrap().label, "test");

        // ContainsPackage edge
        let cp_edges: Vec<_> = output
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ContainsPackage)
            .collect();
        assert_eq!(cp_edges.len(), 1);
        assert_eq!(cp_edges[0].source_id, "repo:cangjie");
        assert_eq!(cp_edges[0].target_id, "pkg:test");

        // OwnsSource edges
        let os_edges: Vec<_> = output
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::OwnsSource)
            .collect();
        assert_eq!(os_edges.len(), 2);
    }

    #[test]
    fn symbols_produce_defines_edges() {
        let project = make_project();
        let mut symbols = BTreeMap::new();
        symbols.insert(
            PathBuf::from("/fake/project/src/main.cj"),
            vec![CangjieSymbol {
                kind: CangjieSymbolKind::Function,
                name: "main".to_string(),
                start_line: 1,
                end_line: 3,
            }],
        );

        let output = emit_cangjie_graph(&project, &symbols);

        // Should have 5 nodes: repo + pkg + 2 files + 1 symbol
        assert_eq!(output.nodes.len(), 5);

        let sym_node = output.nodes.iter().find(|n| n.kind == NodeKind::Symbol);
        assert!(sym_node.is_some());
        assert_eq!(sym_node.unwrap().label, "main");

        // Defines edge
        let def_edges: Vec<_> = output
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Defines)
            .collect();
        assert_eq!(def_edges.len(), 1);
        assert!(def_edges[0].source_id.starts_with("file:"));
        assert!(def_edges[0].target_id.starts_with("sym:"));
    }

    #[test]
    fn output_is_deterministic() {
        let project = make_project();
        let mut symbols = BTreeMap::new();
        symbols.insert(
            PathBuf::from("/fake/project/src/main.cj"),
            vec![
                CangjieSymbol {
                    kind: CangjieSymbolKind::Function,
                    name: "main".to_string(),
                    start_line: 1,
                    end_line: 3,
                },
                CangjieSymbol {
                    kind: CangjieSymbolKind::Class,
                    name: "App".to_string(),
                    start_line: 5,
                    end_line: 10,
                },
            ],
        );

        let a = emit_cangjie_graph(&project, &symbols);
        let b = emit_cangjie_graph(&project, &symbols);

        let json_a = serde_json::to_string_pretty(&a).unwrap();
        let json_b = serde_json::to_string_pretty(&b).unwrap();
        assert_eq!(json_a, json_b);
    }

    #[test]
    fn graph_output_serializes_to_json() {
        let project = make_project();
        let symbols = BTreeMap::new();
        let output = emit_cangjie_graph(&project, &symbols);

        let json = serde_json::to_string_pretty(&output).unwrap();
        assert!(json.contains("\"nodes\""));
        assert!(json.contains("\"edges\""));
        assert!(json.contains("\"repo:cangjie\""));
        assert!(json.contains("\"containsPackage\""));
    }

    #[test]
    fn package_node_id_with_module_dir() {
        let pkg = CangjiePackageInfo {
            name: "mylib".to_string(),
            module_dir: "libs/mylib".to_string(),
            src_dir: "src".to_string(),
            version: None,
            cjc_version: None,
            output_type: None,
        };
        assert_eq!(package_node_id(&pkg), "pkg:libs/mylib/mylib");
    }

    #[test]
    fn package_node_id_root_package() {
        let pkg = CangjiePackageInfo {
            name: "root".to_string(),
            module_dir: String::new(),
            src_dir: "src".to_string(),
            version: None,
            cjc_version: None,
            output_type: None,
        };
        assert_eq!(package_node_id(&pkg), "pkg:root");
    }
}
