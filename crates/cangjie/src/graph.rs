//! Graph output for Cangjie project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.
//!
//! The symbol-to-graph path requires tree-sitter-cangjie and is gated behind
//! the `tree-sitter-cangjie` feature.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
    /// Compiler or linter diagnostic (cjc / cjlint).
    Diagnostic,
    /// Synthetic callable source node (Constructor/Method/Function).
    /// Emitted to fix dangling source endpoints in reference edges.
    /// Marked with `synthetic = true` in properties.
    CallableSource,
}

/// A node in the Cangjie graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    // Init 和 Function symbol node id 添加 #arity 后缀以保证多定义时的 node id 唯一性。
    // Init 格式：sym:<rel-path>:Init:<Owner>.init#<arity>
    // Function 格式：sym:<rel-path>:Function:<[Owner.]name>#<arity>
    match symbol.kind {
        crate::CangjieSymbolKind::Init | crate::CangjieSymbolKind::Function => {
            if let Some(arity) = symbol.arity {
                return format!(
                    "sym:{}:{}:{}#{}",
                    rel.to_string_lossy(),
                    kind_str,
                    symbol.name,
                    arity
                );
            }
        }
        _ => {}
    }
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
// Synthetic source node emission
// ---------------------------------------------------------------------------

/// Emit synthetic callable source nodes to fix dangling source endpoints.
///
/// Reference extraction creates source IDs for Constructor/Method/Function
/// scopes (e.g., `Constructor:/path/to/file:ClassName.init#arity`),
/// but symbol extraction may not emit corresponding nodes for these scopes.
/// This function creates synthetic nodes for all unique source IDs
/// that are not already covered by real symbol nodes (via resolved_source_ids),
/// to ensure endpoint integrity.
///
/// Returns nodes that should be merged into the graph output.
pub fn emit_synthetic_source_nodes(
    references: &[CangjieReference],
    resolved_source_ids: &std::collections::HashSet<String>,
) -> Vec<GraphNode> {
    // Collect unique source IDs
    let unique_source_ids: std::collections::HashSet<_> =
        references.iter().map(|r| r.source_id.clone()).collect();

    let mut nodes = Vec::new();

    for source_id in unique_source_ids {
        // 跳过已被真实 init symbol 覆盖的 source IDs
        if resolved_source_ids.contains(&source_id) {
            continue;
        }

        // Parse source_id to determine kind and extract label
        let (kind, label) = if source_id.starts_with("Constructor:") {
            ("Constructor", extract_constructor_label(&source_id))
        } else if source_id.starts_with("Method:") {
            ("Method", extract_method_label(&source_id))
        } else if source_id.starts_with("Function:") {
            ("Function", extract_function_label(&source_id))
        } else {
            // Skip unknown source ID formats (e.g., SourceFile IDs should already exist)
            continue;
        };

        nodes.push(GraphNode {
            id: source_id.clone(),
            kind: NodeKind::CallableSource,
            label: label.to_string(),
            properties: serde_json::json!({
                "synthetic": true,
                "kind": kind,
            }),
        });
    }

    nodes
}

/// Extract a readable label from a Constructor source ID.
///
/// Example: `Constructor:/path/to/file:ClassName.init#arity` → `ClassName.init`
fn extract_constructor_label(source_id: &str) -> &str {
    // Format: `Constructor:/path/to/file:ClassName.init#arity`
    // Extract the part after the last ':' and before '#'
    if let Some(pos) = source_id.rfind(':') {
        if let Some(hash_pos) = source_id.find('#') {
            &source_id[pos + 1..hash_pos]
        } else {
            &source_id[pos + 1..]
        }
    } else {
        source_id
    }
}

/// Extract a readable label from a Method source ID.
///
/// Example: `Method:/path/to/file:ClassName.methodName#arity` → `ClassName.methodName`
fn extract_method_label(source_id: &str) -> &str {
    // Format: `Method:/path/to/file:ClassName.methodName#arity`
    // Extract the part after the last ':' and before '#'
    if let Some(pos) = source_id.rfind(':') {
        if let Some(hash_pos) = source_id.find('#') {
            &source_id[pos + 1..hash_pos]
        } else {
            &source_id[pos + 1..]
        }
    } else {
        source_id
    }
}

/// Extract a readable label from a Function source ID.
///
/// Example: `Function:/path/to/file:funcName#0` → `funcName`
fn extract_function_label(source_id: &str) -> &str {
    // Format: `Function:/path/to/file:funcName#arity`
    // Extract the part after the last ':' and before '#'
    let after_colon = if let Some(pos) = source_id.rfind(':') {
        &source_id[pos + 1..]
    } else {
        source_id
    };
    if let Some(hash_pos) = after_colon.find('#') {
        &after_colon[..hash_pos]
    } else {
        after_colon
    }
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
/// 当 source_id 格式为 `Constructor:<abs-path>:<Owner>.init#arity` 且
/// 存在对应的 Init symbol 时，将 source_id 映射为 init symbol 的 node_id，
/// 使 edge source 指向真实 definition 而非 synthetic node。
///
/// Returns (edges, resolved_source_ids) — edges 应合并到 graph，
/// resolved_source_ids 是已被真实 init symbol 覆盖的 source ID 集合。
pub fn emit_cangjie_reference_edges(
    references: &[CangjieReference],
    symbols_by_file: &BTreeMap<PathBuf, Vec<CangjieSymbol>>,
    project_root: &Path,
) -> (Vec<GraphEdge>, std::collections::HashSet<String>) {
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

    // 构建 Constructor source_id → init symbol node_id 映射
    // 格式：Constructor:<abs-path>:<Owner>.init#arity → sym:<rel-path>:Init:<Owner>.init#arity
    let mut constructor_to_symbol_id: HashMap<String, String> = HashMap::new();
    // 构建 Method source_id → Function symbol node_id 映射
    // 格式：Method:<abs-path>:<Owner>.<funcName>#arity → sym:<rel-path>:Function:<Owner>.<funcName>#arity
    let mut method_to_symbol_id: HashMap<String, String> = HashMap::new();
    for (file_path, symbols) in symbols_by_file {
        for sym in symbols {
            if sym.kind == crate::CangjieSymbolKind::Init {
                if let Some(ref owner) = sym.owner_name {
                    // 构建 Constructor source_id 格式（含 #arity 后缀，与 references.rs 的 build_source_id 输出对齐）
                    let abs_path = file_path.to_string_lossy();
                    let arity = sym.arity.unwrap_or(0);
                    let constructor_source_id =
                        format!("Constructor:{}:{}.init#{}", abs_path, owner, arity);
                    let sym_id = symbol_node_id(file_path, project_root, sym);
                    constructor_to_symbol_id.insert(constructor_source_id, sym_id);
                }
            }
            if sym.kind == crate::CangjieSymbolKind::Function {
                if sym.owner_name.is_some() {
                    // sym.name 已经是 Owner.funcName 格式
                    let abs_path = file_path.to_string_lossy();
                    let arity = sym.arity.unwrap_or(0);
                    let method_source_id = format!("Method:{}:{}#{}", abs_path, sym.name, arity);
                    let sym_id = symbol_node_id(file_path, project_root, sym);
                    method_to_symbol_id.insert(method_source_id, sym_id);
                }
            }
        }
    }

    let mut edges = Vec::new();
    let mut resolved_source_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for r in references {
        let edge_kind = match r.kind {
            ReferenceKind::Uses => EdgeKind::Uses,
            ReferenceKind::Accesses => EdgeKind::Accesses,
            ReferenceKind::Modifies => EdgeKind::Modifies,
        };

        // 解析 source_id：优先使用真实 symbol node_id（Init/Function）
        let effective_source_id = resolve_source_id(
            &r.source_id,
            &constructor_to_symbol_id,
            &method_to_symbol_id,
        );
        if effective_source_id != r.source_id {
            resolved_source_ids.insert(r.source_id.clone());
        }

        // Resolve target symbol node ID:
        // 1. Cross-file: use target_file if set
        // 2. Same-file: use reference's own file_path
        let lookup_file = r.target_file.as_deref().unwrap_or(&r.file_path);
        let target_id = symbol_id_lookup.get(&(lookup_file.to_string(), r.target_name.clone()));

        if let Some(tid) = target_id {
            edges.push(GraphEdge {
                kind: edge_kind,
                source_id: effective_source_id,
                target_id: tid.clone(),
            });
        }
        // If target not found, skip (no edge)
    }

    (edges, resolved_source_ids)
}

/// 解析 source_id：如果存在对应的真实 symbol node_id，返回 node_id；否则返回原始 source_id。
///
/// 支持 Constructor: 和 Method: 前缀的 source_id 映射。
/// 格式可能包含 #arity 后缀（如 `Constructor:/path:Foo.init#3`），
/// 映射时先尝试精确匹配，再尝试去掉 #arity 后缀匹配。
fn resolve_source_id(
    source_id: &str,
    constructor_to_symbol_id: &HashMap<String, String>,
    method_to_symbol_id: &HashMap<String, String>,
) -> String {
    // 精确匹配
    if source_id.starts_with("Constructor:") {
        if let Some(sym_id) = constructor_to_symbol_id.get(source_id) {
            return sym_id.clone();
        }
        // 去掉 #arity 后缀再匹配
        if let Some(hash_pos) = source_id.find('#') {
            let without_arity = &source_id[..hash_pos];
            if let Some(sym_id) = constructor_to_symbol_id.get(without_arity) {
                return sym_id.clone();
            }
        }
    } else if source_id.starts_with("Method:") {
        if let Some(sym_id) = method_to_symbol_id.get(source_id) {
            return sym_id.clone();
        }
        // 去掉 #arity 后缀再匹配
        if let Some(hash_pos) = source_id.find('#') {
            let without_arity = &source_id[..hash_pos];
            if let Some(sym_id) = method_to_symbol_id.get(without_arity) {
                return sym_id.clone();
            }
        }
    }

    // 无匹配，返回原始 source_id（synthetic node 会覆盖）
    source_id.to_string()
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

    // Parse trees for reference extraction and import extraction
    let mut file_trees: BTreeMap<PathBuf, tree_sitter::Tree> = BTreeMap::new();
    for file_path in &project.source_files {
        if let Ok(source) = std::fs::read_to_string(file_path) {
            if let Ok(tree) = crate::extractors::parse_cangjie_source(&source) {
                file_trees.insert(file_path.clone(), tree);
            }
        }
    }

    // Extract imports first — needed to build the cross-file import binding table
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

    // Build cross-file import binding table for reference resolution
    let import_bindings = crate::extractors::references::ImportBindingTable::build(
        &symbols_by_file,
        &imports_by_file,
        &project,
    );

    // Extract references (same-file + cross-file via import bindings)
    let mut all_references: Vec<CangjieReference> = Vec::new();
    for file_path in &project.source_files {
        if let (Some(symbols), Some(tree)) =
            (symbols_by_file.get(file_path), file_trees.get(file_path))
        {
            if let Ok(source) = std::fs::read_to_string(file_path) {
                if let Ok(refs) = crate::extractors::references::extract_cangjie_references(
                    &source,
                    file_path,
                    symbols,
                    tree,
                    Some(&import_bindings),
                ) {
                    all_references.extend(refs);
                }
            }
        }
    }

    let mut output = emit_cangjie_graph(&project, &symbols_by_file);

    // Reference edges (Uses/Accesses/Modifies) + resolved source IDs
    let (ref_edges, resolved_source_ids) =
        emit_cangjie_reference_edges(&all_references, &symbols_by_file, &project.root);
    output.edges.extend(ref_edges);

    // Synthetic source nodes (fix dangling source endpoints, skip resolved)
    let synthetic_nodes = emit_synthetic_source_nodes(&all_references, &resolved_source_ids);
    output.nodes.extend(synthetic_nodes);

    // Import edges (Imports)
    let import_edges = emit_cangjie_import_edges(&imports_by_file, &project);
    output.edges.extend(import_edges);

    // Diagnostics: graceful degrade when SDK absent (empty Vec)
    let diagnostics = crate::diagnostics::run_all_diagnostics(&project.root, &project.source_files);
    let (diag_nodes, diag_edges) =
        emit_cangjie_diagnostics(&diagnostics, &symbols_by_file, &project.root);
    output.nodes.extend(diag_nodes);
    output.edges.extend(diag_edges);

    // 去重：同一 source file 中同一函数引用同一 struct 多次会产重复 (kind, sourceId, targetId) edge。
    // 这些 edge 代表不同的 reference occurrence，但在 graph identity 层面构成 multigraph 噪音，
    // 因此做确定性去重（保留首次出现）。
    {
        let mut seen_edges: HashSet<(EdgeKind, String, String)> = HashSet::new();
        output
            .edges
            .retain(|e| seen_edges.insert((e.kind, e.source_id.clone(), e.target_id.clone())));
    }

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
                owner_name: None,
                arity: None,
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
                    owner_name: None,
                    arity: None,
                },
                CangjieSymbol {
                    kind: CangjieSymbolKind::Class,
                    name: "App".to_string(),
                    start_line: 5,
                    end_line: 10,
                    owner_name: None,
                    arity: None,
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
