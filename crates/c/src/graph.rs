//! Graph output for C project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extractors::include::CInclude;
use crate::extractors::symbol::CSymbol;
use crate::project::CProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CNodeKind {
    Repository,
    SourceFile,
    HeaderFile,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CGraphNode {
    pub id: String,
    pub kind: CNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CEdgeKind {
    OwnsSource,
    Defines,
    Includes,
    Calls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CGraphEdge {
    #[serde(rename = "type")]
    pub kind: CEdgeKind,
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
pub struct CGraphOutput {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub nodes: Vec<CGraphNode>,
    pub edges: Vec<CGraphEdge>,
}

// ---------------------------------------------------------------------------
// Confidence tiers for CALLS edges
// ---------------------------------------------------------------------------

/// Confidence for direct same-file function call.
const CONFIDENCE_DIRECT_SAME_FILE: f64 = 0.90;
/// Confidence for call to function declared in project header.
const CONFIDENCE_HEADER_DECLARED: f64 = 0.80;
/// Confidence for call matching project function by name only.
const CONFIDENCE_NAME_ONLY: f64 = 0.65;
/// Confidence for unresolved external call.
const CONFIDENCE_UNRESOLVED: f64 = 0.40;

// ---------------------------------------------------------------------------
// Build graph
// ---------------------------------------------------------------------------

/// Build a complete graph from a CProject and extracted per-file data.
pub fn build_c_graph(
    project: &CProject,
    symbols: &BTreeMap<PathBuf, Vec<CSymbol>>,
    includes: &BTreeMap<PathBuf, Vec<CInclude>>,
) -> CGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // 1. Repository node
    let repo_id = "repo:root".to_string();
    let root_name = project
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("root")
        .to_string();
    nodes.push(CGraphNode {
        id: repo_id.clone(),
        kind: CNodeKind::Repository,
        label: root_name,
        properties: serde_json::json!({
            "projectKind": format!("{:?}", project.kind).to_lowercase(),
        }),
    });

    // 2. Source and header file nodes
    let mut file_ids: BTreeMap<PathBuf, String> = BTreeMap::new();
    let all_files = project
        .source_files
        .iter()
        .chain(project.header_files.iter());

    for (idx, file) in all_files.enumerate() {
        let relative = file
            .strip_prefix(&project.root)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        let is_header = file
            .extension()
            .map(|e| e == "h" || e == "inc")
            .unwrap_or(false);
        let kind = if is_header {
            CNodeKind::HeaderFile
        } else {
            CNodeKind::SourceFile
        };
        let file_id = format!("file:{relative}");
        nodes.push(CGraphNode {
            id: file_id.clone(),
            kind,
            label: relative.clone(),
            properties: serde_json::json!({ "path": relative }),
        });
        edges.push(CGraphEdge {
            kind: CEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: file_id.clone(),
            properties: None,
        });
        file_ids.insert(file.clone(), file_id);
    }

    // 3. Symbol nodes
    let mut symbol_ids: BTreeMap<(String, String), String> = BTreeMap::new(); // (name, file_id) -> symbol_id
    for (file, syms) in symbols {
        let file_id = match file_ids.get(file) {
            Some(id) => id.clone(),
            None => continue,
        };
        for (idx, sym) in syms.iter().enumerate() {
            let sym_id = format!("sym:{}:{}:{}", sym.kind, sym.name, idx);
            let mut props = serde_json::json!({
                "visibility": format!("{:?}", sym.visibility).to_lowercase(),
                "isDefinition": sym.is_definition,
            });
            nodes.push(CGraphNode {
                id: sym_id.clone(),
                kind: CNodeKind::Symbol,
                label: sym.name.clone(),
                properties: props,
            });
            edges.push(CGraphEdge {
                kind: CEdgeKind::Defines,
                source: Some(file_id.clone()),
                target: sym_id.clone(),
                properties: None,
            });
            // Index by name for call resolution
            symbol_ids.insert((sym.name.clone(), file_id.clone()), sym_id.clone());
        }
    }

    // 4. Include edges (local only)
    for (file, incs) in includes {
        let source_file_id = match file_ids.get(file) {
            Some(id) => id.clone(),
            None => continue,
        };
        for inc in incs {
            if inc.kind == crate::extractors::include::CIncludeKind::Local {
                // Try to resolve to a project header file
                let target_id = file_ids
                    .iter()
                    .find(|(p, _)| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|name| name == inc.path || inc.path.ends_with(name))
                            .unwrap_or(false)
                    })
                    .map(|(_, id)| id.clone());

                if let Some(target_id) = target_id {
                    edges.push(CGraphEdge {
                        kind: CEdgeKind::Includes,
                        source: Some(source_file_id.clone()),
                        target: target_id,
                        properties: Some(serde_json::json!({
                            "confidence": 1.0,
                            "reason": "local-include",
                        })),
                    });
                }
            }
            // System includes: no edge (Phase A limitation)
        }
    }

    // 5. Call edges — Phase A: simple name-based resolution
    // Build a name → symbol_id index for the whole project
    let mut name_to_symbols: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new(); // name -> [(file_id, sym_id)]
    for ((name, file_id), sym_id) in &symbol_ids {
        name_to_symbols
            .entry(name.clone())
            .or_default()
            .push((file_id.clone(), sym_id.clone()));
    }

    // For now, Phase A call extraction is done in the CLI layer via a simple
    // tree-walk of call_expression nodes. This is a placeholder for the graph
    // builder — calls are added by the CLI's run_c_analysis().

    CGraphOutput {
        schema_version: "v0.1.0".to_string(),
        nodes,
        edges,
    }
}
