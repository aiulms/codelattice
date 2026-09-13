//! `codelattice analyze-workspace` 端到端契约测试（多语言卡 P2）
//!
//! 锁定合并多语言快照契约：
//! - 顶层 languages[]（不写单数 language）、inspectionSummary 三桶摘要
//! - 节点 id 全部带项目命名空间前缀、file 路径为仓库相对
//! - relationKey 按最终 id 重算、无 dangling 边
//! - limitations 声明跨语言调用未解析

use assert_cmd::Command;
use std::collections::HashSet;

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
        .join("workspace")
        .to_string_lossy()
        .to_string()
}

fn analyze_workspace_output() -> serde_json::Value {
    let output = cli()
        .args([
            "analyze-workspace",
            "--root",
            &fixture_root(),
            "--format",
            "webui-snapshot",
        ])
        .output()
        .expect("run codelattice analyze-workspace");
    assert!(
        output.status.success(),
        "analyze-workspace 失败: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("输出必须是合法 JSON")
}

#[test]
fn merged_snapshot_carries_languages_and_inspection_summary() {
    let d = analyze_workspace_output();
    assert_eq!(d["schemaVersion"], "webui.snapshot.v1");
    // 合并快照写 languages[]，不写单数 language
    assert!(d.get("language").is_none(), "合并快照不得写单数 language");
    let langs: Vec<&str> = d["languages"]
        .as_array()
        .expect("languages 必须是数组")
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    // 至少一种语言（fixtures/workspace 含 rust/shell/ts 等项目）；具体语言按
    // feature 自适应——不能写死 typescript（默认 feature 下不可分析）。
    assert!(!langs.is_empty(), "至少合并一种可分析语言: {langs:?}");
    assert!(
        langs.contains(&"rust") || langs.contains(&"shell"),
        "fixtures/workspace 至少含 rust 或 shell: {langs:?}"
    );
    // 字母序去重
    let mut sorted = langs.clone();
    sorted.sort_unstable();
    assert_eq!(langs, sorted, "languages 必须字母序去重");
    // summary 里同步 languages[]，不写单数 language
    assert!(d["summary"].get("language").is_none());
    assert!(d["summary"]["languages"].is_array());
    // inspectionSummary 三桶摘要 + mergedProjectCount
    let insp = &d["inspectionSummary"];
    assert!(insp["projectsTotal"].as_u64().unwrap() > 0);
    assert!(insp["mergedProjectCount"].as_u64().unwrap() > 0);
    assert!(insp["sourceOnlyAreasTotal"].is_u64());
    assert!(insp["unsupportedAreasTotal"].is_u64());
}

#[test]
fn merged_nodes_are_namespaced_and_repo_relative() {
    let d = analyze_workspace_output();
    let nodes = d["graph"]["nodes"].as_array().expect("nodes 必须是数组");
    // 每个节点 id 都带 `::` 命名空间分隔（前缀::原id）
    for n in nodes {
        let id = n["id"].as_str().unwrap();
        assert!(id.contains("::"), "节点 id 必须带项目命名空间前缀: {id}");
    }
    // 至少一个 rust 文件节点 file 是仓库相对（含 fixtures/workspace 前缀）
    let has_repo_relative_file = nodes.iter().any(|n| {
        n["kind"] == "file"
            && n.get("file")
                .and_then(|f| f.as_str())
                .map(|f| {
                    f.starts_with("multi-project/")
                        || f.starts_with("rust-core/")
                        || f.starts_with("ts-ui/")
                })
                .unwrap_or(false)
    });
    assert!(has_repo_relative_file, "file 路径必须改为仓库相对");
}

#[test]
fn merged_edges_have_no_dangling_and_recomputed_relation_keys() {
    let d = analyze_workspace_output();
    let ids: HashSet<&str> = d["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    for e in d["graph"]["edges"].as_array().unwrap() {
        assert!(
            ids.contains(e["source"].as_str().unwrap()),
            "dangling source: {}",
            e["source"]
        );
        assert!(
            ids.contains(e["target"].as_str().unwrap()),
            "dangling target: {}",
            e["target"]
        );
        // relationKey 必须按最终 id 重算（前缀 + rel:sha256: 规则）
        let key = e["relationKey"].as_str().unwrap();
        assert!(key.starts_with("rel:sha256:"), "relationKey 规则: {key}");
    }
}

#[test]
fn limitations_declare_cross_language_unresolved() {
    let d = analyze_workspace_output();
    let notes = d["limitations"]["notes"].as_array().unwrap();
    assert!(
        notes.iter().any(|n| n
            .as_str()
            .unwrap()
            .contains("Cross-language calls are NOT resolved")),
        "limitations 必须声明跨语言调用未解析: {notes:?}"
    );
}
