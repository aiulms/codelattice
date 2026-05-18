//! Shell GraphOutput → Bridge 格式转换
//!
//! 将 Shell adapter 产出的 graph JSON 转换为下游兼容 bridge 格式。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgeRepository,
    BridgeSourceFile, BridgeStats, BridgeSymbol,
};

pub fn convert_shell_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("Shell graph JSON 缺少 nodes 数组")?;
    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("Shell graph JSON 缺少 edges 数组")?;

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("shell-v0.1")
        .to_string();
    let (repo_node, source_files, symbols) = partition_shell_nodes(nodes)?;
    let repository = BridgeRepository {
        id: repo_node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("shell:repo:unknown")
            .to_string(),
        path: root_path.to_string(),
    };
    let bridge_edges = convert_shell_edges(edges);
    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: 0,
        diagnostic_count: graph_json
            .get("diagnostics")
            .and_then(|d| d.as_array())
            .map(|d| d.len() as u32)
            .unwrap_or(0),
        call_edge_count: bridge_edges.calls.len() as u32,
    };

    Ok(BridgeGraphOutput {
        schema_version,
        generated_at: analyzed_at.to_string(),
        language: language.to_string(),
        root: root_path.to_string(),
        repository,
        packages: Vec::new(),
        source_files,
        symbols,
        edges: bridge_edges,
        diagnostics: graph_json
            .get("diagnostics")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default(),
        stats,
    })
}

fn partition_shell_nodes(
    nodes: &[Value],
) -> Result<(Value, Vec<BridgeSourceFile>, Vec<BridgeSymbol>), String> {
    let mut repo_node = None;
    let mut source_files = Vec::new();
    let mut symbols = Vec::new();

    for node in nodes {
        let kind = node.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let id = node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let props = node.get("properties").cloned().unwrap_or(Value::Null);
        match kind {
            "repository" => repo_node = Some(node.clone()),
            "source-file" => source_files.push(BridgeSourceFile {
                id,
                path: node
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                package_id: None,
            }),
            "symbol" => {
                let mut extra_props = HashMap::new();
                if let Value::Object(map) = &props {
                    for (k, v) in map {
                        if !matches!(
                            k.as_str(),
                            "name" | "fileId" | "parentId" | "kind" | "sourcePath"
                        ) {
                            extra_props.insert(k.clone(), v.clone());
                        }
                    }
                }
                symbols.push(BridgeSymbol {
                    id,
                    name: node
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    kind: props
                        .get("symbolKind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("symbol")
                        .to_string(),
                    package_id: None,
                    file_id: props
                        .get("fileId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    source_path: props
                        .get("sourcePath")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    parent_id: None,
                    properties: extra_props,
                });
            }
            _ => {}
        }
    }
    Ok((
        repo_node.ok_or("Shell graph JSON 缺少 repository node")?,
        source_files,
        symbols,
    ))
}

fn convert_shell_edges(edges: &[Value]) -> BridgeEdges {
    let bridge_edges: Vec<BridgeEdge> = edges
        .iter()
        .map(|edge| {
            let props = edge.get("properties");
            BridgeEdge {
                source_id: edge
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                target_id: edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                kind: edge
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                confidence: props
                    .and_then(|p| p.get("confidence"))
                    .and_then(|v| v.as_f64()),
                reason: props
                    .and_then(|p| p.get("reason"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                properties: HashMap::new(),
            }
        })
        .collect();
    group_edges_by_kind(&bridge_edges)
}
