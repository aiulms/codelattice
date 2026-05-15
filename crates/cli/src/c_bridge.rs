//! C GraphOutput → Bridge 格式转换
//!
//! 将 C project model 产出的 graph JSON 转换为 GitNexus-RC 兼容格式。
//! 处理：
//! 1. Node 分类：repository / source-file / header-file / symbol
//! 2. Edge 归一化：source/target → sourceId/targetId，type → kind
//! 3. 从原始 edge properties 提升 confidence/reason 到顶层
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgeRepository,
    BridgeSourceFile, BridgeStats, BridgeSymbol,
};

/// 将 C GraphOutput JSON 转换为 Bridge 格式
pub fn convert_c_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("C graph JSON 缺少 nodes 数组")?;

    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("C graph JSON 缺少 edges 数组")?;

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("v0.1")
        .to_string();

    // 从 nodes 按 label 分类提取
    let (repo_node, source_files, symbols) = partition_c_nodes(nodes)?;

    // 构建 repository 信息
    let repository = BridgeRepository {
        id: repo_node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("repo:unknown")
            .to_string(),
        path: root_path.to_string(),
    };

    // 转换 edges：归一化端点字段 + 按类型分组
    let bridge_edges = convert_c_edges(edges);

    // 统计
    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: 0, // C project 无 package 概念
        diagnostic_count: 0,
        call_edge_count: bridge_edges.calls.len() as u32,
    };

    Ok(BridgeGraphOutput {
        schema_version,
        generated_at: analyzed_at.to_string(),
        language: language.to_string(),
        root: root_path.to_string(),
        repository,
        packages: Vec::new(), // C project 无 package
        source_files,
        symbols,
        edges: bridge_edges,
        diagnostics: Vec::new(),
        stats,
    })
}

/// 将 C nodes 分区为 repository / source-files / symbols
fn partition_c_nodes(
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
        let label = node
            .get("label")
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
            "source-file" | "header-file" => {
                // C graph uses path field
                let path = props
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                source_files.push(BridgeSourceFile {
                    id,
                    path,
                    package_id: None,
                });
            }
            "symbol" => {
                let name = props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let kind = props
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("symbol")
                    .to_string();
                let file_id = props
                    .get("fileId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let parent_id = props
                    .get("parentId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut extra_props = HashMap::new();
                if let Value::Object(map) = &props {
                    for (k, v) in map {
                        if k != "name" && k != "fileId" && k != "parentId" && k != "kind" {
                            extra_props.insert(k.clone(), v.clone());
                        }
                    }
                }

                symbols.push(BridgeSymbol {
                    id,
                    name,
                    kind,
                    package_id: None,
                    file_id,
                    source_path: None,
                    parent_id,
                    properties: extra_props,
                });
            }
            _ => {
                // 跳过未知节点类型
            }
        }
    }

    let repo = repo_node.ok_or("C graph JSON 缺少 repository 节点")?;

    Ok((repo, source_files, symbols))
}

/// 将 C edges 转换为 BridgeEdge 并按类型分组
fn convert_c_edges(edges: &[Value]) -> BridgeEdges {
    let mut bridge_edges: Vec<BridgeEdge> = Vec::new();

    for edge in edges {
        let source = edge.get("source").and_then(|v| v.as_str());
        let target = edge.get("target").and_then(|v| v.as_str()).unwrap_or("");
        let type_ = edge
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        let props = edge.get("properties").cloned();

        // 提取 confidence 和 reason
        let confidence = props
            .as_ref()
            .and_then(|p| p.get("confidence"))
            .and_then(|v| v.as_f64())
            .map(|c| c.min(1.0).max(0.0));

        let reason = props
            .as_ref()
            .and_then(|p| p.get("reason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        bridge_edges.push(BridgeEdge {
            source_id: source.unwrap_or("").to_string(),
            target_id: target.to_string(),
            kind: type_.to_string(),
            confidence,
            reason,
            properties: HashMap::new(),
        });
    }

    group_edges_by_kind(&bridge_edges)
}
