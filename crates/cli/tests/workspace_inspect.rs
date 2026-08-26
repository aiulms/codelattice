//! `codelattice inspect` 端到端契约测试（多语言卡 2）
//!
//! 锁定 workspaceInspection.v1 信封：一行一语言、嵌套规则、阈值、
//! recognition 两档、安全段。analyzable 断言不写死 true/false——
//! 通过 lib 的 language_analyzable（与 bin 同 feature 编译）自适应。

use assert_cmd::Command;

fn cli() -> Command {
    Command::cargo_bin("codelattice").unwrap()
}

fn fixture_root() -> String {
    let base = std::env::current_dir().unwrap();
    let root = if base.join("fixtures").exists() {
        base
    } else if let Some(p) = base.parent().and_then(|p| p.parent()) {
        p.to_path_buf()
    } else {
        base
    };
    root.join("fixtures")
        .join("mixed")
        .join("inspect-smoke")
        .to_string_lossy()
        .to_string()
}

fn inspect_output() -> serde_json::Value {
    let output = cli()
        .args(["inspect", "--root", &fixture_root()])
        .output()
        .expect("run codelattice inspect");
    assert!(
        output.status.success(),
        "inspect 失败: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("输出必须是合法 JSON")
}

fn row<'a>(list: &'a serde_json::Value, rel: &str, key: &str, val: &str) -> &'a serde_json::Value {
    list.as_array()
        .expect("必须是数组")
        .iter()
        .find(|r| r["relativePath"] == rel && r[key] == val)
        .unwrap_or_else(|| panic!("{rel} 的 {key}={val} 行必须在场: {list}"))
}

#[test]
fn inspect_envelope_has_schema_and_safety_sections() {
    let d = inspect_output();
    assert_eq!(d["schemaVersion"], "codelattice.workspaceInspection.v1");
    assert!(d["root"].as_str().unwrap().contains("inspect-smoke"));
    assert!(!d["generatedAt"].as_str().unwrap_or("").is_empty());
    // 安全段必带：没有它体检看起来像"我们扫过源码"
    assert_eq!(d["generatedFrom"]["staticAnalysis"], true);
    assert_eq!(d["generatedFrom"]["projectContentRead"], false);
    assert_eq!(d["generatedFrom"]["scriptsExecuted"], false);
    assert!(!d["cautions"].as_array().unwrap().is_empty());
    assert!(!d["recommendedNextActions"].as_array().unwrap().is_empty());
}

#[test]
fn inspect_manifest_project_row_with_certain_confidence() {
    let d = inspect_output();
    let backend = row(&d["projects"], "backend", "language", "rust");
    assert_eq!(backend["confidence"], "certain");
    assert_eq!(backend["name"], "backend");
    assert_eq!(backend["evidence"]["kind"], "manifest");
    assert_eq!(backend["evidence"]["file"], "Cargo.toml");
    // sourceFileCount = 项目树内 rust 直方图求和（src 两个 .rs）
    assert_eq!(backend["sourceFileCount"], 2);

    // analyzable 与 lib 判定一致（feature 自适应），reason 只在 false 时出现
    let (ok, reason) = gitnexus_rust_core_cli::workspace_inspect::language_analyzable("rust");
    assert_eq!(backend["analyzable"], ok);
    if ok {
        assert!(
            backend.get("reason").is_none(),
            "analyzable 行禁止带 reason"
        );
    } else {
        assert_eq!(backend["reason"], reason);
    }
}

#[test]
fn inspect_one_row_per_language_in_same_directory() {
    let d = inspect_output();
    let py = row(&d["sourceOnlyAreas"], "scripts/tools", "language", "python");
    assert_eq!(py["confidence"], "medium");
    assert_eq!(py["evidence"]["kind"], "extension-histogram");
    assert_eq!(py["evidence"]["extension"], ".py");
    assert_eq!(py["evidence"]["count"], 3);
    assert_eq!(py["sourceFileCount"], 3);

    let sh = row(&d["sourceOnlyAreas"], "scripts/tools", "language", "shell");
    assert_eq!(sh["evidence"]["extension"], ".sh");
    assert_eq!(sh["sourceFileCount"], 2);
    // shell 非 optional：恒可分析
    assert_eq!(sh["analyzable"], true);
    assert!(sh.get("reason").is_none());

    // 1 个 .py 不报（L3 ≥2，别和 java ≥1 混淆）
    let solo = d["sourceOnlyAreas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["relativePath"] == "solo");
    assert!(!solo, "1 个 .py 不得出 sourceOnly 行");
}

#[test]
fn inspect_nested_suppression_keeps_other_languages_visible() {
    let d = inspect_output();
    // 嵌套压制：backend/src 的 rust L3 行不报（同语言被 manifest 罩住）
    let suppressed = d["sourceOnlyAreas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["relativePath"] == "backend/src");
    assert!(!suppressed, "manifest 同语言子目录不得重复上报");

    // 不同语言不被吞：manifest 树内嵌 1 个 .java 也必须单独成行
    let nested = row(&d["unsupportedAreas"], "backend/legacy", "language", "java");
    assert_eq!(nested["sourceFileCount"], 1);
    assert_eq!(nested["recognition"], "known-unsupported");
}

#[test]
fn inspect_unsupported_thresholds_and_recognition_tiers() {
    let d = inspect_output();
    // java ≥1 就报
    let java = row(&d["unsupportedAreas"], "legacy", "language", "java");
    assert_eq!(java["confidence"], "medium");
    assert_eq!(java["evidence"]["extension"], ".java");
    assert_eq!(java["sourceFileCount"], 2);
    assert_eq!(java["analyzable"], false);
    assert_eq!(java["reason"], "language-not-supported");
    assert_eq!(java["recognition"], "known-unsupported");

    // unrecognized：language 字段缺席（不是 null）、confidence low、recognition 必有
    let gen = d["unsupportedAreas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["relativePath"] == "gen")
        .expect("unrecognized 区行必须在场");
    assert!(
        gen.get("language").is_none(),
        "unrecognized 行必须省略 language 字段: {gen}"
    );
    assert_eq!(gen["confidence"], "low");
    assert_eq!(gen["evidence"]["extension"], ".xyzfoo");
    assert_eq!(gen["recognition"], "unrecognized");
    assert_eq!(gen["reason"], "language-not-supported");
}

#[test]
fn inspect_reason_appears_only_on_unanalyzable_rows() {
    let d = inspect_output();
    for bucket in ["projects", "sourceOnlyAreas", "unsupportedAreas"] {
        for row in d[bucket].as_array().unwrap() {
            if row["analyzable"] == true {
                assert!(
                    row.get("reason").is_none(),
                    "analyzable 行禁止带 reason: {row}"
                );
                continue;
            }
            // 不可分析行的 reason 口径不写死：有 language 的行按 lib 的
            // feature 判定（not-supported / disabled 二选一），unrecognized 恒 not-supported
            let expected = match row.get("language").and_then(|l| l.as_str()) {
                Some(lang) => {
                    let (_, reason) =
                        gitnexus_rust_core_cli::workspace_inspect::language_analyzable(lang);
                    reason
                }
                None => "language-not-supported",
            };
            assert_eq!(row["reason"], expected, "不可分析行 reason 口径: {row}");
        }
    }
}

#[test]
fn inspect_rejects_non_json_format() {
    cli()
        .args(["inspect", "--root", &fixture_root(), "--format", "text"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("json"));
}
