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
use crate::project::TsProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
}

/// Build a complete graph from a TsProject and extracted per-file data.
pub fn build_ts_graph(
    project: &TsProject,
    symbols: &BTreeMap<PathBuf, Vec<TsSymbol>>,
    imports: &BTreeMap<PathBuf, Vec<TsImport>>,
    references: &BTreeMap<PathBuf, Vec<TsReference>>,
) -> TsGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Repository node
    let repo_id = format!("repo:{}", project.root.display());
    nodes.push(TsGraphNode {
        id: repo_id.clone(),
        kind: TsNodeKind::Repository,
        label: project
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string(),
        properties: serde_json::json!({
            "language": format!("{:?}", project.kind),
        }),
    });

    // Source file nodes
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        nodes.push(TsGraphNode {
            id: file_id.clone(),
            kind: TsNodeKind::SourceFile,
            label: rel.to_string_lossy().to_string(),
            properties: serde_json::json!({}),
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
                    label: sym.name.clone(),
                    properties: serde_json::json!({
                        "kind": sym.kind.to_string(),
                        "startLine": sym.start_line,
                        "endLine": sym.end_line,
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

        // Import edges
        if let Some(imps) = imports.get(file) {
            for imp in imps {
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

        // Reference edges
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

    TsGraphOutput { nodes, edges }
}
