//! --format webui-snapshot 端到端契约测试
//!
//! 验证 analyze 的 webui-snapshot 输出满足桌面工作台消费契约：
//! - 顶层自述字段（schemaVersion / root / language / generatedAt）
//! - graph 段节点/边形状（kind、confidence/reason 顶层化、relationKey）
//! - graph.summary 五项统计实算、无 dangling 边
//! - moduleGraph 存在且 count 守恒

use assert_cmd::Command;
use std::collections::HashSet;

fn cli() -> Command {
    Command::cargo_bin("codelattice").unwrap()
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

fn analyze_webui_snapshot(fixture_name: &str) -> serde_json::Value {
    let output = cli()
        .args([
            "analyze",
            "--root",
            &fixture(fixture_name),
            "--language",
            "rust",
            "--format",
            "webui-snapshot",
        ])
        .output()
        .expect("run codelattice analyze");
    assert!(
        output.status.success(),
        "analyze 失败: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("输出必须是合法 JSON")
}

#[test]
fn webui_snapshot_format_produces_consumable_snapshot() {
    let d = analyze_webui_snapshot("portable-smoke");
    assert_eq!(d["schemaVersion"], "webui.snapshot.v1");
    assert_eq!(d["language"], "rust");
    assert!(d["root"].as_str().unwrap().contains("portable-smoke"));
    assert!(!d["generatedAt"].as_str().unwrap_or("").is_empty());
    assert_eq!(d["summary"]["language"], "rust");

    // graph.summary 五项统计齐全且非零（实算，不接受硬编码零）
    let s = &d["graph"]["summary"];
    for key in [
        "nodeCount",
        "edgeCount",
        "fileNodeCount",
        "symbolNodeCount",
        "callEdgeCount",
    ] {
        assert!(
            s[key].as_u64().unwrap_or(0) > 0,
            "graph.summary.{key} 必须实算出非零值"
        );
    }

    // 节点/边形状符合 webui.snapshot.v1
    let nodes = d["graph"]["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|n| n["kind"] == "file"));
    assert!(nodes.iter().any(|n| n["kind"] == "symbol"));
    let edges = d["graph"]["edges"].as_array().unwrap();
    let call = edges
        .iter()
        .find(|e| e["kind"] == "calls")
        .expect("必须有 calls 边");
    assert!(call["confidence"].is_number(), "confidence 必须提升到顶层");
    assert!(
        call["relationKey"]
            .as_str()
            .unwrap_or("")
            .starts_with("rel:sha256:"),
        "relationKey 必须按 §6.1 规则生成"
    );

    // 不允许 dangling 边
    let ids: HashSet<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    for e in edges {
        assert!(ids.contains(e["source"].as_str().unwrap()));
        assert!(ids.contains(e["target"].as_str().unwrap()));
    }

    // moduleGraph / limitations / insights 段落齐全
    assert!(d["moduleGraph"]["modules"].is_array());
    assert!(d["limitations"]["notes"].is_array());
    assert!(
        d["insights"]["entryPoints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "main"),
        "portable-smoke 的 main 必须列为入口点"
    );
}

#[test]
fn webui_snapshot_nodes_carry_language_identity() {
    let d = analyze_webui_snapshot("portable-smoke");
    let nodes = d["graph"]["nodes"].as_array().unwrap();
    // 符号/文件节点带扩展名表推出的 language=rust；package 不带（字段必须缺席）
    for n in nodes {
        match n["kind"].as_str().unwrap() {
            "file" | "symbol" => {
                assert_eq!(n["language"], "rust", "节点应带 rust 身份: {n:?}");
            }
            "package" => {
                assert!(
                    n.get("language").is_none(),
                    "package 跨语言容器不得带 language: {n:?}"
                );
            }
            _ => {}
        }
    }
    // moduleGraph 模块 languages = 模块内节点语言并集（字母序去重）
    for m in d["moduleGraph"]["modules"].as_array().unwrap() {
        assert_eq!(m["languages"], serde_json::json!(["rust"]), "模块: {m:?}");
    }
    // .h→c 已知误伤必须写进 limitations
    assert!(
        d["limitations"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains(".h")),
        "limitations 必须包含 .h→c 误伤说明"
    );

    // 冻结规则反向断言：0.3.0 默认 JSON 的图节点不得带 language（MCP 读取面）
    let output = cli()
        .args([
            "analyze",
            "--root",
            &fixture("portable-smoke"),
            "--language",
            "rust",
        ])
        .output()
        .expect("run codelattice analyze");
    assert!(output.status.success());
    let d0: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(d0["schemaVersion"], "0.3.0");
    for n in d0["graph"]["nodes"].as_array().unwrap() {
        assert!(
            n.get("language").is_none(),
            "0.3.0 图节点不得写 language: {n:?}"
        );
    }
}

#[test]
fn webui_snapshot_format_rejects_non_full_profile() {
    cli()
        .args([
            "analyze",
            "--root",
            &fixture("portable-smoke"),
            "--language",
            "rust",
            "--format",
            "webui-snapshot",
            "--profile",
            "symbols",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("profile full"));
}

#[test]
fn webui_snapshot_format_does_not_break_json_default() {
    // 默认 json 格式回归：仍是 0.3.0 信封
    let output = cli()
        .args([
            "analyze",
            "--root",
            &fixture("portable-smoke"),
            "--language",
            "rust",
        ])
        .output()
        .expect("run codelattice analyze");
    assert!(output.status.success());
    let d: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(d["schemaVersion"], "0.3.0");
}
