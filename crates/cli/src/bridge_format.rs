//! Bridge 格式适配器 — 共享类型定义与边分组逻辑
//!
//! 语言特定转换逻辑位于：
//! - `rust_bridge.rs`：Rust GraphOutput → Bridge 格式
//! - `cangjie_bridge.rs`：Cangjie CangjieGraphOutput → Bridge 格式
//!
//! Stop-line: 不修改 GitNexus-RC，不修改 GitNexus-RC-Tool。
//! 本文件仅定义共享类型与通用边分组逻辑，不改变 graph 语义。

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

// 从语言特定模块重新导出公开 API，保持 `bridge_format::convert_*` 调用路径不变
pub use crate::arkts_bridge::convert_arkts_graph;
pub use crate::c_bridge::convert_c_graph;
pub use crate::cangjie_bridge::convert_cangjie_graph;
pub use crate::rust_bridge::convert_rust_graph;

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
    pub source_path: Option<String>,
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
    /// 边置信度 (0.0-1.0)，对齐 GitNexus-RC RelationshipType
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// 边解析原因，对齐 GitNexus-RC reason 字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

// Rust/Cangjie 语言特定转换逻辑已提取至：
// - rust_bridge.rs: convert_rust_graph() + partition_rust_nodes() + convert_rust_edges()
// - cangjie_bridge.rs: convert_cangjie_graph() + partition_cangjie_nodes() + convert_cangjie_edges()
//
// 通过 pub use 重新导出保持调用路径不变。

// ============================================================
// 通用：按 kind 分组 edges
// ============================================================

pub(crate) fn group_edges_by_kind(edges: &[BridgeEdge]) -> BridgeEdges {
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
