//! Cangjie CangjieGraphOutput → Bridge 格式转换
//!
//! 将 Cangjie cangjie crate 产出的 graph JSON 转换为 GitNexus-RC 兼容格式。
//! 处理：
//! 1. Node 分类：repository / package / sourceFile / symbol / callableSource / diagnostic
//! 2. Cangjie graph 已使用 sourceId/targetId 端点字段，无需归一化
//! 3. 统计 diagnostic nodes 数量
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgePackage,
    BridgeRepository, BridgeSourceFile, BridgeStats, BridgeSymbol,
};

/// 将 Cangjie CangjieGraphOutput JSON 转换为 Bridge 格式
pub fn convert_cangjie_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("Cangjie graph JSON 缺少 nodes 数组")?;

    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("Cangjie graph JSON 缺少 edges 数组")?;

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("v1.0.0")
        .to_string();

    let (repo_node, packages, source_files, symbols) = partition_cangjie_nodes(nodes)?;

    let repository = BridgeRepository {
        id: repo_node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("repo:unknown")
            .to_string(),
        path: root_path.to_string(),
    };

    let bridge_edges = convert_cangjie_edges(edges);

    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: packages.len() as u32,
        diagnostic_count: 0, // 下面从 nodes 重新统计
        call_edge_count: bridge_edges.uses.len() as u32,
    };

    // 统计 diagnostics from nodes
    let diag_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("diagnostic"))
        .count();

    Ok(BridgeGraphOutput {
        schema_version,
        generated_at: analyzed_at.to_string(),
        language: language.to_string(),
        root: root_path.to_string(),
        repository,
        packages,
        source_files,
        symbols,
        edges: bridge_edges,
        diagnostics: Vec::new(),
        stats: BridgeStats {
            diagnostic_count: diag_count as u32,
            ..stats
        },
    })
}

/// 从 Cangjie nodes 按 kind 分类
fn partition_cangjie_nodes(
    nodes: &[Value],
) -> Result<
    (
        Value,
        Vec<BridgePackage>,
        Vec<BridgeSourceFile>,
        Vec<BridgeSymbol>,
    ),
    String,
> {
    let mut repo_node: Option<Value> = None;
    let mut packages = Vec::new();
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
        let label = node
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let props = node.get("properties").cloned().unwrap_or(Value::Null);

        match kind {
            "repository" => {
                repo_node = Some(node.clone());
            }
            "package" => {
                let name = label.clone();
                let manifest_path = props
                    .get("manifestPath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                packages.push(BridgePackage {
                    id,
                    name,
                    manifest_path,
                });
            }
            "sourceFile" => {
                let path = label.clone();
                let package_id = props
                    .get("packageId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                source_files.push(BridgeSourceFile {
                    id,
                    path,
                    package_id,
                });
            }
            "symbol" | "callableSource" => {
                let name = label.clone();
                let mut extra_props = HashMap::new();
                if let Value::Object(map) = &props {
                    for (k, v) in map {
                        extra_props.insert(k.clone(), v.clone());
                    }
                }
                let symbol_kind = props
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or(kind)
                    .to_string();
                // Cangjie 没有 file_id/parent_id 顶层字段
                symbols.push(BridgeSymbol {
                    id,
                    name,
                    kind: symbol_kind,
                    package_id: None,
                    file_id: None,
                    parent_id: None,
                    properties: extra_props,
                });
            }
            _ => {
                // diagnostic / 其他类型跳过符号列表
            }
        }
    }

    Ok((
        repo_node.unwrap_or(Value::Null),
        packages,
        source_files,
        symbols,
    ))
}

/// 转换 Cangjie edges：Cangjie 已使用 sourceId/targetId，无需归一化端点字段
fn convert_cangjie_edges(edges: &[Value]) -> BridgeEdges {
    let all_edges: Vec<BridgeEdge> = edges
        .iter()
        .map(|edge| {
            let source_id = edge
                .get("sourceId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_id = edge
                .get("targetId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let kind = edge
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            BridgeEdge {
                source_id,
                target_id,
                kind,
                confidence: None,
                reason: None,
                properties: HashMap::new(),
            }
        })
        .collect();

    group_edges_by_kind(&all_edges)
}
