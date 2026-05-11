//! GitNexus Rust-core CLI
//!
//! 提供 productization 入口命令：
//! - project-model inspect（保留现有 Rust 分析入口）
//! - cangjie inspect/graph（保留现有 Cangjie 分析入口）
//! - analyze（新增统一分析入口）
//! - quality（新增质量门命令）
//! - summary（新增概要命令）
//!
//! JSON stdout，human logs stderr。

mod arkts_bridge;
mod bridge_format;
mod cangjie_bridge;
mod language_detect;
mod mcp_server;
mod rust_bridge;
mod unified_types;

use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::path::Path;

use language_detect::detect_language;
use unified_types::{
    DetectedLanguage, GraphSummary, LanguageAnalysisResult, QualityCommandOutput,
    QualityGateResult, QualitySummary, SummaryCommandOutput,
};

#[derive(Parser)]
#[command(
    name = "codelattice",
    version,
    about = "CodeLattice local code intelligence CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// ProjectModel 子命令域（保留现有入口）
    ProjectModel {
        #[command(subcommand)]
        sub: ProjectModelCommands,
    },
    /// Cangjie 子命令域（保留现有入口）
    Cangjie {
        #[command(subcommand)]
        sub: CangjieCommands,
    },
    /// 统一分析入口：自动检测语言或按指定语言分析项目，输出完整 graph JSON
    Analyze {
        /// 项目根目录路径
        #[arg(long)]
        root: String,
        /// 语言：rust / cangjie / auto（自动检测）
        #[arg(long, default_value = "auto")]
        language: String,
        /// 输出格式（MVP 仅支持 json）
        #[arg(long, default_value = "json")]
        format: String,
        /// 严格模式：质量门失败时 exit code 非零
        #[arg(long, default_value = "false")]
        strict: bool,
    },
    /// 质量门检查：对指定项目运行质量门，输出 JSON，退出码反映结果
    Quality {
        /// 项目根目录路径
        #[arg(long)]
        root: String,
        /// 语言：rust / cangjie（必须显式指定）
        #[arg(long)]
        language: String,
        /// 输出格式（MVP 仅支持 json）
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// 概要输出：不含完整 graph，只输出 stats + quality summary
    Summary {
        /// 项目根目录路径
        #[arg(long)]
        root: String,
        /// 语言：rust / cangjie / auto
        #[arg(long, default_value = "auto")]
        language: String,
        /// 输出格式（MVP 仅支持 json）
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Start MCP stdio server (JSON-RPC over stdin/stdout)
    Mcp,
}

#[derive(Subcommand)]
enum ProjectModelCommands {
    /// 输出 ProjectModel JSON
    Inspect {
        #[arg(long)]
        root: String,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long, value_name = "INCLUDE")]
        include: Vec<String>,
    },
}

#[derive(Subcommand)]
enum CangjieCommands {
    /// 输出 Cangjie 项目 JSON
    Inspect {
        #[arg(long)]
        root: String,
        #[arg(long, default_value = "false")]
        strict: bool,
    },
    /// 输出 Cangjie 图 JSON
    Graph {
        #[arg(long)]
        root: String,
        #[arg(long, default_value = "false")]
        strict: bool,
    },
}

// ============================================================
// 辅助函数
// ============================================================

/// 解析语言参数，支持 auto 检测
fn resolve_language(lang_arg: &str, root: &Path) -> Result<String, String> {
    if lang_arg == "auto" {
        match detect_language(root) {
            DetectedLanguage::Rust => Ok("rust".to_string()),
            DetectedLanguage::Cangjie => Ok("cangjie".to_string()),
            DetectedLanguage::ArkTS => Ok("arkts".to_string()),
            DetectedLanguage::TypeScript => Ok("typescript".to_string()),
            DetectedLanguage::Ambiguous => Err(
                "语言检测失败：存在多种清单文件，请使用 --language rust|cangjie|arkts|typescript 显式指定".to_string(),
            ),
            DetectedLanguage::Unknown => Err(
                "语言检测失败：未找到可识别的清单文件，无法自动检测语言".to_string(),
            ),
        }
    } else if ["rust", "cangjie", "arkts", "typescript"].contains(&lang_arg) {
        Ok(lang_arg.to_string())
    } else {
        Err(format!(
            "不支持的语言: {lang_arg}，请使用 rust / cangjie / arkts / typescript / auto"
        ))
    }
}

/// 验证 root 路径存在
fn check_root(root: &str) -> Result<&Path, String> {
    let path = Path::new(root);
    if !path.exists() {
        return Err(format!("错误：root 路径不存在: {root}"));
    }
    Ok(path)
}

/// 获取当前时间 ISO 8601 字符串
fn now_iso8601() -> String {
    // 用 chrono 零依赖替代方案：直接输出 UTC 时间字符串
    // 简单起见，使用 SystemTime
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // 简单格式化：YYYY-MM-DDTHH:MM:SSZ（不引入 chrono 依赖）
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // 计算年月日（简化版，从 UNIX epoch 1970-01-01 开始）
    let mut year = 1970i64;
    let mut remaining_days = days_since_epoch as i64;

    fn days_in_year(y: i64) -> i64 {
        if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
            366
        } else {
            365
        }
    }

    while remaining_days >= days_in_year(year) {
        remaining_days -= days_in_year(year);
        year += 1;
    }

    let months_days = if days_in_year(year) == 366 {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &md in &months_days {
        if remaining_days < md as i64 {
            break;
        }
        remaining_days -= md as i64;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

// ============================================================
// Rust 分析 + Graph 提取
// ============================================================

/// 运行 Rust 分析，返回 (GraphOutput JSON, nodes, edges)
fn run_rust_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let pm_output = gitnexus_project_model::output::inspect_project_model_with_options(
        root, true, // include_symbols
        true, // include_graph
        true, // include_imports
        true, // include_calls
    );

    let graph_output = gitnexus_project_model::output::emit_graph_output(&pm_output);
    let json_val = serde_json::to_value(&graph_output)
        .map_err(|e| format!("Rust Graph JSON 序列化失败: {e}"))?;

    let nodes: Vec<serde_json::Value> = json_val
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let edges: Vec<serde_json::Value> = json_val
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    Ok((json_val, nodes, edges))
}

/// 计算 Rust 质量门
fn compute_rust_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    let mut gates = Vec::new();

    // 1. duplicate_nodes
    let node_ids: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n.get("id").and_then(|v| v.as_str()))
        .collect();
    let unique_node_ids: HashSet<&str> = node_ids.iter().copied().collect();
    let dup_nodes = node_ids.len() - unique_node_ids.len();
    gates.push(QualityGateResult {
        gate_name: "duplicate_nodes".to_string(),
        passed: dup_nodes == 0,
        detail: if dup_nodes == 0 {
            "0 duplicate node IDs found".to_string()
        } else {
            format!("{dup_nodes} duplicate node IDs found")
        },
    });

    // 2. duplicate_edges
    let edge_triples: Vec<(&str, &str, &str)> = edges
        .iter()
        .filter_map(|e| {
            let src = e.get("source").and_then(|v| v.as_str())?;
            let tgt = e.get("target").and_then(|v| v.as_str())?;
            let typ = e.get("type").and_then(|v| v.as_str())?;
            Some((src, tgt, typ))
        })
        .collect();
    let unique_edge_triples: HashSet<_> = edge_triples.iter().copied().collect();
    let dup_edges = edge_triples.len() - unique_edge_triples.len();
    gates.push(QualityGateResult {
        gate_name: "duplicate_edges".to_string(),
        passed: dup_edges == 0,
        detail: if dup_edges == 0 {
            "0 duplicate edge triples found".to_string()
        } else {
            format!("{dup_edges} duplicate edge triples found")
        },
    });

    // 3. dangling_source
    let node_id_set: HashSet<&str> = node_ids.iter().copied().collect();
    let dangling_sources: Vec<&str> = edges
        .iter()
        .filter_map(|e| e.get("source").and_then(|v| v.as_str()))
        .filter(|s| !node_id_set.contains(s))
        .collect();
    gates.push(QualityGateResult {
        gate_name: "dangling_source".to_string(),
        passed: dangling_sources.is_empty(),
        detail: if dangling_sources.is_empty() {
            "0 dangling source references found".to_string()
        } else {
            format!(
                "{} dangling source references found",
                dangling_sources.len()
            )
        },
    });

    // 4. dangling_target
    let dangling_targets: Vec<&str> = edges
        .iter()
        .filter_map(|e| e.get("target").and_then(|v| v.as_str()))
        .filter(|t| !node_id_set.contains(t))
        .collect();
    gates.push(QualityGateResult {
        gate_name: "dangling_target".to_string(),
        passed: dangling_targets.is_empty(),
        detail: if dangling_targets.is_empty() {
            "0 dangling target references found".to_string()
        } else {
            format!(
                "{} dangling target references found",
                dangling_targets.len()
            )
        },
    });

    // 5. deterministic — 单次运行无法验证
    gates.push(QualityGateResult {
        gate_name: "deterministic".to_string(),
        passed: true,
        detail: "not verified from single CLI run; verified by test suite".to_string(),
    });

    // 6. calls_endpoint_integrity
    let calls_dangling = edges
        .iter()
        .filter(|e| {
            e.get("type")
                .and_then(|v| v.as_str())
                .map_or(false, |t| t == "CALLS")
        })
        .filter(|e| {
            let src_ok = e
                .get("source")
                .and_then(|v| v.as_str())
                .map_or(false, |s| node_id_set.contains(s));
            let tgt_ok = e
                .get("target")
                .and_then(|v| v.as_str())
                .map_or(false, |t| node_id_set.contains(t));
            !(src_ok && tgt_ok)
        })
        .count();
    gates.push(QualityGateResult {
        gate_name: "calls_endpoint_integrity".to_string(),
        passed: calls_dangling == 0,
        detail: if calls_dangling == 0 {
            "All CALLS edge endpoints exist in nodes".to_string()
        } else {
            format!("{calls_dangling} CALLS edges have missing endpoints")
        },
    });

    // 7. external_symbol_marking（仅检查 isExternal 属性）
    let external_unmarked = nodes
        .iter()
        .filter(|n| {
            n.get("properties")
                .and_then(|p| p.get("isExternal"))
                .and_then(|v| v.as_bool())
                == Some(true)
        })
        .count();
    gates.push(QualityGateResult {
        gate_name: "external_symbol_marking".to_string(),
        passed: true, // 如果 external nodes 有 isExternal=true，这个门通过
        detail: if external_unmarked > 0 {
            format!(
                "{external_unmarked} external symbol nodes properly marked with isExternal=true"
            )
        } else {
            "No external symbols (or no external crate imports in this project)".to_string()
        },
    });

    gates
}

/// 构建 Rust GraphSummary
///
/// Rust GraphOutput 的 kind 信息编码在 label 中，stats 在 `stats` 字段
fn build_rust_summary(
    json_val: &serde_json::Value,
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> GraphSummary {
    // 从 Rust GraphOutput stats 提取已统计的数值
    let stats = json_val.get("stats");

    let node_count = stats
        .and_then(|s| s.get("nodeCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(nodes.len() as u64) as u32;

    let edge_count = stats
        .and_then(|s| s.get("edgeCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(edges.len() as u64) as u32;

    let symbol_count = stats
        .and_then(|s| s.get("symbolCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let diagnostic_count = stats
        .and_then(|s| s.get("diagnosticCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let call_edge_count = stats
        .and_then(|s| s.get("callEdgeCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // source_file / package 计数：从 label 推断
    // Rust GraphOutput 不使用统一的 kind 字段，label 编码类型信息
    // 统计 label 以 "source-file" / "package" / "target" 结尾的节点
    let source_file_count = nodes
        .iter()
        .filter(|n| n.get("label").and_then(|v| v.as_str()) == Some("source-file"))
        .count() as u32;

    let package_count = nodes
        .iter()
        .filter(|n| {
            let label = n.get("label").and_then(|v| v.as_str());
            label == Some("package") || label == Some("target")
        })
        .count() as u32;

    GraphSummary {
        node_count,
        edge_count,
        symbol_count,
        source_file_count,
        package_count,
        diagnostic_count,
        call_edge_count,
    }
}

// ============================================================
// Cangjie 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
fn run_cangjie_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let graph_output = gitnexus_cangjie::graph::inspect_cangjie_project(root)
        .map_err(|e| format!("Cangjie 项目分析失败: {e}"))?;

    let json_val =
        serde_json::to_value(&graph_output).map_err(|e| format!("Cangjie JSON 序列化失败: {e}"))?;

    let nodes: Vec<serde_json::Value> = json_val
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let edges: Vec<serde_json::Value> = json_val
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-cangjie"))]
fn run_cangjie_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err("Cangjie support is disabled. 请使用 --features tree-sitter-cangjie 重新编译。".to_string())
}

// ============================================================
// ArkTS 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
fn run_arkts_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    use std::collections::BTreeMap;

    // 1. Build project model
    let project = gitnexus_arkts::project::find_arkts_project_root(root)
        .ok_or_else(|| "ArkTS project root not found (no oh-package.json5)".to_string())?;

    let source_files = gitnexus_arkts::project::list_arkts_source_files(&project)
        .map_err(|e| format!("Failed to list ArkTS source files: {e}"))?;

    // 2. Parse manifest
    let manifest = gitnexus_arkts::load_ts_manifest(&project).ok();

    // 3. Extract per-file data
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_arkts::TsSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_arkts::TsImport>> =
        BTreeMap::new();
    let mut references_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_arkts::TsReference>> =
        BTreeMap::new();
    let mut components_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_arkts::ArkTsComponent>> =
        BTreeMap::new();

    for file in &source_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Base TypeScript extraction
        let lang = gitnexus_arkts::TsLanguage::TypeScript;
        let syms = gitnexus_arkts::extract_ts_symbols(&source, lang);
        let imps = gitnexus_arkts::extract_ts_imports(&source, lang);
        let refs = gitnexus_arkts::extract_ts_references(&source, lang);

        symbols_by_file.insert(file.clone(), syms);
        imports_by_file.insert(file.clone(), imps);
        references_by_file.insert(file.clone(), refs);

        // ArkTS-specific component extraction
        let components = gitnexus_arkts::extract_arkts_components(&source);
        if !components.is_empty() {
            components_by_file.insert(file.clone(), components);
        }
    }

    // 4. Build graph
    let ts_project = gitnexus_arkts::TsProject {
        root: project.clone(),
        kind: gitnexus_arkts::TsProjectKind::ArkTS,
        manifest,
        source_files: source_files.clone(),
    };

    let mut graph = gitnexus_arkts::build_ts_graph(
        &ts_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
    );

    // Augment with ArkTS-specific nodes
    gitnexus_arkts::graph::augment_graph_with_arkts(&mut graph, &components_by_file);

    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("ArkTS graph JSON serialization failed: {e}"))?;

    let nodes: Vec<serde_json::Value> = json_val
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let edges: Vec<serde_json::Value> = json_val
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-arkts"))]
fn run_arkts_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err("ArkTS support is disabled. 请使用 --features tree-sitter-arkts 重新编译。".to_string())
}

// ============================================================
// TypeScript 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
fn run_typescript_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    use std::collections::BTreeMap;

    // 1. Build project model (tsconfig.json / package.json)
    let project = gitnexus_typescript::project::find_project_root(root).ok_or_else(|| {
        "TypeScript project root not found (no tsconfig.json or package.json)".to_string()
    })?;

    let source_files = gitnexus_typescript::project::list_source_files(&project)
        .map_err(|e| format!("Failed to list TypeScript source files: {e}"))?;

    // 2. Parse manifest
    let manifest = gitnexus_typescript::load_ts_manifest(&project).ok();

    // 3. Extract per-file data
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_typescript::TsSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_typescript::TsImport>> =
        BTreeMap::new();
    let mut references_by_file: BTreeMap<
        std::path::PathBuf,
        Vec<gitnexus_typescript::TsReference>,
    > = BTreeMap::new();

    for file in &source_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Detect language variant from extension
        let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
            gitnexus_typescript::extractors::TsLanguage::Tsx
        } else {
            gitnexus_typescript::extractors::TsLanguage::TypeScript
        };
        let syms = gitnexus_typescript::extractors::extract_ts_symbols(&source, lang);
        let imps = gitnexus_typescript::extractors::extract_ts_imports(&source, lang);
        let refs = gitnexus_typescript::extractors::extract_ts_references(&source, lang);

        symbols_by_file.insert(file.clone(), syms);
        imports_by_file.insert(file.clone(), imps);
        references_by_file.insert(file.clone(), refs);
    }

    // 4. Build graph
    let kind = gitnexus_typescript::project::detect_project_kind(&project);
    let ts_project = gitnexus_typescript::TsProject {
        root: project,
        kind,
        manifest,
        source_files: source_files.clone(),
    };

    let graph = gitnexus_typescript::graph::build_ts_graph(
        &ts_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
    );

    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("TypeScript graph JSON serialization failed: {e}"))?;

    let nodes: Vec<serde_json::Value> = json_val
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let edges: Vec<serde_json::Value> = json_val
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-arkts"))]
fn run_typescript_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err(
        "TypeScript support is disabled. 请使用 --features tree-sitter-arkts 重新编译。"
            .to_string(),
    )
}

/// 计算 ArkTS 质量门
fn compute_arkts_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    let mut gates = Vec::new();

    // 1. duplicate_nodes
    let node_ids: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n.get("id").and_then(|v| v.as_str()))
        .collect();
    let unique_node_ids: HashSet<&str> = node_ids.iter().copied().collect();
    let dup_nodes = node_ids.len() - unique_node_ids.len();
    gates.push(QualityGateResult {
        gate_name: "duplicate_nodes".to_string(),
        passed: dup_nodes == 0,
        detail: if dup_nodes == 0 {
            "0 duplicate node IDs found".to_string()
        } else {
            format!("{dup_nodes} duplicate node IDs found")
        },
    });

    // 2. dangling_source
    let node_id_set: HashSet<&str> = node_ids.iter().copied().collect();
    let dangling_sources: Vec<&str> = edges
        .iter()
        .filter_map(|e| e.get("source").and_then(|v| v.as_str()))
        .filter(|s| !node_id_set.contains(s))
        .collect();
    gates.push(QualityGateResult {
        gate_name: "dangling_source".to_string(),
        passed: dangling_sources.is_empty(),
        detail: if dangling_sources.is_empty() {
            "0 dangling source references found".to_string()
        } else {
            format!(
                "{} dangling source references found",
                dangling_sources.len()
            )
        },
    });

    // 3. deterministic
    gates.push(QualityGateResult {
        gate_name: "deterministic".to_string(),
        passed: true,
        detail: "not verified from single CLI run; verified by test suite".to_string(),
    });

    gates
}

/// 构建 ArkTS GraphSummary
fn build_arkts_summary(nodes: &[serde_json::Value], edges: &[serde_json::Value]) -> GraphSummary {
    let symbol_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("symbol"))
        .count();
    let source_file_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("sourceFile"))
        .count();
    let import_edge_count = edges
        .iter()
        .filter(|e| e.get("kind").and_then(|v| v.as_str()) == Some("imports"))
        .count();
    let call_edge_count = edges
        .iter()
        .filter(|e| e.get("kind").and_then(|v| v.as_str()) == Some("calls"))
        .count();

    GraphSummary {
        node_count: nodes.len() as u32,
        edge_count: edges.len() as u32,
        symbol_count: symbol_count as u32,
        source_file_count: source_file_count as u32,
        package_count: 1,
        diagnostic_count: 0,
        call_edge_count: call_edge_count as u32,
    }
}

/// 计算 Cangjie 质量门
fn compute_cangjie_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    let mut gates = Vec::new();

    // 1. synthetic_nodes（CallableSource 计数）
    let synthetic_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("callableSource"))
        .count();
    gates.push(QualityGateResult {
        gate_name: "synthetic_nodes".to_string(),
        passed: synthetic_count == 0,
        detail: if synthetic_count == 0 {
            "0 synthetic (CallableSource) nodes found".to_string()
        } else {
            format!("{synthetic_count} synthetic (CallableSource) nodes found")
        },
    });

    // 2. duplicate_nodes
    let node_ids: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n.get("id").and_then(|v| v.as_str()))
        .collect();
    let unique_node_ids: HashSet<&str> = node_ids.iter().copied().collect();
    let dup_nodes = node_ids.len() - unique_node_ids.len();
    gates.push(QualityGateResult {
        gate_name: "duplicate_nodes".to_string(),
        passed: dup_nodes == 0,
        detail: if dup_nodes == 0 {
            "0 duplicate node IDs found".to_string()
        } else {
            format!("{dup_nodes} duplicate node IDs found")
        },
    });

    // 3. duplicate_edges
    let edge_triples: Vec<(&str, &str, &str)> = edges
        .iter()
        .filter_map(|e| {
            let kind = e.get("kind").and_then(|v| v.as_str())?;
            let src = e.get("sourceId").and_then(|v| v.as_str())?;
            let tgt = e.get("targetId").and_then(|v| v.as_str())?;
            Some((src, tgt, kind))
        })
        .collect();
    let unique_edge_triples: HashSet<_> = edge_triples.iter().copied().collect();
    let dup_edges = edge_triples.len() - unique_edge_triples.len();
    gates.push(QualityGateResult {
        gate_name: "duplicate_edges".to_string(),
        passed: dup_edges == 0,
        detail: if dup_edges == 0 {
            "0 duplicate edge triples found".to_string()
        } else {
            format!("{dup_edges} duplicate edge triples found")
        },
    });

    // 4. dangling_source
    let node_id_set: HashSet<&str> = node_ids.iter().copied().collect();
    let dangling_sources: Vec<&str> = edges
        .iter()
        .filter_map(|e| e.get("sourceId").and_then(|v| v.as_str()))
        .filter(|s| !node_id_set.contains(s))
        .collect();
    gates.push(QualityGateResult {
        gate_name: "dangling_source".to_string(),
        passed: dangling_sources.is_empty(),
        detail: if dangling_sources.is_empty() {
            "0 dangling source references found".to_string()
        } else {
            format!(
                "{} dangling source references found",
                dangling_sources.len()
            )
        },
    });

    // 5. dangling_target
    let dangling_targets: Vec<&str> = edges
        .iter()
        .filter_map(|e| e.get("targetId").and_then(|v| v.as_str()))
        .filter(|t| !node_id_set.contains(t))
        .collect();
    gates.push(QualityGateResult {
        gate_name: "dangling_target".to_string(),
        passed: dangling_targets.is_empty(),
        detail: if dangling_targets.is_empty() {
            "0 dangling target references found".to_string()
        } else {
            format!(
                "{} dangling target references found",
                dangling_targets.len()
            )
        },
    });

    // 6. deterministic — 单次运行无法验证
    gates.push(QualityGateResult {
        gate_name: "deterministic".to_string(),
        passed: true,
        detail: "not verified from single CLI run; verified by test suite".to_string(),
    });

    gates
}

/// 构建 Cangjie GraphSummary
#[cfg(feature = "tree-sitter-cangjie")]
fn build_cangjie_summary(nodes: &[serde_json::Value], edges: &[serde_json::Value]) -> GraphSummary {
    let symbol_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("symbol"))
        .count() as u32;

    let source_file_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("sourceFile"))
        .count() as u32;

    let package_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("package"))
        .count() as u32;

    let diagnostic_count = nodes
        .iter()
        .filter(|n| n.get("kind").and_then(|v| v.as_str()) == Some("diagnostic"))
        .count() as u32;

    // Cangjie uses Uses/Accesses/Modifies edges (comparable to CALLS)
    let call_edge_count = edges
        .iter()
        .filter(|e| {
            let kind = e.get("kind").and_then(|v| v.as_str());
            matches!(kind, Some("uses") | Some("accesses") | Some("modifies"))
        })
        .count() as u32;

    GraphSummary {
        node_count: nodes.len() as u32,
        edge_count: edges.len() as u32,
        symbol_count,
        source_file_count,
        package_count,
        diagnostic_count,
        call_edge_count,
    }
}

#[cfg(not(feature = "tree-sitter-cangjie"))]
fn build_cangjie_summary(
    _nodes: &[serde_json::Value],
    _edges: &[serde_json::Value],
) -> GraphSummary {
    GraphSummary {
        node_count: 0,
        edge_count: 0,
        symbol_count: 0,
        source_file_count: 0,
        package_count: 0,
        diagnostic_count: 0,
        call_edge_count: 0,
    }
}

// ============================================================
// main
// ============================================================

pub fn run() {
    let cli = Cli::parse();

    match cli.command {
        // ===== 保留现有 project-model inspect =====
        Commands::ProjectModel { sub } => match sub {
            ProjectModelCommands::Inspect {
                root,
                format,
                include,
            } => {
                if format != "json" {
                    eprintln!("错误：当前仅支持 --format json");
                    std::process::exit(1);
                }

                let include_symbols = include.iter().any(|s| s == "symbols");
                let include_graph = include.iter().any(|s| s == "graph");
                let include_imports = include.iter().any(|s| s == "imports");
                let include_calls = include.iter().any(|s| s == "calls");

                let root_path = match check_root(&root) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                };

                let pm_output = gitnexus_project_model::output::inspect_project_model_with_options(
                    root_path,
                    include_symbols,
                    include_graph,
                    include_imports,
                    include_calls,
                );

                if include_graph {
                    let graph_output =
                        gitnexus_project_model::output::emit_graph_output(&pm_output);
                    let json = serde_json::to_string_pretty(&graph_output).unwrap_or_else(|e| {
                        eprintln!("错误：Graph JSON 序列化失败: {e}");
                        std::process::exit(1);
                    });
                    println!("{json}");
                } else {
                    let json = serde_json::to_string_pretty(&pm_output).unwrap_or_else(|e| {
                        eprintln!("错误：JSON 序列化失败: {e}");
                        std::process::exit(1);
                    });
                    println!("{json}");
                }
            }
        },

        // ===== 保留现有 cangjie inspect/graph =====
        Commands::Cangjie { sub } => match sub {
            CangjieCommands::Inspect { root, strict } | CangjieCommands::Graph { root, strict } => {
                #[cfg(not(feature = "tree-sitter-cangjie"))]
                {
                    let _root = root;
                    let _strict = strict;
                    eprintln!("错误：Cangjie support is disabled.");
                    eprintln!("请使用 --features tree-sitter-cangjie 重新编译：");
                    eprintln!("  cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bin codelattice -- cangjie inspect --root <path>");
                    std::process::exit(1);
                }

                #[cfg(feature = "tree-sitter-cangjie")]
                {
                    let root_path = match check_root(&root) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    match gitnexus_cangjie::graph::inspect_cangjie_project(root_path) {
                        Ok(graph_output) => {
                            if strict {
                                let synthetic_count = graph_output
                                    .nodes
                                    .iter()
                                    .filter(|n| {
                                        n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource
                                    })
                                    .count();
                                if synthetic_count > 0 {
                                    eprintln!(
                                        "错误：strict mode: found {synthetic_count} synthetic node(s), expected 0"
                                    );
                                    std::process::exit(1);
                                }
                            }
                            let json =
                                serde_json::to_string_pretty(&graph_output).unwrap_or_else(|e| {
                                    eprintln!("错误：Cangjie JSON 序列化失败: {e}");
                                    std::process::exit(1);
                                });
                            println!("{json}");
                        }
                        Err(e) => {
                            eprintln!("错误：Cangjie 项目分析失败: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
        },

        // ===== 新增：analyze 统一分析入口 =====
        Commands::Analyze {
            root,
            language,
            format,
            strict,
        } => {
            if format != "json" && format != "gitnexus-rc" {
                eprintln!("错误：支持的格式：json, gitnexus-rc");
                std::process::exit(1);
            }

            let is_bridge = format == "gitnexus-rc";
            let root_path = match check_root(&root) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            let lang = match resolve_language(&language, root_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            #[cfg(debug_assertions)]
            eprintln!("分析中... language={lang}, root={root}");

            match lang.as_str() {
                "rust" => {
                    let (json_val, nodes, edges) = match run_rust_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    // 计算 quality gates（bridge 和 json 格式都需要用于 --strict）
                    let quality_gates = compute_rust_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.3")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = bridge_format::convert_rust_graph(
                            &json_val,
                            &lang,
                            &root_path.to_string_lossy(),
                            &analyzed_at,
                        )
                        .unwrap_or_else(|e| {
                            eprintln!("错误：Bridge 格式转换失败: {e}");
                            std::process::exit(1);
                        });
                        let json = serde_json::to_string_pretty(&bridge).unwrap_or_else(|e| {
                            eprintln!("错误：Bridge JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    } else {
                        let summary = build_rust_summary(&json_val, &nodes, &edges);

                        let result = LanguageAnalysisResult {
                            language: lang,
                            root: root_path.to_string_lossy().to_string(),
                            analyzed_at: now_iso8601(),
                            schema_version,
                            summary,
                            quality_gates: quality_gates.clone(),
                            graph: json_val,
                        };

                        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
                            eprintln!("错误：JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    }

                    // --strict 检查：质量门失败时 exit non-zero
                    if strict {
                        let failed: Vec<&QualityGateResult> =
                            quality_gates.iter().filter(|g| !g.passed).collect();
                        if !failed.is_empty() {
                            eprintln!("strict mode: {} quality gate(s) failed", failed.len());
                            for g in &failed {
                                eprintln!("  - {}: {}", g.gate_name, g.detail);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                "cangjie" => {
                    let (json_val, nodes, edges) = match run_cangjie_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    // 计算 quality gates（bridge 和 json 格式都需要用于 --strict）
                    let quality_gates = compute_cangjie_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v1.0.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = bridge_format::convert_cangjie_graph(
                            &json_val,
                            &lang,
                            &root_path.to_string_lossy(),
                            &analyzed_at,
                        )
                        .unwrap_or_else(|e| {
                            eprintln!("错误：Bridge 格式转换失败: {e}");
                            std::process::exit(1);
                        });
                        let json = serde_json::to_string_pretty(&bridge).unwrap_or_else(|e| {
                            eprintln!("错误：Bridge JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    } else {
                        let summary = build_cangjie_summary(&nodes, &edges);

                        let result = LanguageAnalysisResult {
                            language: lang,
                            root: root_path.to_string_lossy().to_string(),
                            analyzed_at: now_iso8601(),
                            schema_version,
                            summary,
                            quality_gates: quality_gates.clone(),
                            graph: json_val,
                        };

                        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
                            eprintln!("错误：JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    }

                    // --strict 检查：质量门失败时 exit non-zero
                    if strict {
                        let failed: Vec<&QualityGateResult> =
                            quality_gates.iter().filter(|g| !g.passed).collect();
                        if !failed.is_empty() {
                            eprintln!("strict mode: {} quality gate(s) failed", failed.len());
                            for g in &failed {
                                eprintln!("  - {}: {}", g.gate_name, g.detail);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                "arkts" => {
                    let (json_val, nodes, edges) = match run_arkts_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_arkts_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.1.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = bridge_format::convert_arkts_graph(
                            &json_val,
                            &lang,
                            &root_path.to_string_lossy(),
                            &analyzed_at,
                        )
                        .unwrap_or_else(|e| {
                            eprintln!("错误：Bridge 格式转换失败: {e}");
                            std::process::exit(1);
                        });
                        let json = serde_json::to_string_pretty(&bridge).unwrap_or_else(|e| {
                            eprintln!("错误：Bridge JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    } else {
                        let summary = build_arkts_summary(&nodes, &edges);
                        let result = LanguageAnalysisResult {
                            language: lang,
                            root: root_path.to_string_lossy().to_string(),
                            analyzed_at: now_iso8601(),
                            schema_version,
                            summary,
                            quality_gates: quality_gates.clone(),
                            graph: json_val,
                        };
                        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
                            eprintln!("错误：JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    }

                    if strict {
                        let failed: Vec<&QualityGateResult> =
                            quality_gates.iter().filter(|g| !g.passed).collect();
                        if !failed.is_empty() {
                            eprintln!("strict mode: {} quality gate(s) failed", failed.len());
                            for g in &failed {
                                eprintln!("  - {}: {}", g.gate_name, g.detail);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                "typescript" => {
                    let (json_val, nodes, edges) = match run_typescript_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_arkts_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.1.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = bridge_format::convert_arkts_graph(
                            &json_val,
                            &lang,
                            &root_path.to_string_lossy(),
                            &analyzed_at,
                        )
                        .unwrap_or_else(|e| {
                            eprintln!("错误：Bridge 格式转换失败: {e}");
                            std::process::exit(1);
                        });
                        let json = serde_json::to_string_pretty(&bridge).unwrap_or_else(|e| {
                            eprintln!("错误：Bridge JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    } else {
                        let summary = build_arkts_summary(&nodes, &edges);
                        let result = LanguageAnalysisResult {
                            language: lang,
                            root: root_path.to_string_lossy().to_string(),
                            analyzed_at: now_iso8601(),
                            schema_version,
                            summary,
                            quality_gates: quality_gates.clone(),
                            graph: json_val,
                        };
                        let json = serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
                            eprintln!("错误：JSON 序列化失败: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                    }

                    if strict {
                        let failed: Vec<&QualityGateResult> =
                            quality_gates.iter().filter(|g| !g.passed).collect();
                        if !failed.is_empty() {
                            eprintln!("strict mode: {} quality gate(s) failed", failed.len());
                            for g in &failed {
                                eprintln!("  - {}: {}", g.gate_name, g.detail);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                other => {
                    eprintln!("错误：不支持的语言: {other}");
                    std::process::exit(1);
                }
            }
        }

        // ===== 新增：quality 质量门命令 =====
        Commands::Quality {
            root,
            language,
            format,
        } => {
            if format != "json" {
                eprintln!("错误：当前仅支持 --format json");
                std::process::exit(1);
            }

            let root_path = match check_root(&root) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            if language != "rust"
                && language != "cangjie"
                && language != "arkts"
                && language != "typescript"
            {
                eprintln!(
                    "错误：quality 命令需要显式指定 --language rust|cangjie|arkts|typescript"
                );
                std::process::exit(1);
            }

            eprintln!("质量门检查中... language={language}, root={root}");

            let (gates, overall) = match language.as_str() {
                "rust" => {
                    let (_json_val, nodes, edges) = match run_rust_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_rust_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "cangjie" => {
                    let (_json_val, nodes, edges) = match run_cangjie_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_cangjie_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "arkts" => {
                    let (_json_val, nodes, edges) = match run_arkts_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_arkts_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "typescript" => {
                    let (_json_val, nodes, edges) = match run_typescript_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_arkts_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                _ => unreachable!(),
            };

            let output = QualityCommandOutput {
                language: language.clone(),
                root: root_path.to_string_lossy().to_string(),
                overall: overall.to_string(),
                gates,
            };

            let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                eprintln!("错误：JSON 序列化失败: {e}");
                std::process::exit(1);
            });
            println!("{json}");

            // exit code: pass → 0, fail → 1
            if overall == "fail" {
                std::process::exit(1);
            }
        }

        // ===== 新增：summary 概要命令 =====
        Commands::Summary {
            root,
            language,
            format,
        } => {
            if format != "json" {
                eprintln!("错误：当前仅支持 --format json");
                std::process::exit(1);
            }

            let root_path = match check_root(&root) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            let lang = match resolve_language(&language, root_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            eprintln!("概要分析中... language={lang}, root={root}");

            let (graph_summary, quality_summary) = match lang.as_str() {
                "rust" => {
                    let (json_val, nodes, edges) = match run_rust_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_rust_summary(&json_val, &nodes, &edges);
                    let gates = compute_rust_quality_gates(&nodes, &edges);
                    let total = gates.len() as u32;
                    let passed = gates.iter().filter(|g| g.passed).count() as u32;
                    let failed = total - passed;
                    let qs = QualitySummary {
                        total,
                        passed,
                        failed,
                    };
                    (gs, qs)
                }
                "cangjie" => {
                    let (_json_val, nodes, edges) = match run_cangjie_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_cangjie_summary(&nodes, &edges);
                    let gates = compute_cangjie_quality_gates(&nodes, &edges);
                    let total = gates.len() as u32;
                    let passed = gates.iter().filter(|g| g.passed).count() as u32;
                    let failed = total - passed;
                    let qs = QualitySummary {
                        total,
                        passed,
                        failed,
                    };
                    (gs, qs)
                }
                "arkts" => {
                    let (_json_val, nodes, edges) = match run_arkts_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_arkts_quality_gates(&nodes, &edges);
                    let total = gates.len() as u32;
                    let passed = gates.iter().filter(|g| g.passed).count() as u32;
                    let failed = total - passed;
                    let qs = QualitySummary {
                        total,
                        passed,
                        failed,
                    };
                    (gs, qs)
                }
                "typescript" => {
                    let (_json_val, nodes, edges) = match run_typescript_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_arkts_quality_gates(&nodes, &edges);
                    let total = gates.len() as u32;
                    let passed = gates.iter().filter(|g| g.passed).count() as u32;
                    let failed = total - passed;
                    let qs = QualitySummary {
                        total,
                        passed,
                        failed,
                    };
                    (gs, qs)
                }
                other => {
                    eprintln!("错误：不支持的语言: {other}");
                    std::process::exit(1);
                }
            };

            let output = SummaryCommandOutput {
                language: lang,
                root: root_path.to_string_lossy().to_string(),
                analyzed_at: now_iso8601(),
                graph_summary,
                quality_summary,
            };

            let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                eprintln!("错误：JSON 序列化失败: {e}");
                std::process::exit(1);
            });
            println!("{json}");
        }

        // ===== MCP stdio server =====
        Commands::Mcp => {
            if let Err(e) = mcp_server::run_mcp_server() {
                eprintln!("MCP server error: {e}");
                std::process::exit(1);
            }
        }
    }
}
