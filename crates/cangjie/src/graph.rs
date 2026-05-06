//! Graph output for Cangjie project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.
//!
//! The symbol-to-graph path requires tree-sitter-cangjie and is gated behind
//! the `tree-sitter-cangjie` feature.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::diagnostics::CangjieDiagnostic;
use crate::extractors::imports::CangjieImport;
use crate::extractors::references::{CangjieReference, ReferenceKind};
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
    /// Compiler or linter diagnostic (cjc / cjlint).
    Diagnostic,
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
    /// Diagnostic → Symbol (linter/compiler annotation).
    Annotates,
    /// Reference → Symbol (type annotation).
    Uses,
    /// Reference → Symbol (field read).
    Accesses,
    /// Reference → Symbol (write/mutation).
    Modifies,
    /// SourceFile → Package or SourceFile → SourceFile (import dependency).
    Imports,
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
// Diagnostics graph emission
// ---------------------------------------------------------------------------

/// Build diagnostic nodes and ANNOTATES edges from a diagnostics list.
///
/// Each diagnostic becomes a `Diagnostic` node. When a diagnostic's file path
/// matches a known source file, ANNOTATES edges are created from the diagnostic
/// to the symbols in that file that overlap with the diagnostic's line range.
///
/// Returns `(nodes, edges)` that should be merged into the graph output.
pub fn emit_cangjie_diagnostics(
    diagnostics: &[CangjieDiagnostic],
    symbols_by_file: &BTreeMap<PathBuf, Vec<CangjieSymbol>>,
    project_root: &Path,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (idx, diag) in diagnostics.iter().enumerate() {
        let diag_id = format!("diag:{}:{}", diag.source, idx);

        nodes.push(GraphNode {
            id: diag_id.clone(),
            kind: NodeKind::Diagnostic,
            label: diag.message.clone(),
            properties: serde_json::json!({
                "filePath": diag.file_path,
                "severity": diag.severity,
                "source": diag.source,
                "rule": diag.rule,
                "startLine": diag.start_line,
                "startColumn": diag.start_column,
                "endLine": diag.end_line,
                "endColumn": diag.end_column,
            }),
        });

        // Find symbols in the same file whose line range overlaps the diagnostic.
        // Build a PathBuf key that matches symbols_by_file keys.
        for (file_path, symbols) in symbols_by_file {
            let file_str = file_path.to_string_lossy();
            let diag_file = &diag.file_path;

            // Match by suffix (diag.file_path may be relative or absolute)
            if !file_str.ends_with(diag_file) && !diag_file.ends_with(&*file_str) {
                // Also try matching by filename alone
                let diag_name = Path::new(diag_file)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());
                let file_name = Path::new(&*file_str)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());
                if diag_name != file_name {
                    continue;
                }
            }

            for sym in symbols {
                // Check line range overlap
                if sym.start_line <= diag.end_line && sym.end_line >= diag.start_line {
                    let sym_id = symbol_node_id(file_path, project_root, sym);
                    edges.push(GraphEdge {
                        kind: EdgeKind::Annotates,
                        source_id: diag_id.clone(),
                        target_id: sym_id,
                    });
                }
            }
        }
    }

    (nodes, edges)
}

// ---------------------------------------------------------------------------
// Reference edge emission
// ---------------------------------------------------------------------------

/// Emit graph edges from extracted references.
///
/// Each [`CangjieReference`] maps to a Uses/Accesses/Modifies edge.
/// The source is the enclosing Method/Constructor/Function node ID;
/// the target is the resolved symbol node ID.
///
/// Returns edges that should be merged into the graph output.
pub fn emit_cangjie_reference_edges(
    references: &[CangjieReference],
    symbols_by_file: &BTreeMap<PathBuf, Vec<CangjieSymbol>>,
    project_root: &Path,
) -> Vec<GraphEdge> {
    // Build a lookup from (file_path, symbol_name) → symbol_node_id for all symbols
    // Store owned Strings as keys to avoid borrowing temporaries.
    let mut symbol_id_lookup: HashMap<(String, String), String> = HashMap::new();
    for (file_path, symbols) in symbols_by_file {
        let file_key = file_path.to_string_lossy().to_string();
        for sym in symbols {
            let key = (file_key.clone(), sym.name.clone());
            let node_id = symbol_node_id(file_path, project_root, sym);
            symbol_id_lookup.insert(key, node_id);
        }
    }

    let mut edges = Vec::new();

    for r in references {
        let edge_kind = match r.kind {
            ReferenceKind::Uses => EdgeKind::Uses,
            ReferenceKind::Accesses => EdgeKind::Accesses,
            ReferenceKind::Modifies => EdgeKind::Modifies,
        };

        // Find target symbol node ID from same-file index
        // The reference file_path is the key — we look up (file_path, target_name)
        let target_id = symbol_id_lookup.get(&(r.file_path.clone(), r.target_name.clone()));

        if let Some(tid) = target_id {
            edges.push(GraphEdge {
                kind: edge_kind,
                source_id: r.source_id.clone(),
                target_id: tid.clone(),
            });
        }
        // If target not found, skip — aligns with same-file-only resolution
        // (no-edge for cross-file / no match)
    }

    edges
}

// ---------------------------------------------------------------------------
// Import edge emission
// ---------------------------------------------------------------------------

/// Emit graph edges from extracted imports.
///
/// Each [`CangjieImport`] maps to an Imports edge.
/// For same-project imports that resolve to a known package, the edge connects
/// the SourceFile to the target Package. For unresolved imports (external),
/// no edge is emitted.
///
/// Returns edges that should be merged into the graph output.
pub fn emit_cangjie_import_edges(
    imports_by_file: &std::collections::BTreeMap<PathBuf, Vec<CangjieImport>>,
    project: &CangjieProject,
) -> Vec<GraphEdge> {
    use crate::extractors::imports::{parse_named_import_candidates, resolve_import_target};

    let mut edges = Vec::new();

    for (file_path, imports) in imports_by_file {
        let file_id = source_file_node_id(file_path, &project.root);

        for import in imports {
            // Skip wildcard imports for now (no specific symbol target)
            if import.is_wildcard {
                continue;
            }

            // Parse import candidates from raw path
            let candidates = parse_named_import_candidates(&import.raw_path);
            for candidate in candidates {
                // Try to resolve the package
                if let Some(resolved) = resolve_import_target(&candidate, project) {
                    match resolved.resolution {
                        crate::extractors::imports::ResolutionKind::External => {
                            // Skip external packages
                            continue;
                        }
                        _ => {
                            // Emit Imports edge from source file to target package
                            let target_pkg = project
                                .packages
                                .iter()
                                .find(|p| p.name == resolved.target_package_name);

                            let target_pkg_id = if let Some(pkg) = target_pkg {
                                package_node_id(pkg)
                            } else if let Some(ref target_dir) = resolved.target_dir {
                                // Sub-package within the same project — use owning package
                                infer_owning_package(target_dir, &project.root, &project.packages)
                            } else {
                                continue;
                            };

                            edges.push(GraphEdge {
                                kind: EdgeKind::Imports,
                                source_id: file_id.clone(),
                                target_id: target_pkg_id,
                            });
                        }
                    }
                }
            }
        }
    }

    edges
}

// ---------------------------------------------------------------------------
// Convenience: one-shot inspect
// ---------------------------------------------------------------------------

/// Build project model, extract symbols from all source files, run
/// diagnostics (cjc + cjlint), and emit graph output in a single call.
///
/// Requires the `tree-sitter-cangjie` feature.
///
/// When the Cangjie SDK is not available, diagnostics are skipped gracefully
/// (empty `Vec`), so the graph output still contains repository, package,
/// source-file, and symbol nodes.
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

    // Parse trees for reference extraction (need the tree, not just symbols)
    // Re-parse files to get trees for reference extraction
    let mut file_trees: BTreeMap<PathBuf, tree_sitter::Tree> = BTreeMap::new();
    for file_path in &project.source_files {
        if let Ok(source) = std::fs::read_to_string(file_path) {
            if let Ok(tree) = crate::extractors::parse_cangjie_source(&source) {
                file_trees.insert(file_path.clone(), tree);
            }
        }
    }

    // Extract references (same-file only) from each file
    let mut all_references: Vec<CangjieReference> = Vec::new();
    for file_path in &project.source_files {
        if let (Some(symbols), Some(tree)) =
            (symbols_by_file.get(file_path), file_trees.get(file_path))
        {
            if let Ok(source) = std::fs::read_to_string(file_path) {
                if let Ok(refs) = crate::extractors::references::extract_cangjie_references(
                    &source, file_path, symbols, tree,
                ) {
                    all_references.extend(refs);
                }
            }
        }
    }

    let mut output = emit_cangjie_graph(&project, &symbols_by_file);

    // Reference edges (Uses/Accesses/Modifies)
    let ref_edges = emit_cangjie_reference_edges(&all_references, &symbols_by_file, &project.root);
    output.edges.extend(ref_edges);

    // Import edges (Imports)
    let mut imports_by_file: BTreeMap<PathBuf, Vec<CangjieImport>> = BTreeMap::new();
    for file_path in &project.source_files {
        if let Some(tree) = file_trees.get(file_path) {
            if let Ok(source) = std::fs::read_to_string(file_path) {
                let imports =
                    crate::extractors::imports::extract_cangjie_imports(&source, file_path, tree);
                if !imports.is_empty() {
                    imports_by_file.insert(file_path.clone(), imports);
                }
            }
        }
    }
    let import_edges = emit_cangjie_import_edges(&imports_by_file, &project);
    output.edges.extend(import_edges);

    // Diagnostics: graceful degrade when SDK absent (empty Vec)
    let diagnostics = crate::diagnostics::run_all_diagnostics(&project.root, &project.source_files);
    let (diag_nodes, diag_edges) =
        emit_cangjie_diagnostics(&diagnostics, &symbols_by_file, &project.root);
    output.nodes.extend(diag_nodes);
    output.edges.extend(diag_edges);

    // Re-sort for determinism after merging
    output.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    output.edges.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.target_id.cmp(&b.target_id))
    });

    Ok(output)
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
