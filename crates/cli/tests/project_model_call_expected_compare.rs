//! expected-calls.json comparison harness
//!
//! 对比 Rust-core actual call output 与 expected-calls.json golden fixtures。
//!
//! 匹配策略：按 (sourcePath, span.lineStart, calleeName) 配对，逐字段比较。
//! 核心字段 exact match，confidence 使用阈值比较。

use assert_cmd::Command;
use std::path::PathBuf;

const CALL_FIXTURES: &[&str] = &[
    "c1-same-module",
    "c2-import-binding",
    "c3-crate-path",
    "c4-self-path",
    "c5-super-path",
    "c6-associated-fn",
    "c7-method-call",
    // same-file unique-name heuristic fixtures
    "sf1-unique-helper",
    "sf2-duplicate-name",
    "sf3-method-ignored",
    "sf4-exact-priority",
    "sf5-cross-module-unique",
    // compile-valid fixture: inline module same-module call with flat modulePath
    "sf6-inline-module-flat",
    // enum constructor filter fixture
    "call-enum-filter",
    // bare module path resolution fixture (module::func is crate-relative)
    "call-module-path",
    // blind method name resolution fixtures
    "c8-method-resolution",
    "c9-method-ambiguous",
    // external crate call classification fixture
    "c10-external-crate",
    // receiver-type-aware method resolution fixture (Phase 2)
    "c11-receiver-type",
];

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

fn fixture_dir(name: &str) -> PathBuf {
    workspace_root()
        .join("fixtures")
        .join("call-resolution")
        .join(name)
}

fn expected_calls_path(name: &str) -> PathBuf {
    fixture_dir(name).join("expected-calls.json")
}

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn inspect_calls(fixture_name: &str) -> serde_json::Value {
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
        .arg("--include")
        .arg("imports")
        .arg("--include")
        .arg("calls")
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

fn load_expected(fixture_name: &str) -> serde_json::Value {
    let path = expected_calls_path(fixture_name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 expected-calls.json for {}: {}", fixture_name, e));
    serde_json::from_str(&content).unwrap_or_else(|e| {
        panic!(
            "expected-calls.json 不是合法 JSON for {}: {}",
            fixture_name, e
        )
    })
}

#[derive(Debug)]
struct CallMismatch {
    fixture: String,
    call_key: String,
    field: String,
    expected: String,
    actual: String,
}

impl std::fmt::Display for CallMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Call mismatch [{}]:\n  call: {}\n  field: {}\n  expected: {}\n  actual: {}",
            self.fixture, self.call_key, self.field, self.expected, self.actual
        )
    }
}

fn compare_call_fields(
    fixture: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Vec<CallMismatch> {
    let mut mismatches = Vec::new();
    let key = format!(
        "{}:{}:{}",
        expected["sourcePath"].as_str().unwrap_or("?"),
        expected["span"]["lineStart"].as_u64().unwrap_or(0),
        expected["calleeName"].as_str().unwrap_or("?")
    );

    for field in &[
        "sourcePath",
        "calleePath",
        "calleeName",
        "callKind",
        "reason",
    ] {
        let e = expected[*field].as_str().unwrap_or("");
        let a = actual[*field].as_str().unwrap_or("");
        if e != a {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: key.clone(),
                field: field.to_string(),
                expected: e.to_string(),
                actual: a.to_string(),
            });
        }
    }

    // 可选字符串字段
    for field in &[
        "callerSymbolId",
        "callerName",
        "resolvedSymbolId",
        "resolvedSymbolKind",
        "knownCrate",
    ] {
        let e = expected[*field].as_str();
        let a = actual[*field].as_str();
        if e != a {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: key.clone(),
                field: field.to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
            });
        }
    }

    // modulePath (nullable)
    {
        let e = expected["modulePath"].as_str();
        let a = actual["modulePath"].as_str();
        if e != a {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: key.clone(),
                field: "modulePath".to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
            });
        }
    }

    // span.lineStart / lineEnd
    for field in &["lineStart", "lineEnd"] {
        let e = expected["span"][*field].as_u64();
        let a = actual["span"][*field].as_u64();
        if e != a {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: key.clone(),
                field: format!("span.{}", field),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
            });
        }
    }

    // confidence（浮点数，使用阈值比较）
    {
        let e_conf = expected["confidence"].as_f64().unwrap_or(-1.0);
        let a_conf = actual["confidence"].as_f64().unwrap_or(-1.0);
        if (e_conf - a_conf).abs() > 0.05 {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: key.clone(),
                field: "confidence".to_string(),
                expected: format!("{:.2}", e_conf),
                actual: format!("{:.2}", a_conf),
            });
        }
    }

    // diagnostics code 检查
    {
        let e_diag_codes: Vec<String> = expected["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|d| d["code"].as_str().map(|s| s.to_string()))
            .collect();
        let a_diag_codes: Vec<String> = actual["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|d| d["code"].as_str().map(|s| s.to_string()))
            .collect();

        for expected_code in &e_diag_codes {
            if !a_diag_codes.contains(expected_code) {
                mismatches.push(CallMismatch {
                    fixture: fixture.to_string(),
                    call_key: key.clone(),
                    field: format!("diagnostic:{}", expected_code),
                    expected: "present".to_string(),
                    actual: "missing".to_string(),
                });
            }
        }
    }

    mismatches
}

#[allow(dead_code)] // fixture 用于 debug 输出
struct CallComparisonResult {
    fixture: String,
    mismatches: Vec<CallMismatch>,
}

fn compare_fixture_calls(fixture: &str) -> CallComparisonResult {
    let expected_raw = load_expected(fixture);
    let actual_json = inspect_calls(fixture);

    let mut mismatches = Vec::new();

    let expected_calls = expected_raw.as_array().cloned().unwrap_or_default();
    let actual_calls = actual_json["calls"].as_array().cloned().unwrap_or_default();

    // 计数检查
    if expected_calls.len() != actual_calls.len() {
        mismatches.push(CallMismatch {
            fixture: fixture.to_string(),
            call_key: "(count)".to_string(),
            field: "callCount".to_string(),
            expected: expected_calls.len().to_string(),
            actual: actual_calls.len().to_string(),
        });
    }

    // 按 (sourcePath, lineStart, calleeName) 配对
    type CallKey = (String, u64, String);
    let actual_map: std::collections::HashMap<CallKey, &serde_json::Value> = actual_calls
        .iter()
        .filter_map(|c| {
            let sp = c["sourcePath"].as_str().unwrap_or("").to_string();
            let ls = c["span"]["lineStart"].as_u64().unwrap_or(0);
            let cn = c["calleeName"].as_str().unwrap_or("").to_string();
            if sp.is_empty() || cn.is_empty() {
                None
            } else {
                Some(((sp, ls, cn), c))
            }
        })
        .collect();

    let mut matched_keys: std::collections::HashSet<CallKey> = std::collections::HashSet::new();

    for e_call in &expected_calls {
        let e_sp = e_call["sourcePath"].as_str().unwrap_or("").to_string();
        let e_ls = e_call["span"]["lineStart"].as_u64().unwrap_or(0);
        let e_cn = e_call["calleeName"].as_str().unwrap_or("").to_string();
        let key = (e_sp.clone(), e_ls, e_cn.clone());

        match actual_map.get(&key) {
            None => {
                mismatches.push(CallMismatch {
                    fixture: fixture.to_string(),
                    call_key: format!("{}:{}:{}", e_sp, e_ls, e_cn),
                    field: "match".to_string(),
                    expected: "present".to_string(),
                    actual: "missing in actual".to_string(),
                });
            }
            Some(a_call) => {
                matched_keys.insert(key);
                mismatches.extend(compare_call_fields(fixture, e_call, a_call));
            }
        }
    }

    // 检查 actual 中多余的条目
    for (key, _) in &actual_map {
        if !matched_keys.contains(key) {
            mismatches.push(CallMismatch {
                fixture: fixture.to_string(),
                call_key: format!("{}:{}:{}", key.0, key.1, key.2),
                field: "match".to_string(),
                expected: "not in expected".to_string(),
                actual: "present in actual".to_string(),
            });
        }
    }

    CallComparisonResult {
        fixture: fixture.to_string(),
        mismatches,
    }
}

#[test]
fn call_fixtures_are_discoverable() {
    for name in CALL_FIXTURES {
        let dir = fixture_dir(name);
        assert!(dir.exists(), "fixture 目录不存在: {:?}", dir);
        let expected = expected_calls_path(name);
        assert!(
            expected.exists(),
            "expected-calls.json 不存在: {:?}",
            expected
        );
    }
}

#[test]
fn expected_calls_json_parses_for_all_fixtures() {
    for name in CALL_FIXTURES {
        let expected = load_expected(name);
        assert!(
            expected.is_array(),
            "expected-calls.json 顶层不是数组 for {}",
            name
        );
    }
}

#[test]
fn actual_call_output_parses_for_all_fixtures() {
    for name in CALL_FIXTURES {
        let actual = inspect_calls(name);
        assert!(
            actual.is_object(),
            "actual output 顶层不是对象 for {}",
            name
        );
        assert!(actual["calls"].is_array(), "缺少 calls for {}", name);
    }
}

#[test]
fn call_comparison_passes_for_all_fixtures() {
    let mut total_mismatches = 0;
    for name in CALL_FIXTURES {
        let result = compare_fixture_calls(name);
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
        "call comparison: {} total mismatches across all fixtures",
        total_mismatches
    );
}

#[test]
fn calls_not_present_without_flag() {
    let dir = fixture_dir("c1-same-module");
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

    assert!(output.status.success(), "CLI 应成功退出");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual: serde_json::Value = serde_json::from_str(&stdout).expect("应解析为 JSON");

    let calls = actual["calls"].as_array();
    assert!(calls.is_some(), "calls 字段应存在（空数组）");
    assert!(
        calls.unwrap().is_empty(),
        "calls 应为空数组（无 --include calls）"
    );

    let call_count = actual["stats"]["callCount"].as_u64();
    assert_eq!(call_count, Some(0), "callCount 应为 0");
}

#[test]
fn calls_auto_triggers_symbols_and_imports() {
    let dir = fixture_dir("c1-same-module");
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .arg("--include")
        .arg("calls")
        .output()
        .expect("CLI 调用失败");

    assert!(output.status.success(), "CLI 应成功退出");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual: serde_json::Value = serde_json::from_str(&stdout).expect("应解析为 JSON");

    // calls 应被提取
    let calls = actual["calls"].as_array();
    assert!(calls.is_some(), "calls 字段应存在");
    assert!(
        !calls.unwrap().is_empty(),
        "calls 应非空（auto symbols + imports）"
    );

    // imports 应被提取（auto trigger）
    let imports = actual["imports"].as_array();
    assert!(
        imports.is_some(),
        "imports 字段应存在（auto triggered by calls）"
    );

    // symbols 不应输出（只有 --include symbols 才输出）
    let symbols = actual["symbols"].as_array();
    assert!(symbols.is_some(), "symbols 字段应存在");
    assert!(
        symbols.unwrap().is_empty(),
        "symbols 应为空数组（无 --include symbols）"
    );
}

/// 验证 external crate call stats 非硬编码为 0
/// c10-external-crate fixture 有 8 个 calls，其中 3 个 callKind="external-crate" + knownCrate="std"
#[test]
fn external_crate_stats_are_computed() {
    let raw = inspect_calls("c10-external-crate");

    let call_external_crate_total = raw["stats"]["callExternalCrateTotal"].as_u64();
    let call_external_crate_classified = raw["stats"]["callExternalCrateClassified"].as_u64();

    assert!(
        call_external_crate_total.is_some(),
        "stats.callExternalCrateTotal 应存在"
    );
    assert!(
        call_external_crate_classified.is_some(),
        "stats.callExternalCrateClassified 应存在"
    );

    let total = call_external_crate_total.unwrap();
    let classified = call_external_crate_classified.unwrap();

    assert!(
        total > 0,
        "callExternalCrateTotal 应 > 0（c10 有 3 个 external-crate calls），实际: {}",
        total
    );
    assert!(
        classified > 0,
        "callExternalCrateClassified 应 > 0（c10 有 3 个 knownCrate 非空 calls），实际: {}",
        classified
    );

    // 验证与 actual calls 一致性
    let calls = raw["calls"].as_array().unwrap();
    let actual_external = calls
        .iter()
        .filter(|c| c["callKind"].as_str() == Some("external-crate"))
        .count();
    let actual_classified = calls
        .iter()
        .filter(|c| c["knownCrate"].as_str().is_some())
        .count();

    assert_eq!(
        total, actual_external as u64,
        "callExternalCrateTotal ({}) 应与 callKind=external-crate 的 calls 数 ({}) 一致",
        total, actual_external
    );
    assert_eq!(
        classified, actual_classified as u64,
        "callExternalCrateClassified ({}) 应与 knownCrate 非空 calls 数 ({}) 一致",
        classified, actual_classified
    );
}
