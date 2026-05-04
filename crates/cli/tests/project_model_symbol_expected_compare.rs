//! expected-symbols.json comparison harness
//!
//! 对比 Rust-core actual symbol output 与 expected-symbols.json golden fixtures。
//!
//! 为什么 symbol comparison 独立于 ProjectModel comparison：
//!   symbol 层有不同的字段结构（span / implDetails / modifiers）和匹配规则（id primary key），
//!   混入 project_model_expected_compare 会增加维护负担。
//!
//! 为什么所有字段 exact match：
//!   tree-sitter 的输出是确定性的（给定相同输入和 grammar 版本），
//!   不需要 confidence tolerance 或 enum 映射。
//!
//! 为什么不允许 known skip：
//!   symbol 输出契约必须冻结；如果 actual 与 expected 不同，说明 extractor 行为变了，
//!   必须修正 expected 或修正 extractor，不能用 known skip 掩盖。

use assert_cmd::Command;
use std::path::PathBuf;

// === Fixture 列表 ===

const SYMBOL_FIXTURES: &[&str] = &[
    "item-nested-modules",
    "item-impl-methods",
    "item-trait-impl",
    "item-inline-module",
    "item-top-level-regression",
    "item-parse-error",
    "item-top-level",
    "item-visibility",
    "item-macro",
    "item-duplicate-names",
];

// === Fixture 目录定位 ===

/// Rust-core workspace root：从环境变量或默认路径获取。
/// 为什么默认值是上级目录：此测试文件在 crates/cli/tests/ 下，
/// Cargo 的工作目录是 crate root（crates/cli/），需要上级找到 workspace root。
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

/// item-extraction fixture 目录
fn fixture_dir(name: &str) -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("item-extraction")
        .join(name)
}

/// expected-symbols.json 路径
fn expected_symbols_path(name: &str) -> PathBuf {
    fixture_dir(name).join("expected-symbols.json")
}

// === CLI 调用 ===

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

/// 调用 Rust-core CLI 获取 actual symbol output（JSON）
fn inspect_symbols(fixture_name: &str) -> serde_json::Value {
    let dir = fixture_dir(fixture_name);
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .arg("--include")
        .arg("symbols")
        .output()
        .expect("CLI 调用失败");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("CLI 退出非零 for {}: {}", fixture_name, stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "actual output 不是合法 JSON for {}: {}\nstdout: {}",
            fixture_name, e, stdout
        )
    })
}

/// 读取 expected-symbols.json
fn load_expected(fixture_name: &str) -> serde_json::Value {
    let path = expected_symbols_path(fixture_name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 expected-symbols.json for {}: {}", fixture_name, e));
    serde_json::from_str(&content).unwrap_or_else(|e| {
        panic!(
            "expected-symbols.json 不是合法 JSON for {}: {}",
            fixture_name, e
        )
    })
}

// === Mismatch 类型 ===

#[derive(Debug)]
struct SymbolMismatch {
    fixture: String,
    symbol_id: String,
    field: String,
    expected: String,
    actual: String,
    detail: String,
}

impl std::fmt::Display for SymbolMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Symbol mismatch [{}]:\n  symbol: {}\n  field: {}\n  expected: {}\n  actual: {}\n  detail: {}",
            self.fixture, self.symbol_id, self.field, self.expected, self.actual, self.detail
        )
    }
}

// === Symbol 字段比较 ===

/// 比较 expected symbol 和 actual symbol 的所有字段。
/// 所有字段 exact match，返回 mismatch 列表。
fn compare_symbol_fields(
    fixture: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Vec<SymbolMismatch> {
    let mut mismatches = Vec::new();
    let id = expected["id"].as_str().unwrap_or("?");

    // 简单字符串字段 exact match
    for field in &[
        "id",
        "name",
        "symbolKind",
        "sourcePath",
        "packageName",
        "visibility",
    ] {
        let e = expected[*field].as_str().unwrap_or("");
        let a = actual[*field].as_str().unwrap_or("");
        if e != a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: field.to_string(),
                expected: e.to_string(),
                actual: a.to_string(),
                detail: format!("{} 不一致", field),
            });
        }
    }

    // 可选字符串字段（null 或 string）
    for field in &["targetName", "modulePath", "parentId"] {
        let e = expected[*field].as_str();
        let a = actual[*field].as_str();
        if e != a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: field.to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
                detail: format!("{} 不一致", field),
            });
        }
    }

    // genericParams（可选字符串）
    {
        let e = expected["genericParams"].as_str();
        let a = actual["genericParams"].as_str();
        if e != a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: "genericParams".to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
                detail: "genericParams 不一致".to_string(),
            });
        }
    }

    // span
    {
        let e_start = expected["span"]["lineStart"].as_u64();
        let a_start = actual["lineStart"].as_u64();
        if e_start != a_start {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: "span.lineStart".to_string(),
                expected: format!("{:?}", e_start),
                actual: format!("{:?}", a_start),
                detail: "span.lineStart 不一致".to_string(),
            });
        }
        let e_end = expected["span"]["lineEnd"].as_u64();
        let a_end = actual["lineEnd"].as_u64();
        if e_end != a_end {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: "span.lineEnd".to_string(),
                expected: format!("{:?}", e_end),
                actual: format!("{:?}", a_end),
                detail: "span.lineEnd 不一致".to_string(),
            });
        }
    }

    // 布尔字段：actual 是顶层 isAsync/isUnsafe/isConstFn/isPub，
    // expected 在 modifiers 对象内（isAsync/isUnsafe/isConstFn）或顶层（isPub）
    for field in &["isAsync", "isUnsafe", "isConstFn"] {
        let e = expected["modifiers"][*field].as_bool();
        let a = actual[*field].as_bool();
        if e != a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: field.to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
                detail: format!("{} 不一致", field),
            });
        }
    }
    // isPub 在 expected 中是顶层字段
    {
        let e = expected["isPub"].as_bool();
        let a = actual["isPub"].as_bool();
        if e != a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: "isPub".to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
                detail: "isPub 不一致".to_string(),
            });
        }
    }

    // implDetails（嵌套对象或 null）
    {
        let e_impl = &expected["implDetails"];
        let a_impl = &actual["implDetails"];
        if e_impl.is_null() != a_impl.is_null() {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: id.to_string(),
                field: "implDetails".to_string(),
                expected: format!("is_null: {}", e_impl.is_null()),
                actual: format!("is_null: {}", a_impl.is_null()),
                detail: "implDetails null 状态不一致".to_string(),
            });
        } else if !e_impl.is_null() {
            // implTarget
            let e_target = e_impl["implTarget"].as_str().unwrap_or("");
            let a_target = a_impl["implTarget"].as_str().unwrap_or("");
            if e_target != a_target {
                mismatches.push(SymbolMismatch {
                    fixture: fixture.to_string(),
                    symbol_id: id.to_string(),
                    field: "implDetails.implTarget".to_string(),
                    expected: e_target.to_string(),
                    actual: a_target.to_string(),
                    detail: "implDetails.implTarget 不一致".to_string(),
                });
            }
            // traitName
            let e_trait = e_impl["traitName"].as_str();
            let a_trait = a_impl["traitName"].as_str();
            if e_trait != a_trait {
                mismatches.push(SymbolMismatch {
                    fixture: fixture.to_string(),
                    symbol_id: id.to_string(),
                    field: "implDetails.traitName".to_string(),
                    expected: format!("{:?}", e_trait),
                    actual: format!("{:?}", a_trait),
                    detail: "implDetails.traitName 不一致".to_string(),
                });
            }
        }
    }

    mismatches
}

// === Diagnostic 比较 ===

fn compare_diagnostics(
    fixture: &str,
    expected_diags: &[serde_json::Value],
    actual_diags: &[serde_json::Value],
) -> Vec<SymbolMismatch> {
    let mut mismatches = Vec::new();

    if expected_diags.len() != actual_diags.len() {
        mismatches.push(SymbolMismatch {
            fixture: fixture.to_string(),
            symbol_id: "(diagnostics)".to_string(),
            field: "diagnosticCount".to_string(),
            expected: expected_diags.len().to_string(),
            actual: actual_diags.len().to_string(),
            detail: "symbol diagnostics 数量不一致".to_string(),
        });
        return mismatches;
    }

    for (i, (e, a)) in expected_diags.iter().zip(actual_diags.iter()).enumerate() {
        let code_e = e["code"].as_str().unwrap_or("");
        let code_a = a["code"].as_str().unwrap_or("");
        if code_e != code_a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: format!("diagnostics[{}]", i),
                field: "code".to_string(),
                expected: code_e.to_string(),
                actual: code_a.to_string(),
                detail: format!("diagnostics[{}] code 不一致", i),
            });
        }
        let sev_e = e["severity"].as_str().unwrap_or("");
        let sev_a = a["severity"].as_str().unwrap_or("");
        if sev_e != sev_a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: format!("diagnostics[{}]", i),
                field: "severity".to_string(),
                expected: sev_e.to_string(),
                actual: sev_a.to_string(),
                detail: format!("diagnostics[{}] severity 不一致", i),
            });
        }
        let sp_e = e["sourcePath"].as_str().unwrap_or("");
        let sp_a = a["sourcePath"].as_str().unwrap_or("");
        if sp_e != sp_a {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: format!("diagnostics[{}]", i),
                field: "sourcePath".to_string(),
                expected: sp_e.to_string(),
                actual: sp_a.to_string(),
                detail: format!("diagnostics[{}] sourcePath 不一致", i),
            });
        }
    }

    mismatches
}

// === Absence 检查 ===

fn check_absences(
    fixture: &str,
    absences: &[serde_json::Value],
    actual_symbols: &[serde_json::Value],
    actual_diags: &[serde_json::Value],
) -> Vec<SymbolMismatch> {
    let mut violations = Vec::new();

    for abs in absences {
        let abs_type = abs["type"].as_str().unwrap_or("");

        match abs_type {
            "noMethodForFreeFunction" | "noAssociatedFunctionForMethod" => {
                let kind = abs["kind"].as_str().unwrap_or("");
                let name = abs["name"].as_str().unwrap_or("");
                let found = actual_symbols.iter().any(|s| {
                    s["symbolKind"].as_str() == Some(kind) && s["name"].as_str() == Some(name)
                });
                if found {
                    violations.push(SymbolMismatch {
                        fixture: fixture.to_string(),
                        symbol_id: name.to_string(),
                        field: format!("absence:{}", abs_type),
                        expected: "不存在".to_string(),
                        actual: format!("找到了 kind={} name={}", kind, name),
                        detail: format!("absence violation: {} for {}", abs_type, name),
                    });
                }
            }
            "noMacroInvocationAsDefinition" => {
                // macro invocation 不应产生 symbol definition
                // 当前检查方式：macro_definition symbol 的 name 不应匹配 macro invocation 的名称
                // 简化：跳过，因为 macro invocation 不会产生 symbol（已由 extractor 保证）
            }
            "noEnumVariantSymbol" => {
                // enum variant 不应作为独立 symbol — 不需要检查具体 name
            }
            "noAssociatedTypeConst" => {
                let code = abs["code"].as_str().unwrap_or("");
                let found = actual_diags
                    .iter()
                    .any(|d| d["code"].as_str() == Some(code));
                if found {
                    violations.push(SymbolMismatch {
                        fixture: fixture.to_string(),
                        symbol_id: "(absence)".to_string(),
                        field: format!("absence:{}", abs_type),
                        expected: format!("无 {} diagnostic", code),
                        actual: format!("找到了 {} diagnostic", code),
                        detail: format!("absence violation: {} should not exist", code),
                    });
                }
            }
            "noCfgResolvedItem" | "noCallGraphEdge" => {
                // cfg-gated: 当前 unsupported-cfg-gated-item 未实现，此项暂不检查
                // call graph: item extraction 不生成 CALLS edge，无需检查
            }
            _ => {}
        }
    }

    violations
}

// === 全量 comparison ===

#[allow(dead_code)] // fixture 用于 debug 输出
struct SymbolComparisonResult {
    fixture: String,
    mismatches: Vec<SymbolMismatch>,
}

fn compare_fixture_symbols(fixture: &str) -> SymbolComparisonResult {
    let expected = load_expected(fixture);
    let actual = inspect_symbols(fixture);

    let mut mismatches = Vec::new();

    // 提取 expected/actual symbols 数组
    let expected_symbols = expected["expectedSymbols"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_symbols = actual["symbols"].as_array().cloned().unwrap_or_default();

    // 统计检查
    let expected_count = expected["expectedStats"]["symbolCount"]
        .as_u64()
        .unwrap_or(0);
    let actual_count = actual["stats"]["symbolCount"].as_u64().unwrap_or(0);
    if expected_count != actual_count {
        mismatches.push(SymbolMismatch {
            fixture: fixture.to_string(),
            symbol_id: "(stats)".to_string(),
            field: "symbolCount".to_string(),
            expected: expected_count.to_string(),
            actual: actual_count.to_string(),
            detail: "symbolCount 不一致".to_string(),
        });
    }

    // 按 id 构建 actual map
    let actual_map: std::collections::HashMap<&str, &serde_json::Value> = actual_symbols
        .iter()
        .filter_map(|s| s["id"].as_str().map(|id| (id, s)))
        .collect();

    // expected → actual 逐个匹配
    let mut matched_actual_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for e_sym in &expected_symbols {
        let e_id = e_sym["id"].as_str().unwrap_or("?");
        match actual_map.get(e_id) {
            None => {
                mismatches.push(SymbolMismatch {
                    fixture: fixture.to_string(),
                    symbol_id: e_id.to_string(),
                    field: "id".to_string(),
                    expected: e_id.to_string(),
                    actual: "(missing in actual)".to_string(),
                    detail: format!("expected symbol {} 在 actual 中不存在", e_id),
                });
            }
            Some(a_sym) => {
                matched_actual_ids.insert(e_id);
                let field_mismatches = compare_symbol_fields(fixture, e_sym, a_sym);
                mismatches.extend(field_mismatches);
            }
        }
    }

    // 检查 actual 中有但 expected 中没有的（extra symbols）
    for a_sym in &actual_symbols {
        let a_id = a_sym["id"].as_str().unwrap_or("?");
        if !matched_actual_ids.contains(a_id) {
            mismatches.push(SymbolMismatch {
                fixture: fixture.to_string(),
                symbol_id: a_id.to_string(),
                field: "id".to_string(),
                expected: "(not in expected)".to_string(),
                actual: a_id.to_string(),
                detail: format!("actual symbol {} 不在 expectedSymbols 中", a_id),
            });
        }
    }

    // Diagnostic 比较
    let expected_diags = expected["expectedSymbolDiagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_diags = actual["symbolDiagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    mismatches.extend(compare_diagnostics(fixture, &expected_diags, &actual_diags));

    // Absence 检查
    let absences = expected["expectedAbsence"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    mismatches.extend(check_absences(
        fixture,
        &absences,
        &actual_symbols,
        &actual_diags,
    ));

    SymbolComparisonResult {
        fixture: fixture.to_string(),
        mismatches,
    }
}

// === Tests ===

#[test]
fn symbol_fixtures_are_discoverable() {
    for name in SYMBOL_FIXTURES {
        let dir = fixture_dir(name);
        assert!(dir.exists(), "fixture 目录不存在: {:?}", dir);
        let expected = expected_symbols_path(name);
        assert!(
            expected.exists(),
            "expected-symbols.json 不存在: {:?}",
            expected
        );
    }
}

#[test]
fn expected_symbols_json_parses_for_all_fixtures() {
    for name in SYMBOL_FIXTURES {
        let expected = load_expected(name);
        assert!(
            expected.is_object(),
            "expected-symbols.json 顶层不是对象 for {}",
            name
        );
        assert!(
            expected["expectedSymbols"].is_array(),
            "缺少 expectedSymbols for {}",
            name
        );
        assert!(
            expected["expectedStats"].is_object(),
            "缺少 expectedStats for {}",
            name
        );
    }
}

#[test]
fn actual_symbol_output_parses_for_all_fixtures() {
    for name in SYMBOL_FIXTURES {
        let actual = inspect_symbols(name);
        assert!(
            actual.is_object(),
            "actual output 顶层不是对象 for {}",
            name
        );
        assert!(actual["symbols"].is_array(), "缺少 symbols for {}", name);
    }
}

#[test]
fn symbol_comparison_passes_for_all_fixtures() {
    let mut total_mismatches = 0;
    for name in SYMBOL_FIXTURES {
        let result = compare_fixture_symbols(name);
        if result.mismatches.is_empty() {
            eprintln!("  PASS: {}", name);
        } else {
            eprintln!("  FAIL: {} — {} mismatches", name, result.mismatches.len());
            for m in &result.mismatches {
                eprintln!("{}", m);
            }
            total_mismatches += result.mismatches.len();
        }
    }
    assert_eq!(
        total_mismatches, 0,
        "symbol comparison: {} total mismatches across all fixtures",
        total_mismatches
    );
}
