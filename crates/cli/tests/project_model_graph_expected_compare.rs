//! expected-graph.json comparison harness
//!
//! 对比 Rust-core actual graph output 与 expected-graph.json golden fixtures。
//!
//! 为什么 graph comparison 独立于 symbol/PM comparison：
//!   graph 层有三层数据（nodes/edges/diagnostics）、composite key 对齐、presence/absence invariant，
//!   混入已有 harness 会显著增加复杂度。
//!
//! 比较策略：exact match，不允许 known skip。
//! 排除字段：generatedAt / root（含 runtime 绝对路径）。
//! 对齐规则：node 按 id、edge 按 (type, source, target)、diagnostic 按 id。
//! Properties 比较：JSON deep equal（serde_json::Value 相等性）。

use assert_cmd::Command;
use std::path::PathBuf;

// === Fixture 定义 ===

struct GraphFixture {
    name: &'static str,
    input_root: &'static str,
    command: &'static [&'static str],
}

const GRAPH_FIXTURES: &[GraphFixture] = &[
    GraphFixture {
        name: "root-package",
        input_root: "manifest-scanner/root-package",
        command: &["--include", "graph"],
    },
    GraphFixture {
        name: "virtual-workspace-glob",
        input_root: "manifest-scanner/virtual-workspace-glob",
        command: &["--include", "graph"],
    },
    GraphFixture {
        name: "item-impl-methods",
        input_root: "item-extraction/item-impl-methods",
        command: &["--include", "graph", "--include", "symbols"],
    },
];

// === 目录定位 ===

fn workspace_root() -> PathBuf {
    if let Ok(root) = std::env::var("GITNEXUS_RUST_CORE_ROOT") {
        PathBuf::from(root)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }
}

fn fixture_root(input_root: &str) -> PathBuf {
    workspace_root().join("fixtures").join(input_root)
}

fn expected_graph_path(input_root: &str) -> PathBuf {
    fixture_root(input_root).join("expected-graph.json")
}

// === CLI 调用 ===

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

/// 调用 CLI 获取 actual graph output，使用相对路径使 node ID 与 golden 文件一致。
/// 为什么用相对路径：graph emitter 将 --root 参数值作为 repo node ID，
/// 绝对路径会导致 golden 文件含机器特定路径，不可移植。
fn inspect_graph(fixture: &GraphFixture) -> serde_json::Value {
    let ws_root = workspace_root();
    let rel = std::path::Path::new("fixtures").join(fixture.input_root);
    let mut cmd = cli_bin();
    cmd.current_dir(&ws_root)
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(rel.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json");
    for arg in fixture.command {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("CLI 调用失败");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("CLI 退出非零 for {}: {}", fixture.name, stderr);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "actual output 不是合法 JSON for {}: {}\nstdout: {}",
            fixture.name, e, stdout
        )
    })
}

fn load_expected(input_root: &str) -> serde_json::Value {
    let path = expected_graph_path(input_root);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 expected-graph.json for {}: {}", input_root, e));
    serde_json::from_str(&content).unwrap_or_else(|e| {
        panic!(
            "expected-graph.json 不是合法 JSON for {}: {}",
            input_root, e
        )
    })
}

// === Mismatch 类型 ===

#[derive(Debug)]
struct GraphMismatch {
    fixture: String,
    category: String,
    identifier: String,
    field: String,
    expected: String,
    actual: String,
    detail: String,
}

impl std::fmt::Display for GraphMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Graph mismatch [{}]:\n  category: {}\n  identifier: {}\n  field: {}\n  expected: {}\n  actual: {}\n  detail: {}",
            self.fixture, self.category, self.identifier, self.field, self.expected, self.actual, self.detail
        )
    }
}

// === 比较工具 ===

fn val_summary(v: &serde_json::Value) -> String {
    if v.is_null() {
        "null".to_string()
    } else if let Some(s) = v.as_str() {
        if s.len() > 80 {
            format!("{:?}...", &s[..40])
        } else {
            format!("{:?}", s)
        }
    } else if v.is_number() {
        v.to_string()
    } else if v.is_boolean() {
        v.to_string()
    } else {
        let s = v.to_string();
        if s.len() > 100 {
            format!("{}...", &s[..50])
        } else {
            s
        }
    }
}

fn deep_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    a == b
}

fn find_json_diff(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    path: &str,
) -> Vec<String> {
    let mut diffs = Vec::new();
    if expected.is_object() && actual.is_object() {
        let e_map = expected.as_object().unwrap();
        let a_map = actual.as_object().unwrap();
        for (key, e_val) in e_map {
            let sub_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };
            match a_map.get(key) {
                Some(a_val) => diffs.extend(find_json_diff(e_val, a_val, &sub_path)),
                None => diffs.push(format!(
                    "{}: expected {} but missing in actual",
                    sub_path,
                    val_summary(e_val)
                )),
            }
        }
        for key in a_map.keys() {
            if !e_map.contains_key(key) {
                let sub_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                diffs.push(format!("{}: unexpected in actual", sub_path));
            }
        }
    } else if expected.is_array() && actual.is_array() {
        let e_arr = expected.as_array().unwrap();
        let a_arr = actual.as_array().unwrap();
        if e_arr.len() != a_arr.len() {
            diffs.push(format!(
                "{}: array length expected={} actual={}",
                path,
                e_arr.len(),
                a_arr.len()
            ));
        } else {
            for (i, (e, a)) in e_arr.iter().zip(a_arr.iter()).enumerate() {
                let sub_path = format!("{}[{}]", path, i);
                diffs.extend(find_json_diff(e, a, &sub_path));
            }
        }
    } else if !deep_equal(expected, actual) {
        diffs.push(format!(
            "{}: expected={} actual={}",
            path,
            val_summary(expected),
            val_summary(actual)
        ));
    }
    diffs
}

// === schemaVersion 比较 ===

fn compare_schema_version(
    fixture: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Vec<GraphMismatch> {
    let mut mismatches = Vec::new();
    let e_ver = expected["schemaVersion"].as_str().unwrap_or("");
    let a_ver = actual["schemaVersion"].as_str().unwrap_or("");
    if e_ver != a_ver {
        mismatches.push(GraphMismatch {
            fixture: fixture.to_string(),
            category: "schemaVersion".to_string(),
            identifier: "schemaVersion".to_string(),
            field: "schemaVersion".to_string(),
            expected: e_ver.to_string(),
            actual: a_ver.to_string(),
            detail: "schemaVersion 不一致".to_string(),
        });
    }
    mismatches
}

// === Node 比较 ===

fn compare_nodes(
    fixture: &str,
    expected_nodes: &[serde_json::Value],
    actual_nodes: &[serde_json::Value],
) -> Vec<GraphMismatch> {
    let mut mismatches = Vec::new();

    let actual_map: std::collections::HashMap<&str, &serde_json::Value> = actual_nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(|id| (id, n)))
        .collect();

    let mut matched_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for e_node in expected_nodes {
        let e_id = e_node["id"].as_str().unwrap_or("?");
        match actual_map.get(e_id) {
            None => {
                mismatches.push(GraphMismatch {
                    fixture: fixture.to_string(),
                    category: "node".to_string(),
                    identifier: e_id.to_string(),
                    field: "id".to_string(),
                    expected: e_id.to_string(),
                    actual: "(missing)".to_string(),
                    detail: format!("expected node {} 在 actual 中不存在", e_id),
                });
            }
            Some(a_node) => {
                matched_ids.insert(e_id);
                // label exact match
                let e_label = e_node["label"].as_str().unwrap_or("");
                let a_label = a_node["label"].as_str().unwrap_or("");
                if e_label != a_label {
                    mismatches.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "node".to_string(),
                        identifier: e_id.to_string(),
                        field: "label".to_string(),
                        expected: e_label.to_string(),
                        actual: a_label.to_string(),
                        detail: "node label 不一致".to_string(),
                    });
                }
                // properties deep equal
                let e_props = &e_node["properties"];
                let a_props = &a_node["properties"];
                if !deep_equal(e_props, a_props) {
                    let diffs = find_json_diff(e_props, a_props, "properties");
                    for diff in diffs {
                        mismatches.push(GraphMismatch {
                            fixture: fixture.to_string(),
                            category: "node".to_string(),
                            identifier: e_id.to_string(),
                            field: diff.clone(),
                            expected: val_summary(e_props),
                            actual: val_summary(a_props),
                            detail: format!("node {} properties 不一致: {}", e_id, diff),
                        });
                    }
                }
            }
        }
    }

    // extra nodes in actual
    for a_node in actual_nodes {
        let a_id = a_node["id"].as_str().unwrap_or("?");
        if !matched_ids.contains(a_id) {
            mismatches.push(GraphMismatch {
                fixture: fixture.to_string(),
                category: "node".to_string(),
                identifier: a_id.to_string(),
                field: "id".to_string(),
                expected: "(not in expected)".to_string(),
                actual: a_id.to_string(),
                detail: format!("actual node {} 不在 expectedGraph.nodes 中", a_id),
            });
        }
    }

    mismatches
}

// === Edge 比较 ===

fn edge_key(e: &serde_json::Value) -> (String, String, String) {
    let src = e["source"].as_str().unwrap_or("").to_string();
    let tgt = e["target"].as_str().unwrap_or("").to_string();
    let typ = e["type"].as_str().unwrap_or("").to_string();
    (typ, src, tgt)
}

fn compare_edges(
    fixture: &str,
    expected_edges: &[serde_json::Value],
    actual_edges: &[serde_json::Value],
) -> Vec<GraphMismatch> {
    let mut mismatches = Vec::new();

    let actual_map: std::collections::HashMap<(String, String, String), &serde_json::Value> =
        actual_edges
            .iter()
            .filter_map(|e| {
                let key = edge_key(e);
                if key.0.is_empty() {
                    None
                } else {
                    Some((key, e))
                }
            })
            .collect();

    let mut matched_keys: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();

    for e_edge in expected_edges {
        let key = edge_key(e_edge);
        match actual_map.get(&key) {
            None => {
                mismatches.push(GraphMismatch {
                    fixture: fixture.to_string(),
                    category: "edge".to_string(),
                    identifier: format!("({},{},{})", key.0, key.1, key.2),
                    field: "composite_key".to_string(),
                    expected: format!("({},{},{})", key.0, key.1, key.2),
                    actual: "(missing)".to_string(),
                    detail: format!(
                        "expected edge ({},{},{}) 在 actual 中不存在",
                        key.0, key.1, key.2
                    ),
                });
            }
            Some(a_edge) => {
                matched_keys.insert(key.clone());
                // properties deep equal
                let e_props = &e_edge["properties"];
                let a_props = &a_edge["properties"];
                // edges 在 v0 可能没有 properties 字段
                let e_has = e_props.is_object() || e_props.is_null();
                let a_has = a_props.is_object() || a_props.is_null();
                if e_has && a_has && !deep_equal(e_props, a_props) {
                    mismatches.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "edge".to_string(),
                        identifier: format!("({},{},{})", key.0, key.1, key.2),
                        field: "properties".to_string(),
                        expected: val_summary(e_props),
                        actual: val_summary(a_props),
                        detail: format!("edge ({},{},{}) properties 不一致", key.0, key.1, key.2),
                    });
                }
            }
        }
    }

    // extra edges in actual
    for a_edge in actual_edges {
        let key = edge_key(a_edge);
        if !matched_keys.contains(&key) {
            mismatches.push(GraphMismatch {
                fixture: fixture.to_string(),
                category: "edge".to_string(),
                identifier: format!("({},{},{})", key.0, key.1, key.2),
                field: "composite_key".to_string(),
                expected: "(not in expected)".to_string(),
                actual: format!("({},{},{})", key.0, key.1, key.2),
                detail: format!(
                    "actual edge ({},{},{}) 不在 expectedGraph.edges 中",
                    key.0, key.1, key.2
                ),
            });
        }
    }

    mismatches
}

// === Diagnostic 比较 ===

fn compare_diagnostics(
    fixture: &str,
    expected_diags: &[serde_json::Value],
    actual_diags: &[serde_json::Value],
) -> Vec<GraphMismatch> {
    let mut mismatches = Vec::new();

    let actual_map: std::collections::HashMap<&str, &serde_json::Value> = actual_diags
        .iter()
        .filter_map(|d| d["id"].as_str().map(|id| (id, d)))
        .collect();

    let mut matched_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for e_diag in expected_diags {
        let e_id = e_diag["id"].as_str().unwrap_or("?");
        match actual_map.get(e_id) {
            None => {
                mismatches.push(GraphMismatch {
                    fixture: fixture.to_string(),
                    category: "diagnostic".to_string(),
                    identifier: e_id.to_string(),
                    field: "id".to_string(),
                    expected: e_id.to_string(),
                    actual: "(missing)".to_string(),
                    detail: format!("expected diagnostic {} 在 actual 中不存在", e_id),
                });
            }
            Some(a_diag) => {
                matched_ids.insert(e_id);
                let e_props = &e_diag["properties"];
                let a_props = &a_diag["properties"];
                if !deep_equal(e_props, a_props) {
                    let diffs = find_json_diff(e_props, a_props, "properties");
                    for diff in diffs {
                        mismatches.push(GraphMismatch {
                            fixture: fixture.to_string(),
                            category: "diagnostic".to_string(),
                            identifier: e_id.to_string(),
                            field: diff.clone(),
                            expected: val_summary(e_props),
                            actual: val_summary(a_props),
                            detail: format!("diagnostic {} properties 不一致: {}", e_id, diff),
                        });
                    }
                }
            }
        }
    }

    for a_diag in actual_diags {
        let a_id = a_diag["id"].as_str().unwrap_or("?");
        if !matched_ids.contains(a_id) {
            mismatches.push(GraphMismatch {
                fixture: fixture.to_string(),
                category: "diagnostic".to_string(),
                identifier: a_id.to_string(),
                field: "id".to_string(),
                expected: "(not in expected)".to_string(),
                actual: a_id.to_string(),
                detail: format!(
                    "actual diagnostic {} 不在 expectedGraph.diagnostics 中",
                    a_id
                ),
            });
        }
    }

    mismatches
}

// === Stats 比较 ===

fn compare_stats(
    fixture: &str,
    expected_stats: &serde_json::Value,
    actual_stats: &serde_json::Value,
) -> Vec<GraphMismatch> {
    let mut mismatches = Vec::new();
    for field in &["nodeCount", "edgeCount", "diagnosticCount", "symbolCount"] {
        let e = expected_stats[*field].as_u64();
        let a = actual_stats[*field].as_u64();
        if e != a {
            mismatches.push(GraphMismatch {
                fixture: fixture.to_string(),
                category: "stats".to_string(),
                identifier: "stats".to_string(),
                field: field.to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
                detail: format!("stats.{} 不一致", field),
            });
        }
    }
    mismatches
}

// === expectedPresence 检查 ===

fn check_presence(
    fixture: &str,
    presences: &[serde_json::Value],
    actual_nodes: &[serde_json::Value],
    actual_edges: &[serde_json::Value],
    actual_diags: &[serde_json::Value],
) -> Vec<GraphMismatch> {
    let mut violations = Vec::new();

    for p in presences {
        let p_type = p["type"].as_str().unwrap_or("");

        match p_type {
            "nodeExists" => {
                let id = p["id"].as_str().unwrap_or("");
                let found = actual_nodes.iter().any(|n| n["id"].as_str() == Some(id));
                if !found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "presence".to_string(),
                        identifier: id.to_string(),
                        field: "nodeExists".to_string(),
                        expected: format!("node {} exists", id),
                        actual: "not found".to_string(),
                        detail: format!("expectedPresence: node {} 不存在", id),
                    });
                }
            }
            "edgeExists" => {
                let src = p["source"].as_str().unwrap_or("");
                let tgt = p["target"].as_str().unwrap_or("");
                let et = p["edgeType"].as_str().unwrap_or("");
                let found = actual_edges.iter().any(|e| {
                    e["source"].as_str() == Some(src)
                        && e["target"].as_str() == Some(tgt)
                        && e["type"].as_str() == Some(et)
                });
                if !found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "presence".to_string(),
                        identifier: format!("({},{},{})", et, src, tgt),
                        field: "edgeExists".to_string(),
                        expected: format!("edge ({},{},{}) exists", et, src, tgt),
                        actual: "not found".to_string(),
                        detail: format!("expectedPresence: edge ({},{},{}) 不存在", et, src, tgt),
                    });
                }
            }
            "diagnosticExists" => {
                let code = p["code"].as_str().unwrap_or("");
                let path = p["path"].as_str().unwrap_or("");
                let found = actual_diags.iter().any(|d| {
                    d["properties"]["code"].as_str() == Some(code)
                        && d["properties"]["path"].as_str() == Some(path)
                });
                if !found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "presence".to_string(),
                        identifier: format!("{}:{}", code, path),
                        field: "diagnosticExists".to_string(),
                        expected: format!("diagnostic code={} path={} exists", code, path),
                        actual: "not found".to_string(),
                        detail: format!(
                            "expectedPresence: diagnostic code={} path={} 不存在",
                            code, path
                        ),
                    });
                }
            }
            "nodeKindCount" => {
                let label = p["label"].as_str().unwrap_or("");
                let min = p["min"].as_u64().unwrap_or(0);
                let max = p["max"].as_u64().unwrap_or(u64::MAX);
                let count = actual_nodes
                    .iter()
                    .filter(|n| n["label"].as_str() == Some(label))
                    .count() as u64;
                if count < min || count > max {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "presence".to_string(),
                        identifier: label.to_string(),
                        field: "nodeKindCount".to_string(),
                        expected: format!("count in [{}, {}]", min, max),
                        actual: count.to_string(),
                        detail: format!(
                            "expectedPresence: node label {} count={} 不在 [{},{}] 范围",
                            label, count, min, max
                        ),
                    });
                }
            }
            "edgeKindCount" => {
                let et = p["edgeType"].as_str().unwrap_or("");
                let min = p["min"].as_u64().unwrap_or(0);
                let max = p["max"].as_u64().unwrap_or(u64::MAX);
                let count = actual_edges
                    .iter()
                    .filter(|e| e["type"].as_str() == Some(et))
                    .count() as u64;
                if count < min || count > max {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "presence".to_string(),
                        identifier: et.to_string(),
                        field: "edgeKindCount".to_string(),
                        expected: format!("count in [{}, {}]", min, max),
                        actual: count.to_string(),
                        detail: format!(
                            "expectedPresence: edge type {} count={} 不在 [{},{}] 范围",
                            et, count, min, max
                        ),
                    });
                }
            }
            _ => {
                violations.push(GraphMismatch {
                    fixture: fixture.to_string(),
                    category: "presence".to_string(),
                    identifier: p_type.to_string(),
                    field: "type".to_string(),
                    expected: "known presence type".to_string(),
                    actual: p_type.to_string(),
                    detail: format!("未知 expectedPresence type: {}", p_type),
                });
            }
        }
    }

    violations
}

// === expectedAbsence 检查 ===

fn check_absence(
    fixture: &str,
    absences: &[serde_json::Value],
    actual_nodes: &[serde_json::Value],
    actual_edges: &[serde_json::Value],
    actual_diags: &[serde_json::Value],
) -> Vec<GraphMismatch> {
    let mut violations = Vec::new();

    for a in absences {
        let a_type = a["type"].as_str().unwrap_or("");

        match a_type {
            "noEdgeKind" => {
                let et = a["edgeType"].as_str().unwrap_or("");
                let found = actual_edges.iter().any(|e| e["type"].as_str() == Some(et));
                if found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: et.to_string(),
                        field: "noEdgeKind".to_string(),
                        expected: format!("无 {} edge", et),
                        actual: format!("找到了 {} edge", et),
                        detail: format!("expectedAbsence: 发现 {} edge", et),
                    });
                }
            }
            "noFakeEdgeForDiagnostic" => {
                let code = a["code"].as_str().unwrap_or("");
                let has_diag = actual_diags
                    .iter()
                    .any(|d| d["properties"]["code"].as_str() == Some(code));
                if has_diag {
                    let diag_ids: Vec<&str> = actual_diags
                        .iter()
                        .filter(|d| d["properties"]["code"].as_str() == Some(code))
                        .filter_map(|d| d["id"].as_str())
                        .collect();
                    // 检查该 diagnostic 是否产生了非 ANNOTATES 的 edge
                    let bad_edges: Vec<&serde_json::Value> = actual_edges
                        .iter()
                        .filter(|e| {
                            let src = e["source"].as_str().unwrap_or("");
                            let tgt = e["target"].as_str().unwrap_or("");
                            let et = e["type"].as_str().unwrap_or("");
                            et != "ANNOTATES"
                                && (diag_ids.contains(&src) || diag_ids.contains(&tgt))
                        })
                        .collect();
                    if !bad_edges.is_empty() {
                        violations.push(GraphMismatch {
                            fixture: fixture.to_string(),
                            category: "absence".to_string(),
                            identifier: code.to_string(),
                            field: "noFakeEdgeForDiagnostic".to_string(),
                            expected: format!("diagnostic {} 只产生 ANNOTATES edge", code),
                            actual: format!("找到了 {} 条非 ANNOTATES edge", bad_edges.len()),
                            detail: format!(
                                "expectedAbsence: diagnostic {} 产生了非 ANNOTATES edge",
                                code
                            ),
                        });
                    }
                }
            }
            "noCalls" => {
                let found = actual_edges
                    .iter()
                    .any(|e| e["type"].as_str() == Some("CALLS"));
                if found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: "CALLS".to_string(),
                        field: "noCalls".to_string(),
                        expected: "无 CALLS edge".to_string(),
                        actual: "找到了 CALLS edge".to_string(),
                        detail: "expectedAbsence: 发现 CALLS edge（v0 stop-line）".to_string(),
                    });
                }
            }
            "noUses" => {
                let found = actual_edges
                    .iter()
                    .any(|e| e["type"].as_str() == Some("USES"));
                if found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: "USES".to_string(),
                        field: "noUses".to_string(),
                        expected: "无 USES edge".to_string(),
                        actual: "找到了 USES edge".to_string(),
                        detail: "expectedAbsence: 发现 USES edge（v0 stop-line）".to_string(),
                    });
                }
            }
            "noImports" => {
                let found = actual_edges
                    .iter()
                    .any(|e| e["type"].as_str() == Some("IMPORTS"));
                if found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: "IMPORTS".to_string(),
                        field: "noImports".to_string(),
                        expected: "无 IMPORTS edge".to_string(),
                        actual: "找到了 IMPORTS edge".to_string(),
                        detail: "expectedAbsence: 发现 IMPORTS edge（v0 stop-line）".to_string(),
                    });
                }
            }
            "noImplements" => {
                let found = actual_edges
                    .iter()
                    .any(|e| e["type"].as_str() == Some("IMPLEMENTS"));
                if found {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: "IMPLEMENTS".to_string(),
                        field: "noImplements".to_string(),
                        expected: "无 IMPLEMENTS edge".to_string(),
                        actual: "找到了 IMPLEMENTS edge".to_string(),
                        detail: "expectedAbsence: 发现 IMPLEMENTS edge（v0 stop-line）".to_string(),
                    });
                }
            }
            "noDuplicateNodeId" => {
                let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
                let mut dupes: std::collections::HashSet<&str> = std::collections::HashSet::new();
                for n in actual_nodes {
                    if let Some(id) = n["id"].as_str() {
                        if !seen.insert(id) {
                            dupes.insert(id);
                        }
                    }
                }
                if !dupes.is_empty() {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: format!("{:?}", dupes),
                        field: "noDuplicateNodeId".to_string(),
                        expected: "无重复 node id".to_string(),
                        actual: format!("{} 个重复 id", dupes.len()),
                        detail: format!("expectedAbsence: 发现重复 node id: {:?}", dupes),
                    });
                }
            }
            "noDuplicateEdgeKey" => {
                let mut seen: std::collections::HashSet<(String, String, String)> =
                    std::collections::HashSet::new();
                let mut dupes: Vec<(String, String, String)> = Vec::new();
                for e in actual_edges {
                    let key = edge_key(e);
                    if !seen.insert(key.clone()) {
                        dupes.push(key);
                    }
                }
                if !dupes.is_empty() {
                    violations.push(GraphMismatch {
                        fixture: fixture.to_string(),
                        category: "absence".to_string(),
                        identifier: format!("{:?}", dupes),
                        field: "noDuplicateEdgeKey".to_string(),
                        expected: "无重复 edge key".to_string(),
                        actual: format!("{} 个重复 key", dupes.len()),
                        detail: format!("expectedAbsence: 发现重复 edge key: {:?}", dupes),
                    });
                }
            }
            _ => {
                violations.push(GraphMismatch {
                    fixture: fixture.to_string(),
                    category: "absence".to_string(),
                    identifier: a_type.to_string(),
                    field: "type".to_string(),
                    expected: "known absence type".to_string(),
                    actual: a_type.to_string(),
                    detail: format!("未知 expectedAbsence type: {}", a_type),
                });
            }
        }
    }

    violations
}

// === 全量 comparison ===

struct GraphComparisonResult {
    fixture: String,
    mismatches: Vec<GraphMismatch>,
}

fn compare_fixture_graph(fixture: &GraphFixture) -> GraphComparisonResult {
    let expected = load_expected(fixture.input_root);
    let mut actual = inspect_graph(fixture);

    // 排除 runtime 字段
    if let Some(obj) = actual.as_object_mut() {
        obj.remove("generatedAt");
        obj.remove("root");
    }

    let mut mismatches = Vec::new();

    // schemaVersion
    mismatches.extend(compare_schema_version(fixture.name, &expected, &actual));

    // nodes
    let expected_nodes = expected["expectedGraph"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_nodes = actual["nodes"].as_array().cloned().unwrap_or_default();
    mismatches.extend(compare_nodes(fixture.name, &expected_nodes, &actual_nodes));

    // edges
    let expected_edges = expected["expectedGraph"]["edges"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_edges = actual["edges"].as_array().cloned().unwrap_or_default();
    mismatches.extend(compare_edges(fixture.name, &expected_edges, &actual_edges));

    // diagnostics
    let expected_diags = expected["expectedGraph"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_diags = actual["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    mismatches.extend(compare_diagnostics(
        fixture.name,
        &expected_diags,
        &actual_diags,
    ));

    // stats
    let expected_stats = &expected["expectedStats"];
    let actual_stats = &actual["stats"];
    mismatches.extend(compare_stats(fixture.name, expected_stats, actual_stats));

    // expectedPresence
    let presences = expected["expectedPresence"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    mismatches.extend(check_presence(
        fixture.name,
        &presences,
        &actual_nodes,
        &actual_edges,
        &actual_diags,
    ));

    // expectedAbsence
    let absences = expected["expectedAbsence"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    mismatches.extend(check_absence(
        fixture.name,
        &absences,
        &actual_nodes,
        &actual_edges,
        &actual_diags,
    ));

    GraphComparisonResult {
        fixture: fixture.name.to_string(),
        mismatches,
    }
}

// === Tests ===

#[test]
fn graph_fixtures_are_discoverable() {
    for f in GRAPH_FIXTURES {
        let dir = fixture_root(f.input_root);
        assert!(dir.exists(), "fixture 目录不存在: {:?}", dir);
        let expected = expected_graph_path(f.input_root);
        assert!(
            expected.exists(),
            "expected-graph.json 不存在: {:?}",
            expected
        );
    }
}

#[test]
fn expected_graph_json_parses_for_all_fixtures() {
    for f in GRAPH_FIXTURES {
        let expected = load_expected(f.input_root);
        assert!(
            expected.is_object(),
            "expected-graph.json 顶层不是对象 for {}",
            f.name
        );
        assert!(
            expected["expectedGraph"].is_object(),
            "缺少 expectedGraph for {}",
            f.name
        );
        assert!(
            expected["expectedGraph"]["nodes"].is_array(),
            "缺少 expectedGraph.nodes for {}",
            f.name
        );
        assert!(
            expected["expectedGraph"]["edges"].is_array(),
            "缺少 expectedGraph.edges for {}",
            f.name
        );
        assert!(
            expected["expectedGraph"]["diagnostics"].is_array(),
            "缺少 expectedGraph.diagnostics for {}",
            f.name
        );
        assert!(
            expected["expectedStats"].is_object(),
            "缺少 expectedStats for {}",
            f.name
        );
        assert!(
            expected["expectedPresence"].is_array(),
            "缺少 expectedPresence for {}",
            f.name
        );
        assert!(
            expected["expectedAbsence"].is_array(),
            "缺少 expectedAbsence for {}",
            f.name
        );
    }
}

#[test]
fn actual_graph_output_parses_for_all_fixtures() {
    for f in GRAPH_FIXTURES {
        let actual = inspect_graph(f);
        assert!(
            actual.is_object(),
            "actual output 顶层不是对象 for {}",
            f.name
        );
        assert!(actual["nodes"].is_array(), "缺少 nodes for {}", f.name);
        assert!(actual["edges"].is_array(), "缺少 edges for {}", f.name);
        assert!(
            actual["diagnostics"].is_array(),
            "缺少 diagnostics for {}",
            f.name
        );
        assert!(actual["stats"].is_object(), "缺少 stats for {}", f.name);
    }
}

#[test]
fn graph_comparison_passes_for_all_fixtures() {
    let mut total_mismatches = 0;
    for f in GRAPH_FIXTURES {
        let result = compare_fixture_graph(f);
        if result.mismatches.is_empty() {
            eprintln!("  PASS: {}", result.fixture);
        } else {
            eprintln!(
                "  FAIL: {} — {} mismatches",
                result.fixture,
                result.mismatches.len()
            );
            for m in &result.mismatches {
                eprintln!("{}", m);
            }
            total_mismatches += result.mismatches.len();
        }
    }
    assert_eq!(
        total_mismatches, 0,
        "graph comparison: {} total mismatches across all fixtures",
        total_mismatches
    );
}
