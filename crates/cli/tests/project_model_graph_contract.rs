//! Rust graph contract regression tests
//!
//! 验证 Rust graph output 的合约稳定性（仿照 Cangjie graph_contract.rs 模式）：
//! - 质量门（0 duplicate, 0 dangling, 确定性输出）
//! - 节点/边类型覆盖
//! - 已知 symbol ID 存在
//! - 已知 edge triple 存在
//!
//! 这些测试验证语义合约 —— 检查必须存在的特定 node ID 和 edge triple。
//! 如果任何测试失败，说明 Rust graph contract 已退化。
//!
//! # 运行
//!
//! ```sh
//! cargo test --test project_model_graph_contract -- --nocapture
//! ```

use assert_cmd::Command;
use std::collections::{HashMap, HashSet};

fn cli() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn fixture(name: &str) -> String {
    let base = std::env::current_dir().unwrap();
    // 可能在 crates/cli 子目录，需要回退到 workspace root
    let root = if base.join("fixtures").exists() {
        base
    } else if let Some(p) = base.parent().and_then(|p| p.parent()) {
        p.to_path_buf()
    } else {
        base
    };
    root.join("fixtures")
        .join("rust")
        .join(name)
        .to_string_lossy()
        .to_string()
}

fn run_graph(fixture_name: &str) -> serde_json::Value {
    let root = fixture(fixture_name);
    let output = cli()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(&root)
        .arg("--include")
        .arg("graph")
        .arg("--include")
        .arg("calls")
        .arg("--include")
        .arg("symbols")
        .output()
        .expect("failed to run CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("Graph output is not valid JSON")
}

struct GraphData {
    node_ids: HashSet<String>,
    node_kinds: HashMap<String, usize>,
    edge_triples: HashSet<(String, String, String)>,
    edge_kinds: HashMap<String, usize>,
    duplicate_nodes: usize,
    duplicate_edges: usize,
    dangling_sources: usize,
    dangling_targets: usize,
    deterministic: bool,
}

fn collect_graph(fixture_name: &str) -> GraphData {
    let graph1 = run_graph(fixture_name);
    let graph2 = run_graph(fixture_name);

    let nodes1 = graph1["nodes"].as_array().unwrap();
    let edges1 = graph1["edges"].as_array().unwrap();

    // 收集 node ID
    let mut node_ids = HashSet::new();
    for n in nodes1 {
        node_ids.insert(n["id"].as_str().unwrap().to_string());
    }

    // 节点类型统计
    let mut node_kinds = HashMap::new();
    let mut seen_ids = HashSet::new();
    let mut duplicate_nodes = 0usize;
    for n in nodes1 {
        let label = n["label"].as_str().unwrap().to_string();
        *node_kinds.entry(label).or_insert(0) += 1;
        if !seen_ids.insert(n["id"].as_str().unwrap().to_string()) {
            duplicate_nodes += 1;
        }
    }

    // 边统计
    let mut edge_triples = HashSet::new();
    let mut seen_edges = HashSet::new();
    let mut duplicate_edges = 0usize;
    let mut edge_kinds = HashMap::new();
    let mut dangling_sources = 0usize;
    let mut dangling_targets = 0usize;

    for e in edges1 {
        let kind = e["type"].as_str().unwrap().to_string();
        let source = e["source"].as_str().unwrap().to_string();
        let target = e["target"].as_str().unwrap().to_string();

        *edge_kinds.entry(kind.clone()).or_insert(0) += 1;
        let triple = (kind, source.clone(), target.clone());
        if !seen_edges.insert(triple.clone()) {
            duplicate_edges += 1;
        }
        edge_triples.insert(triple);

        if !node_ids.contains(&source) {
            dangling_sources += 1;
        }
        if !node_ids.contains(&target) {
            dangling_targets += 1;
        }
    }

    // 确定性检查
    let str1 = serde_json::to_string(&graph1).unwrap();
    let str2 = serde_json::to_string(&graph2).unwrap();
    let deterministic = str1 == str2;

    GraphData {
        node_ids,
        node_kinds,
        edge_triples,
        edge_kinds,
        duplicate_nodes,
        duplicate_edges,
        dangling_sources,
        dangling_targets,
        deterministic,
    }
}

// ============================================================
// 质量门测试
// ============================================================

#[test]
fn rust_graph_contract_portable_smoke_quality_gates() {
    let data = collect_graph("portable-smoke");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

// ============================================================
// 节点类型覆盖测试
// ============================================================

#[test]
fn rust_graph_contract_portable_smoke_node_kind_set() {
    let data = collect_graph("portable-smoke");

    // 必须包含的核心节点类型
    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");
    assert!(
        data.node_kinds.contains_key("source-file"),
        "应有 source-file 节点"
    );
    assert!(data.node_kinds.contains_key("symbol"), "应有 symbol 节点");

    // 至少 2 个 source file（lib.rs + main.rs）
    let source_files = data.node_kinds.get("source-file").copied().unwrap_or(0);
    assert!(
        source_files >= 2,
        "至少应有 2 个 source file，实际: {}",
        source_files
    );
}

// ============================================================
// 边类型覆盖测试
// ============================================================

#[test]
fn rust_graph_contract_portable_smoke_edge_kind_set() {
    let data = collect_graph("portable-smoke");

    // 必须包含的核心边类型
    assert!(
        data.edge_kinds.contains_key("CONTAINS_PACKAGE"),
        "应有 CONTAINS_PACKAGE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_TARGET"),
        "应有 HAS_TARGET 边"
    );
    assert!(
        data.edge_kinds.contains_key("OWNS_SOURCE"),
        "应有 OWNS_SOURCE 边"
    );
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");
    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(
        data.edge_kinds.contains_key("DESIGNATION"),
        "应有 DESIGNATION 边（impl block → struct）"
    );
    assert!(
        data.edge_kinds.contains_key("ACCESSES"),
        "应有 ACCESSES 边（类型注解引用）"
    );

    // CALLS edge 数量
    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(calls >= 2, "至少应有 2 条 CALLS edge，实际: {}", calls);
}

// ============================================================
// 已知 symbol ID 存在测试
// ============================================================

#[test]
fn rust_graph_contract_portable_smoke_known_symbols() {
    let data = collect_graph("portable-smoke");

    // lib.rs 中的核心 symbol 必须存在
    let required = [
        "symbol:portable-smoke::crate::Calculator",
        "symbol:portable-smoke::crate::add",
        "symbol:portable-smoke::crate::multiply",
        "symbol:portable-smoke::crate::create_calculator",
        "symbol:portable-smoke::crate::main",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

// ============================================================
// 已知 edge triple 存在测试
// ============================================================

#[test]
fn rust_graph_contract_portable_smoke_known_defines_edges() {
    let data = collect_graph("portable-smoke");

    // 核心 DEFINES 边
    let required = [
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:portable-smoke::crate::Calculator",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:portable-smoke::crate::add",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:portable-smoke::crate::multiply",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:portable-smoke::crate::create_calculator",
        ),
        (
            "DEFINES",
            "file:src/main.rs",
            "symbol:portable-smoke::crate::main",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_portable_smoke_known_calls_edges() {
    let data = collect_graph("portable-smoke");

    // 核心 CALLS 边：main 调用 add, multiply
    let required = [
        (
            "CALLS",
            "symbol:portable-smoke::crate::main",
            "symbol:portable-smoke::crate::add",
        ),
        (
            "CALLS",
            "symbol:portable-smoke::crate::main",
            "symbol:portable-smoke::crate::multiply",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_portable_smoke_known_designation_edge() {
    let data = collect_graph("portable-smoke");

    // impl Calculator → Calculator struct (DESIGNATION)
    let designation_triples: Vec<_> = data
        .edge_triples
        .iter()
        .filter(|(k, _, t)| k == "DESIGNATION" && t == "symbol:portable-smoke::crate::Calculator")
        .collect();
    assert!(
        !designation_triples.is_empty(),
        "应有 impl Calculator → Calculator DESIGNATION edge"
    );
}

#[test]
fn rust_graph_contract_portable_smoke_calls_endpoint_integrity() {
    let data = collect_graph("portable-smoke");

    // 每条 CALLS edge 的 source 和 target 都必须是 symbol node
    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                source.starts_with("symbol:"),
                "CALLS source 必须是 symbol: {}",
                source
            );
            assert!(
                target.starts_with("symbol:"),
                "CALLS target 必须是 symbol: {}",
                target
            );
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在于 nodes 中: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在于 nodes 中: {}",
                target
            );
        }
    }
}

// ============================================================
// imports-cross-crate fixture tests
// ============================================================

#[test]
fn rust_graph_contract_imports_cross_crate_quality_gates() {
    let data = collect_graph("imports-cross-crate");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

#[test]
fn rust_graph_contract_imports_cross_crate_node_kind_set() {
    let data = collect_graph("imports-cross-crate");

    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");
    assert!(
        data.node_kinds.contains_key("source-file"),
        "应有 source-file 节点"
    );
    assert!(data.node_kinds.contains_key("symbol"), "应有 symbol 节点");
}

#[test]
fn rust_graph_contract_imports_cross_crate_edge_kind_set() {
    let data = collect_graph("imports-cross-crate");

    assert!(
        data.edge_kinds.contains_key("CONTAINS_PACKAGE"),
        "应有 CONTAINS_PACKAGE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_TARGET"),
        "应有 HAS_TARGET 边"
    );
    assert!(
        data.edge_kinds.contains_key("OWNS_SOURCE"),
        "应有 OWNS_SOURCE 边"
    );
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");
    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(
        data.edge_kinds.contains_key("DESIGNATION"),
        "应有 DESIGNATION 边"
    );
    assert!(data.edge_kinds.contains_key("ACCESSES"), "应有 ACCESSES 边");

    // 至少 4 条 CALLS（含 external crate 调用）
    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(calls >= 4, "至少应有 4 条 CALLS edge，实际: {}", calls);
}

#[test]
fn rust_graph_contract_imports_cross_crate_known_symbols() {
    let data = collect_graph("imports-cross-crate");

    let required = [
        "symbol:imports-cross-crate::crate::DataStore",
        "symbol:imports-cross-crate::crate::new",
        "symbol:imports-cross-crate::crate::insert",
        "symbol:imports-cross-crate::crate::get",
        "symbol:imports-cross-crate::crate::create_store",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

#[test]
fn rust_graph_contract_imports_cross_crate_known_calls_edges() {
    let data = collect_graph("imports-cross-crate");

    // 外部 crate 函数调用（external-crate-path-resolved）
    let required = [
        (
            "CALLS",
            "symbol:imports-cross-crate::crate::new",
            "symbol:std::vec::Vec::new",
        ),
        (
            "CALLS",
            "symbol:imports-cross-crate::crate::new",
            "symbol:std::collections::HashMap::new",
        ),
        (
            "CALLS",
            "symbol:imports-cross-crate::crate::get",
            "symbol:std::string::String::from",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_imports_cross_crate_external_symbol_nodes() {
    let data = collect_graph("imports-cross-crate");

    // 验证有外部 symbol node 且数量正确
    let expected_external = [
        "symbol:std::vec::Vec::new",
        "symbol:std::collections::HashMap::new",
        "symbol:std::string::String::from",
        "symbol:std::clone::Clone::clone",
    ];

    for ext_id in &expected_external {
        assert!(
            data.node_ids.contains(*ext_id),
            "必须存在外部 symbol node: {}",
            ext_id
        );
    }

    // 外部 symbol node 应该都有对应的 CALLS target
    for (kind, _source, target) in &data.edge_triples {
        if kind == "CALLS" && target.starts_with("symbol:std::") {
            assert!(
                data.node_ids.contains(target),
                "外部 CALLS target 必须有对应 node: {}",
                target
            );
        }
    }
}

#[test]
fn rust_graph_contract_imports_cross_crate_calls_endpoint_integrity() {
    let data = collect_graph("imports-cross-crate");

    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在: {}",
                target
            );
        }
    }
}

#[test]
fn rust_graph_contract_imports_cross_crate_known_designation_edge() {
    let data = collect_graph("imports-cross-crate");

    // impl DataStore → DataStore struct (DESIGNATION)
    let designation_triples: Vec<_> = data
        .edge_triples
        .iter()
        .filter(|(k, _, t)| {
            k == "DESIGNATION" && t == "symbol:imports-cross-crate::crate::DataStore"
        })
        .collect();
    assert!(
        !designation_triples.is_empty(),
        "应有 impl DataStore → DataStore DESIGNATION edge"
    );
}

// ============================================================
// multi-module fixture tests
// ============================================================

#[test]
fn rust_graph_contract_multi_module_quality_gates() {
    let data = collect_graph("multi-module");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

#[test]
fn rust_graph_contract_multi_module_node_kind_set() {
    let data = collect_graph("multi-module");

    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");

    // 关键：多模块至少应有 2 个 source file
    let source_files = data.node_kinds.get("source-file").copied().unwrap_or(0);
    assert!(
        source_files >= 2,
        "至少应有 2 个 source file，实际: {}",
        source_files
    );

    let symbols = data.node_kinds.get("symbol").copied().unwrap_or(0);
    assert!(symbols >= 4, "至少应有 4 个 symbol，实际: {}", symbols);
}

#[test]
fn rust_graph_contract_multi_module_edge_kind_set() {
    let data = collect_graph("multi-module");

    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");

    // 关键：2 个 source file 应有 2 条 OWNS_SOURCE 边
    let owns = data.edge_kinds.get("OWNS_SOURCE").copied().unwrap_or(0);
    assert!(owns >= 2, "应有至少 2 条 OWNS_SOURCE 边，实际: {}", owns);

    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(calls >= 3, "应有至少 3 条 CALLS edge，实际: {}", calls);
}

#[test]
fn rust_graph_contract_multi_module_known_symbols() {
    let data = collect_graph("multi-module");

    let required = [
        "symbol:multi-module::crate::process_data",
        "symbol:multi-module::crate::run_pipeline",
        "symbol:multi-module::crate::utils::double_value",
        "symbol:multi-module::crate::utils::format_result",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

#[test]
fn rust_graph_contract_multi_module_known_defines_edges() {
    let data = collect_graph("multi-module");

    // 跨文件 DEFINES：utils.rs 定义自己的 symbol
    let required = [
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:multi-module::crate::process_data",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:multi-module::crate::run_pipeline",
        ),
        (
            "DEFINES",
            "file:src/utils.rs",
            "symbol:multi-module::crate::utils::double_value",
        ),
        (
            "DEFINES",
            "file:src/utils.rs",
            "symbol:multi-module::crate::utils::format_result",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_multi_module_known_calls_edges() {
    let data = collect_graph("multi-module");

    // crate:: 路径调用 + same-module 调用
    let required = [
        (
            "CALLS",
            "symbol:multi-module::crate::process_data",
            "symbol:multi-module::crate::utils::double_value",
        ),
        (
            "CALLS",
            "symbol:multi-module::crate::process_data",
            "symbol:multi-module::crate::utils::format_result",
        ),
        (
            "CALLS",
            "symbol:multi-module::crate::run_pipeline",
            "symbol:multi-module::crate::process_data",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_multi_module_calls_endpoint_integrity() {
    let data = collect_graph("multi-module");

    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在: {}",
                target
            );
        }
    }
}

// ============================================================
// module-hierarchy fixture tests
// ============================================================

#[test]
fn rust_graph_contract_module_hierarchy_quality_gates() {
    let data = collect_graph("module-hierarchy");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

#[test]
fn rust_graph_contract_module_hierarchy_node_kind_set() {
    let data = collect_graph("module-hierarchy");

    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");

    // 嵌套模块层次：3 个 source file
    let source_files = data.node_kinds.get("source-file").copied().unwrap_or(0);
    assert!(
        source_files >= 3,
        "应有至少 3 个 source file（lib.rs + utils/mod.rs + utils/calculations.rs），实际: {}",
        source_files
    );

    let symbols = data.node_kinds.get("symbol").copied().unwrap_or(0);
    assert!(symbols >= 5, "应有至少 5 个 symbol，实际: {}", symbols);
}

#[test]
fn rust_graph_contract_module_hierarchy_edge_kind_set() {
    let data = collect_graph("module-hierarchy");

    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");

    // 3 个 source file → 3 条 OWNS_SOURCE 边
    let owns = data.edge_kinds.get("OWNS_SOURCE").copied().unwrap_or(0);
    assert!(owns >= 3, "应有至少 3 条 OWNS_SOURCE 边，实际: {}", owns);

    // 3 条 CALLS（crate:: 路径 + super:: 路径 + import-resolved）
    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(calls >= 3, "应有至少 3 条 CALLS edge，实际: {}", calls);
}

#[test]
fn rust_graph_contract_module_hierarchy_known_symbols() {
    let data = collect_graph("module-hierarchy");

    let required = [
        "symbol:module-hierarchy::crate::top_level",
        "symbol:module-hierarchy::crate::call_via_crate_path",
        "symbol:module-hierarchy::crate::utils",
        "symbol:module-hierarchy::crate::utils::double",
        "symbol:module-hierarchy::crate::utils::calculations",
        "symbol:module-hierarchy::crate::utils::calculations::multiply",
        "symbol:module-hierarchy::crate::utils::calculations::call_super_direct",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

#[test]
fn rust_graph_contract_module_hierarchy_known_defines_edges() {
    let data = collect_graph("module-hierarchy");

    // 跨文件 DEFINES：3 个 source file 各自定义 symbol
    let required = [
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:module-hierarchy::crate::call_via_crate_path",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:module-hierarchy::crate::top_level",
        ),
        (
            "DEFINES",
            "file:src/utils/mod.rs",
            "symbol:module-hierarchy::crate::utils::double",
        ),
        (
            "DEFINES",
            "file:src/utils/calculations.rs",
            "symbol:module-hierarchy::crate::utils::calculations::multiply",
        ),
        (
            "DEFINES",
            "file:src/utils/calculations.rs",
            "symbol:module-hierarchy::crate::utils::calculations::call_super_direct",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_module_hierarchy_known_calls_edges() {
    let data = collect_graph("module-hierarchy");

    // crate:: 路径调用 + super:: 路径调用 + import-resolved 调用
    let required = [
        (
            "CALLS",
            "symbol:module-hierarchy::crate::call_via_crate_path",
            "symbol:module-hierarchy::crate::utils::calculations::multiply",
        ),
        (
            "CALLS",
            "symbol:module-hierarchy::crate::utils::calculations::multiply",
            "symbol:module-hierarchy::crate::utils::double",
        ),
        (
            "CALLS",
            "symbol:module-hierarchy::crate::utils::calculations::call_super_direct",
            "symbol:module-hierarchy::crate::utils::double",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_module_hierarchy_calls_endpoint_integrity() {
    let data = collect_graph("module-hierarchy");

    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在: {}",
                target
            );
        }
    }
}

// ============================================================
// inline-module fixture — 验证 inline module 的 symbol 结构和 CALLS
// ============================================================

#[test]
fn rust_graph_contract_inline_module_quality_gates() {
    let data = collect_graph("inline-module");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

#[test]
fn rust_graph_contract_inline_module_node_kind_set() {
    let data = collect_graph("inline-module");

    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");
    assert!(
        data.node_kinds.contains_key("source-file"),
        "应有 source-file 节点"
    );

    let source_files = data.node_kinds.get("source-file").copied().unwrap_or(0);
    assert_eq!(
        source_files, 1,
        "应有 1 个 source file（唯一 lib.rs），实际: {}",
        source_files
    );

    let symbols = data.node_kinds.get("symbol").copied().unwrap_or(0);
    assert!(
        symbols >= 8,
        "应有至少 8 个 symbol（含 2 个 module symbol），实际: {}",
        symbols
    );
}

#[test]
fn rust_graph_contract_inline_module_edge_kind_set() {
    let data = collect_graph("inline-module");

    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");
    assert!(
        data.edge_kinds.contains_key("OWNS_SOURCE"),
        "应有 OWNS_SOURCE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_TARGET"),
        "应有 HAS_TARGET 边"
    );
    assert!(
        data.edge_kinds.contains_key("CONTAINS_PACKAGE"),
        "应有 CONTAINS_PACKAGE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_PARENT"),
        "应有 HAS_PARENT 边（inline module 特有）"
    );

    let has_parent = data.edge_kinds.get("HAS_PARENT").copied().unwrap_or(0);
    assert!(
        has_parent >= 6,
        "应有至少 6 条 HAS_PARENT 边，实际: {}",
        has_parent
    );

    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(calls >= 1, "应有至少 1 条 CALLS edge，实际: {}", calls);
}

#[test]
fn rust_graph_contract_inline_module_known_symbols() {
    let data = collect_graph("inline-module");

    let required = [
        "symbol:inline-module::crate::root_fn",
        "symbol:inline-module::crate::inner",
        "symbol:inline-module::crate::inner::inner_fn",
        "symbol:inline-module::crate::inner::call_self",
        "symbol:inline-module::crate::inner::call_super",
        "symbol:inline-module::crate::inner::nested",
        "symbol:inline-module::crate::inner::nested::call_crate",
        "symbol:inline-module::crate::inner::nested::call_super_to_parent",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

#[test]
fn rust_graph_contract_inline_module_known_defines_edges() {
    let data = collect_graph("inline-module");

    let required = [
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:inline-module::crate::root_fn",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:inline-module::crate::inner::call_self",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:inline-module::crate::inner::call_super",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:inline-module::crate::inner::inner_fn",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:inline-module::crate::inner::nested::call_crate",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_inline_module_known_calls_edges() {
    let data = collect_graph("inline-module");

    let required = [(
        "CALLS",
        "symbol:inline-module::crate::inner::nested::call_crate",
        "symbol:inline-module::crate::root_fn",
    )];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_inline_module_calls_endpoint_integrity() {
    let data = collect_graph("inline-module");

    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在: {}",
                target
            );
        }
    }
}

// ============================================================
// self-path fixture — 验证 self:: 路径解析、模块结构、HAS_PARENT
// ============================================================

#[test]
fn rust_graph_contract_self_path_quality_gates() {
    let data = collect_graph("self-path");

    assert_eq!(data.duplicate_nodes, 0, "不应有重复节点 ID");
    assert_eq!(data.duplicate_edges, 0, "不应有重复边");
    assert_eq!(data.dangling_sources, 0, "不应有悬空 source 引用");
    assert_eq!(data.dangling_targets, 0, "不应有悬空 target 引用");
    assert!(data.deterministic, "输出必须是确定性的");
}

#[test]
fn rust_graph_contract_self_path_node_kind_set() {
    let data = collect_graph("self-path");

    assert!(
        data.node_kinds.contains_key("repository"),
        "应有 repository 节点"
    );
    assert!(data.node_kinds.contains_key("package"), "应有 package 节点");
    assert!(data.node_kinds.contains_key("target"), "应有 target 节点");
    assert!(
        data.node_kinds.contains_key("source-file"),
        "应有 source-file 节点"
    );

    let source_files = data.node_kinds.get("source-file").copied().unwrap_or(0);
    assert_eq!(
        source_files, 1,
        "应有 1 个 source file，实际: {}",
        source_files
    );

    let symbols = data.node_kinds.get("symbol").copied().unwrap_or(0);
    assert!(
        symbols >= 13,
        "应有至少 13 个 symbol（含 module、struct、impl、method），实际: {}",
        symbols
    );
}

#[test]
fn rust_graph_contract_self_path_edge_kind_set() {
    let data = collect_graph("self-path");

    assert!(data.edge_kinds.contains_key("CALLS"), "应有 CALLS 边");
    assert!(data.edge_kinds.contains_key("DEFINES"), "应有 DEFINES 边");
    assert!(
        data.edge_kinds.contains_key("OWNS_SOURCE"),
        "应有 OWNS_SOURCE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_TARGET"),
        "应有 HAS_TARGET 边"
    );
    assert!(
        data.edge_kinds.contains_key("CONTAINS_PACKAGE"),
        "应有 CONTAINS_PACKAGE 边"
    );
    assert!(
        data.edge_kinds.contains_key("HAS_PARENT"),
        "应有 HAS_PARENT 边"
    );
    assert!(
        data.edge_kinds.contains_key("DESIGNATION"),
        "应有 DESIGNATION 边"
    );

    let calls = data.edge_kinds.get("CALLS").copied().unwrap_or(0);
    assert!(
        calls >= 2,
        "应有至少 2 条 CALLS edge（direct_caller + self_caller），实际: {}",
        calls
    );

    let has_parent = data.edge_kinds.get("HAS_PARENT").copied().unwrap_or(0);
    assert!(
        has_parent >= 5,
        "应有至少 5 条 HAS_PARENT 边，实际: {}",
        has_parent
    );
}

#[test]
fn rust_graph_contract_self_path_known_symbols() {
    let data = collect_graph("self-path");

    let required = [
        "symbol:self-path::crate::top_level_fn",
        "symbol:self-path::crate::direct_caller",
        "symbol:self-path::crate::self_caller",
        "symbol:self-path::crate::self_associated_caller",
        "symbol:self-path::crate::Calculator",
        "symbol:self-path::crate::_impl_Calculator",
        "symbol:self-path::crate::new",
        "symbol:self-path::crate::add",
        "symbol:self-path::crate::inner",
        "symbol:self-path::crate::inner::inner_fn",
        "symbol:self-path::crate::deeper",
        "symbol:self-path::crate::deeper::nested",
        "symbol:self-path::crate::deeper::nested::deep_fn",
    ];

    for sym_id in &required {
        assert!(
            data.node_ids.contains(*sym_id),
            "必须存在 symbol: {}",
            sym_id
        );
    }
}

#[test]
fn rust_graph_contract_self_path_known_defines_edges() {
    let data = collect_graph("self-path");

    let required = [
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:self-path::crate::top_level_fn",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:self-path::crate::self_caller",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:self-path::crate::direct_caller",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:self-path::crate::Calculator",
        ),
        (
            "DEFINES",
            "file:src/lib.rs",
            "symbol:self-path::crate::inner::inner_fn",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_self_path_known_calls_edges() {
    let data = collect_graph("self-path");

    // self:: 路径解析：self_caller → top_level_fn
    let required = [
        (
            "CALLS",
            "symbol:self-path::crate::direct_caller",
            "symbol:self-path::crate::top_level_fn",
        ),
        (
            "CALLS",
            "symbol:self-path::crate::self_caller",
            "symbol:self-path::crate::top_level_fn",
        ),
    ];

    for (kind, source, target) in &required {
        let triple = (kind.to_string(), source.to_string(), target.to_string());
        assert!(
            data.edge_triples.contains(&triple),
            "必须存在 edge: {}: {} → {}",
            kind,
            source,
            target
        );
    }
}

#[test]
fn rust_graph_contract_self_path_calls_endpoint_integrity() {
    let data = collect_graph("self-path");

    for (kind, source, target) in &data.edge_triples {
        if kind == "CALLS" {
            assert!(
                data.node_ids.contains(source),
                "CALLS source 必须存在: {}",
                source
            );
            assert!(
                data.node_ids.contains(target),
                "CALLS target 必须存在: {}",
                target
            );
        }
    }
}
