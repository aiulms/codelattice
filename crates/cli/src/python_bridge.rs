//! Python GraphOutput → Bridge 格式转换
//!
//! 将 Python project model 产出的 graph JSON 转换为 GitNexus-RC 兼容格式。
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgeRepository,
    BridgeSourceFile, BridgeStats, BridgeSymbol,
};

/// 将 Python GraphOutput JSON 转换为 Bridge 格式
pub fn convert_python_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("Python graph JSON 缺少 nodes 数组")?;

    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("Python graph JSON 缺少 edges 数组")?;

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("v0.1")
        .to_string();

    let (repo_node, source_files, symbols) = partition_python_nodes(nodes)?;

    let repository = BridgeRepository {
        id: repo_node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("repo:unknown")
            .to_string(),
        path: root_path.to_string(),
    };

    let bridge_edges = convert_python_edges(edges);

    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: 0,
        diagnostic_count: 0,
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
        diagnostics: Vec::new(),
        stats,
    })
}

/// 将 Python nodes 分区为 repository / source-files / symbols
fn partition_python_nodes(
    nodes: &[Value],
) -> Result<(Value, Vec<BridgeSourceFile>, Vec<BridgeSymbol>), String> {
    let mut repo_node: Option<Value> = None;
    let mut source_files = Vec::new();
    let mut symbols = Vec::new();

    for node in nodes {
        let kind = node
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let id = node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let props = node.get("properties").cloned().unwrap_or(Value::Null);

        match kind {
            "repository" => {
                repo_node = Some(node.clone());
            }
            "source-file" => {
                let label = node
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                source_files.push(BridgeSourceFile {
                    id,
                    path: label,
                    package_id: None,
                });
            }
            "symbol" => {
                let name = node
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let sym_kind = props
                    .get("symbolKind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("symbol")
                    .to_string();
                let file_id = props
                    .get("fileId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let parent_id = props
                    .get("parentName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let source_path = props
                    .get("sourcePath")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

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
                    name,
                    kind: sym_kind,
                    package_id: None,
                    file_id,
                    source_path,
                    parent_id,
                    properties: extra_props,
                });
            }
            _ => {}
        }
    }

    let repo = repo_node.ok_or("Python graph JSON 缺少 repository node")?;
    Ok((repo, source_files, symbols))
}

/// 将 Python edges 转换为 Bridge 格式并按 kind 分组
fn convert_python_edges(edges: &[Value]) -> BridgeEdges {
    let bridge_edges: Vec<BridgeEdge> = edges
        .iter()
        .map(|edge| {
            let kind = edge
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN");

            let source_id = edge
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_id = edge
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let props = edge.get("properties");
            let confidence = props
                .and_then(|p| p.get("confidence"))
                .and_then(|v| v.as_f64());
            let reason = props
                .and_then(|p| p.get("reason"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            BridgeEdge {
                source_id,
                target_id,
                kind: kind.to_string(),
                confidence,
                reason,
                properties: HashMap::new(),
            }
        })
        .collect();

    group_edges_by_kind(&bridge_edges)
}
