//! Rust GraphOutput → Bridge 格式转换
//!
//! 将 Rust project-model 产出的 graph JSON 转换为 GitNexus-RC 兼容格式。
//! 处理：
//! 1. Node 分类：repository / package / target / source-file / symbol
//! 2. Edge 归一化：source/target → sourceId/targetId，type → kind
//! 3. 从原始 edge properties 提升 confidence/reason 到顶层
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。

use serde_json::Value;
use std::collections::HashMap;

use crate::bridge_format::{
    group_edges_by_kind, BridgeEdge, BridgeEdges, BridgeGraphOutput, BridgePackage,
    BridgeRepository, BridgeSourceFile, BridgeStats, BridgeSymbol,
};

/// 将 Rust GraphOutput JSON 转换为 Bridge 格式
pub fn convert_rust_graph(
    graph_json: &Value,
    language: &str,
    root_path: &str,
    analyzed_at: &str,
) -> Result<BridgeGraphOutput, String> {
    let nodes = graph_json
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or("Rust graph JSON 缺少 nodes 数组")?;

    let edges = graph_json
        .get("edges")
        .and_then(|e| e.as_array())
        .ok_or("Rust graph JSON 缺少 edges 数组")?;

    let diagnostics = graph_json
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();

    let schema_version = graph_json
        .get("schemaVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("v0.3")
        .to_string();

    // 从 nodes 按 label 分类提取（传入 edges 用于解析 package_id）
    let (repo_node, packages, source_files, symbols) = partition_rust_nodes(nodes, edges)?;

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
    let bridge_edges = convert_rust_edges(edges);

    // 统计
    let stats = BridgeStats {
        node_count: nodes.len() as u32,
        edge_count: bridge_edges.total_count() as u32,
        symbol_count: symbols.len() as u32,
        source_file_count: source_files.len() as u32,
        package_count: packages.len() as u32,
        diagnostic_count: diagnostics.len() as u32,
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
        diagnostics,
        stats,
    })
}

/// 从 Rust nodes 按 label 分类。
/// edges 用于解析 source-file → package 关系（OWNS_SOURCE + HAS_TARGET 两跳）。
fn partition_rust_nodes(
    nodes: &[Value],
    edges: &[Value],
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

    // 构建 edge 查找表：target → package（HAS_TARGET: package → target）
    let target_to_pkg: HashMap<&str, &str> = edges
        .iter()
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("HAS_TARGET"))
        .filter_map(|e| {
            let source = e.get("source").and_then(|v| v.as_str())?;
            let target = e.get("target").and_then(|v| v.as_str())?;
            Some((target, source))
        })
        .collect();

    // 构建 edge 查找表：source-file → target（OWNS_SOURCE: target → source-file）
    let sf_to_target: HashMap<&str, &str> = edges
        .iter()
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("OWNS_SOURCE"))
        .filter_map(|e| {
            let source = e.get("source").and_then(|v| v.as_str())?;
            let target = e.get("target").and_then(|v| v.as_str())?;
            Some((target, source))
        })
        .collect();

    for node in nodes {
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

        match label {
            "repository" => {
                repo_node = Some(node.clone());
            }
            "package" | "target" => {
                let name = props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
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
            "source-file" => {
                // Rust graph 使用 sourcePath 字段（非 path）
                let path = props
                    .get("sourcePath")
                    .or_else(|| props.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // package_id 解析：source-file → target（OWNS_SOURCE）→ package（HAS_TARGET）
                let package_id = sf_to_target
                    .get(id.as_str())
                    .and_then(|target_id| target_to_pkg.get(target_id))
                    .map(|pkg_id| pkg_id.to_string());

                source_files.push(BridgeSourceFile {
                    id,
                    path,
                    package_id,
                });
            }
            "symbol" => {
                // 显式 symbol 节点：提取具体符号类型
                // Rust graph 使用 symbolKind 字段（非 kind）
                let name = props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let kind = props
                    .get("symbolKind")
                    .or_else(|| props.get("kind"))
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
            // 跳过 module / 其他结构节点，不计入 symbol
            // workspace 节点需要保留作为 CONTAINS_PACKAGE / CONTAINS_WORKSPACE 的端点
            "module" => {
                // 模块节点不属于 symbol 列表
            }
            // workspace 节点：需要作为 package 列表的一部分输出，
            // 以便 bridge adapter validator 能解析 CONTAINS_WORKSPACE / CONTAINS_PACKAGE 端点
            "workspace" => {
                let manifest_path = props
                    .get("manifestPath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                packages.push(BridgePackage {
                    id,
                    name: "workspace".to_string(),
                    manifest_path,
                });
            }
            // diagnostic 节点：需要作为 symbol 列表的一部分输出，
            // 以便 bridge adapter validator 能解析 ANNOTATES edge 的 source 端点
            "diagnostic" => {
                let code = props
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("diagnostic")
                    .to_string();
                let message = props
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                symbols.push(BridgeSymbol {
                    id,
                    name: code,
                    kind: "Diagnostic".to_string(),
                    package_id: None,
                    file_id: None,
                    source_path: None,
                    parent_id: None,
                    properties: {
                        let mut p = HashMap::new();
                        p.insert("message".to_string(), serde_json::Value::String(message));
                        p
                    },
                });
            }
            _ => {
                // 未知 label：保守跳过，不计入 symbol
                // 未来新增 label 时需在此显式处理
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

/// 转换 Rust edges：归一化端点字段 + 按类型分组
fn convert_rust_edges(edges: &[Value]) -> BridgeEdges {
    let all_edges: Vec<BridgeEdge> = edges
        .iter()
        .map(|edge| {
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
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 提取 confidence 和 reason（对齐 GitNexus-RC GraphRelationship 字段）
            let confidence = edge
                .get("properties")
                .and_then(|p| p.get("confidence"))
                .and_then(|v| v.as_f64());
            let reason = edge
                .get("properties")
                .and_then(|p| p.get("reason"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut properties = HashMap::new();
            if let Some(props) = edge.get("properties").and_then(|v| v.as_object()) {
                for (k, v) in props {
                    // 跳过已提升到顶层的字段
                    if k != "type" && k != "confidence" && k != "reason" {
                        properties.insert(k.clone(), v.clone());
                    }
                }
            }

            BridgeEdge {
                source_id,
                target_id,
                kind,
                confidence,
                reason,
                properties,
            }
        })
        .collect();

    group_edges_by_kind(&all_edges)
}
