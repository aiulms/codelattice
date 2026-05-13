//! ArkTS/TypeScript TsGraphOutput → Bridge 格式转换
//!
//! 将 ArkTS/TypeScript crate 产出的 graph JSON 转换为 GitNexus-RC 兼容格式。
//! 处理：
//! 1. Node 分类：repository / package / sourceFile / symbol
//! 2. Edge 归一化：source/target → sourceId/targetId
//! 3. ArkTS augment 节点 (component/stateProperty/buildMethod) 映射为 symbol
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgePackage,
    BridgeRepository, BridgeSourceFile, BridgeStats, BridgeSymbol,
};

/// 将 ArkTS/TypeScript TsGraphOutput JSON 转换为 Bridge 格式
pub fn convert_arkts_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("ArkTS graph JSON 缺少 nodes 数组")?;

    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("ArkTS graph JSON 缺少 edges 数组")?;

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("v0.1.0")
        .to_string();

    let (repo_node, packages, source_files, symbols) = partition_arkts_nodes(nodes)?;

    let repository = BridgeRepository {
        id: repo_node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("repo:unknown")
            .to_string(),
        path: root_path.to_string(),
    };

    let bridge_edges = convert_arkts_edges(edges);

    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: packages.len() as u32,
        diagnostic_count: 0,
        call_edge_count: bridge_edges.calls.len() as u32,
    };

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
        stats,
    })
}

/// 从 ArkTS nodes 按 kind 分类
fn partition_arkts_nodes(
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
            "symbol" | "component" | "stateProperty" | "buildMethod" => {
                let name = label.clone();
                let mut extra_props = HashMap::new();
                if let Value::Object(map) = &props {
                    for (k, v) in map {
                        extra_props.insert(k.clone(), v.clone());
                    }
                }
                // Determine the bridge symbol kind:
                // - ArkTS augment nodes use "arktsKind" (component, buildMethod)
                // - Base TS symbol nodes use "symbolKind" (method, function, class, etc.)
                let symbol_kind = props
                    .get("arktsKind")
                    .or_else(|| props.get("symbolKind"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(kind)
                    .to_string();
                let file_id = props
                    .get("fileId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                // Derive sourcePath from fileId (strip "file:" prefix)
                let source_path = file_id
                    .as_ref()
                    .map(|fid| fid.strip_prefix("file:").unwrap_or(fid).to_string());
                symbols.push(BridgeSymbol {
                    id,
                    name,
                    kind: symbol_kind,
                    package_id: None,
                    file_id,
                    source_path,
                    parent_id: None,
                    properties: extra_props,
                });
            }
            _ => {
                // Unknown node types — skip
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

/// 转换 ArkTS edges：source/target → sourceId/targetId 归一化
fn convert_arkts_edges(edges: &[Value]) -> BridgeEdges {
    let all_edges: Vec<BridgeEdge> = edges
        .iter()
        .map(|edge| {
            // ArkTS uses "source" / "target" — normalize to "sourceId" / "targetId"
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
