// webui_snapshot —— analyze 结果（LanguageAnalysisResult，0.3.0 信封）→ webui.snapshot.v1。
//
// P3 起（2026-09-13）本模块是 webui.snapshot.v1 的唯一事实源：Phase A enriched
// 全量段（summary / quality / explore / cleanup / releaseReview / insights /
// workflowPresets / graph / moduleGraph / limitations）全部在此实算产出，
// scripts/codelattice-snapshot-gen.py（Python 聚合实现）已退役删除。
// --redact-root 路径脱敏也在此完成，脱敏后 relationKey 按最终端点重算。

use gitnexus_workspace_model::SOURCE_EXTENSIONS;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const SCHEMA_VERSION: &str = "webui.snapshot.v1";
/// 与 Python 脚本一致的预览图规模上限：大项目符号成千上万，先保 file/package 上下文
/// 再填符号，否则 DEFINES/OWNS 的文件端点被挤出，图看起来是空的。
const MAX_NODES: usize = 150;
const MAX_EDGES: usize = 300;
const MAX_MODULES: usize = 200;

/// 把 analyze 结果（完整 full profile JSON）转换为 webui.snapshot.v1 快照。
pub fn convert_analyze_result(analyze: &Value, tool_version: &str) -> Value {
    convert_analyze_result_with_options(analyze, tool_version, false)
}

/// 带 --redact-root 选项的完整转换（P3 退役 Python snapshot-gen 后的唯一生成路径）。
pub fn convert_analyze_result_with_options(
    analyze: &Value,
    tool_version: &str,
    redact_root: bool,
) -> Value {
    let graph = analyze.get("graph").cloned().unwrap_or(json!({}));
    let nodes = json_array(&graph, "nodes");
    let edges = flatten_edges(&graph);

    let language = analyze
        .get("metadata")
        .and_then(|m| m.get("language"))
        .and_then(Value::as_str)
        .or_else(|| analyze.get("language").and_then(Value::as_str))
        .or_else(|| {
            analyze
                .get("summary")
                .and_then(|s| s.get("language"))
                .and_then(Value::as_str)
        })
        .unwrap_or("unknown")
        .to_string();
    let generated_at = analyze
        .get("analyzedAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let root = analyze
        .get("root")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let graph_section = build_graph_section(&nodes, &edges);
    let module_graph = build_module_graph(&graph_section, &language);
    let insights = build_insights(&nodes, &edges);

    // summary 全量统计基于未截断的 graph 实算，不从 analyze 顶层 summary 抄数字
    // （AGENTS.md 防守规则：stats 必须从数据源计算，不接受硬编码/转抄默认值）。
    // 分类口径对齐 Python build_summary 的 normalized_node_kind：label 为 target 的
    // 节点不算 package（与图节点选择器口径不同是有意的，保持与现有生成器一致）。
    let mut symbol_count = 0u64;
    let mut source_file_count = 0u64;
    let mut module_count = 0u64;
    let mut package_count = 0u64;
    for n in &nodes {
        match normalized_node_kind(n) {
            "symbol" => symbol_count += 1,
            "source-file" => source_file_count += 1,
            "module" => module_count += 1,
            "package" => package_count += 1,
            _ => {}
        }
    }

    let mut limitation_notes = vec![
        "All results derived from static source analysis via CodeLattice CLI.".to_string(),
        "No project code was executed during snapshot generation.".to_string(),
        "Call graph is built from syntactic patterns; method dispatch is heuristic.".to_string(),
        "External crate symbols are resolved only for std/core/alloc direct imports.".to_string(),
        "Trait resolution and type inference are NOT performed.".to_string(),
        "Macro expansion is NOT performed.".to_string(),
        "Results are informative, not prescriptive. Always verify before acting.".to_string(),
        // 语言身份已知误伤：共享扩展名表把 .h 统一标为 c，C++ 项目里会误标。
        // 判定 .h 归属需要编译器级 include 上下文，超出静态扩展名标注边界，
        // 按冻结规则写进 limitations，不在转换器里猜。
        "Node language identity comes from properties.language or the shared extension table; '.h' files are labeled 'c' and are known to be mislabeled in C++ projects.".to_string(),
    ];

    // P3 起信封为 Phase A enriched 全量段（原 Python snapshot-gen 的桌面外消费面：
    // quality/explore/cleanup/releaseReview/workflowPresets 全部收敛到本转换器实算）。
    let mut snapshot = json!({
        "schemaVersion": SCHEMA_VERSION,
        "generatedAt": generated_at,
        "generatedFrom": {
            "tool": "CodeLattice",
            "toolVersion": tool_version,
            "snapshotSchema": SCHEMA_VERSION,
            "staticAnalysis": true,
            "runtimeVerified": false,
            "generationMethod": "cli-format-webui-snapshot",
        },
        "root": root,
        "language": language,
        "summary": {
            "schemaVersion": SCHEMA_VERSION,
            "nodeCount": nodes.len(),
            "edgeCount": edges.len(),
            "symbolCount": symbol_count,
            "sourceFileCount": source_file_count,
            "moduleCount": module_count,
            "packageCount": package_count,
            "generatedAt": generated_at,
            "language": language,
            "toolVersion": tool_version,
        },
        "quality": build_quality_section(analyze),
        "explore": build_explore_section(&nodes),
        "cleanup": build_cleanup_section(&nodes, &edges),
        "releaseReview": build_release_review_section(&nodes),
        "workflowPresets": workflow_presets_section(),
        "graph": graph_section,
        "insights": insights,
        "limitations": {
            "runtimeVerified": false,
            "externalUsageVerified": false,
            "coverageVerified": false,
            "deletionSafetyVerified": false,
            "projectCodeExecuted": false,
            "notes": limitation_notes,
        },
    });

    // --redact-root：先做全局路径脱敏（含 relationKey 按脱敏后的最终端点重算），
    // 模块图必须在脱敏之后基于最终 graph 段归并（对齐 Python 的段落顺序）。
    if redact_root {
        redact_all_paths(&mut snapshot, &root);
        replace_project_path_fragments(&mut snapshot);
        if let Some(edges) = snapshot
            .get_mut("graph")
            .and_then(|g| g.get_mut("edges"))
            .and_then(Value::as_array_mut)
        {
            for e in edges.iter_mut() {
                let src = e.get("source").and_then(Value::as_str).unwrap_or("");
                let tgt = e.get("target").and_then(Value::as_str).unwrap_or("");
                let kind = e.get("kind").and_then(Value::as_str).unwrap_or("related");
                e["relationKey"] = json!(relation_key(src, kind, tgt));
            }
        }
    }
    let redacted_graph = snapshot.get("graph").cloned().unwrap_or(json!({}));
    let module_graph = build_module_graph(&redacted_graph, &language);

    // 扁平项目或未知归属必须写进 limitations，避免前端当成推断结果
    if let Some(obj) = snapshot.as_object_mut() {
        obj.insert("moduleGraph".to_string(), module_graph.clone());
    }
    let modules = module_graph["modules"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(limitations) = snapshot
        .get_mut("limitations")
        .and_then(Value::as_object_mut)
    {
        if let Some(notes) = limitations.get_mut("notes").and_then(Value::as_array_mut) {
            if modules.len() <= 1 {
                notes.push(json!(
                    "Module graph collapsed to a single module because the project is flat (one directory / one crate); directory-prefix aggregation is the intended fallback."
                ));
            }
            if modules.iter().any(|m| m["id"] == "(unknown)") {
                notes.push(json!(
                    "Some nodes have no file/modulePath and were assigned module '(unknown)'; no module membership was inferred."
                ));
            }
        }
    }

    snapshot
}

/// summary 分类口径：对齐 Python build_summary 的 normalized_node_kind
/// （label 优先；repository/repo 归 package；module 独立计数不进 package）。
fn normalized_node_kind(n: &Value) -> &str {
    let label = n.get("label").and_then(Value::as_str).unwrap_or("");
    let kind = n.get("kind").and_then(Value::as_str).unwrap_or("");
    if ["symbol", "source-file", "package", "module"].contains(&label) {
        label
    } else if kind == "sourceFile" {
        "source-file"
    } else if ["symbol", "source-file"].contains(&kind) {
        kind
    } else if ["repository", "repo"].contains(&kind) {
        "package"
    } else if ["package", "module"].contains(&kind) {
        kind
    } else {
        // 与 Python `kind or label or "?"` 对齐：空串回退 label 再回退 "?"
        if !kind.is_empty() {
            kind
        } else if !label.is_empty() {
            label
        } else {
            "?"
        }
    }
}

fn json_array(v: &Value, key: &str) -> Vec<Value> {
    v.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 0.3.0 的 edges 是数组；少数格式按类型分桶成 dict，一并拍平（对齐 Python 行为）。
fn flatten_edges(graph: &Value) -> Vec<Value> {
    match graph.get("edges") {
        Some(Value::Array(list)) => list.clone(),
        Some(Value::Object(map)) => map
            .values()
            .filter_map(Value::as_array)
            .flatten()
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

fn is_symbol_node(n: &Value) -> bool {
    n.get("label").and_then(Value::as_str) == Some("symbol")
        || n.get("kind").and_then(Value::as_str) == Some("symbol")
}

fn is_source_file_node(n: &Value) -> bool {
    let label = n.get("label").and_then(Value::as_str).unwrap_or("");
    let kind = n.get("kind").and_then(Value::as_str).unwrap_or("");
    label == "source-file" || kind == "source-file" || kind == "sourceFile"
}

fn is_package_node(n: &Value) -> bool {
    const PKG: [&str; 5] = ["package", "target", "module", "repository", "repo"];
    let label = n.get("label").and_then(Value::as_str).unwrap_or("");
    let kind = n.get("kind").and_then(Value::as_str).unwrap_or("");
    PKG.contains(&label) || PKG.contains(&kind)
}

fn prop<'a>(n: &'a Value, key: &str) -> Option<&'a Value> {
    n.get("properties").and_then(|p| p.get(key))
}

fn prop_str<'a>(n: &'a Value, key: &str) -> Option<&'a str> {
    prop(n, key).and_then(Value::as_str)
}

fn edge_endpoint<'a>(e: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|k| e.get(k).and_then(Value::as_str))
        .unwrap_or("")
}

/// 从节点 id 里抠文件路径（py:src:src/main.py / c:src:main.c 这类内嵌路径的 id）。
pub(crate) fn extract_path_from_id(node_id: &str) -> String {
    const EXTS: [&str; 12] = [
        ".py", ".rs", ".c", ".cpp", ".h", ".ts", ".tsx", ".ets", ".sh", ".bash", ".zsh", ".ksh",
    ];
    if !node_id.contains(':') {
        return String::new();
    }
    let parts: Vec<&str> = node_id.split(':').collect();
    for p in &parts {
        if EXTS.iter().any(|ext| p.ends_with(ext)) {
            return p.to_string();
        }
    }
    if parts.len() >= 3 && parts[parts.len() - 1].contains('/') {
        return parts[parts.len() - 1].to_string();
    }
    String::new()
}

/// §6.1：relationKey = sha256(source\0kind\0target)，NUL 分隔避免拼接歧义。
/// 与 Python hashlib / 前端 defaultRelationKey 同一规则，保证跨端身份一致。
fn relation_key(source: &str, kind: &str, target: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.update([0u8]);
    hasher.update(kind.as_bytes());
    hasher.update([0u8]);
    hasher.update(target.as_bytes());
    format!("rel:sha256:{:x}", hasher.finalize())
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub(crate) fn node_file_path(n: &Value) -> String {
    prop_str(n, "sourcePath")
        .or_else(|| prop_str(n, "file"))
        .or_else(|| prop_str(n, "path"))
        .map(str::to_string)
        .unwrap_or_else(|| extract_path_from_id(n.get("id").and_then(Value::as_str).unwrap_or("")))
}

/// 路径 → 语言身份（节点语言身份冻结规则）。
/// 只查共享扩展名表（workspace-model::SOURCE_EXTENSIONS），不在本模块另写映射——
/// 两份表必漂。身份 ≠ 可分析性：表含 csharp/go/java 等不可分析语言是有意的。
fn language_for_path(path: &str) -> Option<&'static str> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (_, ext) = name.rsplit_once('.')?;
    let dotted = format!(".{}", ext.to_lowercase());
    SOURCE_EXTENSIONS
        .iter()
        .find(|(suffix, _)| *suffix == dotted)
        .map(|(_, lang)| *lang)
}

fn build_graph_section(nodes: &[Value], edge_list: &[Value]) -> Value {
    // 边上引用到的节点 id（只扫前 MAX_EDGES 条，与选中窗口一致）
    let mut edge_node_ids: HashSet<&str> = HashSet::new();
    for e in edge_list.iter().take(MAX_EDGES) {
        edge_node_ids.insert(edge_endpoint(e, &["source", "sourceId", "from"]));
        edge_node_ids.insert(edge_endpoint(e, &["target", "targetId", "to"]));
    }

    let sym_nodes: Vec<&Value> = nodes.iter().filter(|n| is_symbol_node(n)).collect();
    let sf_nodes: Vec<&Value> = nodes.iter().filter(|n| is_source_file_node(n)).collect();
    let pkg_nodes: Vec<&Value> = nodes.iter().filter(|n| is_package_node(n)).collect();
    let edge_connected: Vec<&Value> = nodes
        .iter()
        .filter(|n| {
            let id = n.get("id").and_then(Value::as_str).unwrap_or("");
            edge_node_ids.contains(id)
                && !is_symbol_node(n)
                && !is_source_file_node(n)
                && !is_package_node(n)
        })
        .collect();

    let pkg_limit = 10.min(MAX_NODES);
    let file_limit = 50.min(MAX_NODES.saturating_sub(pkg_limit));
    let symbol_limit = MAX_NODES.saturating_sub(pkg_limit + file_limit);
    let mut selected: Vec<&Value> = Vec::new();
    selected.extend(pkg_nodes.iter().take(pkg_limit).copied());
    selected.extend(sf_nodes.iter().take(file_limit).copied());
    selected.extend(sym_nodes.iter().take(symbol_limit).copied());
    selected.extend(edge_connected.iter().take(10).copied());
    selected.truncate(MAX_NODES);

    let selected_ids: HashSet<&str> = selected
        .iter()
        .map(|n| n.get("id").and_then(Value::as_str).unwrap_or(""))
        .collect();

    let mut graph_nodes = Vec::new();
    for n in &selected {
        let label = n.get("label").and_then(Value::as_str).unwrap_or("?");
        let raw_kind = n.get("kind").and_then(Value::as_str).unwrap_or("");
        let kind = node_kind(label, raw_kind);
        let name = prop_str(n, "name")
            .or_else(|| prop_str(n, "sourcePath"))
            .map(str::to_string)
            .unwrap_or_else(|| {
                truncate_chars(n.get("id").and_then(Value::as_str).unwrap_or(""), 60)
            });
        let file = truncate_chars(&node_file_path(n), 200);

        let mut gn = json!({
            "id": n.get("id").and_then(Value::as_str).unwrap_or(""),
            "label": name,
            "kind": kind,
            "file": file,
        });
        if let Some(line) = prop(n, "lineStart").or_else(|| prop(n, "line")) {
            if line.is_number() {
                gn["line"] = line.clone();
            }
        }
        if let Some(vis) = prop_str(n, "visibility") {
            if !vis.is_empty() {
                gn["visibility"] = vis.into();
            }
        }
        if let Some(mp) = prop_str(n, "modulePath") {
            gn["modulePath"] = mp.into();
        }
        // 节点语言身份（冻结规则）：文件节点 properties.language 优先、共享扩展名表兜底；
        // 符号节点跟随所属文件的扩展名（file 为空则省略）；package 跨语言容器不带；
        // 缺席即未知——禁止写 ""/null，判定不了就不写字段。
        match kind.as_str() {
            "file" => {
                let lang = prop_str(n, "language")
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| language_for_path(&file).map(str::to_string));
                if let Some(l) = lang {
                    gn["language"] = json!(l);
                }
            }
            "symbol" => {
                if !file.is_empty() {
                    if let Some(l) = language_for_path(&file) {
                        gn["language"] = json!(l);
                    }
                }
            }
            _ => {}
        }
        graph_nodes.push(gn);
    }

    let mut graph_edges = Vec::new();
    for e in edge_list {
        let src = edge_endpoint(e, &["source", "sourceId", "from"]);
        let tgt = edge_endpoint(e, &["target", "targetId", "to"]);
        // 不允许 dangling edge：端点必须都落在选中节点集合内
        if !selected_ids.contains(src) || !selected_ids.contains(tgt) {
            continue;
        }
        let etype = edge_endpoint(e, &["type", "kind", "label"]);
        let kind = edge_kind(if etype.is_empty() { "related" } else { etype });

        let mut ge = json!({
            "source": src,
            "target": tgt,
            "kind": kind,
            "relationKey": relation_key(src, kind, tgt),
        });
        if let Some(conf) = prop(e, "confidence").and_then(json_f64) {
            ge["confidence"] = json!(conf);
        }
        if let Some(reason) = prop_str(e, "reason") {
            if !reason.is_empty() {
                ge["reason"] = truncate_chars(reason, 100).into();
            }
        }
        graph_edges.push(ge);
        if graph_edges.len() >= MAX_EDGES {
            break;
        }
    }

    let file_node_count = graph_nodes.iter().filter(|n| n["kind"] == "file").count();
    let symbol_node_count = graph_nodes.iter().filter(|n| n["kind"] == "symbol").count();
    let call_edge_count = graph_edges.iter().filter(|e| e["kind"] == "calls").count();
    let has_nodes = !graph_nodes.is_empty();

    json!({
        "status": if has_nodes { "collected" } else { "not_collected" },
        "stability": "preview",
        "nodes": graph_nodes,
        "edges": graph_edges,
        "summary": {
            "nodeCount": graph_nodes.len(),
            "edgeCount": graph_edges.len(),
            "fileNodeCount": file_node_count,
            "symbolNodeCount": symbol_node_count,
            "callEdgeCount": call_edge_count,
        },
        "truncated": sym_nodes.len() > MAX_NODES || edge_list.len() > MAX_EDGES,
        "cautions": if has_nodes {
            json!([
                "Graph is a static preview — limited to top nodes and edges.",
                "Not all nodes/edges are shown; use CLI/MCP for full analysis.",
                "Call edges are heuristic with confidence scores — not compiler-verified.",
                "Dynamic dispatch / reflection / plugins may hide actual callers.",
            ])
        } else {
            json!([])
        },
    })
}

fn node_kind(label: &str, raw_kind: &str) -> String {
    if raw_kind == "sourceFile" {
        return "file".to_string();
    }
    const LABEL_MAP: [(&str, &str); 7] = [
        ("symbol", "symbol"),
        ("source-file", "file"),
        ("package", "package"),
        ("target", "package"),
        ("module", "package"),
        ("repository", "package"),
        ("diagnostic", "risk"),
    ];
    if ["symbol", "package", "module", "repository", "repo"].contains(&raw_kind)
        && !LABEL_MAP.iter().any(|(l, _)| *l == label)
    {
        return if ["repository", "repo", "module"].contains(&raw_kind) {
            "package".to_string()
        } else {
            raw_kind.to_string()
        };
    }
    LABEL_MAP
        .iter()
        .find(|(l, _)| *l == label)
        .map(|(_, k)| k.to_string())
        .unwrap_or_else(|| label.to_string())
}

fn edge_kind(etype: &str) -> &'static str {
    match etype {
        "CALLS" | "calls" | "uses" => "calls",
        "DEFINES" | "defines" => "defines",
        "IMPORTS" | "imports" => "imports",
        "ownsSource" | "containsPackage" | "OWNS_SOURCE" | "CONTAINS_PACKAGE" => "owns",
        _ => "related",
    }
}

fn json_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// ── moduleGraph ──────────────────────────────────────────────────────────────

/// 相对路径前两级目录作为模块归属（对齐 crates/ + src/ 架构图）；根目录文件归 (root)。
fn module_id_from_file(file_path: &str) -> Option<String> {
    const PLACEHOLDERS: [&str; 4] = ["<redacted-root>", "<redacted-user>", ".", ".."];
    let normalized = file_path.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|p| !p.is_empty() && !PLACEHOLDERS.contains(p))
        .collect();
    if parts.is_empty() {
        return None;
    }
    // 最后一段带扩展名视为文件名，不计入目录层级
    let dirs: Vec<&str> = if parts[parts.len() - 1].contains('.') {
        parts[..parts.len() - 1].to_vec()
    } else {
        parts
    };
    if dirs.is_empty() {
        return Some("(root)".to_string());
    }
    Some(dirs[..dirs.len().min(2)].join("/"))
}

/// crate 内模块路径取前两级（:: 分段）。
fn module_id_from_rust_module_path(module_path: &str) -> Option<String> {
    let segs: Vec<&str> = module_path.split("::").filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return None;
    }
    Some(segs[..segs.len().min(2)].join("::"))
}

/// 模块归属：文件路径优先；rust 无路径回退 modulePath。符号无路径归 (unknown)，不做
/// 猜测（stop-line）；package/file 容器没有路径则不参与模块图，避免造出空的 (unknown) 块。
fn module_id_for_node(node: &Value, language: &str) -> Option<String> {
    let file = node.get("file").and_then(Value::as_str).unwrap_or("");
    if let Some(mid) = module_id_from_file(file) {
        return Some(mid);
    }
    if language == "rust" {
        if let Some(mp) = node.get("modulePath").and_then(Value::as_str) {
            if let Some(mid) = module_id_from_rust_module_path(mp) {
                return Some(mid);
            }
        }
    }
    let kind = node.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind == "package" || kind == "file" || kind.is_empty() {
        return None;
    }
    Some("(unknown)".to_string())
}

/// 把已有 graph 边向上归并为模块图，不新造任何符号级边。
/// 可聚合边 = 两端都能归到模块且模块不同；count 之和必须等于可聚合边数；
/// minConfidence 取最弱链路，不取平均。
fn build_module_graph(graph_section: &Value, language: &str) -> Value {
    let nodes = json_array(graph_section, "nodes");
    let edges = json_array(graph_section, "edges");

    let mut node_mod: HashMap<String, String> = HashMap::new();
    for n in &nodes {
        let id = n.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        if let Some(mid) = module_id_for_node(n, language) {
            node_mod.insert(id.to_string(), mid);
        }
    }

    struct ModuleRec {
        files: HashSet<String>,
        symbols: u64,
        // 模块内节点语言并集；BTreeSet 序列化即字母序去重，全未知时输出省略字段
        languages: BTreeSet<String>,
    }
    let mut modules: BTreeMap<String, ModuleRec> = BTreeMap::new();
    for n in &nodes {
        let id = n.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(mid) = node_mod.get(id) else {
            continue;
        };
        let rec = modules.entry(mid.clone()).or_insert_with(|| ModuleRec {
            files: HashSet::new(),
            symbols: 0,
            languages: BTreeSet::new(),
        });
        if n.get("kind").and_then(Value::as_str) == Some("symbol") {
            rec.symbols += 1;
        }
        let file = n.get("file").and_then(Value::as_str).unwrap_or("");
        if !file.is_empty() {
            rec.files.insert(file.to_string());
        }
        if let Some(lang) = n.get("language").and_then(Value::as_str) {
            if !lang.is_empty() {
                rec.languages.insert(lang.to_string());
            }
        }
    }

    #[derive(Default)]
    struct AggRec {
        count: u64,
        kinds: HashSet<String>,
        min_confidence: Option<f64>,
        reasons: Vec<String>,
    }
    let mut agg: BTreeMap<(String, String), AggRec> = BTreeMap::new();
    for e in &edges {
        let src = node_mod.get(edge_endpoint(e, &["source"]));
        let tgt = node_mod.get(edge_endpoint(e, &["target"]));
        // 缺端点或不跨模块：不是可聚合边，直接跳过，不发明模块边
        let (Some(src), Some(tgt)) = (src, tgt) else {
            continue;
        };
        if src == tgt {
            continue;
        }
        let rec = agg.entry((src.clone(), tgt.clone())).or_default();
        rec.count += 1;
        rec.kinds.insert(
            e.get("kind")
                .and_then(Value::as_str)
                .unwrap_or("related")
                .to_string(),
        );
        if let Some(conf) = e.get("confidence").and_then(json_f64) {
            rec.min_confidence = Some(match rec.min_confidence {
                None => conf,
                Some(prev) => prev.min(conf),
            });
        }
        if let Some(reason) = e.get("reason").and_then(Value::as_str) {
            if !reason.is_empty()
                && !rec.reasons.iter().any(|r| r == reason)
                && rec.reasons.len() < 2
            {
                rec.reasons.push(truncate_chars(reason, 100));
            }
        }
    }

    // 模块排序：符号数降序 + id 稳定次序；超限截断并丢弃跨界模块边
    let mut module_list: Vec<(String, u64, u64, BTreeSet<String>)> = modules
        .iter()
        .map(|(id, r)| {
            (
                id.clone(),
                r.files.len() as u64,
                r.symbols,
                r.languages.clone(),
            )
        })
        .collect();
    module_list.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let truncated = module_list.len() > MAX_MODULES;
    if truncated {
        let keep: HashSet<String> = module_list
            .iter()
            .take(MAX_MODULES)
            .map(|m| m.0.clone())
            .collect();
        module_list.retain(|m| keep.contains(&m.0));
        agg.retain(|k, _| keep.contains(&k.0) && keep.contains(&k.1));
    }

    let edge_list: Vec<Value> = agg
        .into_iter()
        .map(|((src, tgt), rec)| {
            let mut kinds: Vec<&str> = rec.kinds.iter().map(String::as_str).collect();
            kinds.sort_unstable();
            let mut item = json!({
                "source": src,
                "target": tgt,
                "count": rec.count,
                "kinds": kinds,
                "reasons": rec.reasons,
            });
            if let Some(mc) = rec.min_confidence {
                item["minConfidence"] = json!(mc);
            }
            item
        })
        .collect();

    json!({
        "modules": module_list
            .into_iter()
            .map(|(id, files, symbols, languages)| {
                let mut item = json!({"id": id, "files": files, "symbols": symbols});
                // languages 字母序去重；全未知省略字段（缺席即未知，禁止 []）
                if !languages.is_empty() {
                    item["languages"] = json!(languages);
                }
                item
            })
            .collect::<Vec<_>>(),
        "edges": edge_list,
        "truncated": truncated,
    })
}

// ── insights ─────────────────────────────────────────────────────────────────

/// hotspots / entryPoints 基于未截断的 analyze 图统计；入口判定是启发式，
/// 可能漏掉动态/条件入口或误报，cautions 必须随行。
fn build_insights(nodes: &[Value], edge_list: &[Value]) -> Value {
    let mut fan_out: HashMap<&str, u64> = HashMap::new();
    let mut fan_in: HashMap<&str, u64> = HashMap::new();
    for e in edge_list {
        let s = edge_endpoint(e, &["source", "from"]);
        let t = edge_endpoint(e, &["target", "to"]);
        if !s.is_empty() {
            *fan_out.entry(s).or_insert(0) += 1;
        }
        if !t.is_empty() {
            *fan_in.entry(t).or_insert(0) += 1;
        }
    }

    let sym_map: HashMap<&str, &Value> = nodes
        .iter()
        .filter(|n| is_symbol_node(n))
        .map(|n| (n.get("id").and_then(Value::as_str).unwrap_or(""), n))
        .collect();

    let mut fan_out_ranked: Vec<(&str, u64)> = fan_out
        .iter()
        .map(|(k, v)| (*k, *v))
        .filter(|(_, c)| *c >= 3)
        .collect();
    fan_out_ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let hotspots: Vec<Value> = fan_out_ranked
        .iter()
        .take(5)
        .map(|(sym_id, count)| {
            let n = sym_map.get(sym_id).copied();
            json!({
                "id": sym_id,
                "name": n.and_then(|v| prop_str(v, "name")).unwrap_or(sym_id),
                "fanOut": count,
                "fanIn": fan_in.get(sym_id).copied().unwrap_or(0),
                "file": n.and_then(|v| prop_str(v, "sourcePath").or_else(|| prop_str(v, "file"))).unwrap_or(""),
                "kind": n.and_then(|v| prop_str(v, "symbolKind").or_else(|| prop_str(v, "kind"))).unwrap_or(""),
            })
        })
        .collect();

    let mut entry_points = Vec::new();
    for n in nodes {
        if !is_symbol_node(n) {
            continue;
        }
        let id = n.get("id").and_then(Value::as_str).unwrap_or("");
        let name = prop_str(n, "name").unwrap_or("");
        let kind = prop_str(n, "symbolKind")
            .or_else(|| prop_str(n, "kind"))
            .unwrap_or("");
        let vis = prop_str(n, "visibility").unwrap_or("");
        let file = prop_str(n, "sourcePath")
            .or_else(|| prop_str(n, "file"))
            .unwrap_or("");
        if ["main", "test_main", "_start", "init", "Main"].contains(&name) {
            entry_points.push(json!({
                "id": id, "name": name, "kind": kind, "file": file,
                "reason": "well-known-entry-point",
            }));
        } else if ["pub", "public"].contains(&vis)
            && ["function", "fn"].contains(&kind)
            && fan_in.get(id).copied().unwrap_or(0) == 0
        {
            // pub 函数且无任何入边 → 可能是对外 API 入口
            entry_points.push(json!({
                "id": id, "name": name, "kind": kind, "file": file,
                "reason": "public-no-callers-api-candidate",
            }));
        }
        if entry_points.len() >= 10 {
            break;
        }
    }

    let has_data = !hotspots.is_empty() || !entry_points.is_empty();
    json!({
        "status": if has_data { "partial" } else { "not_collected" },
        "hotspots": hotspots,
        "entryPoints": entry_points,
        // reviewFirst = 热点前 3 的文件清单（对齐 Python：high-fan-out 优先复核）
        "reviewFirst": hotspots
            .iter()
            .take(3)
            .map(|h| {
                json!({
                    "file": h.get("file").cloned().unwrap_or(json!("?")),
                    "reason": "high-fan-out-hotspot",
                })
            })
            .collect::<Vec<_>>(),
        "cautions": if has_data {
            json!([
                "Fan-in/out counts are based on resolved call edges only; unresolved calls are excluded.",
                "Entry-point detection is heuristic; may miss dynamic/conditional entries or include false positives.",
            ])
        } else {
            json!([])
        },
    })
}

// ── Phase A enrichment 段（P3 自 scripts/codelattice-snapshot-gen.py 移植；
//    Python 脚本退役后本模块是 webui.snapshot.v1 的唯一事实源）────────────────

const MAX_EXPLORE_SYMBOLS: usize = 500;
const MAX_EXPLORE_SOURCE_FILES: usize = 200;

/// 符号 kind → 展示标签（对齐 Python symbol_kind_label）。
fn symbol_kind_label(kind: &str) -> String {
    match kind {
        "function" | "fn" => "Function",
        "method" => "Method",
        "struct" => "Struct",
        "enum" => "Enum",
        "trait" => "Trait",
        "impl" => "Impl",
        "mod" => "Module",
        "const" => "Constant",
        "static" => "Static",
        "type" => "Type Alias",
        "macro" => "Macro",
        "interface" => "Interface",
        "class" => "Class",
        "variable" => "Variable",
        "parameter" => "Parameter",
        "unknown" => "Unknown",
        other => return other.to_string(),
    }
    .to_string()
}

/// quality 段：门来自 analyze.qualityGates（与 `codelattice quality` 命令同一份
/// 计算），diagnostics 摘要来自 graph.diagnostics。overall 全过=pass、有败=fail、
/// 无门=unknown；passed/failed 计数按 `passed` 字段实算（Python 旧输出读 `status`
/// 字段恒得 0/0，属于历史缺陷，此处按 AGENTS.md stats 实算规则修正）。
fn build_quality_section(analyze: &Value) -> Value {
    let gates = analyze
        .get("qualityGates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let passed = gates
        .iter()
        .filter(|g| g.get("passed").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let failed = gates
        .iter()
        .filter(|g| g.get("passed").and_then(Value::as_bool) == Some(false))
        .count() as u64;
    let overall = if gates.is_empty() {
        "unknown"
    } else if failed == 0 {
        "pass"
    } else {
        "fail"
    };

    let diagnostics = analyze
        .get("graph")
        .and_then(|g| g.get("diagnostics"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let level_count = |level: &str| -> u64 {
        diagnostics
            .iter()
            .filter(|d| d.get("level").and_then(Value::as_str) == Some(level))
            .count() as u64
    };
    let diagnostics_summary = if diagnostics.is_empty() {
        Value::Null
    } else {
        json!({
            "total": diagnostics.len(),
            "error": level_count("error"),
            "warning": level_count("warning"),
            "info": level_count("info"),
        })
    };

    json!({
        "overall": overall,
        "gates": gates,
        "passedGateCount": passed,
        "failedGateCount": failed,
        "diagnosticsSummary": diagnostics_summary,
        "cautions": [
            "Quality gates are based on static analysis only.",
            "Pass/fail status does not guarantee runtime correctness.",
            "External crate resolution is bounded; stdlib-only.",
        ],
    })
}

/// explore 段：符号 + 源文件清单（对齐 Python build_explore_section 的字段与上限）。
fn build_explore_section(nodes: &[Value]) -> Value {
    let is_repo_node = |n: &Value| {
        let label = n.get("label").and_then(Value::as_str).unwrap_or("");
        let kind = n.get("kind").and_then(Value::as_str).unwrap_or("");
        ["repository", "repo"].contains(&label) || ["repository", "repo"].contains(&kind)
    };
    let sym_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|n| is_symbol_node(n) && !is_repo_node(n))
        .collect();
    let sf_nodes: Vec<&Value> = nodes.iter().filter(|n| is_source_file_node(n)).collect();

    let mut symbols: Vec<Value> = Vec::new();
    // 计数器用 BTreeMap 保证确定性（Python Counter.most_common 的并列序不保证）
    let mut sf_symbol_counts: BTreeMap<String, usize> = BTreeMap::new();
    for n in sym_nodes.iter().take(MAX_EXPLORE_SYMBOLS) {
        let props = n.get("properties").cloned().unwrap_or(json!({}));
        let prop = |key: &str| props.get(key).and_then(Value::as_str);
        let node_id = n.get("id").and_then(Value::as_str).unwrap_or("");
        let src_path = prop("sourcePath")
            .or_else(|| prop("file"))
            .or_else(|| prop("path"))
            .map(str::to_string)
            .or_else(|| {
                let p = extract_path_from_id(node_id);
                (!p.is_empty()).then_some(p)
            })
            .unwrap_or_else(|| "?".to_string());
        let kind = prop("symbolKind")
            .or_else(|| prop("kind"))
            .unwrap_or("unknown");
        let mut entry = json!({
            "id": node_id,
            "name": prop("name").unwrap_or(node_id),
            "kind": kind,
            "kindLabel": symbol_kind_label(kind),
            "file": src_path,
            // Python 恒写 line/endLine（缺省 null），保持信封形状一致
            "line": props.get("lineStart").cloned().or_else(|| props.get("line").cloned()).unwrap_or(Value::Null),
            "endLine": props.get("lineEnd").cloned().unwrap_or(Value::Null),
        });
        if let Some(vis) = prop("visibility") {
            if !vis.is_empty() {
                entry["visibility"] = json!(vis);
                if ["pub", "public", "exported", "export"].contains(&vis) {
                    entry["exported"] = json!(true);
                }
            }
        }
        *sf_symbol_counts.entry(src_path).or_insert(0) += 1;
        symbols.push(entry);
    }

    let mut source_files: Vec<Value> = Vec::new();
    for n in sf_nodes.iter().take(MAX_EXPLORE_SOURCE_FILES) {
        let props = n.get("properties").cloned().unwrap_or(json!({}));
        let prop = |key: &str| props.get(key).and_then(Value::as_str);
        let node_id = n.get("id").and_then(Value::as_str).unwrap_or("");
        let label = n.get("label").and_then(Value::as_str).unwrap_or("");
        let path = prop("path")
            .or_else(|| prop("sourcePath"))
            .map(str::to_string)
            .or_else(|| (!label.is_empty()).then(|| label.to_string()))
            .or_else(|| {
                let p = extract_path_from_id(node_id);
                (!p.is_empty()).then_some(p)
            })
            .unwrap_or_else(|| "?".to_string());
        let count = sf_symbol_counts.get(&path).copied().unwrap_or(0);
        source_files.push(json!({
            "path": path,
            "language": prop("language").unwrap_or(""),
            "symbolCount": count,
        }));
    }
    // 有符号但缺 source-file 节点的路径补进清单（对齐 Python）
    let mut seen: HashSet<String> = source_files
        .iter()
        .filter_map(|sf| sf.get("path").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut by_count: Vec<(String, usize)> = sf_symbol_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    by_count.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (path, count) in by_count.iter().take(MAX_EXPLORE_SOURCE_FILES) {
        if !path.is_empty() && path != "?" && !seen.contains(path) {
            source_files.push(json!({"path": path, "language": "", "symbolCount": count}));
            seen.insert(path.clone());
        }
    }

    let top_files: Vec<Value> = by_count
        .iter()
        .take(10)
        .filter(|(p, _)| !p.is_empty() && p != "?")
        .map(|(p, c)| json!({"path": p, "symbolCount": c, "reason": "highest-symbol-count"}))
        .collect();

    let has_data = !symbols.is_empty() || !source_files.is_empty();
    json!({
        "status": if has_data { "collected" } else { "empty" },
        "sourceFiles": source_files.into_iter().take(MAX_EXPLORE_SOURCE_FILES).collect::<Vec<_>>(),
        "symbols": symbols,
        "topFiles": top_files,
        "totalSymbols": sym_nodes.len(),
        "totalSourceFiles": sf_nodes.len(),
        "truncated": sym_nodes.len() > MAX_EXPLORE_SYMBOLS || sf_nodes.len() > MAX_EXPLORE_SOURCE_FILES,
    })
}

/// cleanup 段：基于调用图形状的死代码候选启发式（对齐 Python build_cleanup_section；
/// 只认 label=="symbol"，未被任何 CALLS 边指向的符号即候选，绝不判定可删）。
fn build_cleanup_section(nodes: &[Value], edge_list: &[Value]) -> Value {
    let sym_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|n| n.get("label").and_then(Value::as_str) == Some("symbol"))
        .collect();
    let mut call_targets: HashSet<&str> = HashSet::new();
    for e in edge_list {
        let etype = e
            .get("type")
            .or_else(|| e.get("label"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        if etype.contains("call") {
            if let Some(t) = e.get("target").and_then(Value::as_str) {
                call_targets.insert(t);
            }
        }
    }
    let uncalled: Vec<&&Value> = sym_nodes
        .iter()
        .filter(|s| {
            let id = s.get("id").and_then(Value::as_str).unwrap_or("");
            !call_targets.contains(id)
        })
        .collect();
    let external_api = sym_nodes
        .iter()
        .filter(|s| {
            matches!(
                prop_str(s, "visibility"),
                Some("pub") | Some("public") | Some("exported")
            )
        })
        .count();

    let candidates = uncalled.len().min(sym_nodes.len());
    json!({
        "status": if !sym_nodes.is_empty() { "partial" } else { "not_collected" },
        "deadCodeCandidateCount": if candidates > 0 { json!(candidates) } else { Value::Null },
        "unreachableCandidateCount": if !uncalled.is_empty() { json!(uncalled.len()) } else { Value::Null },
        "externalApiSurfaceCount": if external_api > 0 { json!(external_api) } else { Value::Null },
        "frameworkEntryHintCount": Value::Null, // 需更深分析，Python 同样恒 None
        "cautions": [
            "Dead-code detection is heuristic-based on call-graph shape.",
            "Candidates are NOT proven unused — they may be called via reflection/dynamic dispatch/tests.",
            "Public/exported symbols may be used by external crates not analyzed here.",
            "Framework entry points (main/test/bin) should NEVER be removed based on this analysis.",
            "Auto-deletion is explicitly forbidden without human review + test regression check.",
        ],
    })
}

/// releaseReview 段：发布前静态审查摘要（对齐 Python build_release_review_section）。
fn build_release_review_section(nodes: &[Value]) -> Value {
    let sym_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|n| n.get("label").and_then(Value::as_str) == Some("symbol"))
        .collect();
    let pub_symbols = sym_nodes
        .iter()
        .filter(|s| {
            matches!(
                prop_str(s, "visibility"),
                Some("pub") | Some("public") | Some("exported")
            )
        })
        .count();

    let mut all_paths: HashSet<&str> = HashSet::new();
    for n in nodes {
        if let Some(p) = prop_str(n, "sourcePath")
            .or_else(|| prop_str(n, "path"))
            .or_else(|| prop_str(n, "file"))
        {
            if !p.is_empty() {
                all_paths.insert(p);
            }
        }
    }
    let doc_files: Vec<&&str> = all_paths
        .iter()
        .filter(|p| {
            let lower = p.to_lowercase();
            [".md", ".rst", ".txt", "doc/", "docs/", "readme"]
                .iter()
                .any(|ext| lower.contains(ext))
        })
        .collect();

    json!({
        "status": if !sym_nodes.is_empty() { "partial" } else { "not_collected" },
        "breakingChangeRisk": if pub_symbols > 20 { "medium" } else { "low" },
        "breakingChangeSurface": pub_symbols,
        "staleDocCandidateCount": if !doc_files.is_empty() { json!(doc_files.len()) } else { Value::Null },
        "missingTestCandidateCount": Value::Null,  // 需覆盖率数据
        "configExampleIssueCount": Value::Null,    // 需配置文件解析
        "cautions": [
            "Release review is based on static analysis only — does not run tests or verify docs accuracy.",
            "Breaking-change risk assessment is heuristic; actual impact depends on downstream usage.",
            "Documentation staleness requires manual review — this only lists doc files found.",
            "Test coverage gaps require test runner integration — not available in static mode.",
            "Config example drift needs template comparison — not performed in this snapshot.",
        ],
    })
}

/// workflowPresets 段：10 个固定工作流预设（对齐 Python WORKFLOW_PRESETS 原表）。
fn workflow_presets_section() -> Value {
    json!({
        "status": "collected",
        "presets": [
            {"id": "onboarding", "name": "项目接入 / Onboarding",
             "description": "首次接入 CodeLattice：理解项目结构、符号分布、入口点",
             "tools": ["analyze", "summary", "explore"],
             "stopLines": ["不执行目标项目代码", "不修改源码", "静态分析结果仅供参考"]},
            {"id": "before_edit", "name": "编辑前检查 / Before Edit",
             "description": "修改代码前了解影响范围、调用链、风险点",
             "tools": ["impact_preview", "context", "analyze"],
             "stopLines": ["不替代 code review", "运行时行为需实测确认", "trait 解析为启发式"]},
            {"id": "after_edit", "name": "编辑后验证 / After Edit",
             "description": "修改后快速检查格式、符号完整性、基本质量门禁",
             "tools": ["quality", "analyze", "detect-changes"],
             "stopLines": ["不运行测试套件", "不执行 package manager", "不保证无回归"]},
            {"id": "delete_code", "name": "删除代码前评估 / Delete Code Assessment",
             "description": "删除代码/模块前识别引用关系、死代码候选、外部使用风险",
             "tools": ["impact_preview", "context", "analyze --include calls"],
             "stopLines": ["dead-code candidate ≠ 可安全删除", "外部 API heuristic 不等于真实使用者", "必须人工复核"]},
            {"id": "release_check", "name": "发布前检查 / Release Check",
             "description": "版本发布前的静态审查：breaking change 风险、文档一致性、配置示例",
             "tools": ["quality", "analyze", "release_review"],
             "stopLines": ["不是 GA 质量证明", "不覆盖运行时测试", "不验证外部依赖兼容性"]},
            {"id": "legacy_cleanup", "name": "遗留代码清理 / Legacy Cleanup",
             "description": "识别未使用的符号、过时的模块、可简化的调用链",
             "tools": ["analyze --include calls", "quality", "cleanup_summary"],
             "stopLines": ["低置信度标记需逐一核实", "不自动删除任何代码", "framework entry 点不可轻移"]},
            {"id": "public_api_change", "name": "公共 API 变更评估 / Public API Change",
             "description": "变更 public/exported 符号前评估下游影响、ABI 兼容性",
             "tools": ["impact_preview", "context", "analyze --include graph"],
             "stopLines": ["external usage 为启发式推断", "文档同步需人工处理", "semantic versioning 需人工判断"]},
            {"id": "framework_route_change", "name": "框架路由变更 / Framework Route Change",
             "description": "修改框架入口/路由/控制器时的影响分析",
             "tools": ["analyze", "impact_preview", "entry_points"],
             "stopLines": ["路由解析为模式匹配", "动态路由不可完全覆盖", "需结合框架文档"]},
            {"id": "docs_tests_sync", "name": "文档-测试同步检查 / Docs-Tests Sync",
             "description": "发现文档与测试覆盖不一致的区域、缺失的 API 文档候选",
             "tools": ["analyze", "quality", "release_review"],
             "stopLines": ["基于文件名/符号名的启发式", "不解析文档内容语义", "不判断测试充分性"]},
            {"id": "config_examples_sync", "name": "配置-示例同步检查 / Config-Examples Sync",
             "description": "发现配置项与示例/文档不同步的问题",
             "tools": ["analyze", "quality", "release_review"],
             "stopLines": ["不验证配置值正确性", "不执行配置加载", "模板/占位符可能误报"]}
        ],
    })
}

// ── --redact-root 路径脱敏（对齐 Python redact_path / _looks_like_absolute_path /
//    _redact_all_paths；脱敏后 relationKey 由调用方按最终端点重算）────────────────

/// 把 `/Users/<name>/`、`/home/<name>/` 段替换为 `<redacted-user>/`（全部出现）。
fn replace_user_dir_segments(s: &str, marker: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.find(marker) {
        let after = &rest[idx + marker.len()..];
        match after.find('/') {
            Some(end) if !after[..end].is_empty() && !after[..end].contains('/') => {
                out.push_str(&rest[..idx]);
                out.push_str("<redacted-user>/");
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..idx + marker.len()]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 路径脱敏：项目根 → <redacted-root>；用户目录 → <redacted-user>；
/// 绝对路径 → <redacted-abs>（带 file:/repo:/py:repo:/c:repo: 前缀保留）。
fn redact_path(path: &str, root: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }
    // 语义 id 内嵌绝对根（arkts:component:/Users/... 等） anywhere 替换
    if !root.is_empty() && path.contains(root) {
        return path.replace(root, "<redacted-root>");
    }
    let user_redacted =
        replace_user_dir_segments(&replace_user_dir_segments(path, "/Users/"), "/home/");
    if user_redacted != path {
        return user_redacted;
    }

    let mut uri_prefix = "";
    let mut check = path;
    for prefix in ["file:", "repo:", "py:repo:", "c:repo:"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            uri_prefix = prefix;
            check = rest;
            break;
        }
    }

    // 1. 项目根（剥前缀后）
    if !root.is_empty() {
        if let Some(rest) = check.strip_prefix(root) {
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            return if rest.is_empty() {
                format!("{uri_prefix}<redacted-root>")
            } else {
                format!("{uri_prefix}<redacted-root>/{rest}")
            };
        }
    }

    // 2. 常见用户目录
    for prefix in ["/Users/", "/home/", "/tmp/"] {
        if let Some(rest) = check.strip_prefix(prefix) {
            let parts: Vec<&str> = check.split('/').collect();
            if parts.len() > 2 {
                return format!("{uri_prefix}<redacted-user>/{}", parts[3..].join("/"));
            }
            let _ = rest;
            return format!("{uri_prefix}<redacted-path>");
        }
    }

    // 3. 像工作区根的绝对路径：留最后 ≤3 段
    if check.starts_with('/') && check[1..].contains('/') {
        let parts: Vec<&str> = check.split('/').collect();
        if parts.len() >= 3 {
            let keep = parts.len().min(3);
            return format!(
                "{uri_prefix}<redacted-abs>/{}",
                parts[parts.len() - keep..].join("/")
            );
        }
    }

    path.to_string()
}

/// 判断字符串是否像需要脱敏的绝对路径（对齐 Python _looks_like_absolute_path）。
fn looks_like_absolute_path(s: &str) -> bool {
    if s.len() < 5 {
        return false;
    }
    if s.contains("/Users/") || s.contains("/home/") || s.contains("/tmp/") {
        return true;
    }
    let is_prefixed = ["file:/", "repo:/", "py:repo:/"]
        .iter()
        .any(|p| s.starts_with(p));
    let is_abs = s.starts_with('/');
    if !is_prefixed && !is_abs {
        return false;
    }
    let check = if is_prefixed { s } else { &s[1..] };
    if !check.contains('/') {
        return false;
    }
    let indicators = [
        "/Users/",
        "/home/",
        "/tmp/",
        "/Desktop/",
        "Desktop/",
        "/opt/",
        "/usr/local/",
        "/var/",
        "fixtures/",
        "codelattice",
    ];
    if indicators.iter().any(|ind| s.contains(ind)) {
        return true;
    }
    [".py", ".rs", ".ts", ".c", ".cpp", ".sh", ".bash"]
        .iter()
        .any(|ext| {
            [format!("{ext}\""), format!("{ext},")]
                .iter()
                .any(|pat| s.contains(pat.as_str()))
        })
}

/// 递归脱敏 JSON 里所有像绝对路径的字符串值（对齐 Python _redact_all_paths）。
fn redact_all_paths(obj: &mut Value, root: &str) {
    match obj {
        Value::Object(map) => {
            for (_, value) in map.iter_mut() {
                if let Some(s) = value.as_str() {
                    if looks_like_absolute_path(s) {
                        *value = Value::String(redact_path(s, root));
                    } else {
                        redact_all_paths(value, root);
                    }
                } else {
                    redact_all_paths(value, root);
                }
            }
        }
        Value::Array(list) => {
            for item in list.iter_mut() {
                if let Some(s) = item.as_str() {
                    if looks_like_absolute_path(s) {
                        *item = Value::String(redact_path(s, root));
                    } else {
                        redact_all_paths(item, root);
                    }
                } else {
                    redact_all_paths(item, root);
                }
            }
        }
        _ => {}
    }
}

/// 顽固路径片段的字符串级兜底替换（对齐 Python Phase B fallback）。
fn replace_project_path_fragments(obj: &mut Value) {
    match obj {
        Value::Object(map) => {
            for (_, value) in map.iter_mut() {
                replace_project_path_fragments(value);
            }
        }
        Value::Array(list) => {
            for item in list.iter_mut() {
                replace_project_path_fragments(item);
            }
        }
        Value::String(s) => {
            let mut result = s.replace("Desktop/codelattice", "project/codelattice");
            result = result.replace("codelattice/fixtures", "project/fixtures");
            if &result != s {
                *s = result;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 最小 LanguageAnalysisResult 信封：package + 两个文件 + 三个符号 + 各类边。
    /// alpha(src/lib.rs, pub) → beta(src/lib.rs) CALLS 0.9
    /// alpha → gamma(tests/helper.rs, pub) CALLS 0.5；beta → gamma CALLS 0.9
    /// 另有一条指向不存在节点的 CALLS，必须被丢弃（不允许 dangling edge）。
    fn analyze_fixture() -> Value {
        json!({
            "schemaVersion": "0.3.0",
            "analyzedAt": "2026-08-24T00:00:00Z",
            "language": "rust",
            "root": "fixtures/rust/portable-smoke",
            "summary": {"nodeCount": 6, "edgeCount": 6, "symbolCount": 3,
                        "sourceFileCount": 2, "packageCount": 1, "callEdgeCount": 3},
            "graph": {
                "nodes": [
                    {"id": "package:portable-smoke", "label": "package",
                     "properties": {"name": "portable-smoke"}},
                    {"id": "file:src/lib.rs", "label": "source-file",
                     "properties": {"sourcePath": "src/lib.rs"}},
                    {"id": "file:tests/helper.rs", "label": "source-file",
                     "properties": {"sourcePath": "tests/helper.rs"}},
                    {"id": "symbol:crate::alpha", "label": "symbol",
                     "properties": {"name": "alpha", "symbolKind": "function",
                                    "sourcePath": "src/lib.rs", "lineStart": 3,
                                    "visibility": "pub", "modulePath": "crate"}},
                    {"id": "symbol:crate::beta", "label": "symbol",
                     "properties": {"name": "beta", "symbolKind": "function",
                                    "sourcePath": "src/lib.rs", "lineStart": 9,
                                    "modulePath": "crate"}},
                    {"id": "symbol:crate::gamma", "label": "symbol",
                     "properties": {"name": "gamma", "symbolKind": "function",
                                    "sourcePath": "tests/helper.rs", "lineStart": 1,
                                    "visibility": "pub", "modulePath": "crate::util"}}
                ],
                "edges": [
                    {"source": "symbol:crate::alpha", "target": "symbol:crate::beta",
                     "type": "CALLS", "properties": {"confidence": 0.9, "reason": "call-direct"}},
                    {"source": "symbol:crate::alpha", "target": "symbol:crate::gamma",
                     "type": "CALLS", "properties": {"confidence": 0.5, "reason": "call-blind"}},
                    {"source": "symbol:crate::beta", "target": "symbol:crate::gamma",
                     "type": "CALLS", "properties": {"confidence": 0.9, "reason": "call-direct"}},
                    {"source": "file:src/lib.rs", "target": "symbol:crate::beta",
                     "type": "DEFINES", "properties": {"confidence": 1.0}},
                    {"source": "package:portable-smoke", "target": "file:src/lib.rs",
                     "type": "OWNS_SOURCE", "properties": {}},
                    {"source": "symbol:crate::alpha", "target": "symbol:not-selected",
                     "type": "CALLS", "properties": {"confidence": 0.7}}
                ]
            }
        })
    }

    fn convert(analyze: &Value) -> Value {
        convert_analyze_result(analyze, "9.9.9-test")
    }

    #[test]
    fn converts_to_webui_snapshot_schema_with_self_describing_meta() {
        let out = convert(&analyze_fixture());
        assert_eq!(out["schemaVersion"], "webui.snapshot.v1");
        assert_eq!(out["generatedAt"], "2026-08-24T00:00:00Z");
        assert_eq!(out["root"], "fixtures/rust/portable-smoke");
        assert_eq!(out["language"], "rust");
        // 桌面端 meta_of 读 summary.language；语言按产物自述取，不回退 rust
        assert_eq!(out["summary"]["language"], "rust");
        assert_eq!(out["summary"]["symbolCount"], 3);
        assert_eq!(out["summary"]["sourceFileCount"], 2);
        assert_eq!(out["summary"]["packageCount"], 1);
        assert_eq!(out["generatedFrom"]["snapshotSchema"], "webui.snapshot.v1");
        assert_eq!(out["generatedFrom"]["toolVersion"], "9.9.9-test");
        assert!(out["limitations"]["notes"].is_array());
    }

    #[test]
    fn maps_node_and_edge_kinds_and_lifts_confidence_reason() {
        let out = convert(&analyze_fixture());
        let nodes = out["graph"]["nodes"].as_array().unwrap();
        let file = nodes.iter().find(|n| n["id"] == "file:src/lib.rs").unwrap();
        assert_eq!(file["kind"], "file");
        assert_eq!(file["file"], "src/lib.rs");
        let pkg = nodes
            .iter()
            .find(|n| n["id"] == "package:portable-smoke")
            .unwrap();
        assert_eq!(pkg["kind"], "package");
        let alpha = nodes
            .iter()
            .find(|n| n["id"] == "symbol:crate::alpha")
            .unwrap();
        assert_eq!(alpha["kind"], "symbol");
        assert_eq!(alpha["label"], "alpha");
        assert_eq!(alpha["line"], 3);
        assert_eq!(alpha["visibility"], "pub");
        assert_eq!(alpha["modulePath"], "crate");

        let edges = out["graph"]["edges"].as_array().unwrap();
        let call = edges
            .iter()
            .find(|e| e["target"] == "symbol:crate::beta")
            .unwrap();
        assert_eq!(call["kind"], "calls");
        assert_eq!(call["confidence"], 0.9);
        assert_eq!(call["reason"], "call-direct");
        let owns = edges
            .iter()
            .find(|e| e["source"] == "package:portable-smoke")
            .unwrap();
        assert_eq!(owns["kind"], "owns");
    }

    #[test]
    fn relation_key_matches_contract_sha256_rule() {
        // §6.1：rel:sha256:sha256(source\0kind\0target)，与 Python hashlib 对齐的已知答案
        let out = convert(&analyze_fixture());
        let edges = out["graph"]["edges"].as_array().unwrap();
        let call = edges
            .iter()
            .find(|e| e["source"] == "symbol:crate::alpha" && e["target"] == "symbol:crate::beta")
            .unwrap();
        assert_eq!(
            call["relationKey"],
            "rel:sha256:76c106d7a8aca8d853b1cee58ebe7ada1d223f4b0aa36048b5162048d1062e27"
        );
        let defines = edges.iter().find(|e| e["kind"] == "defines").unwrap();
        assert_eq!(
            defines["relationKey"],
            "rel:sha256:51f031e4539e63188b4fb93f63e480c3a524279369b291a2408d5e5c52c6f3ca"
        );
    }

    #[test]
    fn graph_summary_counts_are_computed_never_hardcoded() {
        let out = convert(&analyze_fixture());
        let s = &out["graph"]["summary"];
        assert_eq!(s["nodeCount"], 6);
        // 6 条输入边中 1 条 dangling 被丢弃
        assert_eq!(s["edgeCount"], 5);
        assert_eq!(s["fileNodeCount"], 2);
        assert_eq!(s["symbolNodeCount"], 3);
        assert_eq!(s["callEdgeCount"], 3);
    }

    #[test]
    fn drops_edges_whose_endpoints_are_not_selected() {
        let out = convert(&analyze_fixture());
        let ids: HashSet<&str> = out["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        for e in out["graph"]["edges"].as_array().unwrap() {
            assert!(
                ids.contains(e["source"].as_str().unwrap()),
                "dangling source"
            );
            assert!(
                ids.contains(e["target"].as_str().unwrap()),
                "dangling target"
            );
        }
    }

    #[test]
    fn module_graph_aggregates_only_cross_module_edges_with_weakest_confidence() {
        let out = convert(&analyze_fixture());
        let mg = &out["moduleGraph"];
        let modules: Vec<&str> = mg["modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert!(modules.contains(&"src"));
        assert!(modules.contains(&"tests"));
        let edges = mg["edges"].as_array().unwrap();
        // 只有 alpha→gamma、beta→gamma 两条 CALLS 跨模块（src→tests）
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["source"], "src");
        assert_eq!(edges[0]["target"], "tests");
        assert_eq!(edges[0]["count"], 2);
        // minConfidence 取最弱链路 0.5，不取平均
        assert_eq!(edges[0]["minConfidence"], 0.5);
        assert_eq!(edges[0]["kinds"], json!(["calls"]));
    }

    #[test]
    fn module_graph_count_sum_equals_aggregatable_edge_count() {
        let out = convert(&analyze_fixture());
        let sum: u64 = out["moduleGraph"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["count"].as_u64().unwrap())
            .sum();
        assert_eq!(sum, 2, "模块边 count 之和必须等于可聚合边数");
    }

    #[test]
    fn insights_marks_public_uncalled_function_as_entry_point() {
        let out = convert(&analyze_fixture());
        let eps = out["insights"]["entryPoints"].as_array().unwrap();
        // alpha：pub + function + 无入边 → API 入口候选；gamma 有入边，beta 非 pub
        assert!(
            eps.iter()
                .any(|e| e["name"] == "alpha" && e["reason"] == "public-no-callers-api-candidate"),
            "pub 无调用者函数应列为入口候选: {eps:?}"
        );
        assert!(!eps.iter().any(|e| e["name"] == "gamma"));
    }

    #[test]
    fn flat_project_module_graph_adds_limitation_note() {
        // 全部文件在同一目录 → 模块图坍缩成单模块，必须写 limitations 说明
        let flat = json!({
            "schemaVersion": "0.3.0",
            "analyzedAt": "2026-08-24T00:00:00Z",
            "language": "rust",
            "root": "x",
            "graph": {
                "nodes": [
                    {"id": "symbol:crate::a", "label": "symbol",
                     "properties": {"name": "a", "symbolKind": "function", "sourcePath": "src/a.rs"}},
                    {"id": "symbol:crate::b", "label": "symbol",
                     "properties": {"name": "b", "symbolKind": "function", "sourcePath": "src/b.rs"}}
                ],
                "edges": [
                    {"source": "symbol:crate::a", "target": "symbol:crate::b",
                     "type": "CALLS", "properties": {"confidence": 0.9}}
                ]
            }
        });
        let out = convert(&flat);
        assert_eq!(out["moduleGraph"]["modules"].as_array().unwrap().len(), 1);
        let notes = out["limitations"]["notes"].as_array().unwrap();
        assert!(
            notes
                .iter()
                .any(|n| n.as_str().unwrap().contains("single module")),
            "单模块坍缩必须写说明: {notes:?}"
        );
    }

    /// 语言身份专用 fixture：覆盖冻结规则表的每一行。
    /// - file:src/lib.rs / src/glue.ts → 扩展名表（rust / typescript）
    /// - file:include/util.h → 扩展名表 .h→c（已知误伤路径）
    /// - file:Makefile → 无扩展名，properties.language=shell 必须优先
    /// - file:src/odd.ts + properties.language=shell → 优先级锁定（属性胜过扩展名）
    /// - symbol alpha（src/lib.rs）→ 跟随文件扩展名 → rust
    /// - symbol loose（无 file，仅 modulePath）→ 省略 language
    /// - package → 不带 language
    fn language_fixture() -> Value {
        json!({
            "schemaVersion": "0.3.0",
            "analyzedAt": "2026-08-24T00:00:00Z",
            "language": "rust",
            "root": "x",
            "graph": {
                "nodes": [
                    {"id": "package:proj", "label": "package",
                     "properties": {"name": "proj"}},
                    {"id": "file:src/lib.rs", "label": "source-file",
                     "properties": {"sourcePath": "src/lib.rs"}},
                    {"id": "file:src/glue.ts", "label": "source-file",
                     "properties": {"sourcePath": "src/glue.ts"}},
                    {"id": "file:src/odd.ts", "label": "source-file",
                     "properties": {"sourcePath": "src/odd.ts", "language": "shell"}},
                    {"id": "file:include/util.h", "label": "source-file",
                     "properties": {"sourcePath": "include/util.h"}},
                    {"id": "file:Makefile", "label": "source-file",
                     "properties": {"sourcePath": "Makefile", "language": "shell"}},
                    {"id": "symbol:crate::alpha", "label": "symbol",
                     "properties": {"name": "alpha", "symbolKind": "function",
                                    "sourcePath": "src/lib.rs", "modulePath": "crate"}},
                    {"id": "symbol:crate::loose", "label": "symbol",
                     "properties": {"name": "loose", "symbolKind": "function",
                                    "modulePath": "crate"}}
                ],
                "edges": [
                    {"source": "symbol:crate::alpha", "target": "symbol:crate::loose",
                     "type": "CALLS", "properties": {"confidence": 0.9}}
                ]
            }
        })
    }

    #[test]
    fn file_and_symbol_nodes_carry_language_identity() {
        let out = convert(&language_fixture());
        let nodes = out["graph"]["nodes"].as_array().unwrap();
        let get = |id: &str| nodes.iter().find(|n| n["id"] == id).unwrap();
        // 文件节点：扩展名表（.rs→rust、.ts→typescript、.h→c）
        assert_eq!(get("file:src/lib.rs")["language"], "rust");
        assert_eq!(get("file:src/glue.ts")["language"], "typescript");
        assert_eq!(get("file:include/util.h")["language"], "c");
        // 文件节点：properties.language 优先于扩展名（有/无扩展名两种情形）
        assert_eq!(get("file:Makefile")["language"], "shell");
        assert_eq!(get("file:src/odd.ts")["language"], "shell");
        // 符号节点：跟随所属文件的扩展名
        assert_eq!(get("symbol:crate::alpha")["language"], "rust");
        // 无 file 符号：整个字段省略（缺席即未知，禁止 null/"")
        let loose = get("symbol:crate::loose");
        assert!(
            loose.get("language").is_none(),
            "无 file 符号必须省略 language: {loose:?}"
        );
        // package 容器跨语言：不带 language（字段必须缺席）
        let pkg = get("package:proj");
        assert!(
            pkg.get("language").is_none(),
            "package 节点不得带 language: {pkg:?}"
        );
    }

    #[test]
    fn no_node_writes_empty_string_language() {
        // 全量扫描：凡出现 language 必须是非空字符串；缺席以字段不存在表达
        let out = convert(&language_fixture());
        for n in out["graph"]["nodes"].as_array().unwrap() {
            if let Some(lang) = n.get("language") {
                assert!(lang.is_string(), "language 必须是字符串: {n:?}");
                assert!(
                    !lang.as_str().unwrap().is_empty(),
                    "禁止写空串 language: {n:?}"
                );
            }
        }
    }

    #[test]
    fn module_graph_languages_are_sorted_deduped_and_omitted_when_all_unknown() {
        let out = convert(&language_fixture());
        let modules = out["moduleGraph"]["modules"].as_array().unwrap();
        let find = |id: &str| modules.iter().find(|m| m["id"] == id).unwrap();
        // src 模块：lib.rs(rust) + glue.ts(ts) + odd.ts(shell) + alpha(rust) → 字母序去重
        assert_eq!(
            find("src")["languages"],
            json!(["rust", "shell", "typescript"])
        );
        assert_eq!(find("include")["languages"], json!(["c"]));
        // 无点文件名被既有归属启发式当作目录段（模块 id=Makefile），沿用不改
        assert_eq!(find("Makefile")["languages"], json!(["shell"]));
        // 全未知模块（只有无 file 符号）：省略字段，禁止 []
        let unknown_only = find("crate");
        assert!(
            unknown_only.get("languages").is_none(),
            "全未知模块必须省略 languages: {unknown_only:?}"
        );
    }

    #[test]
    fn h_extension_mislabel_limitation_is_documented() {
        // .h→c 误伤是身份标注的已知边界，必须写 limitations，不在转换器里猜
        let out = convert(&language_fixture());
        let notes = out["limitations"]["notes"].as_array().unwrap();
        assert!(
            notes.iter().any(|n| {
                let s = n.as_str().unwrap();
                s.contains(".h") && s.contains("'c'") && s.contains("C++")
            }),
            "limitations 必须包含 .h→c 误伤说明: {notes:?}"
        );
    }

    // ── P3：Phase A enriched 段 + --redact-root（退役 Python snapshot-gen 的契约锁）──

    /// 死代码候选必须按 CALLS 边实算（未被调用 = 候选）。这是对 Python
    /// snapshot-gen 真 bug 的回归锁：其 call_targets 恒为空，候选数恒等于
    /// 符号总数；core 按段语义实算，不允许退役时把 bug 一起搬进来。
    #[test]
    fn cleanup_counts_uncalled_symbols_from_call_edges() {
        let out = convert(&analyze_fixture());
        let c = &out["cleanup"];
        assert_eq!(c["status"], "partial");
        // fixture 3 个符号：beta（alpha→beta）、gamma（alpha/beta→gamma）有入边；
        // alpha 无任何 CALLS 入边 → 候选 1；external API：alpha/gamma pub → 2
        assert_eq!(c["unreachableCandidateCount"], 1);
        assert_eq!(c["deadCodeCandidateCount"], 1);
        assert_eq!(c["externalApiSurfaceCount"], 2);
        assert_eq!(c["frameworkEntryHintCount"], Value::Null);
    }

    #[test]
    fn quality_section_computes_overall_and_gate_counts_from_passed_field() {
        let mut analyze = analyze_fixture();
        analyze["qualityGates"] = json!([
            {"gateName": "a", "passed": true, "detail": "ok"},
            {"gateName": "b", "passed": true, "detail": "ok"},
            {"gateName": "c", "passed": false, "detail": "bad"}
        ]);
        let out = convert(&analyze);
        let q = &out["quality"];
        assert_eq!(q["overall"], "fail");
        // stats 实算回归锁：Python 旧输出读 status 字段恒得 0/0
        assert_eq!(q["passedGateCount"], 2);
        assert_eq!(q["failedGateCount"], 1);
        assert_eq!(q["gates"].as_array().unwrap().len(), 3);
        // 无门时 overall=unknown
        analyze["qualityGates"] = json!([]);
        let out = convert(&analyze);
        assert_eq!(out["quality"]["overall"], "unknown");
    }

    #[test]
    fn explore_section_lists_symbols_with_identity_and_source_files() {
        let out = convert(&analyze_fixture());
        let e = &out["explore"];
        assert_eq!(e["status"], "collected");
        assert_eq!(e["totalSymbols"], 3);
        assert_eq!(e["totalSourceFiles"], 2);
        assert_eq!(e["truncated"], false);
        let syms = e["symbols"].as_array().unwrap();
        let alpha = syms
            .iter()
            .find(|s| s["id"] == "symbol:crate::alpha")
            .unwrap();
        assert_eq!(alpha["name"], "alpha");
        assert_eq!(alpha["kind"], "function");
        assert_eq!(alpha["kindLabel"], "Function");
        assert_eq!(alpha["file"], "src/lib.rs");
        assert_eq!(alpha["visibility"], "pub");
        assert_eq!(alpha["exported"], true);
        // 源文件清单带符号计数（alpha/beta 在 src/lib.rs，gamma 在 tests/helper.rs）
        let sfs = e["sourceFiles"].as_array().unwrap();
        let lib = sfs.iter().find(|f| f["path"] == "src/lib.rs").unwrap();
        assert_eq!(lib["symbolCount"], 2);
        // topFiles：按符号数排序带 reason
        let top = e["topFiles"].as_array().unwrap();
        assert!(top
            .iter()
            .any(|t| t["path"] == "src/lib.rs" && t["symbolCount"] == 2));
        assert!(top.iter().all(|t| t["reason"] == "highest-symbol-count"));
    }

    #[test]
    fn release_review_counts_public_surface_and_risk_level() {
        let out = convert(&analyze_fixture());
        let r = &out["releaseReview"];
        assert_eq!(r["status"], "partial");
        // fixture pub 符号 alpha/gamma → surface 2，≤20 → low
        assert_eq!(r["breakingChangeSurface"], 2);
        assert_eq!(r["breakingChangeRisk"], "low");
        assert_eq!(r["missingTestCandidateCount"], Value::Null);
    }

    #[test]
    fn workflow_presets_carry_ten_frozen_entries() {
        let out = convert(&analyze_fixture());
        let w = &out["workflowPresets"];
        assert_eq!(w["status"], "collected");
        let presets = w["presets"].as_array().unwrap();
        assert_eq!(presets.len(), 10);
        assert!(presets
            .iter()
            .all(|p| p["id"].is_string() && p["stopLines"].is_array()));
    }

    #[test]
    fn insights_review_first_lists_top_hotspot_files() {
        // fixture 唯一热点 alpha→beta（fan-out 1 < 3 无热点）→ reviewFirst 空；构造高扇出：
        let mut analyze = analyze_fixture();
        let mut edges = Vec::new();
        for i in 0..4 {
            edges.push(json!({
                "source": "symbol:crate::alpha",
                "target": format!("symbol:crate::t{i}"),
                "type": "CALLS",
                "properties": {"confidence": 0.9}
            }));
            analyze["graph"]["nodes"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "id": format!("symbol:crate::t{i}"), "label": "symbol",
                    "properties": {"name": format!("t{i}"), "sourcePath": "src/t.rs"}
                }));
        }
        analyze["graph"]["edges"] = json!(edges);
        let out = convert(&analyze);
        let rf = out["insights"]["reviewFirst"].as_array().unwrap();
        assert_eq!(rf.len(), 1);
        assert_eq!(rf[0]["file"], "src/lib.rs");
        assert_eq!(rf[0]["reason"], "high-fan-out-hotspot");
    }

    #[test]
    fn redact_root_replaces_paths_and_recomputes_relation_keys_afterwards() {
        let mut analyze = analyze_fixture();
        let root = "/Users/dev/work/portable-smoke";
        analyze["root"] = json!(root);
        let nodes = analyze["graph"]["nodes"].as_array_mut().unwrap();
        // repo 容器 id 内嵌绝对根；符号 sourcePath 用绝对路径
        nodes[0]["properties"]["sourcePath"] = json!(format!("{root}/src"));
        for n in nodes.iter_mut().skip(1) {
            let sp = n["properties"]["sourcePath"].as_str().unwrap().to_string();
            n["properties"]["sourcePath"] = json!(format!("{root}/{sp}"));
        }
        let edges = analyze["graph"]["edges"].as_array_mut().unwrap();
        for e in edges.iter_mut() {
            let t = e["target"].as_str().unwrap().to_string();
            e["source"] = json!("symbol:x::abs:/Users/dev/other/a.rs");
            e["target"] = json!(t);
        }
        let out = convert_analyze_result_with_options(&analyze, "9.9.9", true);
        let raw = serde_json::to_string(&out).unwrap();
        assert!(!raw.contains("/Users/dev"), "机器路径泄漏: {raw}");
        // 根字段也脱敏
        assert_eq!(out["root"], "<redacted-root>");
        // relationKey 必须基于脱敏后的最终端点重算（拿 CALLS 边验证）
        let edges = out["graph"]["edges"].as_array().unwrap();
        for e in edges {
            let s = e["source"].as_str().unwrap();
            let t = e["target"].as_str().unwrap();
            let k = e["kind"].as_str().unwrap();
            let expect = format!("rel:sha256:{}", {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(s.as_bytes());
                h.update([0u8]);
                h.update(k.as_bytes());
                h.update([0u8]);
                h.update(t.as_bytes());
                h.finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            });
            assert_eq!(e["relationKey"], expect, "脱敏后 relationKey 未重算: {e}");
        }
        // 未开 redact 时同一路径保持原样
        let out_plain = convert_analyze_result_with_options(&analyze, "9.9.9", false);
        assert!(serde_json::to_string(&out_plain)
            .unwrap()
            .contains("/Users/dev"));
    }

    #[test]
    fn enriched_envelope_carries_all_contract_top_keys() {
        let out = convert(&analyze_fixture());
        for k in [
            "schemaVersion",
            "generatedAt",
            "generatedFrom",
            "summary",
            "quality",
            "limitations",
            "explore",
            "cleanup",
            "releaseReview",
            "insights",
            "workflowPresets",
            "graph",
            "moduleGraph",
        ] {
            assert!(out.get(k).is_some(), "缺顶层键: {k}");
        }
        // 卡 1 既有断言不受 enrichment 影响：summary 计数与 moduleGraph languages
        assert_eq!(out["summary"]["moduleCount"], 0);
        assert_eq!(out["summary"]["packageCount"], 1);
    }

    /// module-id 规则冻结用例（承接 scripts/test_module_graph.py 退役后的覆盖）：
    /// 前两级目录、(root)、<redacted-root> 占位段剥离、rust modulePath 回退。
    #[test]
    fn module_id_rules_freeze_placeholder_strip_and_module_path_fallback() {
        assert_eq!(
            module_id_from_file("crates/cli/src/main.rs").as_deref(),
            Some("crates/cli")
        );
        assert_eq!(module_id_from_file("src/lib.rs").as_deref(), Some("src"));
        assert_eq!(module_id_from_file("main.rs").as_deref(), Some("(root)"));
        assert_eq!(
            module_id_from_file("<redacted-root>/crates/core/src/lib.rs").as_deref(),
            Some("crates/core")
        );
        assert_eq!(module_id_from_file("").as_deref(), None);
        assert_eq!(
            module_id_from_rust_module_path("crate::foo::bar").as_deref(),
            Some("crate::foo")
        );
        assert_eq!(
            module_id_from_rust_module_path("crate").as_deref(),
            Some("crate")
        );
        assert_eq!(module_id_from_rust_module_path("").as_deref(), None);
    }
}
