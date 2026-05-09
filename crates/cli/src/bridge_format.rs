//! Bridge 格式适配器 — 将 Rust/Cangjie graph output 转换为 GitNexus-RC 兼容格式
//!
//! 归一化处理：
//! 1. Edge 端点字段：source/target → sourceId/targetId，type → kind
//! 2. Node 显式 kind 字段：从 label 推断（Rust）或保留 kind（Cangjie）
//! 3. 顶层结构重组：repository/packages/sourceFiles/symbols + edges 分组
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。
//! 本文件仅做格式转换（adapter），不改变 graph 语义。

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

// ============================================================
// Bridge 输出类型定义
// ============================================================

/// GitNexus-RC 兼容的仓库信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRepository {
    pub id: String,
    pub path: String,
}

/// GitNexus-RC 兼容的包信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePackage {
    pub id: String,
    pub name: String,
    pub manifest_path: String,
}

/// GitNexus-RC 兼容的源文件信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSourceFile {
    pub id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
}

/// GitNexus-RC 兼容的符号信息（含显式 kind 字段）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSymbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, Value>,
}

/// GitNexus-RC 兼容的边（归一化端点字段）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEdge {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "targetId")]
    pub target_id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, Value>,
}

/// 分组边
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEdges {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub defines: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accesses: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub designations: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contains: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub owns: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub annotates: Vec<BridgeEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other: Vec<BridgeEdge>,
}

impl BridgeEdges {
    pub fn total_count(&self) -> usize {
        self.calls.len()
            + self.defines.len()
            + self.uses.len()
            + self.accesses.len()
            + self.designations.len()
            + self.imports.len()
            + self.contains.len()
            + self.owns.len()
            + self.annotates.len()
            + self.other.len()
    }
}

/// GitNexus-RC 兼容的顶层输出
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeGraphOutput {
    pub schema_version: String,
    pub generated_at: String,
    pub language: String,
    pub root: String,
    pub repository: BridgeRepository,
    pub packages: Vec<BridgePackage>,
    pub source_files: Vec<BridgeSourceFile>,
    pub symbols: Vec<BridgeSymbol>,
    pub edges: BridgeEdges,
    pub diagnostics: Vec<Value>,
    pub stats: BridgeStats,
}

/// Bridge 格式的统计信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStats {
    pub node_count: u32,
    pub edge_count: u32,
    pub symbol_count: u32,
    pub source_file_count: u32,
    pub package_count: u32,
    pub diagnostic_count: u32,
    pub call_edge_count: u32,
}

// ============================================================
// Rust GraphOutput → Bridge 格式转换
// ============================================================

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

    let mut pkg_name_by_id: HashMap<String, String> = HashMap::new();

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
                if label == "package" {
                    pkg_name_by_id.insert(id.clone(), name.clone());
                }
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
                // 显式 symbol 节点：提取 kind 来自 properties.kind
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
                    parent_id,
                    properties: extra_props,
                });
            }
            // 跳过 diagnostic / workspace / 其他结构节点，不计入 symbol
            "diagnostic" | "workspace" | "module" => {
                // 这些是结构/诊断节点，不属于 symbol 列表
            }
            _ => {
                // 未知 label：保守跳过，不计入 symbol
                // 未来新增 label 时需在此显式处理
            }
        }
    }

    // 为没有 manifest_path 的 target 补充信息
    for pkg in &mut packages {
        if pkg.manifest_path.is_empty() {
            pkg.manifest_path = String::new();
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

            let mut properties = HashMap::new();
            if let Some(props) = edge.get("properties").and_then(|v| v.as_object()) {
                for (k, v) in props {
                    if k != "type" {
                        properties.insert(k.clone(), v.clone());
                    }
                }
            }

            BridgeEdge {
                source_id,
                target_id,
                kind,
                properties,
            }
        })
        .collect();

    group_edges_by_kind(&all_edges)
}

// ============================================================
// Cangjie CangjieGraphOutput → Bridge 格式转换
// ============================================================

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
        diagnostic_count: 0, // Cangjie diagnostics count from nodes
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
                properties: HashMap::new(),
            }
        })
        .collect();

    group_edges_by_kind(&all_edges)
}

// ============================================================
// 通用：按 kind 分组 edges
// ============================================================

fn group_edges_by_kind(edges: &[BridgeEdge]) -> BridgeEdges {
    let mut calls = Vec::new();
    let mut defines = Vec::new();
    let mut uses = Vec::new();
    let mut accesses = Vec::new();
    let mut designations = Vec::new();
    let mut imports = Vec::new();
    let mut contains = Vec::new();
    let mut owns = Vec::new();
    let mut annotates = Vec::new();
    let mut other = Vec::new();

    for edge in edges {
        match edge.kind.as_str() {
            "CALLS" | "calls" => calls.push(edge.clone()),
            "DEFINES" | "defines" => defines.push(edge.clone()),
            "USES" | "uses" => uses.push(edge.clone()),
            "ACCESSES" | "accesses" => accesses.push(edge.clone()),
            "DESIGNATION" | "designations" => designations.push(edge.clone()),
            "IMPORTS" | "imports" => imports.push(edge.clone()),
            "CONTAINS_PACKAGE" | "CONTAINS_WORKSPACE" | "HAS_TARGET" | "containsPackage"
            | "containsWorkspace" | "hasTarget" => contains.push(edge.clone()),
            "OWNS_SOURCE" | "ownsSource" => owns.push(edge.clone()),
            "ANNOTATES" | "annotates" => annotates.push(edge.clone()),
            "HAS_PARENT" | "hasParent" | "RESOLVES_TO" | "resolvesTo" | "Modifies" | "modifies" => {
                other.push(edge.clone())
            }
            _ => other.push(edge.clone()),
        }
    }

    BridgeEdges {
        calls,
        defines,
        uses,
        accesses,
        designations,
        imports,
        contains,
        owns,
        annotates,
        other,
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小 Rust graph JSON 用于测试
    /// 使用与实际 Rust GraphOutput 一致的 label 约定：
    ///   - repository / package / target / source-file → 结构节点
    ///   - symbol → 显式 symbol 节点（properties.kind 编码具体类型）
    ///   - diagnostic → 诊断节点（不计入 symbol）
    fn make_rust_graph_json() -> Value {
        serde_json::json!({
            "schemaVersion": "v0.3",
            "generatedAt": "2026-05-09T00:00:00Z",
            "nodes": [
                {"id": "repo:test", "label": "repository", "properties": {"name": "test-repo"}},
                {"id": "pkg:test", "label": "package", "properties": {"name": "test-pkg", "manifestPath": "Cargo.toml"}},
                {"id": "sf:src/lib.rs", "label": "source-file", "properties": {"path": "src/lib.rs"}},
                {"id": "sym:test::func", "label": "symbol", "properties": {"name": "func", "kind": "function"}}
            ],
            "edges": [
                {"type": "CALLS", "source": "sym:test::caller", "target": "sym:test::func", "properties": {"confidence": 0.9}},
                {"type": "DEFINES", "source": "sf:src/lib.rs", "target": "sym:test::func", "properties": {}}
            ],
            "diagnostics": [],
            "stats": {"nodeCount": 4, "edgeCount": 2, "symbolCount": 1}
        })
    }

    /// 构造一个最小 Cangjie graph JSON 用于测试
    fn make_cangjie_graph_json() -> Value {
        serde_json::json!({
            "nodes": [
                {"id": "repo:test", "kind": "repository", "label": "test-repo", "properties": null},
                {"id": "pkg:test", "kind": "package", "label": "test-pkg", "properties": {"manifestPath": "cjpm.toml"}},
                {"id": "sf:src/main.cj", "kind": "sourceFile", "label": "src/main.cj", "properties": {"packageId": "pkg:test"}},
                {"id": "sym:test::main", "kind": "symbol", "label": "main", "properties": {"kind": "function"}}
            ],
            "edges": [
                {"kind": "uses", "sourceId": "sym:test::caller", "targetId": "sym:test::main"},
                {"kind": "defines", "sourceId": "sf:src/main.cj", "targetId": "sym:test::main"}
            ]
        })
    }

    #[test]
    fn convert_rust_graph_produces_bridge_output() {
        let input = make_rust_graph_json();
        let result =
            convert_rust_graph(&input, "rust", "/test/project", "2026-05-09T00:00:00Z").unwrap();

        assert_eq!(result.language, "rust");
        assert_eq!(result.root, "/test/project");
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, "test-pkg");
        assert_eq!(result.source_files.len(), 1);
        assert_eq!(result.symbols.len(), 1);

        // edges 已归一化
        assert!(result.edges.calls.len() + result.edges.defines.len() == 2);
    }

    #[test]
    fn convert_cangjie_graph_produces_bridge_output() {
        let input = make_cangjie_graph_json();
        let result =
            convert_cangjie_graph(&input, "cangjie", "/test/project", "2026-05-09T00:00:00Z")
                .unwrap();

        assert_eq!(result.language, "cangjie");
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.source_files.len(), 1);
        assert_eq!(result.symbols.len(), 1);

        // Cangjie edges 已按 kind 分组
        assert_eq!(result.edges.uses.len(), 1);
        assert_eq!(result.edges.defines.len(), 1);
    }

    #[test]
    fn bridge_edges_normalize_endpoint_fields() {
        let input = make_rust_graph_json();
        let result = convert_rust_graph(&input, "rust", "/test", "2026-05-09T00:00:00Z").unwrap();

        // CALLS edge 端点应归一化为 sourceId/targetId
        if let Some(call) = result.edges.calls.first() {
            assert_eq!(call.source_id, "sym:test::caller");
            assert_eq!(call.target_id, "sym:test::func");
            assert_eq!(call.kind, "CALLS");
        } else {
            panic!("expected at least one CALLS edge");
        }
    }

    #[test]
    fn bridge_stats_match_node_counts() {
        let input = make_rust_graph_json();
        let result = convert_rust_graph(&input, "rust", "/test", "2026-05-09T00:00:00Z").unwrap();

        assert_eq!(result.stats.node_count, 4);
        assert_eq!(result.stats.symbol_count, 1);
        assert_eq!(result.stats.source_file_count, 1);
        assert_eq!(result.stats.package_count, 1);
    }

    #[test]
    fn missing_nodes_field_returns_error() {
        let bad_input = serde_json::json!({"schemaVersion": "v0.3"});
        let result = convert_rust_graph(&bad_input, "rust", "/test", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("nodes"));
    }

    #[test]
    fn rust_source_file_gets_package_id_from_edges() {
        // 验证通过 OWNS_SOURCE + HAS_TARGET edges 解析 package_id
        let input = serde_json::json!({
            "schemaVersion": "v0.3",
            "generatedAt": "2026-05-09T00:00:00Z",
            "nodes": [
                {"id": "repo:test", "label": "repository", "properties": {"name": "test-repo"}},
                {"id": "pkg:Cargo.toml", "label": "package", "properties": {"name": "test-pkg", "manifestPath": "Cargo.toml"}},
                {"id": "target:test::lib", "label": "target", "properties": {"name": "test"}},
                {"id": "file:src/lib.rs", "label": "source-file", "properties": {"path": "src/lib.rs"}},
                {"id": "sym:test::func", "label": "symbol", "properties": {"name": "func", "kind": "function"}}
            ],
            "edges": [
                {"type": "HAS_TARGET", "source": "pkg:Cargo.toml", "target": "target:test::lib"},
                {"type": "OWNS_SOURCE", "source": "target:test::lib", "target": "file:src/lib.rs"},
                {"type": "DEFINES", "source": "file:src/lib.rs", "target": "sym:test::func"}
            ],
            "diagnostics": [],
            "stats": {"nodeCount": 5, "edgeCount": 3, "symbolCount": 1}
        });
        let result = convert_rust_graph(&input, "rust", "/test", "").unwrap();

        assert_eq!(result.source_files.len(), 1);
        assert_eq!(
            result.source_files[0].package_id,
            Some("pkg:Cargo.toml".to_string()),
            "source-file 的 package_id 应通过 edges 解析为 pkg:Cargo.toml"
        );
    }

    #[test]
    fn cangjie_callable_source_as_symbol() {
        let input = serde_json::json!({
            "nodes": [
                {"id": "repo:test", "kind": "repository", "label": "repo", "properties": null},
                {"id": "pkg:test", "kind": "package", "label": "pkg", "properties": {"manifestPath": "cjpm.toml"}},
                {"id": "sym:test::callable", "kind": "callableSource", "label": "MyClass.init", "properties": {"kind": "init"}}
            ],
            "edges": []
        });
        let result = convert_cangjie_graph(&input, "cangjie", "/test", "").unwrap();
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].kind, "init");
    }

    #[test]
    fn cangjie_diagnostic_not_counted_as_symbol() {
        let input = serde_json::json!({
            "nodes": [
                {"id": "repo:test", "kind": "repository", "label": "repo", "properties": null},
                {"id": "pkg:test", "kind": "package", "label": "pkg", "properties": {"manifestPath": "cjpm.toml"}},
                {"id": "diag:1", "kind": "diagnostic", "label": "warning", "properties": {"message": "test"}},
                {"id": "sym:test::func", "kind": "symbol", "label": "func", "properties": {"kind": "function"}}
            ],
            "edges": []
        });
        let result = convert_cangjie_graph(&input, "cangjie", "/test", "").unwrap();
        assert_eq!(result.symbols.len(), 1); // diagnostic 不计入 symbols
        assert_eq!(result.stats.diagnostic_count, 1);
    }
}
