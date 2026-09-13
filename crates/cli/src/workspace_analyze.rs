// workspace_analyze —— `codelattice analyze-workspace`（多语言卡 P2：合并多语言快照）。
//
// 目标：对多项目根目录一次操作，产出一张覆盖全部可分析语言的合并 webui.snapshot.v1。
// 冻结契约见 docs/plans/2026-08-25-polyglot-merge-and-alignment-execution-card.md：
//   1. 内部调 inspect_workspace_inventory 拿三桶；
//   2. 候选 = projects ∪ sourceOnlyAreas 中 analyzable 行（复用 language_analyzable）；
//   3. 逐行 analyze 拿「未截断」内部图；
//   4. 内存合并 → 一次转换 → 一次截断 → 输出一个 webui.snapshot.v1 信封；
//   5. 节点 id 加项目命名空间、file 路径改仓库相对、relationKey 按最终 id 重算；
//   6. 单语言失败 → 跳过并继续，在 limitations 逐条点名；全部失败才 exit 非 0。
//
// 不发明跨语言边；不产 dangling 边；禁止沾 `auto` 名（--language auto 语义 N3 不动）。

use crate::webui_snapshot::{convert_analyze_result, node_file_path};
use crate::workspace_inspect::language_analyzable;
use gitnexus_workspace_model::inspect_workspace_inventory;
use serde_json::{json, Value};
use std::path::Path;

/// 单语言 analyze，返回 (json_val, nodes, edges)。
/// 各 run_*_analysis 都是 lib.rs 私有 fn，本模块作为 lib.rs 子模块可经 crate:: 访问。
fn analyze_project(root: &Path, language: &str) -> Result<(Value, Vec<Value>, Vec<Value>), String> {
    match language {
        "rust" => {
            let (j, n, e, _trace) = crate::run_rust_analysis(root)?;
            Ok((j, n, e))
        }
        "typescript" => crate::run_typescript_analysis(root),
        "javascript" => crate::run_javascript_analysis(root),
        "python" => crate::run_python_analysis(root),
        "c" => crate::run_c_analysis(root),
        "cpp" => crate::run_cpp_analysis(root),
        "shell" => crate::run_shell_analysis(root),
        "arkts" => crate::run_arkts_analysis(root),
        "cangjie" => crate::run_cangjie_analysis(root),
        other => Err(format!("unsupported language: {other}")),
    }
}

/// 命名空间：项目 relativePath 作为 id 前缀（根目录用 "root"）。
fn namespace_for(rel_path: &str) -> String {
    if rel_path.is_empty() || rel_path == "." {
        "root".to_string()
    } else {
        rel_path.trim_end_matches('/').to_string()
    }
}

/// file 路径前缀（仓库相对）：根目录项目无前缀，子项目拼 relativePath。
fn rel_prefix_for(rel_path: &str) -> String {
    if rel_path.is_empty() || rel_path == "." {
        String::new()
    } else {
        rel_path.trim_end_matches('/').to_string()
    }
}

/// 合并单个项目的节点：id 加命名空间前缀，file 路径统一改写为仓库相对。
/// file 字段以 sourcePath 为准（对无 sourcePath 的节点注入仓库相对 sourcePath），
/// 保证结构树能按仓库相对路径并目录。绝对路径（个别语言分析器的 repo: 容器节点
/// 带整个项目根）先剥掉项目根前缀，禁止把机器路径泄进合并图。
fn merge_node(node: &mut Value, ns: &str, rel_prefix: &str, proj_root: &Path) {
    let root_str = proj_root.to_string_lossy().to_string();
    // 1. 先用原始 node 提取「项目内相对路径」（加前缀前）。
    let mut orig_file = node_file_path(node);
    // 路径字段全缺席时，id 可能内嵌本项目根（repo:<abs-root> 容器）；命名空间
    // 前缀的 :: 会让转换器的 id 抠路径把绝对根当路径段吐出去，必须以项目根为锚
    // 归一化（open-nwe 实测暴露的「Users/xxx」幽灵模块根因）。
    let id_anchored_root = orig_file.is_empty()
        && node
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.contains(root_str.as_str()));
    if id_anchored_root {
        orig_file = root_str.clone();
    }
    // 2. id 加命名空间前缀。
    let prefixed_id = node
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|id| format!("{ns}::{id}"));
    if let Some(pid) = prefixed_id {
        node["id"] = json!(pid);
    }
    // 3. file 路径改仓库相对：统一覆盖 properties.sourcePath。
    if !orig_file.is_empty() {
        // 绝对路径以本项目根为前缀 → 剥成项目内相对；相对路径原样保留。
        let project_rel = Path::new(&orig_file)
            .strip_prefix(proj_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| orig_file.clone());
        // 项目即根（project_rel 空）→ 回退 relativePath 本身；根级项目（rel_prefix
        // 空）→ 直接用 project_rel；根级项目的根容器两者皆空 → 写 "."，仍要盖掉
        // id 抠路径的绝对回退。
        let repo_rel = match (rel_prefix.is_empty(), project_rel.is_empty()) {
            (false, true) => rel_prefix.to_string(),
            (true, true) => ".".to_string(),
            (true, false) | (false, false) => {
                if rel_prefix.is_empty() {
                    project_rel
                } else {
                    format!("{rel_prefix}/{project_rel}")
                }
            }
        };
        if let Some(obj) = node.as_object_mut() {
            let props = obj.entry("properties").or_insert_with(|| json!({}));
            if let Some(props_obj) = props.as_object_mut() {
                props_obj.insert("sourcePath".to_string(), json!(repo_rel));
            }
        }
    }
}

/// 合并单个项目的边：source/target 加与节点一致的命名空间前缀。
fn merge_edge(edge: &mut Value, ns: &str) {
    for key in ["source", "target"] {
        if let Some(s) = edge
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            let s = s.to_string();
            edge[key] = json!(format!("{ns}::{s}"));
        }
    }
}

/// 编排核心：候选逐行 analyze（可注入，测试据此锁定单语言失败跳过策略）→
/// 未截断子图内存合并 → 一次 convert_analyze_result（内含一次截断、relationKey
/// 按最终 id 重算）→ 信封后处理（languages[] / inspectionSummary / limitations）。
///
/// 返回 Ok(合并信封)；Err(()) 仅在全部候选失败时出现（CLI 层据此 exit 非 0）。
/// 三桶计数 (projects, sourceOnly, unsupported) 只进 inspectionSummary 摘要。
pub(crate) fn build_merged_snapshot<F>(
    root_path: &Path,
    bucket_totals: (usize, usize, usize),
    candidates: &[(String, String)],
    analyze_fn: F,
) -> Result<Value, ()>
where
    F: Fn(&Path, &str) -> Result<(Value, Vec<Value>, Vec<Value>), String>,
{
    let (project_total, source_only_total, unsupported_total) = bucket_totals;

    // 3+4. 逐行 analyze（未截断内部图）并内存合并。
    let mut merged_nodes: Vec<Value> = Vec::new();
    let mut merged_edges: Vec<Value> = Vec::new();
    let mut languages: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut failures: Vec<Value> = Vec::new();
    let mut merged_count = 0usize;

    for (idx, (rel_path, lang)) in candidates.iter().enumerate() {
        // 取消条件：CLI 场景由 OS 信号直接终止进程（SIGINT/SIGTERM 默认行为），
        // 无需显式检查点；每段独立 try 保证单语言失败不阻塞后续编排。
        eprintln!(
            "分析中 {lang} ({}/{}): {}",
            idx + 1,
            candidates.len(),
            rel_path
        );

        let proj_root = root_path.join(rel_path);
        match analyze_fn(&proj_root, lang) {
            Ok((_json_val, nodes, edges)) => {
                let ns = namespace_for(rel_path);
                let rel_prefix = rel_prefix_for(rel_path);
                let mut ns_nodes = nodes;
                let mut ns_edges = edges;
                for n in &mut ns_nodes {
                    merge_node(n, &ns, &rel_prefix, &proj_root);
                }
                for e in &mut ns_edges {
                    merge_edge(e, &ns);
                }
                merged_nodes.append(&mut ns_nodes);
                merged_edges.append(&mut ns_edges);
                languages.insert(lang.clone());
                merged_count += 1;
            }
            Err(e) => {
                failures.push(json!({
                    "language": lang,
                    "path": rel_path,
                    "reason": e,
                }));
                eprintln!("  ⚠ 跳过 {lang} @ {rel_path}: {e}");
            }
        }
    }

    // 全部失败才让 CLI exit 非 0。
    if merged_count == 0 {
        return Err(());
    }

    // 5. 构造合成 analyze Value 供转换器复用（relationKey 在转换器里按最终 id 重算）。
    let synthetic = json!({
        "schemaVersion": "0.3.0",
        "analyzedAt": crate::now_iso8601(),
        "language": "rust", // 仅用于 module_id_for_node 的 rust modulePath 回退；信封后处理移除
        "root": root_path.to_string_lossy(),
        "graph": {
            "nodes": merged_nodes,
            "edges": merged_edges,
        },
    });

    let mut snapshot = convert_analyze_result(&synthetic, env!("CARGO_PKG_VERSION"));

    // 6. 信封后处理：合并快照写 languages[] 不写单数 language；补 inspectionSummary；
    //    limitations 声明跨语言调用未解析 + 单语言失败逐条点名。
    let languages: Vec<String> = languages.into_iter().collect();
    if let Some(obj) = snapshot.as_object_mut() {
        let languages_value = json!(languages);
        obj.insert("languages".to_string(), languages_value.clone());
        obj.remove("language");
        // summary 里 language 字段同步为 languages 并集展示，不写单数。
        if let Some(summary) = obj.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove("language");
            summary.insert("languages".to_string(), languages_value);
        }

        let mut inspection_summary = json!({
            "projectsTotal": project_total,
            "sourceOnlyAreasTotal": source_only_total,
            "unsupportedAreasTotal": unsupported_total,
            "mergedProjectCount": merged_count,
        });
        if !failures.is_empty() {
            inspection_summary["failedProjects"] = json!(failures.len());
        }
        obj.insert("inspectionSummary".to_string(), inspection_summary);

        // limitations：跨语言调用未解析 + 单语言失败点名。
        if let Some(limitations) = obj.get_mut("limitations").and_then(Value::as_object_mut) {
            if let Some(notes) = limitations.get_mut("notes").and_then(Value::as_array_mut) {
                notes.push(json!(
                    "Cross-language calls are NOT resolved; this is a union of per-language subgraphs, not a cross-language graph."
                ));
                for f in &failures {
                    // Value 的 Display 会带 JSON 引号，点名必须取原字符串
                    notes.push(json!(format!(
                        "Skipped {} @ {}: {}",
                        f["language"].as_str().unwrap_or("?"),
                        f["path"].as_str().unwrap_or("?"),
                        f["reason"].as_str().unwrap_or("?"),
                    )));
                }
            }
        }
    }

    Ok(snapshot)
}

/// analyze-workspace 子命令入口。
pub fn run_analyze_workspace_command(root: &str, format: &str) {
    if format != "webui-snapshot" {
        eprintln!("错误：analyze-workspace 当前仅支持 --format webui-snapshot");
        std::process::exit(1);
    }

    let root_path = match crate::check_root(root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // 1. 体检拿三桶。
    let inspection = match inspect_workspace_inventory(root_path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("错误：workspace 体检失败: {e}");
            std::process::exit(1);
        }
    };

    // 2. 候选 = projects ∪ sourceOnlyAreas 中 analyzable 行。
    let mut candidates: Vec<(String, String)> = Vec::new();
    for area in inspection
        .projects
        .iter()
        .chain(inspection.source_only_areas.iter())
    {
        let Some(lang) = area.language.as_deref() else {
            continue;
        };
        let (ok, _reason) = language_analyzable(lang);
        if ok {
            candidates.push((area.relative_path.clone(), lang.to_string()));
        }
    }

    if candidates.is_empty() {
        eprintln!("错误：workspace 内没有可分析的项目/语言区");
        std::process::exit(1);
    }

    let bucket_totals = (
        inspection.projects.len(),
        inspection.source_only_areas.len(),
        inspection.unsupported_areas.len(),
    );

    // stdout 只出最终 JSON；进度/失败已在编排核心写 stderr。
    match build_merged_snapshot(&root_path, bucket_totals, &candidates, analyze_project) {
        Ok(snapshot) => {
            let json_out = serde_json::to_string_pretty(&snapshot).unwrap_or_else(|e| {
                eprintln!("错误：webui-snapshot JSON 序列化失败: {e}");
                std::process::exit(1);
            });
            println!("{json_out}");
        }
        Err(()) => {
            eprintln!("错误：workspace 内所有候选分析均失败");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// 单语言子图 fixture：package + file + 两个符号 + CALLS/DEFINES 边。
    /// 4 节点远小于 MAX_NODES，全部入选，便于直接断言端点完整性。
    fn sample_graph() -> (Vec<Value>, Vec<Value>) {
        (
            vec![
                json!({"id": "package:proj", "label": "package",
                       "properties": {"name": "proj"}}),
                json!({"id": "file:src/lib.rs", "label": "source-file",
                       "properties": {"sourcePath": "src/lib.rs"}}),
                json!({"id": "symbol:crate::alpha", "label": "symbol",
                       "properties": {"name": "alpha", "symbolKind": "function",
                                      "sourcePath": "src/lib.rs", "visibility": "pub"}}),
                json!({"id": "symbol:crate::beta", "label": "symbol",
                       "properties": {"name": "beta", "symbolKind": "function",
                                      "sourcePath": "src/lib.rs"}}),
            ],
            vec![
                json!({"source": "symbol:crate::alpha", "target": "symbol:crate::beta",
                       "type": "CALLS", "properties": {"confidence": 0.9}}),
                json!({"source": "file:src/lib.rs", "target": "symbol:crate::alpha",
                       "type": "DEFINES", "properties": {"confidence": 1.0}}),
            ],
        )
    }

    /// 注入式成功 analyze：忽略 root/lang，返回 sample_graph。
    fn ok_analyze(_root: &Path, _lang: &str) -> Result<(Value, Vec<Value>, Vec<Value>), String> {
        let (nodes, edges) = sample_graph();
        Ok((json!({}), nodes, edges))
    }

    /// 失败场景（执行卡缺口 A）：good 成功、broken 失败。断言部分合并成功、
    /// limitations 逐条点名（语言 + 路径 + 原因）、其余契约不回归。
    #[test]
    fn partial_failure_merges_successes_and_names_skips() {
        let candidates = vec![
            ("good".to_string(), "rust".to_string()),
            ("broken".to_string(), "shell".to_string()),
        ];
        let analyze_fn =
            |root: &Path, lang: &str| -> Result<(Value, Vec<Value>, Vec<Value>), String> {
                if root.ends_with("broken") {
                    return Err("shell analyzer exploded".to_string());
                }
                let _ = lang;
                ok_analyze(root, lang)
            };
        let snapshot = build_merged_snapshot(Path::new("/ws"), (1, 1, 1), &candidates, analyze_fn)
            .expect("部分成功必须产出合并信封");

        // 合并快照写 languages[]，不写单数 language（顶层与 summary 都是）
        assert!(snapshot.get("language").is_none(), "不得写单数 language");
        assert_eq!(snapshot["languages"], json!(["rust"]));
        assert!(snapshot["summary"].get("language").is_none());
        assert_eq!(snapshot["summary"]["languages"], json!(["rust"]));

        // inspectionSummary：三桶摘要 + mergedProjectCount + 失败计数
        let insp = &snapshot["inspectionSummary"];
        assert_eq!(insp["projectsTotal"], 1);
        assert_eq!(insp["sourceOnlyAreasTotal"], 1);
        assert_eq!(insp["unsupportedAreasTotal"], 1);
        assert_eq!(insp["mergedProjectCount"], 1);
        assert_eq!(insp["failedProjects"], 1);

        // limitations：跨语言声明 + 跳过点名（语言 + 路径 + 原因）
        let notes = snapshot["limitations"]["notes"].as_array().unwrap();
        assert!(
            notes.iter().any(|n| n
                .as_str()
                .unwrap()
                .contains("Cross-language calls are NOT resolved")),
            "必须声明跨语言调用未解析: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.as_str().unwrap() == "Skipped shell @ broken: shell analyzer exploded"),
            "必须逐条点名失败的语言/路径/原因: {notes:?}"
        );

        // 成功子图节点 id 全部带项目命名空间、file 为仓库相对
        let nodes = snapshot["graph"]["nodes"].as_array().unwrap();
        assert!(!nodes.is_empty());
        for n in nodes {
            let id = n["id"].as_str().unwrap();
            assert!(id.starts_with("good::"), "节点 id 必须带项目前缀: {id}");
        }
        let file = nodes.iter().find(|n| n["kind"] == "file").unwrap();
        assert_eq!(file["file"], "good/src/lib.rs", "file 必须是仓库相对路径");

        // 无 dangling；relationKey 按最终 id 重算（rel:sha256: 规则）
        let ids: HashSet<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
        for e in snapshot["graph"]["edges"].as_array().unwrap() {
            assert!(
                ids.contains(e["source"].as_str().unwrap()),
                "dangling source"
            );
            assert!(
                ids.contains(e["target"].as_str().unwrap()),
                "dangling target"
            );
            assert!(
                e["relationKey"]
                    .as_str()
                    .unwrap()
                    .starts_with("rel:sha256:"),
                "relationKey 规则: {e}"
            );
        }
    }

    /// 全部候选失败 → Err(())，CLI 层据此 exit 非 0（执行卡：全部失败才整单失败）。
    #[test]
    fn all_candidates_failing_is_err() {
        let candidates = vec![
            ("a".to_string(), "rust".to_string()),
            ("b".to_string(), "shell".to_string()),
        ];
        let analyze_fn =
            |_root: &Path, _lang: &str| -> Result<(Value, Vec<Value>, Vec<Value>), String> {
                Err("nope".to_string())
            };
        assert!(
            build_merged_snapshot(Path::new("/ws"), (2, 0, 0), &candidates, analyze_fn).is_err(),
            "全部失败必须返回 Err"
        );
    }

    /// 根目录自身是项目（relativePath "."）时命名空间用 `root::`（冻结契约）。
    #[test]
    fn root_level_project_uses_root_namespace() {
        let candidates = vec![(".".to_string(), "rust".to_string())];
        let snapshot = build_merged_snapshot(Path::new("/ws"), (1, 0, 0), &candidates, ok_analyze)
            .expect("单根项目必须产出合并信封");
        let nodes = snapshot["graph"]["nodes"].as_array().unwrap();
        assert!(nodes
            .iter()
            .all(|n| n["id"].as_str().unwrap().starts_with("root::")));
    }

    /// 绝对路径归一化（语言分析器的 repo: 容器节点带整个项目根）：必须剥成
    /// 仓库相对，禁止机器路径泄进合并图（open-nwe 实测暴露的 moduleGraph
    /// 「Users/xxx」幽灵模块根因）。覆盖两条路径：属性带绝对 sourcePath、
    /// 属性全缺席但 id 内嵌项目根（命名空间 :: 前缀会让转换器从 id 抠出绝对根）。
    #[test]
    fn absolute_source_paths_are_normalized_to_repo_relative() {
        let candidates = vec![
            ("sub".to_string(), "rust".to_string()),
            (".".to_string(), "shell".to_string()),
        ];
        let analyze_fn =
            |root: &Path, lang: &str| -> Result<(Value, Vec<Value>, Vec<Value>), String> {
                let abs_root = root.to_string_lossy().to_string();
                let nodes = if lang == "rust" {
                    vec![
                        json!({"id": format!("repo:{abs_root}"), "label": "repository",
                           "properties": {"name": "sub", "sourcePath": abs_root}}),
                        json!({"id": "file:src/lib.rs", "label": "source-file",
                           "properties": {"sourcePath": format!("{abs_root}/src/lib.rs")}}),
                    ]
                } else {
                    // 根级项目的 repo 容器：无任何路径属性，id 内嵌绝对根
                    vec![
                        json!({"id": format!("repo:{abs_root}"), "label": "repository",
                           "properties": {"name": "."}}),
                    ]
                };
                Ok((json!({}), nodes, vec![]))
            };
        let snapshot = build_merged_snapshot(Path::new("/ws"), (2, 0, 0), &candidates, analyze_fn)
            .expect("必须产出合并信封");
        let nodes = snapshot["graph"]["nodes"].as_array().unwrap();
        // 嵌套项目：repo 容器剥完等于项目本身 → 回退 relativePath
        let repo = nodes.iter().find(|n| n["file"] == "sub").unwrap();
        assert_eq!(repo["kind"], "package", "repo 容器必须归一化: {repo:?}");
        let file = nodes.iter().find(|n| n["kind"] == "file").unwrap();
        assert_eq!(file["file"], "sub/src/lib.rs", "文件必须仓库相对: {file:?}");
        // 根级项目的根容器：两者皆空 → 写 "."，不得回退成绝对 id 抠路径
        let root_repo = nodes.iter().find(|n| n["file"] == ".").unwrap();
        assert_eq!(
            root_repo["kind"], "package",
            "根容器必须归一化: {root_repo:?}"
        );
        for n in nodes {
            let f = n["file"].as_str().unwrap_or("");
            assert!(!f.starts_with('/'), "机器路径泄漏: {f}");
        }
    }
}
