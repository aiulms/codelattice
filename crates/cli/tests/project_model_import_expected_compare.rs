//! expected-imports.json comparison harness
//!
//! 对比 Rust-core actual import output 与 expected-imports.json golden fixtures。
//!
//! 为什么 import comparison 独立于 symbol comparison：
//!   import 有不同的字段结构（resolvedTo / pathKind / alias / isReExport）和匹配规则，
//!   混入 symbol harness 会增加维护负担。
//!
//! 为什么核心字段 exact match：
//!   text-level extractor 的输出是确定性的（给定相同输入），
//!   不需要 confidence tolerance 或 enum 映射。
//!
//! 匹配策略：按 (sourcePath, originalPath) 配对，逐字段比较。

use assert_cmd::Command;
use std::path::PathBuf;

const IMPORT_FIXTURES: &[&str] = &[
    "use-crate-simple",
    "use-grouped",
    "use-alias",
    "use-reexport",
    "use-self-super",
    "use-unsupported",
    "use-self-super-out-of-line",
    "s6-ambiguous-symbol",
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
        .join("import-use")
        .join(name)
}

fn expected_imports_path(name: &str) -> PathBuf {
    fixture_dir(name).join("expected-imports.json")
}

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn inspect_imports(fixture_name: &str) -> serde_json::Value {
    let dir = fixture_dir(fixture_name);
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .arg("--include")
        .arg("imports")
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
    let path = expected_imports_path(fixture_name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("无法读取 expected-imports.json for {}: {}", fixture_name, e));
    serde_json::from_str(&content).unwrap_or_else(|e| {
        panic!(
            "expected-imports.json 不是合法 JSON for {}: {}",
            fixture_name, e
        )
    })
}

#[derive(Debug)]
struct ImportMismatch {
    fixture: String,
    import_key: String,
    field: String,
    expected: String,
    actual: String,
}

impl std::fmt::Display for ImportMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Import mismatch [{}]:\n  import: {}\n  field: {}\n  expected: {}\n  actual: {}",
            self.fixture, self.import_key, self.field, self.expected, self.actual
        )
    }
}

fn compare_import_fields(
    fixture: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Vec<ImportMismatch> {
    let mut mismatches = Vec::new();
    let key = format!(
        "{}:{}",
        expected["sourcePath"].as_str().unwrap_or("?"),
        expected["originalPath"].as_str().unwrap_or("?")
    );

    for field in &[
        "sourcePath",
        "originalPath",
        "pathKind",
        "targetName",
        "visibility",
        "reason",
    ] {
        let e = expected[*field].as_str().unwrap_or("");
        let a = actual[*field].as_str().unwrap_or("");
        if e != a {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: field.to_string(),
                expected: e.to_string(),
                actual: a.to_string(),
            });
        }
    }

    // 可选字符串字段
    for field in &["alias"] {
        let e = expected[*field].as_str();
        let a = actual[*field].as_str();
        if e != a {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: field.to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
            });
        }
    }

    // 布尔字段
    {
        let e = expected["isReExport"].as_bool();
        let a = actual["isReExport"].as_bool();
        if e != a {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "isReExport".to_string(),
                expected: format!("{:?}", e),
                actual: format!("{:?}", a),
            });
        }
    }

    // resolvedTo
    {
        let e_null = expected["resolvedTo"].is_null();
        let a_null = actual["resolvedTo"].is_null();
        if e_null != a_null {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "resolvedTo".to_string(),
                expected: if e_null {
                    "null".to_string()
                } else {
                    "present".to_string()
                },
                actual: if a_null {
                    "null".to_string()
                } else {
                    "present".to_string()
                },
            });
        } else if !e_null && !a_null {
            let e_resolved = expected["resolvedTo"]["resolvedPath"].as_str();
            let a_resolved = actual["resolvedTo"]["resolvedPath"].as_str();
            if e_resolved != a_resolved {
                mismatches.push(ImportMismatch {
                    fixture: fixture.to_string(),
                    import_key: key.clone(),
                    field: "resolvedTo.resolvedPath".to_string(),
                    expected: format!("{:?}", e_resolved),
                    actual: format!("{:?}", a_resolved),
                });
            }
            let e_kind = expected["resolvedTo"]["resolvedKind"].as_str();
            let a_kind = actual["resolvedTo"]["resolvedKind"].as_str();
            if e_kind != a_kind {
                mismatches.push(ImportMismatch {
                    fixture: fixture.to_string(),
                    import_key: key.clone(),
                    field: "resolvedTo.resolvedKind".to_string(),
                    expected: format!("{:?}", e_kind),
                    actual: format!("{:?}", a_kind),
                });
            }
        }
    }

    // confidence（浮点数，使用阈值比较）
    {
        let e_conf = expected["confidence"].as_f64().unwrap_or(-1.0);
        let a_conf = actual["confidence"].as_f64().unwrap_or(-1.0);
        if (e_conf - a_conf).abs() > 0.05 {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "confidence".to_string(),
                expected: format!("{:.2}", e_conf),
                actual: format!("{:.2}", a_conf),
            });
        }
    }

    // modulePath（exact string, nullable）
    {
        let e_mp = expected["modulePath"].as_str();
        let a_mp = actual["modulePath"].as_str();
        if e_mp != a_mp {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "modulePath".to_string(),
                expected: format!("{:?}", e_mp),
                actual: format!("{:?}", a_mp),
            });
        }
    }

    // expandedPath（exact string, nullable）
    {
        let e_ep = expected["expandedPath"].as_str();
        let a_ep = actual["expandedPath"].as_str();
        if e_ep != a_ep {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "expandedPath".to_string(),
                expected: format!("{:?}", e_ep),
                actual: format!("{:?}", a_ep),
            });
        }
    }

    // resolutionLevel（exact string）
    {
        let e_rl = expected["resolutionLevel"].as_str().unwrap_or("");
        let a_rl = actual["resolutionLevel"].as_str().unwrap_or("");
        if e_rl != a_rl {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: key.clone(),
                field: "resolutionLevel".to_string(),
                expected: e_rl.to_string(),
                actual: a_rl.to_string(),
            });
        }
    }

    // resolvedTo symbol-level 字段（exact string, nullable）
    {
        let e_rt = &expected["resolvedTo"];
        let a_rt = &actual["resolvedTo"];
        for field in &[
            "resolvedSymbolId",
            "resolvedSymbolKind",
            "resolvedSymbolName",
            "resolvedSymbolSourcePath",
        ] {
            let e_val = if e_rt.is_object() {
                e_rt[*field].as_str()
            } else {
                None
            };
            let a_val = if a_rt.is_object() {
                a_rt[*field].as_str()
            } else {
                None
            };
            if e_val != a_val {
                mismatches.push(ImportMismatch {
                    fixture: fixture.to_string(),
                    import_key: key.clone(),
                    field: format!("resolvedTo.{}", field),
                    expected: format!("{:?}", e_val),
                    actual: format!("{:?}", a_val),
                });
            }
        }
    }

    mismatches
}

struct ImportComparisonResult {
    fixture: String,
    mismatches: Vec<ImportMismatch>,
}

fn compare_fixture_imports(fixture: &str) -> ImportComparisonResult {
    let expected = load_expected(fixture);
    let actual = inspect_imports(fixture);

    let mut mismatches = Vec::new();

    let expected_imports = expected["expectedImports"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actual_imports = actual["imports"].as_array().cloned().unwrap_or_default();

    // 计数检查
    if expected_imports.len() != actual_imports.len() {
        mismatches.push(ImportMismatch {
            fixture: fixture.to_string(),
            import_key: "(count)".to_string(),
            field: "importCount".to_string(),
            expected: expected_imports.len().to_string(),
            actual: actual_imports.len().to_string(),
        });
    }

    // 按 (sourcePath, originalPath) 配对
    let actual_map: std::collections::HashMap<(String, String), &serde_json::Value> =
        actual_imports
            .iter()
            .filter_map(|imp| {
                let sp = imp["sourcePath"].as_str().unwrap_or("").to_string();
                let op = imp["originalPath"].as_str().unwrap_or("").to_string();
                if sp.is_empty() {
                    None
                } else {
                    Some(((sp, op), imp))
                }
            })
            .collect();

    let mut matched_keys: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    for e_imp in &expected_imports {
        let e_sp = e_imp["sourcePath"].as_str().unwrap_or("").to_string();
        let e_op = e_imp["originalPath"].as_str().unwrap_or("").to_string();
        let key = (e_sp.clone(), e_op.clone());

        match actual_map.get(&key) {
            None => {
                mismatches.push(ImportMismatch {
                    fixture: fixture.to_string(),
                    import_key: format!("{}:{}", e_sp, e_op),
                    field: "match".to_string(),
                    expected: "present".to_string(),
                    actual: "missing in actual".to_string(),
                });
            }
            Some(a_imp) => {
                matched_keys.insert(key);
                mismatches.extend(compare_import_fields(fixture, e_imp, a_imp));
            }
        }
    }

    // 检查 actual 中多余的条目
    for (key, _) in &actual_map {
        if !matched_keys.contains(key) {
            mismatches.push(ImportMismatch {
                fixture: fixture.to_string(),
                import_key: format!("{}:{}", key.0, key.1),
                field: "match".to_string(),
                expected: "not in expected".to_string(),
                actual: "present in actual".to_string(),
            });
        }
    }

    // expectedDiagnostics 检查：确保 actual 中每条 import 的 diagnostics 包含所有 expected code
    let expected_diag_codes: Vec<String> = expected["expectedDiagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d.as_str().map(|s| s.to_string()))
        .collect();

    if !expected_diag_codes.is_empty() {
        let actual_diag_codes: Vec<String> = actual_imports
            .iter()
            .flat_map(|imp| {
                imp["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|d| d["code"].as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .collect();

        for expected_code in &expected_diag_codes {
            let count_in_expected = expected_diag_codes
                .iter()
                .filter(|c| c == &expected_code)
                .count();
            let count_in_actual = actual_diag_codes
                .iter()
                .filter(|c| *c == expected_code)
                .count();
            if count_in_actual < count_in_expected {
                mismatches.push(ImportMismatch {
                    fixture: fixture.to_string(),
                    import_key: "(diagnostics)".to_string(),
                    field: format!("diagnostic:{}", expected_code),
                    expected: format!("at least {} occurrences", count_in_expected),
                    actual: format!("{} occurrences", count_in_actual),
                });
            }
        }
    }

    // expectedAbsence 检查
    let absences = expected["expectedAbsence"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    for abs in &absences {
        let abs_str = abs.as_str().unwrap_or("");
        match abs_str {
            "noResolvedImport" => {
                let has_resolved = actual_imports
                    .iter()
                    .any(|imp| !imp["resolvedTo"].is_null());
                if has_resolved {
                    mismatches.push(ImportMismatch {
                        fixture: fixture.to_string(),
                        import_key: "(absence)".to_string(),
                        field: "noResolvedImport".to_string(),
                        expected: "no import with resolvedTo".to_string(),
                        actual: "found import with resolvedTo".to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    ImportComparisonResult {
        fixture: fixture.to_string(),
        mismatches,
    }
}

#[test]
fn import_fixtures_are_discoverable() {
    for name in IMPORT_FIXTURES {
        let dir = fixture_dir(name);
        assert!(dir.exists(), "fixture 目录不存在: {:?}", dir);
        let expected = expected_imports_path(name);
        assert!(
            expected.exists(),
            "expected-imports.json 不存在: {:?}",
            expected
        );
    }
}

#[test]
fn expected_imports_json_parses_for_all_fixtures() {
    for name in IMPORT_FIXTURES {
        let expected = load_expected(name);
        assert!(
            expected.is_object(),
            "expected-imports.json 顶层不是对象 for {}",
            name
        );
        assert!(
            expected["expectedImports"].is_array(),
            "缺少 expectedImports for {}",
            name
        );
    }
}

#[test]
fn actual_import_output_parses_for_all_fixtures() {
    for name in IMPORT_FIXTURES {
        let actual = inspect_imports(name);
        assert!(
            actual.is_object(),
            "actual output 顶层不是对象 for {}",
            name
        );
        assert!(actual["imports"].is_array(), "缺少 imports for {}", name);
    }
}

#[test]
fn import_comparison_passes_for_all_fixtures() {
    let mut total_mismatches = 0;
    for name in IMPORT_FIXTURES {
        let result = compare_fixture_imports(name);
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
        "import comparison: {} total mismatches across all fixtures",
        total_mismatches
    );
}

#[test]
fn imports_not_present_without_flag() {
    let dir = fixture_dir("use-crate-simple");
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .output()
        .expect("CLI 调用失败");

    assert!(output.status.success(), "CLI 应成功退出");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual: serde_json::Value = serde_json::from_str(&stdout).expect("应解析为 JSON");

    let imports = actual["imports"].as_array();
    assert!(imports.is_some(), "imports 字段应存在（空数组）");
    assert!(
        imports.unwrap().is_empty(),
        "imports 应为空数组（无 --include imports）"
    );

    let import_count = actual["stats"]["importCount"].as_u64();
    assert_eq!(import_count, Some(0), "importCount 应为 0");
}
