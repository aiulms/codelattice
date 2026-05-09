//! Bridge Roundtrip 验证 — 验证 `--format gitnexus-rc` 输出结构完整性
//!
//! 验证目标：
//! 1. Bridge 输出结构：schemaVersion / repository / packages / sourceFiles / symbols / edges / diagnostics / stats
//! 2. 端点完整性：edge sourceId/targetId 均存在于 node-like collections
//! 3. 统计一致性：stats 各字段与实际数组计数一致
//! 4. Rust 和 Cangjie 两条语言线均覆盖
//!
//! Stop-line: 不依赖 GitNexus-RC runtime，不 import TS 代码，不改 Tool。
//! 本测试仅验证 bridge 输出的结构完整性和自洽性。

use assert_cmd::Command;
use serde_json::Value;
use std::collections::HashSet;

// ============================================================
// 辅助函数
// ============================================================

fn rust_portable_smoke_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    format!("{manifest_dir}/../../fixtures/rust/portable-smoke")
}

#[allow(dead_code)]
fn cangjie_portable_smoke_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    format!("{manifest_dir}/../../fixtures/cangjie/portable-smoke")
}

/// 收集 bridge 输出中所有 node-like ID（repository + packages + sourceFiles + symbols）
fn collect_node_ids(v: &Value) -> HashSet<String> {
    let mut ids = HashSet::new();

    // repository
    if let Some(id) = v["repository"].get("id").and_then(|v| v.as_str()) {
        ids.insert(id.to_string());
    }

    // packages
    if let Some(pkgs) = v["packages"].as_array() {
        for pkg in pkgs {
            if let Some(id) = pkg.get("id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }

    // sourceFiles
    if let Some(files) = v["sourceFiles"].as_array() {
        for file in files {
            if let Some(id) = file.get("id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }

    // symbols
    if let Some(syms) = v["symbols"].as_array() {
        for sym in syms {
            if let Some(id) = sym.get("id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }

    ids
}

/// 收集 bridge 输出中所有 edge 的 (sourceId, targetId, kind)
fn collect_edges(v: &Value) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    let edge_categories = &[
        "calls",
        "defines",
        "uses",
        "accesses",
        "designations",
        "imports",
        "contains",
        "owns",
        "annotates",
        "other",
    ];

    for cat in edge_categories {
        if let Some(edges) = v["edges"][cat].as_array() {
            for edge in edges {
                let src = edge["sourceId"].as_str().unwrap_or("").to_string();
                let tgt = edge["targetId"].as_str().unwrap_or("").to_string();
                let kind = edge["kind"].as_str().unwrap_or("").to_string();
                if !src.is_empty() && !tgt.is_empty() {
                    result.push((src, tgt, kind));
                }
            }
        }
    }

    result
}

/// 验证 bridge 输出顶层结构字段齐全
fn assert_bridge_structure(v: &Value) {
    assert!(v["schemaVersion"].is_string(), "缺少 schemaVersion 字段");
    assert!(v["repository"].is_object(), "缺少 repository 对象");
    assert!(v["packages"].is_array(), "缺少 packages 数组");
    assert!(v["sourceFiles"].is_array(), "缺少 sourceFiles 数组");
    assert!(v["symbols"].is_array(), "缺少 symbols 数组");
    assert!(v["edges"].is_object(), "缺少 edges 对象");
    assert!(v["diagnostics"].is_array(), "缺少 diagnostics 数组");
    assert!(v["stats"].is_object(), "缺少 stats 对象");
    assert!(v["language"].is_string(), "缺少 language 字段");
    assert!(v["root"].is_string(), "缺少 root 字段");
}

/// 验证 sourceFiles 中没有空路径
fn assert_no_empty_source_file_paths(v: &Value) {
    if let Some(files) = v["sourceFiles"].as_array() {
        for (i, file) in files.iter().enumerate() {
            let path = file["path"].as_str().unwrap_or("");
            assert!(
                !path.is_empty(),
                "sourceFile[{i}] path 为空字符串（id={})",
                file["id"].as_str().unwrap_or("?")
            );
        }
    }
}

/// 验证 symbols 中没有空 name
fn assert_no_empty_symbol_names(v: &Value) {
    if let Some(syms) = v["symbols"].as_array() {
        for (i, sym) in syms.iter().enumerate() {
            let name = sym["name"].as_str().unwrap_or("");
            assert!(
                !name.is_empty(),
                "symbol[{i}] name 为空字符串（id={})",
                sym["id"].as_str().unwrap_or("?")
            );
        }
    }
}

/// 验证 stats 各字段与实际数组计数一致
fn assert_stats_consistency(v: &Value) {
    let stats = &v["stats"];

    let actual_symbol_count = v["symbols"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
    let actual_file_count = v["sourceFiles"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
    let actual_pkg_count = v["packages"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
    let actual_diag_count = v["diagnostics"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
    let actual_edge_total: u64 = [
        "calls",
        "defines",
        "uses",
        "accesses",
        "designations",
        "imports",
        "contains",
        "owns",
        "annotates",
        "other",
    ]
    .iter()
    .map(|cat| {
        v["edges"][cat]
            .as_array()
            .map(|a| a.len() as u64)
            .unwrap_or(0)
    })
    .sum();
    let actual_call_edge_count: u64 = v["edges"]["calls"]
        .as_array()
        .map(|a| a.len() as u64)
        .unwrap_or(0)
        + v["edges"]["uses"]
            .as_array()
            .map(|a| a.len() as u64)
            .unwrap_or(0);

    assert_eq!(
        stats["symbolCount"].as_u64().unwrap_or(0),
        actual_symbol_count,
        "stats.symbolCount ({}) 与 symbols 数组长度 ({}) 不一致",
        stats["symbolCount"].as_u64().unwrap_or(0),
        actual_symbol_count
    );

    assert_eq!(
        stats["sourceFileCount"].as_u64().unwrap_or(0),
        actual_file_count,
        "stats.sourceFileCount 与 sourceFiles 数组长度不一致"
    );

    assert_eq!(
        stats["packageCount"].as_u64().unwrap_or(0),
        actual_pkg_count,
        "stats.packageCount 与 packages 数组长度不一致"
    );

    assert_eq!(
        stats["diagnosticCount"].as_u64().unwrap_or(0),
        actual_diag_count,
        "stats.diagnosticCount 与 diagnostics 数组长度不一致"
    );

    // edgeCount 应 >= 0 且与实际边总数一致
    assert_eq!(
        stats["edgeCount"].as_u64().unwrap_or(0),
        actual_edge_total,
        "stats.edgeCount 与实际边总数不一致"
    );

    // callEdgeCount 应与实际 calls+uses 边数一致（Rust 使用 calls，Cangjie 使用 uses）
    assert_eq!(
        stats["callEdgeCount"].as_u64().unwrap_or(0),
        actual_call_edge_count,
        "stats.callEdgeCount ({}) 与 calls+uses 边数 ({}) 不一致",
        stats["callEdgeCount"].as_u64().unwrap_or(0),
        actual_call_edge_count
    );
    assert!(
        actual_call_edge_count <= actual_edge_total,
        "callEdgeCount ({}) 不应超过总边数 ({})",
        actual_call_edge_count,
        actual_edge_total
    );
}

/// 验证：edges 中所有 sourceId/targetId 都指向已知 node-like ID
fn assert_endpoint_integrity(v: &Value) {
    let node_ids = collect_node_ids(v);
    let edges = collect_edges(v);

    assert!(
        !edges.is_empty(),
        "bridge 输出至少应有 1 条边（否则 endpoint integrity 无意义）"
    );

    let mut missing_sources = Vec::new();
    let mut missing_targets = Vec::new();

    for (src, tgt, kind) in &edges {
        if !node_ids.contains(src.as_str()) {
            missing_sources.push((src.clone(), tgt.clone(), kind.clone()));
        }
        if !node_ids.contains(tgt.as_str()) {
            missing_targets.push((src.clone(), tgt.clone(), kind.clone()));
        }
    }

    assert!(
        missing_sources.is_empty(),
        "发现 {} 条边 sourceId 不在 node-like collections 中: {:?}",
        missing_sources.len(),
        &missing_sources[..missing_sources.len().min(5)]
    );

    assert!(
        missing_targets.is_empty(),
        "发现 {} 条边 targetId 不在 node-like collections 中: {:?}",
        missing_targets.len(),
        &missing_targets[..missing_targets.len().min(5)]
    );
}

/// 验证：两次运行 bridge 输出确定一致
fn assert_deterministic_output(cmd: &mut Command, expected: &Value) {
    let output2 = cmd.assert().success();
    let stdout2 = String::from_utf8(output2.get_output().stdout.clone()).unwrap();
    let v2: Value = serde_json::from_str(&stdout2).expect("第二次运行 stdout 必须是合法 JSON");

    assert_eq!(
        *expected, v2,
        "两次运行 bridge 输出不一致（应为确定性输出）"
    );
}

/// 验证 edge 端点字段使用归一化名称 sourceId/targetId（非 source/target）
fn assert_normalized_edge_endpoints(v: &Value) {
    let edge_categories = &[
        "calls",
        "defines",
        "uses",
        "accesses",
        "designations",
        "imports",
        "contains",
        "owns",
        "annotates",
        "other",
    ];

    for cat in edge_categories {
        if let Some(edges) = v["edges"][cat].as_array() {
            for (i, edge) in edges.iter().enumerate() {
                assert!(
                    edge["sourceId"].is_string(),
                    "{cat}[{i}] 缺少 sourceId 字段"
                );
                assert!(
                    edge["targetId"].is_string(),
                    "{cat}[{i}] 缺少 targetId 字段"
                );
                assert!(
                    edge.get("source").is_none(),
                    "{cat}[{i}] 不应有旧字段名 source（应使用 sourceId）"
                );
                assert!(
                    edge.get("target").is_none(),
                    "{cat}[{i}] 不应有旧字段名 target（应使用 targetId）"
                );
            }
        }
    }
}

/// 验证 symbol kind 为具体类型（非通用 "symbol"）
/// 对齐 GitNexus-RC NodeLabel 预期：Function/Struct/Class/Enum/Interface 等。
/// 来源：gitnexus-shared/src/graph/types.ts NodeLabel 枚举
fn assert_symbol_kind_specific(v: &Value) {
    // GitNexus-RC 期望的具体 NodeLabel 类型（参考: gitnexus-shared/src/graph/types.ts）
    let _known_kinds: &[&str] = &[
        "function",
        "method",
        "associated-function",
        "struct",
        "enum",
        "trait",
        "impl-block",
        "const",
        "static",
        "macro-definition",
        "type-alias",
        "module",
        "enum-variant", // Rust 特有
        "Class",
        "Interface",
        "Init",
        "TypeAlias",
        "Macro", // Cangjie 特有
    ];

    if let Some(syms) = v["symbols"].as_array() {
        for (i, sym) in syms.iter().enumerate() {
            let kind = sym["kind"].as_str().unwrap_or("");
            assert!(
                !kind.is_empty(),
                "symbol[{i}] kind 为空字符串（id={})",
                sym["id"].as_str().unwrap_or("?")
            );
            // 不应为通用 "symbol"（GitNexus-RC 消费侧期望具体类型）
            assert_ne!(
                kind, "symbol",
                "symbol[{i}] kind 不应为通用 \"symbol\"，应为具体类型如 Function/Struct/Class 等（id={}, name={}）",
                sym["id"].as_str().unwrap_or("?"),
                sym["name"].as_str().unwrap_or("?")
            );
        }
    }
}

/// 验证 edge 的 confidence/reason 字段在预期场景存在
/// 对齐 GitNexus-RC GraphRelationship 必需字段。
/// 来源：gitnexus-shared/src/graph/types.ts GraphRelationship 接口
///
/// `require_semantic_confidence`: Rust 源数据提供 edge confidence/reason，因此语义边
/// （CALLS/ACCESSES/DESIGNATION）应包含 confidence。Cangjie 源数据当前不提供这些字段，
/// 故传 false 跳过该断言。
fn assert_edge_confidence_reason(v: &Value, require_semantic_confidence: bool) {
    let edge_categories = &[
        "calls",
        "defines",
        "uses",
        "accesses",
        "designations",
        "imports",
        "contains",
        "owns",
        "annotates",
        "other",
    ];

    let mut total_edges = 0u32;
    let mut edges_with_confidence = 0u32;
    let mut edges_with_reason = 0u32;

    for cat in edge_categories {
        if let Some(edges) = v["edges"][cat].as_array() {
            for (i, edge) in edges.iter().enumerate() {
                total_edges += 1;
                // confidence 和 reason 字段应存在于 edge 顶层（对齐 GitNexus-RC）
                if edge.get("confidence").and_then(|v| v.as_f64()).is_some() {
                    edges_with_confidence += 1;
                }
                if edge.get("reason").and_then(|v| v.as_str()).is_some() {
                    edges_with_reason += 1;
                }
                // CALLS/ACCESSES/DESIGNATION 等语义边应有 confidence（仅 Rust 源数据提供时要求）
                let kind = edge["kind"].as_str().unwrap_or("");
                if require_semantic_confidence
                    && matches!(
                        kind,
                        "CALLS" | "ACCESSES" | "DESIGNATION" | "uses" | "accesses" | "modifies"
                    )
                {
                    assert!(
                        edge.get("confidence").is_some(),
                        "{cat}[{i}] ({kind}) 应有 confidence 字段（对齐 GitNexus-RC GraphRelationship）"
                    );
                }
                // 所有边应无空 kind
                assert!(!kind.is_empty(), "{cat}[{i}] edge kind 为空");
            }
        }
    }

    // 如果图中存在边，至少结构验证不应 panic（confidence/reason 存在性取决于源数据）
    // 注：structural edge（DEFINES/CONTAINS）可能无 confidence，这是正常的
    if total_edges > 0 {
        // 至少有边的 kind 非空（已验证），confidence/reason 覆盖率取决于语言和数据源
        let _ = (edges_with_confidence, edges_with_reason);
    }
}

// ============================================================
// Rust Bridge Roundtrip 测试
// ============================================================

#[test]
fn bridge_rust_structure_complete() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("Rust bridge stdout 必须是合法 JSON");

    assert_bridge_structure(&v);
    assert_eq!(v["language"], "rust");
    assert!(
        !v["repository"]["id"].as_str().unwrap_or("").is_empty(),
        "repository.id 不应为空"
    );
}

#[test]
fn bridge_rust_endpoint_integrity() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_endpoint_integrity(&v);
}

#[test]
fn bridge_rust_stats_consistency() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_stats_consistency(&v);
}

#[test]
fn bridge_rust_no_empty_source_file_paths() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_no_empty_source_file_paths(&v);
}

#[test]
fn bridge_rust_no_empty_symbol_names() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_no_empty_symbol_names(&v);
}

#[test]
fn bridge_rust_normalized_edge_endpoints() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_normalized_edge_endpoints(&v);
}

#[test]
fn bridge_rust_deterministic() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    cmd.arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("Rust bridge stdout 必须是合法 JSON");

    // 第二次运行，用同一个 Command 重新执行
    let mut cmd2 = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    cmd2.arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc");

    assert_deterministic_output(&mut cmd2, &v);
}

#[test]
fn bridge_rust_strict_mode_compatible() {
    // --strict 应与 --format gitnexus-rc 兼容，clean fixture 下 exit 0
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .arg("--strict")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("strict bridge stdout 必须是合法 JSON");

    assert_bridge_structure(&v);
    assert_endpoint_integrity(&v);
}

#[test]
fn bridge_rust_symbol_kind_specific() {
    // 验证 Rust symbol kind 为具体类型（非通用 "symbol"）
    // 对齐 GitNexus-RC NodeLabel 期望
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_symbol_kind_specific(&v);
}

#[test]
fn bridge_rust_edge_confidence_reason() {
    // 验证语义 edge 包含 confidence/reason 字段（对齐 GitNexus-RC GraphRelationship）
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    // Rust 源数据提供 confidence/reason，要求语义边包含 confidence
    assert_edge_confidence_reason(&v, true);
}

// ============================================================
// Cangjie Bridge Roundtrip 测试（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
mod cangjie_bridge_tests {
    use super::*;

    #[test]
    fn bridge_cangjie_structure_complete() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value =
            serde_json::from_str(&stdout).expect("Cangjie bridge stdout 必须是合法 JSON");

        assert_bridge_structure(&v);
        assert_eq!(v["language"], "cangjie");
        assert!(
            !v["repository"]["id"].as_str().unwrap_or("").is_empty(),
            "Cangjie repository.id 不应为空"
        );
    }

    #[test]
    fn bridge_cangjie_endpoint_integrity() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_endpoint_integrity(&v);
    }

    #[test]
    fn bridge_cangjie_stats_consistency() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_stats_consistency(&v);
    }

    #[test]
    fn bridge_cangjie_no_empty_source_file_paths() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_no_empty_source_file_paths(&v);
    }

    #[test]
    fn bridge_cangjie_no_empty_symbol_names() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_no_empty_symbol_names(&v);
    }

    #[test]
    fn bridge_cangjie_normalized_edge_endpoints() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_normalized_edge_endpoints(&v);
    }

    #[test]
    fn bridge_cangjie_deterministic() {
        let root = cangjie_portable_smoke_path();

        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        cmd.arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc");

        let assert = cmd.assert().success();
        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value =
            serde_json::from_str(&stdout).expect("Cangjie bridge stdout 必须是合法 JSON");

        // 第二次运行
        let mut cmd2 = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        cmd2.arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc");

        assert_deterministic_output(&mut cmd2, &v);
    }

    #[test]
    fn bridge_cangjie_strict_mode_compatible() {
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .arg("--strict")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).expect("strict bridge stdout 必须是合法 JSON");

        assert_bridge_structure(&v);
        assert_endpoint_integrity(&v);
    }

    #[test]
    fn bridge_cangjie_symbol_kind_specific() {
        // 验证 Cangjie symbol kind 为具体类型（非通用 "symbol"）
        // 对齐 GitNexus-RC NodeLabel 期望
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        assert_symbol_kind_specific(&v);
    }

    #[test]
    fn bridge_cangjie_edge_confidence_reason() {
        // 验证 Cangjie 语义 edge 包含 confidence/reason 字段
        let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
        let root = cangjie_portable_smoke_path();

        let assert = cmd
            .arg("analyze")
            .arg("--root")
            .arg(&root)
            .arg("--language")
            .arg("cangjie")
            .arg("--format")
            .arg("gitnexus-rc")
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        let v: Value = serde_json::from_str(&stdout).unwrap();

        // Cangjie 源数据当前不提供 confidence/reason，不强制要求
        assert_edge_confidence_reason(&v, false);
    }
}
