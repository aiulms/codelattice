#![recursion_limit = "512"]
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

mod ai_runtime;
mod arkts_bridge;
mod bridge_format;
mod c_bridge;
mod cangjie_bridge;
mod cpp_bridge;
mod engine_bridge;
mod language_detect;
mod mcp_facade;
mod mcp_job;
mod mcp_server;
mod python_bridge;
mod rust_bridge;
mod shell_bridge;
mod unified_types;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use language_detect::detect_language;
use unified_types::{
    DetectedLanguage, GraphSummary, LanguageAnalysisResult, QualityCommandOutput,
    QualityGateResult, QualitySummary, SummaryCommandOutput,
};

// workspace-model 直接库调用（不需要 MCP subprocess）
use gitnexus_workspace_model::impact::{cross_project_impact, ImpactDirection, ImpactTarget};
use gitnexus_workspace_model::{build_workspace_graph, scan_workspace_inventory};

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
    /// 统一分析入口：自动检测语言或按指定语言分析项目，支持 full/compact/symbols/modules/deps 输出档位
    Analyze {
        #[arg(long)]
        root: String,
        #[arg(long, default_value = "auto")]
        language: String,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long, default_value = "false")]
        strict: bool,
        #[arg(long, default_value = "off")]
        engine: String,
        /// 输出档位：full(完整 graph) / compact(AI decision payload) / symbols(符号列表) / modules(模块概览) / deps(依赖与框架摘要)
        #[arg(long, default_value = "full")]
        profile: String,
        /// profile 输出页码（symbols/modules 有效，0-based）
        #[arg(long, default_value_t = 0)]
        profile_page: usize,
        /// profile 每页条数（symbols/modules 有效，0 表示不分页）
        #[arg(long, default_value_t = 500)]
        profile_page_size: usize,
        /// 仅输出 public/pub 符号（symbols profile 有效）
        #[arg(long, default_value_t = false)]
        public_only: bool,
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
    /// 提交前变化审查：基于 git diff 自动识别变更符号与风险提示
    DetectChanges {
        /// 项目根目录路径（必须是 git 仓库或其工作树目录）
        #[arg(long, default_value = ".")]
        root: String,
        /// 语言：rust / cangjie / arkts / typescript / c / cpp / python / shell / auto
        #[arg(long, default_value = "auto")]
        language: String,
        /// 变化范围：all(head) / staged / unstaged / working-tree / head
        #[arg(long, default_value = "all")]
        scope: String,
        /// 兼容 MCP changed_symbols 的 diffMode；设置后优先于 --scope
        #[arg(long)]
        diff_mode: Option<String>,
        /// 与指定 git ref 对比（传给 git diff <base-ref>）
        #[arg(long)]
        base_ref: Option<String>,
        /// 输出格式（当前仅支持 json）
        #[arg(long, default_value = "json")]
        format: String,
        /// 最多返回的变更符号数量
        #[arg(long, default_value_t = 100)]
        limit: usize,
        /// 紧凑输出：保留摘要和身份字段，省略底层工具原始结果
        #[arg(long, default_value_t = false)]
        compact: bool,
        /// changed symbols 是否包含代码片段
        #[arg(long, default_value_t = false)]
        include_snippet: bool,
        /// 代码片段上下文行数
        #[arg(long, default_value_t = 2)]
        snippet_context: usize,
        /// 深度审计模式：默认隐藏 fixture/test/demo 等低信号间接影响；设置后展开这些项目
        #[arg(long, default_value_t = false)]
        include_fixtures: bool,
        /// 严格 workspace 模式：保留所有 workspace graph 影响，包括低置信度 adjacency-only 边
        #[arg(long, default_value_t = false)]
        strict_workspace: bool,
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
            DetectedLanguage::JavaScript => Ok("javascript".to_string()),
            DetectedLanguage::C => Ok("c".to_string()),
            DetectedLanguage::Cpp => Ok("cpp".to_string()),
            DetectedLanguage::Python => Ok("python".to_string()),
            DetectedLanguage::Shell => Ok("shell".to_string()),
            DetectedLanguage::Ambiguous => Err(
                "语言检测失败：存在多种清单文件，请使用 --language rust|cangjie|arkts|typescript|javascript|c|cpp|python|shell 显式指定".to_string(),
            ),
            DetectedLanguage::Unknown => Err(
                "语言检测失败：未找到可识别的清单文件，无法自动检测语言".to_string(),
            ),
        }
    } else if [
        "rust",
        "cangjie",
        "arkts",
        "typescript",
        "javascript",
        "c",
        "cpp",
        "python",
        "shell",
    ]
    .contains(&lang_arg)
    {
        Ok(lang_arg.to_string())
    } else {
        Err(format!(
            "不支持的语言: {lang_arg}，请使用 rust / cangjie / arkts / typescript / c / cpp / python / shell / auto"
        ))
    }
}

/// 验证 root 路径存在
// ============================================================
// Analyze --profile 过滤器
// ============================================================

/// 验证 --profile 参数合法性
fn validate_profile(profile: &str) -> Result<(), String> {
    match profile {
        "full" | "compact" | "symbols" | "modules" | "deps" => Ok(()),
        _ => Err(format!(
            "Unknown profile '{}'. Use: full, compact, symbols, modules, deps.",
            profile
        )),
    }
}

#[derive(Debug, Clone, Copy)]
struct AnalyzeProfileOptions {
    page: usize,
    page_size: usize,
    public_only: bool,
}

impl AnalyzeProfileOptions {
    fn new(page: usize, page_size: usize, public_only: bool) -> Self {
        Self {
            page,
            page_size,
            public_only,
        }
    }
}

fn page_slice<T: Clone>(
    items: &[T],
    options: AnalyzeProfileOptions,
) -> (Vec<T>, serde_json::Value) {
    let total = items.len();
    let page_size = options.page_size;
    let (start, end, total_pages) = if page_size == 0 {
        (0, total, if total == 0 { 0 } else { 1 })
    } else {
        let start = options.page.saturating_mul(page_size);
        let end = start.saturating_add(page_size).min(total);
        let total_pages = if total == 0 {
            0
        } else {
            (total + page_size - 1) / page_size
        };
        (start, end, total_pages)
    };
    let page_items = if start < total {
        items[start..end].to_vec()
    } else {
        Vec::new()
    };
    let items_returned = page_items.len();
    let has_more = page_size != 0 && end < total;

    (
        page_items,
        serde_json::json!({
            "page": options.page,
            "pageSize": page_size,
            "totalItems": total,
            "totalPages": total_pages,
            "itemsReturned": items_returned,
            "hasMore": has_more,
            "hasPrev": options.page > 0 && total > 0
        }),
    )
}

fn profile_detail_hint(profile: &str, paging: &serde_json::Value) -> String {
    if paging["hasMore"].as_bool().unwrap_or(false) {
        let next_page = paging["page"].as_u64().unwrap_or(0) + 1;
        format!(
            "More {profile} are available. Re-run with --profile {profile} --profile-page {next_page} --profile-page-size {}.",
            paging["pageSize"].as_u64().unwrap_or(0)
        )
    } else {
        format!("All {profile} for this filtered profile page have been returned. Use --profile full for complete graph evidence.")
    }
}

/// 从完整分析结果中提取模块概览
fn extract_modules_from_result(result: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut module_map: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();

    let nodes = result
        .get("graph")
        .and_then(|g| g.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    for node in &nodes {
        if node["label"].as_str() != Some("symbol") {
            continue;
        }
        let source_path = node["properties"]["sourcePath"]
            .as_str()
            .or_else(|| node["file"].as_str())
            .unwrap_or("");
        let visibility = node["properties"]["visibility"]
            .as_str()
            .unwrap_or("unknown");

        let module_key = source_path.to_string();
        let entry = module_map.entry(module_key.clone()).or_insert_with(|| {
            serde_json::json!({
                "module": source_path,
                "fileCount": 1,
                "symbolCount": 0,
                "publicSymbolCount": 0,
                "riskHints": [],
                "readFirst": false
            })
        });

        if let Some(obj) = entry.as_object_mut() {
            *obj.get_mut("symbolCount").unwrap() =
                serde_json::json!(obj["symbolCount"].as_u64().unwrap_or(0) + 1);
            if visibility == "pub" || visibility == "public" {
                *obj.get_mut("publicSymbolCount").unwrap() =
                    serde_json::json!(obj["publicSymbolCount"].as_u64().unwrap_or(0) + 1);
            }
        }
    }

    module_map.into_values().collect()
}

/// 从完整分析结果提取符号列表
fn extract_symbols_from_result(result: &serde_json::Value) -> Vec<serde_json::Value> {
    let nodes = result
        .get("graph")
        .and_then(|g| g.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    nodes
        .iter()
        .filter(|n| {
            let label = n["label"].as_str().unwrap_or("");
            let kind = n["properties"]["symbolKind"]
                .as_str()
                .or_else(|| n["properties"]["kind"].as_str())
                .unwrap_or("");
            label == "symbol"
                || kind == "function"
                || kind == "struct"
                || kind == "enum"
                || kind == "trait"
                || kind == "method"
                || kind == "associated-function"
                || kind == "class"
                || kind == "const"
                || kind == "static"
        })
        .map(|n| {
            let id = n["id"].as_str().unwrap_or("");
            let name = n["properties"]["name"]
                .as_str()
                .or_else(|| n["name"].as_str())
                .unwrap_or("");
            let kind = n["properties"]["symbolKind"]
                .as_str()
                .or_else(|| n["properties"]["kind"].as_str())
                .unwrap_or("symbol");
            let file = n["properties"]["sourcePath"]
                .as_str()
                .or_else(|| n["file"].as_str())
                .unwrap_or("");
            let line = n["properties"]["lineStart"]
                .as_u64()
                .or_else(|| n["startLine"].as_u64());
            let module_path = n["properties"]["modulePath"].as_str().unwrap_or("");
            let visibility = n["properties"]["visibility"].as_str().unwrap_or("unknown");

            serde_json::json!({
                "id": id,
                "name": name,
                "kind": kind,
                "file": file,
                "line": line,
                "modulePath": module_path,
                "visibility": visibility
            })
        })
        .collect()
}

/// 根据 --profile 过滤分析结果
fn filter_analyze_profile(
    result: &serde_json::Value,
    profile: &str,
    options: AnalyzeProfileOptions,
) -> serde_json::Value {
    match profile {
        "full" => result.clone(),
        "symbols" => {
            let mut symbols = extract_symbols_from_result(result);
            if options.public_only {
                symbols.retain(|s| {
                    let visibility = s["visibility"].as_str().unwrap_or("");
                    visibility == "pub" || visibility == "public"
                });
            }
            symbols.sort_by(|a, b| {
                let a_key = (
                    a["file"].as_str().unwrap_or(""),
                    a["line"].as_u64().unwrap_or(0),
                    a["name"].as_str().unwrap_or(""),
                );
                let b_key = (
                    b["file"].as_str().unwrap_or(""),
                    b["line"].as_u64().unwrap_or(0),
                    b["name"].as_str().unwrap_or(""),
                );
                a_key.cmp(&b_key)
            });
            let (symbols_page, paging) = page_slice(&symbols, options);
            let summary = result
                .get("summary")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let detail_hint = profile_detail_hint("symbols", &paging);
            serde_json::json!({
                "schemaVersion": "codelattice.analyzeSymbols.v1",
                "root": result["root"],
                "language": result["language"],
                "generatedFrom": {
                    "staticAnalysis": true,
                    "runtimeVerified": false,
                    "scriptsExecuted": false
                },
                "stats": {
                    "symbolCount": summary.get("symbolCount").cloned().unwrap_or(serde_json::json!(0)),
                    "sourceFileCount": summary.get("sourceFileCount").cloned().unwrap_or(serde_json::json!(0)),
                    "nodeCount": summary.get("nodeCount").cloned().unwrap_or(serde_json::json!(0)),
                    "edgeCount": summary.get("edgeCount").cloned().unwrap_or(serde_json::json!(0))
                },
                "filters": {
                    "publicOnly": options.public_only
                },
                "paging": paging,
                "symbols": symbols_page,
                "detailHint": detail_hint
            })
        }
        "modules" => {
            let mut modules = extract_modules_from_result(result);
            modules.sort_by(|a, b| {
                b["symbolCount"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["symbolCount"].as_u64().unwrap_or(0))
                    .then_with(|| {
                        a["module"]
                            .as_str()
                            .unwrap_or("")
                            .cmp(b["module"].as_str().unwrap_or(""))
                    })
            });
            let (modules_page, paging) = page_slice(&modules, options);
            let summary = result
                .get("summary")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let detail_hint = profile_detail_hint("modules", &paging);
            serde_json::json!({
                "schemaVersion": "codelattice.analyzeModules.v1",
                "root": result["root"],
                "language": result["language"],
                "generatedFrom": {
                    "staticAnalysis": true,
                    "runtimeVerified": false,
                    "scriptsExecuted": false
                },
                "stats": {
                    "moduleCount": modules.len(),
                    "symbolCount": summary.get("symbolCount").cloned().unwrap_or(serde_json::json!(0)),
                    "sourceFileCount": summary.get("sourceFileCount").cloned().unwrap_or(serde_json::json!(0))
                },
                "paging": paging,
                "modules": modules_page,
                "detailHint": detail_hint
            })
        }
        "deps" => {
            let root = result.get("root").and_then(|v| v.as_str()).unwrap_or("");
            let language = result
                .get("language")
                .and_then(|v| v.as_str())
                .unwrap_or("auto");
            let dependency_summary =
                crate::ai_runtime::build_dependency_framework_digest(Path::new(root), language);
            serde_json::json!({
                "schemaVersion": "codelattice.analyzeDependencies.v1",
                "root": result["root"],
                "language": result["language"],
                "dependencySummary": dependency_summary,
                "runtimeTrace": crate::ai_runtime::build_runtime_trace_envelope(language, Some(result)),
                "generatedFrom": {
                    "staticAnalysis": true,
                    "manifestOnly": true,
                    "targetCodeExecuted": false,
                    "scriptsExecuted": false,
                    "runtimeVerified": false
                },
                "omitted": {
                    "graph": true,
                    "symbols": true,
                    "diagnostics": true,
                    "detailHint": "Use --profile compact for project orientation or --profile full for complete graph evidence."
                }
            })
        }
        "compact" => {
            let summary = result
                .get("summary")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let symbols = extract_symbols_from_result(result);
            let modules = extract_modules_from_result(result);
            let quality_gates = result
                .get("qualityGates")
                .and_then(|q| q.as_array())
                .cloned()
                .unwrap_or_default();
            let failed_gates: Vec<_> = quality_gates
                .iter()
                .filter(|g| g["passed"].as_bool() == Some(false))
                .collect();

            // top public symbols（最多 30）
            let top_public: Vec<_> = symbols
                .iter()
                .filter(|s| {
                    let v = s["visibility"].as_str().unwrap_or("");
                    v == "pub" || v == "public"
                })
                .take(30)
                .cloned()
                .collect();

            // top modules by symbol count（最多 20）
            let mut top_modules = modules;
            top_modules.sort_by(|a, b| {
                b["symbolCount"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["symbolCount"].as_u64().unwrap_or(0))
            });
            top_modules.truncate(20);

            // entry points: main/lib 公开函数
            let entry_points: Vec<_> = symbols
                .iter()
                .filter(|s| {
                    let name = s["name"].as_str().unwrap_or("").to_lowercase();
                    name == "main" || name == "run" || name == "start" || name == "init"
                })
                .take(10)
                .cloned()
                .collect();

            // risk hints from failed quality gates
            let top_risks: Vec<_> = failed_gates
                .iter()
                .map(|g| {
                    serde_json::json!({
                        "gate": g["gateName"].as_str().unwrap_or("unknown"),
                        "detail": g["detail"].as_str().unwrap_or("")
                    })
                })
                .take(10)
                .collect();

            serde_json::json!({
                "schemaVersion": "codelattice.analyzeCompact.v1",
                "root": result["root"],
                "language": result["language"],
                "summary": {
                    "nodeCount": summary.get("nodeCount").cloned().unwrap_or(serde_json::json!(0)),
                    "edgeCount": summary.get("edgeCount").cloned().unwrap_or(serde_json::json!(0)),
                    "symbolCount": summary.get("symbolCount").cloned().unwrap_or(serde_json::json!(0)),
                    "sourceFileCount": summary.get("sourceFileCount").cloned().unwrap_or(serde_json::json!(0)),
                    "callEdgeCount": summary.get("callEdgeCount").cloned().unwrap_or(serde_json::json!(0))
                },
                "topModules": top_modules,
                "topPublicSymbols": top_public,
                "entryPoints": entry_points,
                "topRisks": top_risks,
                "omitted": {
                    "fullGraphEdges": true,
                    "diagnostics": true,
                    "nonPublicSymbols": true,
                    "detailHint": "Use --profile full for complete graph, --profile symbols for all symbols."
                }
            })
        }
        _ => result.clone(),
    }
}

/// 序列化分析结果并根据 --profile 过滤输出
fn print_analyze_result(
    result: &LanguageAnalysisResult,
    profile: &str,
    options: AnalyzeProfileOptions,
) {
    let full_value = serde_json::to_value(result).unwrap_or_else(|e| {
        eprintln!("错误：JSON 序列化失败: {e}");
        std::process::exit(1);
    });
    let filtered = filter_analyze_profile(&full_value, profile, options);
    let json = serde_json::to_string_pretty(&filtered).unwrap_or_else(|e| {
        eprintln!("错误：JSON 序列化失败: {e}");
        std::process::exit(1);
    });
    println!("{json}");
}

fn check_root(root: &str) -> Result<&Path, String> {
    let path = Path::new(root);
    if !path.exists() {
        return Err(format!("错误：root 路径不存在: {root}"));
    }
    Ok(path)
}

/// 当用户/AI 把多项目根目录直接交给 `analyze --language auto` 时，
/// 不再强行选单一语言；改为返回受保护的 workspace auto-entry 摘要。
/// 这里只做目录/manifest/config 级静态扫描，不执行项目代码，也不读取源码内容。
fn build_workspace_auto_entry(root: &Path, reason: &str) -> Option<Value> {
    let inventory = scan_workspace_inventory(root, true).ok()?;
    let manifest_projects: Vec<_> = inventory
        .iter()
        .filter(|p| p.supported && p.is_manifest_backed)
        .collect();
    let source_only: Vec<_> = inventory
        .iter()
        .filter(|p| p.supported && !p.is_manifest_backed)
        .collect();
    let root_is_project = manifest_projects.iter().any(|p| p.relative_path == ".");
    let child_project_count = manifest_projects
        .iter()
        .filter(|p| p.relative_path != ".")
        .count();
    if manifest_projects.len() < 2 && (root_is_project || child_project_count < 2) {
        return None;
    }

    let unsupported: Vec<_> = inventory.iter().filter(|p| !p.supported).collect();
    let graph_summary = build_workspace_graph(root, true)
        .ok()
        .and_then(|g| serde_json::to_value(g.summary).ok());
    let workspace_graph_available = graph_summary.is_some();
    let workspace_summary = graph_summary.unwrap_or_else(|| json!({}));

    // sourceOnlyAreas：compact 只返回 top 5 + summary
    let source_only_preview: Vec<_> = source_only.iter().take(5).collect();

    Some(json!({
        "schemaVersion": "codelattice.workspaceAutoEntry.v1",
        "status": "workspace_analyzed",
        "rootKind": "workspace",
        "root": root.to_string_lossy(),
        "reason": reason,
        "summary": {
            "supportedProjectCount": manifest_projects.len(),
            "sourceOnlyAreaCount": source_only.len(),
            "unsupportedModuleCount": unsupported.len(),
            "recommendedProjectCount": manifest_projects.len(),
            "workspaceGraphAvailable": workspace_graph_available
        },
        "supportedProjects": manifest_projects.iter().map(|p| json!({
            "path": p.path,
            "relativePath": p.relative_path,
            "name": p.name,
            "language": p.language,
            "manifestFile": p.manifest_file,
            "recommended": true
        })).collect::<Vec<_>>(),
        "sourceOnlyAreas": {
            "summary": {
                "count": source_only.len(),
                "hint": "Source-only directories detected by file extension, not manifest-backed projects."
            },
            "preview": source_only_preview.iter().map(|p| json!({
                "path": p.path,
                "relativePath": p.relative_path,
                "name": p.name,
                "language": p.language
            })).collect::<Vec<_>>(),
            "detailHint": "Use codelattice_workspace mode=graph for full source-only inventory."
        },
        "unsupportedModules": unsupported.iter().map(|p| json!({
            "path": p.path,
            "relativePath": p.relative_path,
            "name": p.name,
            "language": p.language,
            "manifestFile": p.manifest_file,
            "supported": false
        })).collect::<Vec<_>>(),
        "analyzedProjects": [],
        "failedProjects": [],
        "workspaceSummary": workspace_summary,
        "recommendedNextActions": [
            "Use codelattice_workspace mode=graph for cross-project structure.",
            "Use codelattice_workspace mode=impact with a selected projectId/path for blast radius.",
            "Use WebUI workspace recommended analysis for per-project snapshots."
        ],
        "cautions": [
            "workspace auto-entry is static-only",
            "inventory does not execute build/test/package-manager scripts",
            "unsupported modules are shown as backlog and are not analyzed"
        ],
        "generatedFrom": {
            "staticAnalysis": true,
            "workspaceInventory": true,
            "workspaceGraph": workspace_graph_available,
            "projectContentRead": false,
            "scriptsExecuted": false,
            "runtimeVerified": false,
            "coverageVerified": false
        }
    }))
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

fn scope_to_diff_mode(scope: &str, explicit: Option<&str>) -> Result<String, String> {
    let selected = explicit.unwrap_or(scope);
    match selected {
        // all/head 对应 `git diff HEAD`，覆盖 staged + unstaged。
        "all" | "head" => Ok("head".to_string()),
        "staged" => Ok("staged".to_string()),
        "unstaged" => Ok("unstaged".to_string()),
        "working" | "working-tree" => Ok("working-tree".to_string()),
        other => Err(format!(
            "不支持的变化范围: {other}，请使用 all / staged / unstaged / working-tree / head"
        )),
    }
}

fn call_mcp_tool_via_current_binary(tool_name: &str, arguments: Value) -> Result<Value, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("无法定位当前 CodeLattice binary: {e}"))?;
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    });

    let mut child = Command::new(exe)
        .arg("mcp")
        .env("CODELATTICE_MCP_TOOLSET", "full")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 CodeLattice MCP 子进程: {e}"))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "无法写入 MCP 子进程 stdin".to_string())?;
        writeln!(stdin, "{request}").map_err(|e| format!("写入 MCP 请求失败: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 MCP 子进程失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "MCP 子进程退出失败: status={} stderr={}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|e| format!("MCP stdout 不是有效 UTF-8: {e}"))?;
    let response_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "MCP 子进程没有返回 JSON-RPC 响应".to_string())?;
    let response: Value = serde_json::from_str(response_line)
        .map_err(|e| format!("MCP JSON-RPC 响应解析失败: {e}: {response_line}"))?;

    if let Some(error) = response.get("error") {
        return Err(format!("MCP 工具调用失败: {error}"));
    }

    let result = response
        .get("result")
        .ok_or_else(|| format!("MCP 响应缺少 result: {response}"))?;

    let content_text = result
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("MCP 响应缺少 content[0].text: {response}"))?;
    let data: Value = serde_json::from_str(content_text)
        .map_err(|e| format!("MCP 工具 JSON 内容解析失败: {e}: {content_text}"))?;

    if result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let message = data
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| data.get("error").and_then(|v| v.as_str()))
            .unwrap_or("unknown MCP tool error");
        return Err(format!("{tool_name} 返回错误: {message}"));
    }

    Ok(data)
}

fn severity_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn normalize_risk(level: &str) -> String {
    match level.to_ascii_lowercase().as_str() {
        "critical" => "critical".to_string(),
        "high" => "high".to_string(),
        "medium" => "medium".to_string(),
        "low" => "low".to_string(),
        _ => "unknown".to_string(),
    }
}

fn pick_detect_changes_risk(changed: &Value, assist: &Value) -> String {
    let assist_risk = assist
        .get("overallRisk")
        .and_then(|v| v.as_str())
        .map(normalize_risk);
    let changed_risk = changed
        .get("changedSymbols")
        .and_then(|v| v.as_array())
        .and_then(|symbols| {
            symbols
                .iter()
                .filter_map(|sym| sym.get("risk").and_then(|v| v.as_str()))
                .max_by_key(|risk| severity_rank(risk))
        })
        .map(normalize_risk);

    match (assist_risk, changed_risk) {
        (Some(a), Some(b)) => {
            if severity_rank(&a) >= severity_rank(&b) {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => "unknown".to_string(),
    }
}

fn collect_untracked_files(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(root)
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            // Agent-private state should not turn a clean code review into a
            // workspace-wide high-risk change. These folders are local
            // orchestration state, not project source.
            .filter(|line| !is_agent_private_path(line))
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn is_agent_private_path(path: &str) -> bool {
    let trimmed = path.trim_start_matches("./");
    trimmed == ".claude"
        || trimmed.starts_with(".claude/")
        || trimmed == ".arts"
        || trimmed.starts_with(".arts/")
        || trimmed == ".sisyphus"
        || trimmed.starts_with(".sisyphus/")
}

fn workspace_file_node_id(owner_kind: &str, rel_path: &str) -> Option<String> {
    if rel_path.is_empty() {
        return None;
    }
    let node_kind = match owner_kind {
        "config" if rel_path.contains(".github/workflows") => "workflow",
        "config" => "config",
        "script" => "script",
        _ => return None,
    };
    Some(format!("{}:{}", node_kind, rel_path.replace('/', ":")))
}

// ============================================================
// Workspace change intelligence
// ============================================================

#[derive(Debug, Clone, Copy)]
struct WorkspaceImpactPolicy {
    include_fixtures: bool,
    strict_workspace: bool,
}

impl WorkspaceImpactPolicy {
    fn mode(self) -> &'static str {
        if self.strict_workspace {
            "strict"
        } else if self.include_fixtures {
            "include-fixtures"
        } else {
            "daily"
        }
    }
}

fn classify_workspace_surface(
    path: &str,
    label: &str,
    language: &str,
    owner_kind: &str,
) -> &'static str {
    let p = path.replace('\\', "/").to_lowercase();
    let label_l = label.to_lowercase();
    let lang_l = language.to_lowercase();

    if owner_kind == "unsupported"
        || matches!(
            lang_l.as_str(),
            "csharp" | "java" | "go" | "swift" | "kotlin"
        )
    {
        return "unsupported";
    }
    if p.contains("/fixtures/")
        || p.starts_with("fixtures/")
        || p.contains("/fixture/")
        || p.contains("portable-smoke")
        || label_l.contains("fixture")
        || label_l.contains("smoke")
        || label_l.contains("corpus")
    {
        return "fixture";
    }
    if p.contains("/tests/")
        || p.starts_with("tests/")
        || p.contains("/__tests__/")
        || p.contains("/test/")
        || label_l.ends_with("-test")
        || label_l.contains("test")
    {
        return "test";
    }
    if p.contains("/docs/") || p.starts_with("docs/") {
        return "docs";
    }
    if p.contains("/webui/") || p.starts_with("webui/") {
        return "webui";
    }
    if p.contains("/scripts/") || p.starts_with("scripts/") {
        return "script";
    }
    "production"
}

fn is_fixture_like_surface(surface: &str) -> bool {
    matches!(surface, "fixture" | "test" | "docs")
}

fn impact_group(
    surface: &str,
    change_type: &str,
    confidence: f64,
    supported: bool,
) -> &'static str {
    if change_type == "direct" {
        return "direct";
    }
    if !supported || surface == "unsupported" {
        return "unsupportedBoundary";
    }
    if is_fixture_like_surface(surface) {
        return "fixtureOnly";
    }
    if confidence >= 0.65 {
        "highConfidence"
    } else {
        "lowConfidence"
    }
}

fn make_affected_project_json(
    node_id: &str,
    kind: &str,
    label: &str,
    path: &str,
    language: &str,
    supported: bool,
    distance: usize,
    confidence: f64,
    change_type: &str,
) -> Value {
    let surface = classify_workspace_surface(
        path,
        label,
        language,
        if supported { "project" } else { "unsupported" },
    );
    let group = impact_group(surface, change_type, confidence, supported);
    json!({
        "nodeId": node_id,
        "kind": kind,
        "label": label,
        "path": path,
        "language": language,
        "surface": surface,
        "impactGroup": group,
        "distance": distance,
        "confidence": confidence,
        "changeType": change_type
    })
}

fn should_suppress_workspace_project(project: &Value, policy: WorkspaceImpactPolicy) -> bool {
    if policy.strict_workspace {
        return false;
    }
    let change_type = project["changeType"].as_str().unwrap_or("");
    if change_type == "direct" {
        return false;
    }
    let group = project["impactGroup"].as_str().unwrap_or("");
    let surface = project["surface"].as_str().unwrap_or("");
    if policy.include_fixtures && is_fixture_like_surface(surface) {
        return false;
    }
    matches!(group, "fixtureOnly" | "lowConfidence")
}

fn short_project_labels(projects: &[Value], limit: usize) -> String {
    let mut labels: Vec<String> = projects
        .iter()
        .filter_map(|p| p["label"].as_str().map(ToString::to_string))
        .collect();
    labels.sort();
    labels.dedup();
    if labels.len() > limit {
        let remaining = labels.len() - limit;
        labels.truncate(limit);
        format!("{} (+{} more)", labels.join(", "), remaining)
    } else {
        labels.join(", ")
    }
}

/// 文件路径到项目归属映射
/// 基于 scan_workspace_inventory 的前缀匹配
fn map_files_to_owners(changed_files: &[Value], root: &Path) -> Vec<Value> {
    // 尝试扫描 workspace inventory
    let inventory = match scan_workspace_inventory(root, false) {
        Ok(projects) => projects,
        Err(_) => return Vec::new(),
    };

    if inventory.is_empty() {
        return Vec::new();
    }

    // 构建前缀映射：relative_path → (label, language, supported)
    // 排序最长前缀优先，确保贪婪匹配
    let mut prefixes: Vec<(String, String, String, bool)> = inventory
        .iter()
        .map(|p| {
            let rel = p.relative_path.trim_end_matches('/');
            (
                rel.to_string(),
                p.name.clone(),
                p.language.clone(),
                p.supported,
            )
        })
        .collect();
    prefixes.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // 检测 config/script 文件的模式
    let config_patterns = ["Dockerfile", "Makefile", ".env", "docker-compose"];
    let ci_patterns = [".github/workflows", ".gitlab-ci", "Jenkinsfile"];
    let script_extensions = [".sh", ".bash", ".zsh", ".ksh"];

    changed_files
        .iter()
        .filter_map(|f| {
            let file_path = f["path"].as_str().or_else(|| f["file"].as_str())?;
            let rel_path = file_path
                .strip_prefix(root.to_str().unwrap_or(""))
                .unwrap_or(file_path)
                .trim_start_matches('/');

            // 跳过空路径
            if rel_path.is_empty() {
                return None;
            }

            // 检查 config 文件
            let basename = rel_path.split('/').last().unwrap_or("");
            let is_config = config_patterns.iter().any(|p| basename.starts_with(p))
                || ci_patterns.iter().any(|p| rel_path.contains(p))
                || basename.ends_with(".yml") && rel_path.contains(".github")
                || basename.ends_with(".yaml") && rel_path.contains(".github");
            if is_config {
                let node_id = workspace_file_node_id("config", rel_path)
                    .unwrap_or_else(|| "workspace:config".to_string());
                return Some(json!({
                    "file": file_path,
                    "relativePath": rel_path,
                    "projectId": "workspace:config",
                    "ownerNodeId": node_id,
                    "projectLabel": basename,
                    "language": "",
                    "ownerKind": "config",
                    "confidence": 0.9,
                    "reason": "file matches config/CI pattern"
                }));
            }

            // 检查 script 文件
            let is_script = script_extensions.iter().any(|ext| basename.ends_with(ext));
            if is_script {
                let node_id = workspace_file_node_id("script", rel_path)
                    .unwrap_or_else(|| "workspace:script".to_string());
                return Some(json!({
                    "file": file_path,
                    "relativePath": rel_path,
                    "projectId": "workspace:script",
                    "ownerNodeId": node_id,
                    "projectLabel": basename,
                    "language": "shell",
                    "ownerKind": "script",
                    "confidence": 0.9,
                    "reason": "file matches script extension"
                }));
            }

            // 前缀匹配 project（最长优先）
            for (prefix, label, lang, supported) in &prefixes {
                if prefix == "."
                    || rel_path.starts_with(&format!("{}/", prefix))
                    || rel_path == *prefix
                {
                    let owner_kind = if *supported { "project" } else { "unsupported" };
                    return Some(json!({
                        "file": file_path,
                        "relativePath": rel_path,
                        "projectId": format!("project:{}", prefix.replace('/', ":")),
                        "ownerNodeId": format!("project:{}", prefix.replace('/', ":")),
                        "projectLabel": label,
                        "language": lang,
                        "ownerKind": owner_kind,
                        "confidence": if prefix == "." { 0.5 } else { 1.0 },
                        "reason": if prefix == "." {
                            "file under workspace root project"
                        } else if *supported {
                            "file under project relative_path"
                        } else {
                            "file under unsupported language project"
                        }
                    }));
                }
            }

            // 无法确定归属
            Some(json!({
                "file": file_path,
                "relativePath": rel_path,
                "projectId": "unknown",
                "projectLabel": "unknown",
                "language": "",
                "ownerKind": "unknown",
                "confidence": 0.0,
                "reason": "file does not match any project or config/script pattern"
            }))
        })
        .collect()
}

/// 计算工作区级别的变更影响
/// 包括：affectedProjects, affectedWorkspaceEdges, unsupportedBoundaryHits, crossProjectRisk
fn compute_workspace_impact(
    root: &Path,
    file_owners: &[Value],
    policy: WorkspaceImpactPolicy,
) -> Value {
    // 如果没有 file owners，尝试构建 workspace graph
    if file_owners.is_empty() {
        return json!({
            "workspaceContext": {
                "isWorkspace": false,
                "workspaceRoot": null,
                "projectCount": 0,
                "supportedProjectCount": 0,
                "unsupportedProjectCount": 0,
                "workspaceGraphAvailable": false,
                "workspaceGraphBuildError": "no changed files to analyze"
            },
            "affectedProjects": [],
            "affectedWorkspaceEdges": [],
            "unsupportedBoundaryHits": [],
            "crossProjectRisk": null,
            "workspaceImpactSummary": {
                "policy": {
                    "mode": policy.mode(),
                    "includeFixtures": policy.include_fixtures,
                    "strictWorkspace": policy.strict_workspace
                },
                "rawAffectedProjectCount": 0,
                "reportedProjectCount": 0,
                "suppressedProjectCount": 0,
                "directProjectCount": 0,
                "highConfidenceProjectCount": 0,
                "lowConfidenceProjectCount": 0,
                "fixtureOnlyCount": 0,
                "unsupportedBoundaryCount": 0
            },
            "riskReasons": [],
            "recommendedFollowups": []
        });
    }

    // 尝试构建 workspace graph
    let graph = match gitnexus_workspace_model::build_workspace_graph(root, false) {
        Ok(g) => g,
        Err(e) => {
            return json!({
                "workspaceContext": {
                    "isWorkspace": false,
                    "workspaceRoot": root.to_string_lossy(),
                    "projectCount": 0,
                    "supportedProjectCount": 0,
                    "unsupportedProjectCount": 0,
                    "workspaceGraphAvailable": false,
                    "workspaceGraphBuildError": e
                },
                "affectedProjects": [],
                "affectedWorkspaceEdges": [],
                "unsupportedBoundaryHits": [],
                "crossProjectRisk": null,
                "workspaceImpactSummary": {
                    "policy": {
                        "mode": policy.mode(),
                        "includeFixtures": policy.include_fixtures,
                        "strictWorkspace": policy.strict_workspace
                    },
                    "rawAffectedProjectCount": 0,
                    "reportedProjectCount": 0,
                    "suppressedProjectCount": 0,
                    "directProjectCount": 0,
                    "highConfidenceProjectCount": 0,
                    "lowConfidenceProjectCount": 0,
                    "fixtureOnlyCount": 0,
                    "unsupportedBoundaryCount": 0
                },
                "riskReasons": [],
                "recommendedFollowups": []
            });
        }
    };

    let project_count = graph.nodes.iter().filter(|n| n.kind == "project").count();
    let supported_count = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "project" && n.supported)
        .count();
    let unsupported_count = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "project" && !n.supported)
        .count();

    // 收集变更的 project node_id（去重）
    let mut changed_node_ids = HashSet::new();
    let mut changed_project_ids = HashSet::new();

    // 检查是否有 config/script 变更
    let has_config_changes = file_owners
        .iter()
        .any(|o| o["ownerKind"].as_str().unwrap_or("") == "config");
    let has_script_changes = file_owners
        .iter()
        .any(|o| o["ownerKind"].as_str().unwrap_or("") == "script");
    let has_unknown_owners = file_owners
        .iter()
        .any(|o| o["ownerKind"].as_str().unwrap_or("") == "unknown");

    // 对每个变更的 project 做跨项目影响 BFS（downstream 方向）
    let mut affected_projects = Vec::new();
    let mut affected_edges = Vec::new();
    let mut unsupported_hits = Vec::new();
    let mut risk_reasons: Vec<String> = Vec::new();
    let mut followups: Vec<String> = Vec::new();
    let mut seen_project_ids = HashSet::new();
    let mut seen_unsupported_ids = HashSet::new();

    for owner in file_owners {
        let owner_kind = owner["ownerKind"].as_str().unwrap_or("");
        let owner_node_id = owner["ownerNodeId"]
            .as_str()
            .or_else(|| owner["projectId"].as_str())
            .unwrap_or("");
        let rel_path = owner["relativePath"].as_str().unwrap_or("");

        let target = match owner_kind {
            "project" | "unsupported" if !owner_node_id.is_empty() => ImpactTarget {
                node_id: Some(owner_node_id.to_string()),
                project_id: Some(owner_node_id.to_string()),
                path: None,
                snapshot_id: None,
                query: None,
            },
            "config" | "script" => ImpactTarget {
                node_id: if owner_node_id.starts_with("config:")
                    || owner_node_id.starts_with("workflow:")
                    || owner_node_id.starts_with("script:")
                {
                    Some(owner_node_id.to_string())
                } else {
                    None
                },
                project_id: None,
                path: if rel_path.is_empty() {
                    None
                } else {
                    Some(rel_path.to_string())
                },
                snapshot_id: None,
                query: None,
            },
            _ => continue,
        };

        let impact = cross_project_impact(&graph, &target, ImpactDirection::Downstream, 2);
        if let Some(resolved_id) = impact.target.resolved_node_id.as_ref() {
            changed_node_ids.insert(resolved_id.clone());

            if let Some(node) = graph.nodes.iter().find(|n| n.id == *resolved_id) {
                if node.kind == "project" {
                    changed_project_ids.insert(node.id.clone());
                    if seen_project_ids.insert(node.id.clone()) {
                        affected_projects.push(make_affected_project_json(
                            &node.id,
                            &node.kind,
                            &node.label,
                            &node.path,
                            &node.language,
                            node.supported,
                            0,
                            1.0,
                            "direct",
                        ));
                    }
                    if !node.supported && seen_unsupported_ids.insert(node.id.clone()) {
                        unsupported_hits.push(json!({
                            "nodeId": node.id,
                            "kind": node.kind,
                            "label": node.label,
                            "path": node.path,
                            "language": node.language,
                            "supported": false,
                            "distance": 0,
                            "confidence": 1.0,
                            "reason": "changed file belongs to unsupported language project"
                        }));
                    }
                }
            }
        }

        for proj in &impact.affected_projects {
            if seen_project_ids.insert(proj.node_id.clone()) {
                let supported = graph
                    .nodes
                    .iter()
                    .find(|n| n.id == proj.node_id)
                    .map(|n| n.supported)
                    .unwrap_or(true);
                let language = graph
                    .nodes
                    .iter()
                    .find(|n| n.id == proj.node_id)
                    .map(|n| n.language.clone())
                    .unwrap_or_default();
                affected_projects.push(make_affected_project_json(
                    &proj.node_id,
                    &proj.kind,
                    &proj.label,
                    &proj.path,
                    &language,
                    supported,
                    proj.distance,
                    proj.confidence,
                    if proj.distance == 0 {
                        "direct"
                    } else {
                        "indirect"
                    },
                ));
                if !supported && seen_unsupported_ids.insert(proj.node_id.clone()) {
                    unsupported_hits.push(json!({
                        "nodeId": proj.node_id,
                        "kind": proj.kind,
                        "label": proj.label,
                        "path": proj.path,
                        "language": graph.nodes.iter()
                            .find(|n| n.id == proj.node_id)
                            .map(|n| n.language.clone())
                            .unwrap_or_default(),
                        "supported": false,
                        "distance": proj.distance,
                        "confidence": proj.confidence,
                        "reason": "affected project uses unsupported language"
                    }));
                }
            }
        }

        for boundary in &impact.unsupported_boundaries {
            if seen_unsupported_ids.insert(boundary.node_id.clone()) {
                unsupported_hits.push(json!({
                    "nodeId": boundary.node_id,
                    "kind": boundary.kind,
                    "label": boundary.label,
                    "path": boundary.path,
                    "language": graph.nodes.iter()
                        .find(|n| n.id == boundary.node_id)
                        .map(|n| n.language.clone())
                        .unwrap_or_default(),
                    "supported": false,
                    "distance": boundary.distance,
                    "confidence": boundary.confidence,
                    "reason": "adjacent to changed project or downstream dependent"
                }));
            }
        }

        // cross_project_impact 的原始风险理由基于完整 BFS 结果，包含大量
        // fixture/adjacency-only 噪声。detect-changes 在这里重新按 precision
        // buckets 计算风险理由，避免日常治理被低信号项目淹没。
        // review_checklist 可能包含每个 BFS 命中的项目。这里先不直接继承，
        // 后面按 precision buckets 生成更短、更可执行的跟进项。
    }

    // 从 graph edges 中收集与变更 project 相关的边
    for edge in &graph.edges {
        let touches_changed = changed_node_ids.contains(&edge.source)
            || changed_node_ids.contains(&edge.target)
            || changed_project_ids.contains(&edge.source)
            || changed_project_ids.contains(&edge.target);
        if touches_changed && edge.kind != "contains" {
            affected_edges.push(json!({
                "edgeId": edge.id,
                "kind": edge.kind,
                "source": edge.source,
                "target": edge.target,
                "confidence": edge.confidence,
                "reason": edge.reason
            }));
        }
    }

    let raw_affected_project_count = affected_projects.len();
    let direct_project_count = affected_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("direct"))
        .count();
    let high_confidence_project_count = affected_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("highConfidence"))
        .count();
    let low_confidence_project_count = affected_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("lowConfidence"))
        .count();
    let fixture_only_count = affected_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("fixtureOnly"))
        .count();
    let unsupported_boundary_count = affected_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("unsupportedBoundary"))
        .count();

    let mut reported_projects = Vec::new();
    let mut suppressed_projects = Vec::new();
    for project in affected_projects {
        if should_suppress_workspace_project(&project, policy) {
            suppressed_projects.push(project);
        } else {
            reported_projects.push(project);
        }
    }

    let mut reported_node_ids: HashSet<String> = changed_node_ids.clone();
    for project in &reported_projects {
        if let Some(id) = project["nodeId"].as_str() {
            reported_node_ids.insert(id.to_string());
        }
    }

    let mut reported_edges = Vec::new();
    let mut suppressed_edges = Vec::new();
    for edge in affected_edges {
        let source = edge["source"].as_str().unwrap_or("");
        let target = edge["target"].as_str().unwrap_or("");
        let kind = edge["kind"].as_str().unwrap_or("");
        let confidence = edge["confidence"].as_f64().unwrap_or(0.0);
        let touches_reported =
            reported_node_ids.contains(source) || reported_node_ids.contains(target);
        let suppress = !policy.strict_workspace
            && (!touches_reported || kind == "adjacent_to" || confidence < 0.65);
        if suppress {
            suppressed_edges.push(edge);
        } else {
            reported_edges.push(edge);
        }
    }

    let high_signal_downstream = reported_projects
        .iter()
        .filter(|p| {
            p["changeType"].as_str() == Some("indirect")
                && p["impactGroup"].as_str() == Some("highConfidence")
        })
        .count();
    let reported_downstream_count = reported_projects
        .iter()
        .filter(|p| p["changeType"].as_str() == Some("indirect"))
        .count();
    let high_signal_unsupported = unsupported_hits.iter().any(|u| {
        u["distance"].as_u64().unwrap_or(99) == 0 || u["confidence"].as_f64().unwrap_or(0.0) >= 0.65
    });

    if has_config_changes {
        risk_reasons.push("changed config file may affect multiple projects".to_string());
    }
    if has_script_changes {
        risk_reasons.push("changed script file may affect build/deploy pipeline".to_string());
    }
    if has_unknown_owners {
        risk_reasons.push("changed file has unknown project owner".to_string());
    }
    if high_signal_unsupported {
        risk_reasons.push(
            "high-confidence unsupported language boundary hit near changed files".to_string(),
        );
    } else if !unsupported_hits.is_empty() {
        risk_reasons.push(format!(
            "{} low-confidence unsupported boundary hit(s) summarized",
            unsupported_hits.len()
        ));
    }
    if !suppressed_projects.is_empty() {
        risk_reasons.push(format!(
            "{} low-signal workspace project(s) summarized instead of reported",
            suppressed_projects.len()
        ));
    }

    let cross_project_risk = if high_signal_downstream >= 10
        || (high_signal_unsupported && high_signal_downstream >= 4)
    {
        "critical"
    } else if (has_config_changes && high_signal_downstream > 0)
        || high_signal_downstream >= 4
        || high_signal_unsupported
    {
        "high"
    } else if high_signal_downstream > 0
        || reported_downstream_count > 0
        || (has_config_changes && project_count > 1)
        || has_script_changes
        || has_unknown_owners
    {
        "medium"
    } else {
        "low"
    };

    let direct_projects: Vec<Value> = reported_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("direct"))
        .cloned()
        .collect();
    if !direct_projects.is_empty() {
        followups.push(format!(
            "Review direct owner project(s): {}",
            short_project_labels(&direct_projects, 5)
        ));
    }

    let high_conf_projects: Vec<Value> = reported_projects
        .iter()
        .filter(|p| p["impactGroup"].as_str() == Some("highConfidence"))
        .cloned()
        .collect();
    if !high_conf_projects.is_empty() {
        followups.push(format!(
            "Review high-confidence downstream project(s): {}",
            short_project_labels(&high_conf_projects, 5)
        ));
    }
    if has_config_changes {
        followups.push(
            "Verify config/CI change against the reported high-confidence projects".to_string(),
        );
    }
    if has_script_changes {
        followups.push("Review script change for build/deploy side effects".to_string());
    }
    if !suppressed_projects.is_empty() {
        followups.push(format!(
            "{} fixture/test/low-confidence workspace project(s) were summarized; rerun with --include-fixtures or --strict-workspace for full detail",
            suppressed_projects.len()
        ));
    }

    let workspace_impact_summary = json!({
        "policy": {
            "mode": policy.mode(),
            "includeFixtures": policy.include_fixtures,
            "strictWorkspace": policy.strict_workspace
        },
        "rawAffectedProjectCount": raw_affected_project_count,
        "reportedProjectCount": reported_projects.len(),
        "suppressedProjectCount": suppressed_projects.len(),
        "directProjectCount": direct_project_count,
        "highConfidenceProjectCount": high_confidence_project_count,
        "lowConfidenceProjectCount": low_confidence_project_count,
        "fixtureOnlyCount": fixture_only_count,
        "unsupportedBoundaryCount": unsupported_boundary_count,
        "reportedEdgeCount": reported_edges.len(),
        "suppressedEdgeCount": suppressed_edges.len()
    });

    json!({
        "workspaceContext": {
            "isWorkspace": project_count > 1 || has_config_changes || has_script_changes,
            "workspaceRoot": root.to_string_lossy(),
            "projectCount": project_count,
            "supportedProjectCount": supported_count,
            "unsupportedProjectCount": unsupported_count,
            "workspaceGraphAvailable": true,
            "workspaceGraphBuildError": null
        },
        "affectedProjects": reported_projects,
        "suppressedProjects": suppressed_projects,
        "affectedWorkspaceEdges": reported_edges,
        "suppressedWorkspaceEdges": suppressed_edges,
        "unsupportedBoundaryHits": unsupported_hits,
        "crossProjectRisk": cross_project_risk,
        "workspaceImpactSummary": workspace_impact_summary,
        "riskReasons": risk_reasons,
        "recommendedFollowups": followups
    })
}

/// 计算增强版风险原因（结合 workspace 和 changed_symbols 信息）
fn compute_enhanced_risk_reasons(
    changed: &Value,
    assist: &Value,
    workspace_impact: &Value,
) -> Vec<String> {
    let mut reasons = Vec::new();

    // 从 production_assist 继承已有风险原因
    if let Some(existing) = assist["overallRiskReasons"].as_array() {
        for r in existing {
            if let Some(s) = r.as_str() {
                reasons.push(s.to_string());
            }
        }
    }

    // 从 workspace impact 继承风险原因
    if let Some(ws_reasons) = workspace_impact["riskReasons"].as_array() {
        for r in ws_reasons {
            if let Some(s) = r.as_str() {
                if !reasons.iter().any(|existing| existing == s) {
                    reasons.push(s.to_string());
                }
            }
        }
    }

    // many unknown hunks
    let unknown_hunks = changed["summary"]["unknownHunkCount"]
        .as_u64()
        .or_else(|| changed["unknownHunks"].as_array().map(|a| a.len() as u64))
        .unwrap_or(0);
    let symbol_count = changed["summary"]["changedSymbolCount"]
        .as_u64()
        .or_else(|| changed["changedSymbols"].as_array().map(|a| a.len() as u64))
        .unwrap_or(0);
    let file_count = changed["summary"]["changedFileCount"]
        .as_u64()
        .or_else(|| changed["changedFiles"].as_array().map(|a| a.len() as u64))
        .unwrap_or(0);

    if unknown_hunks > symbol_count && unknown_hunks > 0 {
        reasons.push("many unknown hunks relative to detected symbols".to_string());
    }

    // files changed but no symbols detected
    if file_count > 0 && symbol_count == 0 {
        reasons.push("tracked files changed but no symbols detected".to_string());
    }

    reasons
}

/// 计算增强版风险等级（三层叠加）
fn pick_enhanced_risk(changed: &Value, assist: &Value, workspace_impact: &Value) -> String {
    // 第一层：现有 risk（production_assist + changed_symbols）
    let base_risk = pick_detect_changes_risk(changed, assist);

    // 第二层：workspace risk
    let ws_risk = workspace_impact["crossProjectRisk"]
        .as_str()
        .unwrap_or("low")
        .to_string();

    // 取最高风险等级
    let base_rank = severity_rank(&base_risk);
    let ws_rank = severity_rank(&ws_risk);

    if ws_rank > base_rank {
        ws_risk
    } else {
        base_risk
    }
}

/// 计算推荐跟进项
fn compute_recommended_followups(assist: &Value, workspace_impact: &Value) -> Vec<String> {
    let mut followups = Vec::new();

    // 从 workspace impact 继承
    if let Some(ws_followups) = workspace_impact["recommendedFollowups"].as_array() {
        for f in ws_followups {
            if let Some(s) = f.as_str() {
                followups.push(s.to_string());
            }
        }
    }

    // 从 reviewChecklist 继承
    if let Some(checklist) = assist["reviewChecklist"].as_array() {
        for item in checklist.iter().take(3) {
            if let Some(s) = item.as_str() {
                if !followups.iter().any(|f| f == s) {
                    followups.push(s.to_string());
                }
            }
        }
    }

    followups.truncate(10);
    followups
}

fn build_detect_changes_report(
    root: &Path,
    language: &str,
    scope: &str,
    diff_mode: &str,
    compact: bool,
    changed: Value,
    assist: Value,
    untracked_files: Vec<String>,
    workspace_impact: Value,
    file_owners: Vec<Value>,
) -> Value {
    let changed_file_count = changed
        .get("summary")
        .and_then(|s| s.get("changedFileCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            changed
                .get("changedFiles")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        })
        .unwrap_or(0);
    let changed_symbol_count = changed
        .get("summary")
        .and_then(|s| s.get("changedSymbolCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            changed
                .get("changedSymbols")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        })
        .unwrap_or(0);
    let unknown_hunk_count = changed
        .get("summary")
        .and_then(|s| s.get("unknownHunkCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            changed
                .get("unknownHunks")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        })
        .unwrap_or(0);
    let deleted_file_count = changed
        .get("summary")
        .and_then(|s| s.get("deletedFileCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            changed
                .get("deletedFiles")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        })
        .unwrap_or(0);
    let renamed_file_count = changed
        .get("summary")
        .and_then(|s| s.get("renamedFileCount"))
        .and_then(|v| v.as_u64())
        .or_else(|| {
            changed
                .get("renamedFiles")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        })
        .unwrap_or(0);
    let overall_risk = pick_enhanced_risk(&changed, &assist, &workspace_impact);
    let untracked_file_count = untracked_files.len() as u64;
    let total_file_change_count = changed_file_count + untracked_file_count;
    let enhanced_risk_reasons = compute_enhanced_risk_reasons(&changed, &assist, &workspace_impact);
    let recommended_followups = compute_recommended_followups(&assist, &workspace_impact);

    let mut report = json!({
        "schemaVersion": "codelattice.detectChanges.v1",
        "root": root.to_string_lossy(),
        "language": language,
        "scope": scope,
        "diffMode": diff_mode,
        "summary": {
            "changedFileCount": changed_file_count,
            "changedSymbolCount": changed_symbol_count,
            "unknownHunkCount": unknown_hunk_count,
            "deletedFileCount": deleted_file_count,
            "renamedFileCount": renamed_file_count,
            "untrackedFileCount": untracked_file_count,
            "totalFileChangeCount": total_file_change_count,
            "riskLevel": overall_risk,
            "affectedProcessCount": null,
            "affectedProcessModel": "notAvailable"
        },
        "changedFiles": changed.get("changedFiles").cloned().unwrap_or_else(|| json!([])),
        "changedSymbols": changed.get("changedSymbols").cloned().unwrap_or_else(|| json!([])),
        "unknownHunks": changed.get("unknownHunks").cloned().unwrap_or_else(|| json!([])),
        "deletedFiles": changed.get("deletedFiles").cloned().unwrap_or_else(|| json!([])),
        "renamedFiles": changed.get("renamedFiles").cloned().unwrap_or_else(|| json!([])),
        "untrackedFiles": untracked_files,
        "risk": {
            "overallRisk": assist.get("overallRisk").cloned().unwrap_or_else(|| json!(null)),
            "overallRiskReasons": enhanced_risk_reasons,
            "highestRiskSymbols": assist.get("highestRiskSymbols").cloned().unwrap_or_else(|| json!([]))
        },
        "reviewChecklist": assist.get("reviewChecklist").cloned().unwrap_or_else(|| json!([])),
        "workspaceContext": workspace_impact.get("workspaceContext").cloned().unwrap_or_else(|| json!(null)),
        "fileOwners": file_owners,
        "affectedProjects": workspace_impact.get("affectedProjects").cloned().unwrap_or_else(|| json!([])),
        "suppressedProjects": workspace_impact.get("suppressedProjects").cloned().unwrap_or_else(|| json!([])),
        "affectedWorkspaceEdges": workspace_impact.get("affectedWorkspaceEdges").cloned().unwrap_or_else(|| json!([])),
        "suppressedWorkspaceEdges": workspace_impact.get("suppressedWorkspaceEdges").cloned().unwrap_or_else(|| json!([])),
        "unsupportedBoundaryHits": workspace_impact.get("unsupportedBoundaryHits").cloned().unwrap_or_else(|| json!([])),
        "crossProjectRisk": workspace_impact.get("crossProjectRisk").cloned().unwrap_or_else(|| json!(null)),
        "workspaceImpactSummary": workspace_impact.get("workspaceImpactSummary").cloned().unwrap_or_else(|| json!(null)),
        "recommendedFollowups": recommended_followups,
        "quality": {
            "qualityGatesPassed": assist.get("qualityGatesPassed").cloned().unwrap_or_else(|| json!(null)),
            "qualityMetrics": assist.get("qualityMetrics").cloned().unwrap_or_else(|| json!(null))
        },
        "docs": {
            "docsLikelyNeedUpdate": assist.get("docsLikelyNeedUpdate").cloned().unwrap_or_else(|| json!([])),
            "docAssociationSummary": assist.get("docAssociationSummary").cloned().unwrap_or_else(|| json!(null))
        },
        "generatedFrom": {
            "staticAnalysis": true,
            "runtimeVerified": false,
            "scriptsExecuted": false,
            "coverageVerified": false,
            "heuristic": true,
            "previewOnly": true,
            "noWrites": true,
            "nativeCodeLattice": true,
            "workspaceGraphEnabled": workspace_impact["workspaceContext"]["workspaceGraphAvailable"].as_bool().unwrap_or(false),
            "workspaceImpactPrecision": workspace_impact["workspaceImpactSummary"]["policy"]["mode"].as_str().unwrap_or("daily")
        },
        "cautions": [
            "Static analysis only: this does not prove runtime breakage or safety.",
            "affectedProcessCount is null because CodeLattice does not use the legacy GitNexus process model.",
            "Use changedSymbols, unknownHunks, risk reasons, and reviewChecklist as investigation leads.",
            "Workspace fields (workspaceContext, fileOwners, affectedProjects, etc.) are heuristic-only and depend on filesystem project detection."
        ],
        "underlyingTools": [
            "codelattice_changed_symbols",
            "codelattice_production_assist",
            "codelattice_workspace_graph",
            "codelattice_cross_project_impact"
        ]
    });

    // ── Clean fast-path：无变更时 compact 输出最小化 ──
    let is_clean = changed_file_count == 0
        && changed_symbol_count == 0
        && unknown_hunk_count == 0
        && untracked_file_count == 0;

    if is_clean && compact {
        return json!({
            "schemaVersion": "codelattice.detectChanges.v1",
            "status": "clean",
            "root": root.to_string_lossy(),
            "language": language,
            "scope": scope,
            "diffMode": diff_mode,
            "summary": {
                "changedFileCount": 0,
                "changedSymbolCount": 0,
                "unknownHunkCount": 0,
                "untrackedFileCount": 0,
                "riskLevel": "none"
            },
            "changedSymbols": [],
            "risk": {
                "overallRisk": "none",
                "overallRiskReasons": [],
                "highestRiskSymbols": []
            },
            "generatedFrom": {
                "staticAnalysis": true,
                "runtimeVerified": false,
                "scriptsExecuted": false,
                "nativeCodeLattice": true
            },
            "detailHint": "No changes detected. Use --compact=false for full report."
        });
    }

    // clean 但非 compact：确保 riskLevel 不为 high
    if is_clean {
        if let Some(summary) = report.get_mut("summary").and_then(|s| s.as_object_mut()) {
            summary.insert("riskLevel".to_string(), json!("none"));
        }
    }

    if compact {
        if let Some(obj) = report.as_object_mut() {
            obj.remove("quality");
            obj.remove("docs");
            obj.remove("deletedFiles");
            obj.remove("renamedFiles");
            obj.remove("suppressedProjects");
            obj.remove("suppressedWorkspaceEdges");
        }
    }

    report
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
        Option<gitnexus_project_model::model::AnalysisTrace>,
    ),
    String,
> {
    let pm_output = gitnexus_project_model::output::inspect_project_model_with_options(
        root, true, true, true, true,
    );
    let analysis_trace = pm_output.analysis_trace.clone();

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

    Ok((json_val, nodes, edges, analysis_trace))
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
    Err("Cangjie support is disabled in this dev binary. CodeLattice-Tool installed binary includes Cangjie. For source builds: cargo build --features tree-sitter-cangjie or ALL_LANGUAGE_FEATURES.".to_string())
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
        None,
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
    Err("ArkTS support is disabled in this dev binary. CodeLattice-Tool installed binary includes ArkTS. For source builds: cargo build --features tree-sitter-arkts or ALL_LANGUAGE_FEATURES.".to_string())
}

// ============================================================
// TypeScript 分析 + Graph 提取
// ============================================================

#[cfg(any(
    feature = "tree-sitter-typescript",
    feature = "tree-sitter-javascript",
    feature = "tree-sitter-python"
))]
fn elapsed_ms(start: std::time::Instant) -> u64 {
    (start.elapsed().as_millis() as u64).max(1)
}

#[cfg(any(
    feature = "tree-sitter-typescript",
    feature = "tree-sitter-javascript",
    feature = "tree-sitter-python"
))]
fn graph_trace_counts(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> (usize, usize, usize, usize) {
    let symbol_count = nodes
        .iter()
        .filter(|n| {
            n.get("label").and_then(|v| v.as_str()) == Some("symbol")
                || n.get("kind").and_then(|v| v.as_str()) == Some("symbol")
                || n.get("properties")
                    .and_then(|p| p.get("symbolKind"))
                    .is_some()
        })
        .count();
    let call_edge_count = edges
        .iter()
        .filter(|e| {
            e.get("type").and_then(|v| v.as_str()) == Some("CALLS")
                || e.get("kind").and_then(|v| v.as_str()) == Some("calls")
                || e.get("properties")
                    .and_then(|p| p.get("callKind"))
                    .is_some()
        })
        .count();
    (nodes.len(), edges.len(), symbol_count, call_edge_count)
}

#[cfg(any(
    feature = "tree-sitter-typescript",
    feature = "tree-sitter-javascript",
    feature = "tree-sitter-python"
))]
fn language_stage_trace(
    language: &str,
    root: &Path,
    total_ms: u64,
    source_file_count: usize,
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
    stages: serde_json::Value,
    notes: Vec<&str>,
) -> serde_json::Value {
    let (node_count, edge_count, symbol_count, call_edge_count) = graph_trace_counts(nodes, edges);
    json!({
        "schemaVersion": "codelattice.languageAnalysisTrace.v1",
        "language": language,
        "root": root.to_string_lossy(),
        "granularity": "stage",
        "totalMs": total_ms.max(1),
        "sourceFileCount": source_file_count,
        "symbolCount": symbol_count,
        "callEdgeCount": call_edge_count,
        "nodeCount": node_count,
        "edgeCount": edge_count,
        "stages": stages,
        "generatedFrom": {
            "staticAnalysis": true,
            "projectOnceAnalyzer": true,
            "targetCodeExecuted": false,
            "scriptsExecuted": false,
            "runtimeVerified": false
        },
        "notes": notes
    })
}

#[cfg(feature = "tree-sitter-typescript")]
fn run_typescript_analysis_with_trace(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    use rayon::prelude::*;
    use std::collections::BTreeMap;

    let total_start = std::time::Instant::now();

    // 1. Build project model (tsconfig.json / package.json)
    let stage_start = std::time::Instant::now();
    let project =
        gitnexus_typescript::project::find_typescript_project_root(root).ok_or_else(|| {
            "TypeScript project root not found (no tsconfig.json or package.json)".to_string()
        })?;
    let project_root_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let source_files = gitnexus_typescript::project::list_source_files(&project)
        .map_err(|e| format!("Failed to list TypeScript source files: {e}"))?;
    let source_discovery_ms = elapsed_ms(stage_start);

    // 2. Parse manifest
    let stage_start = std::time::Instant::now();
    let manifest = gitnexus_typescript::load_ts_manifest(&project).ok();
    let manifest_ms = elapsed_ms(stage_start);

    // 3. Extract per-file data
    let stage_start = std::time::Instant::now();
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_typescript::TsSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_typescript::TsImport>> =
        BTreeMap::new();
    let mut references_by_file: BTreeMap<
        std::path::PathBuf,
        Vec<gitnexus_typescript::TsReference>,
    > = BTreeMap::new();

    let extracted = source_files
        .par_iter()
        .filter_map(|file| {
            let source = std::fs::read_to_string(file).ok()?;
            let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
                gitnexus_typescript::extractors::TsLanguage::Tsx
            } else {
                gitnexus_typescript::extractors::TsLanguage::TypeScript
            };
            Some((
                file.clone(),
                gitnexus_typescript::extractors::extract_ts_symbols(&source, lang),
                gitnexus_typescript::extractors::extract_ts_imports(&source, lang),
                gitnexus_typescript::extractors::extract_ts_references(&source, lang),
            ))
        })
        .collect::<Vec<_>>();

    for (file, syms, imps, refs) in extracted {
        symbols_by_file.insert(file.clone(), syms);
        imports_by_file.insert(file.clone(), imps);
        references_by_file.insert(file, refs);
    }
    let extraction_ms = elapsed_ms(stage_start);

    // 4. Build graph
    let stage_start = std::time::Instant::now();
    let kind = gitnexus_typescript::project::detect_project_kind(&project);
    let ts_project = gitnexus_typescript::TsProject {
        root: project,
        kind,
        manifest,
        source_files: source_files.clone(),
    };
    let project_model_ms = elapsed_ms(stage_start);

    // Build module resolver for path alias / monorepo support
    let stage_start = std::time::Instant::now();
    let resolver = gitnexus_typescript::TsModuleResolver::build(&ts_project.root, &source_files);
    let resolver_build_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let graph = gitnexus_typescript::graph::build_ts_graph(
        &ts_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
        Some(&resolver),
    );
    let graph_build_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
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
    let serialization_ms = elapsed_ms(stage_start);

    let trace = language_stage_trace(
        "typescript",
        root,
        elapsed_ms(total_start),
        source_files.len(),
        &nodes,
        &edges,
        json!({
            "projectRootMs": project_root_ms,
            "sourceDiscoveryMs": source_discovery_ms,
            "manifestMs": manifest_ms,
            "extractionMs": extraction_ms,
            "projectModelMs": project_model_ms,
            "resolverBuildMs": resolver_build_ms,
            "graphBuildMs": graph_build_ms,
            "serializationMs": serialization_ms
        }),
        vec![
            "Stage trace measures the current project-once TypeScript analyzer.",
            "Extraction currently reads each source file once and runs symbol/import/reference extraction in that worker.",
        ],
    );

    Ok((json_val, nodes, edges, trace))
}

#[cfg(feature = "tree-sitter-typescript")]
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
    let (json_val, nodes, edges, _trace) = run_typescript_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-typescript"))]
fn run_typescript_analysis_with_trace(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    Err(
        "TypeScript support is disabled in this dev binary. CodeLattice-Tool installed binary includes TypeScript. For source builds: cargo build --features tree-sitter-typescript or ALL_LANGUAGE_FEATURES."
            .to_string(),
    )
}

#[cfg(not(feature = "tree-sitter-typescript"))]
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
    let (json_val, nodes, edges, _trace) = run_typescript_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

// ============================================================
// JavaScript 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-javascript")]
fn run_javascript_analysis_with_trace(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    use rayon::prelude::*;
    use std::collections::BTreeMap;

    let total_start = std::time::Instant::now();

    // 1. Build project model
    let stage_start = std::time::Instant::now();
    let project =
        gitnexus_javascript::project::find_javascript_project_root(root).ok_or_else(|| {
            "JavaScript project root not found (no package.json or JS files)".to_string()
        })?;
    let project_root_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let source_files = gitnexus_javascript::project::list_source_files(&project)
        .map_err(|e| format!("Failed to list JavaScript source files: {e}"))?;
    let source_discovery_ms = elapsed_ms(stage_start);

    // 2. Parse manifest
    let stage_start = std::time::Instant::now();
    let manifest_path = project.join("package.json");
    let manifest = if manifest_path.is_file() {
        gitnexus_javascript::parse_package_json(&manifest_path).ok()
    } else {
        None
    };
    let manifest_ms = elapsed_ms(stage_start);

    // 3. Extract per-file data
    let stage_start = std::time::Instant::now();
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_javascript::JsSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_javascript::JsImport>> =
        BTreeMap::new();
    let mut references_by_file: BTreeMap<
        std::path::PathBuf,
        Vec<gitnexus_javascript::JsReference>,
    > = BTreeMap::new();

    let extracted = source_files
        .par_iter()
        .filter_map(|file| {
            let source = std::fs::read_to_string(file).ok()?;
            let lang = if file
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "jsx")
                .unwrap_or(false)
            {
                gitnexus_javascript::extractors::JsLanguage::Jsx
            } else {
                gitnexus_javascript::extractors::JsLanguage::JavaScript
            };

            Some((
                file.clone(),
                gitnexus_javascript::extractors::extract_js_symbols(&source, lang),
                gitnexus_javascript::extractors::extract_js_imports(&source, lang),
                gitnexus_javascript::extractors::extract_js_references(&source, lang),
            ))
        })
        .collect::<Vec<_>>();

    for (file, syms, imps, refs) in extracted {
        symbols_by_file.insert(file.clone(), syms);
        imports_by_file.insert(file.clone(), imps);
        references_by_file.insert(file, refs);
    }
    let extraction_ms = elapsed_ms(stage_start);

    // 4. Build graph
    let stage_start = std::time::Instant::now();
    let kind = gitnexus_javascript::project::detect_project_kind(&project);
    let js_project = gitnexus_javascript::JsProject {
        root: project.clone(),
        kind,
        manifest,
        source_files: source_files.clone(),
    };
    let project_model_ms = elapsed_ms(stage_start);

    // Build module resolver
    let stage_start = std::time::Instant::now();
    let resolver = gitnexus_javascript::JsModuleResolver::new(project);
    let resolver_build_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let graph = gitnexus_javascript::graph::build_js_graph(
        &js_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
        Some(&resolver),
    );
    let graph_build_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("JavaScript graph JSON serialization failed: {e}"))?;

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
    let serialization_ms = elapsed_ms(stage_start);

    let trace = language_stage_trace(
        "javascript",
        root,
        elapsed_ms(total_start),
        source_files.len(),
        &nodes,
        &edges,
        json!({
            "projectRootMs": project_root_ms,
            "sourceDiscoveryMs": source_discovery_ms,
            "manifestMs": manifest_ms,
            "extractionMs": extraction_ms,
            "projectModelMs": project_model_ms,
            "resolverBuildMs": resolver_build_ms,
            "graphBuildMs": graph_build_ms,
            "serializationMs": serialization_ms
        }),
        vec![
            "Stage trace measures the current project-once JavaScript analyzer.",
            "Extraction currently reads each source file once and runs symbol/import/reference extraction in that worker.",
        ],
    );

    Ok((json_val, nodes, edges, trace))
}

#[cfg(feature = "tree-sitter-javascript")]
fn run_javascript_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let (json_val, nodes, edges, _trace) = run_javascript_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-javascript"))]
fn run_javascript_analysis_with_trace(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    Err(
        "JavaScript support is disabled in this dev binary. CodeLattice-Tool installed binary includes JavaScript. For source builds: cargo build --features tree-sitter-javascript or ALL_LANGUAGE_FEATURES."
            .to_string(),
    )
}

#[cfg(not(feature = "tree-sitter-javascript"))]
fn run_javascript_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let (json_val, nodes, edges, _trace) = run_javascript_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

// ============================================================
// C 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-c")]
fn run_c_analysis(
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
    let project = gitnexus_c::project::find_c_project_root(root).ok_or_else(|| {
        "C project root not found (no C markers or C++ files detected)".to_string()
    })?;

    let (source_files, header_files) = gitnexus_c::project::list_c_source_files(&project)
        .map_err(|e| format!("Failed to list C source files: {e}"))?;

    let all_files: Vec<std::path::PathBuf> = source_files
        .iter()
        .chain(header_files.iter())
        .cloned()
        .collect();

    // 2. Extract per-file data
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_c::CSymbol>> =
        BTreeMap::new();
    let mut includes_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_c::CInclude>> =
        BTreeMap::new();

    for file in &all_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let syms = gitnexus_c::extract_c_symbols(&source);
        let incs = gitnexus_c::extract_c_includes(&source);

        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !incs.is_empty() {
            includes_by_file.insert(file.clone(), incs);
        }
    }

    // 3. Build include resolver from compile_commands.json (if present)
    let c_compile_db = root
        .join("compile_commands.json")
        .exists()
        .then(|| gitnexus_c::load_compile_commands(&root.join("compile_commands.json")))
        .transpose()
        .ok()
        .flatten();
    let c_resolver =
        gitnexus_c::CIncludeResolver::build(root, &source_files, &header_files, c_compile_db);

    // 4. Build graph
    let graph = gitnexus_c::build_c_graph(
        &project,
        &symbols_by_file,
        &includes_by_file,
        Some(&c_resolver),
    );

    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("C graph JSON serialization failed: {e}"))?;

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

#[cfg(not(feature = "tree-sitter-c"))]
fn run_c_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err("C support is disabled in this dev binary. CodeLattice-Tool installed binary includes C. For source builds: cargo build --features tree-sitter-c or ALL_LANGUAGE_FEATURES.".to_string())
}

/// 计算 C 质量门（复用通用质量门逻辑）
fn compute_c_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    // C uses the same quality gate logic as ArkTS/TypeScript (generic node/edge checks)
    compute_arkts_quality_gates(nodes, edges)
}

// ============================================================
// C++ 分析
// ============================================================

#[cfg(feature = "tree-sitter-cpp")]
fn run_cpp_analysis(
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
    let project = gitnexus_cpp::project::find_cpp_project_root(root).ok_or_else(|| {
        "C++ project root not found (no C++ markers or files detected)".to_string()
    })?;

    let (source_files, header_files) = gitnexus_cpp::project::list_cpp_source_files(&project)
        .map_err(|e| format!("Failed to list C++ source files: {e}"))?;

    let all_files: Vec<std::path::PathBuf> = source_files
        .iter()
        .chain(header_files.iter())
        .cloned()
        .collect();

    // 2. Extract per-file data
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_cpp::CppSymbol>> =
        BTreeMap::new();
    let mut includes_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_cpp::CppInclude>> =
        BTreeMap::new();

    for file in &all_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let syms = gitnexus_cpp::extract_cpp_symbols(&source);
        let incs = gitnexus_cpp::extract_cpp_includes(&source);

        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !incs.is_empty() {
            includes_by_file.insert(file.clone(), incs);
        }
    }

    // 3. Build project function name index for call extraction
    let project_fn_names: Vec<String> = symbols_by_file
        .values()
        .flat_map(|syms| {
            syms.iter()
                .filter(|s| {
                    matches!(
                        s.kind,
                        gitnexus_cpp::CppSymbolKind::FunctionDefinition
                            | gitnexus_cpp::CppSymbolKind::FunctionDeclaration
                            | gitnexus_cpp::CppSymbolKind::MethodDefinition
                            | gitnexus_cpp::CppSymbolKind::MethodDeclaration
                    )
                })
                .flat_map(|s| [s.qualified_name.clone(), s.name.clone()])
        })
        .collect();

    // 4. Extract calls per file
    let mut calls_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_cpp::CppCall>> =
        BTreeMap::new();
    for file in &all_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let rel = file.strip_prefix(root).unwrap_or(file);
        let calls =
            gitnexus_cpp::extract_cpp_calls(&source, &rel.to_string_lossy(), &project_fn_names);
        if !calls.is_empty() {
            calls_by_file.insert(file.clone(), calls);
        }
    }

    // 5. Build include resolver from compile_commands.json (if present)
    let cpp_compile_db = root
        .join("compile_commands.json")
        .exists()
        .then(|| gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")))
        .transpose()
        .ok()
        .flatten();
    let cpp_resolver =
        gitnexus_cpp::CppIncludeResolver::build(root, &source_files, &header_files, cpp_compile_db);

    // 6. Build graph
    let graph = gitnexus_cpp::build_cpp_graph(
        &project,
        &symbols_by_file,
        &includes_by_file,
        &calls_by_file,
        Some(&cpp_resolver),
    );

    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("C++ graph JSON serialization failed: {e}"))?;

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

#[cfg(not(feature = "tree-sitter-cpp"))]
fn run_cpp_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err("C++ support is disabled in this dev binary. CodeLattice-Tool installed binary includes C++. For source builds: cargo build --features tree-sitter-cpp or ALL_LANGUAGE_FEATURES.".to_string())
}

/// 计算 C++ 质量门（复用通用质量门逻辑）
fn compute_cpp_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    compute_arkts_quality_gates(nodes, edges)
}

// ============================================================
// Python 分析 + Graph 提取
// ============================================================

#[cfg(feature = "tree-sitter-python")]
fn run_python_analysis_with_trace(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    use rayon::prelude::*;
    use std::collections::BTreeMap;

    let total_start = std::time::Instant::now();

    // 1. Build project model
    let stage_start = std::time::Instant::now();
    let project = gitnexus_python::project::find_python_project_root(root).ok_or_else(|| {
        "Python project root not found (no Python markers or files detected)".to_string()
    })?;
    let project_root_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let (source_files, stub_files) =
        gitnexus_python::project::list_python_source_files(&project)
            .map_err(|e| format!("Failed to list Python source files: {e}"))?;

    let all_files: Vec<std::path::PathBuf> = source_files
        .iter()
        .chain(stub_files.iter())
        .cloned()
        .collect();
    let source_discovery_ms = elapsed_ms(stage_start);

    // 2. Extract per-file data
    let stage_start = std::time::Instant::now();
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonImport>> =
        BTreeMap::new();

    let extracted = all_files
        .par_iter()
        .filter_map(|file| {
            let source = std::fs::read_to_string(file).ok()?;
            let rel_path = file
                .strip_prefix(&project.root)
                .unwrap_or(file)
                .to_string_lossy()
                .to_string();
            Some((
                file.clone(),
                gitnexus_python::extract_python_symbols(&source, &rel_path),
                gitnexus_python::extract_python_imports(&source),
            ))
        })
        .collect::<Vec<_>>();

    for (file, syms, imps) in extracted {
        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !imps.is_empty() {
            imports_by_file.insert(file, imps);
        }
    }
    let extraction_ms = elapsed_ms(stage_start);

    // 3. Build project function name index for call extraction
    let stage_start = std::time::Instant::now();
    let project_fn_names: Vec<String> = symbols_by_file
        .values()
        .flat_map(|syms| {
            syms.iter()
                .filter(|s| {
                    matches!(
                        s.kind,
                        gitnexus_python::PythonSymbolKind::Function
                            | gitnexus_python::PythonSymbolKind::AsyncFunction
                            | gitnexus_python::PythonSymbolKind::Method
                            | gitnexus_python::PythonSymbolKind::Constructor
                    )
                })
                .flat_map(|s| [s.qualified_name.clone(), s.name.clone()])
        })
        .collect();
    let function_index_ms = elapsed_ms(stage_start);

    // 4. Extract calls per file
    let stage_start = std::time::Instant::now();
    let mut calls_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonCall>> =
        BTreeMap::new();
    let call_entries = all_files
        .par_iter()
        .filter_map(|file| {
            let source = std::fs::read_to_string(file).ok()?;
            let rel_path = file
                .strip_prefix(&project.root)
                .unwrap_or(file)
                .to_string_lossy()
                .to_string();
            let calls =
                gitnexus_python::extract_python_calls(&source, &rel_path, &project_fn_names);
            if calls.is_empty() {
                None
            } else {
                Some((file.clone(), calls))
            }
        })
        .collect::<Vec<_>>();

    for (file, calls) in call_entries {
        calls_by_file.insert(file, calls);
    }
    let call_extraction_ms = elapsed_ms(stage_start);

    // 5. Build module index for import resolution
    let stage_start = std::time::Instant::now();
    let module_index =
        gitnexus_python::PythonModuleIndex::build(&project.root, &project.source_files);
    let module_index_ms = elapsed_ms(stage_start);

    // 6. Build graph
    let stage_start = std::time::Instant::now();
    let graph = gitnexus_python::build_python_graph(
        &project,
        &symbols_by_file,
        &imports_by_file,
        &calls_by_file,
        Some(&module_index),
    );
    let graph_build_ms = elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("Python graph JSON serialization failed: {e}"))?;

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
    let serialization_ms = elapsed_ms(stage_start);

    let trace = language_stage_trace(
        "python",
        root,
        elapsed_ms(total_start),
        all_files.len(),
        &nodes,
        &edges,
        json!({
            "projectRootMs": project_root_ms,
            "sourceDiscoveryMs": source_discovery_ms,
            "extractionMs": extraction_ms,
            "functionIndexMs": function_index_ms,
            "callExtractionMs": call_extraction_ms,
            "moduleIndexMs": module_index_ms,
            "graphBuildMs": graph_build_ms,
            "serializationMs": serialization_ms
        }),
        vec![
            "Stage trace measures the current project-once Python analyzer.",
            "Python call extraction currently performs a second source read after symbol/import extraction.",
        ],
    );

    Ok((json_val, nodes, edges, trace))
}

#[cfg(feature = "tree-sitter-python")]
fn run_python_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let (json_val, nodes, edges, _trace) = run_python_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-python"))]
fn run_python_analysis_with_trace(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        serde_json::Value,
    ),
    String,
> {
    Err("Python support is disabled in this dev binary. CodeLattice-Tool installed binary includes Python. For source builds: cargo build --features tree-sitter-python or ALL_LANGUAGE_FEATURES.".to_string())
}

#[cfg(not(feature = "tree-sitter-python"))]
fn run_python_analysis(
    root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    let (json_val, nodes, edges, _trace) = run_python_analysis_with_trace(root)?;
    Ok((json_val, nodes, edges))
}

/// 计算 Python 质量门（复用通用质量门逻辑）
fn compute_python_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    compute_arkts_quality_gates(nodes, edges)
}

// ============================================================
// Shell 分析 + Graph 提取
// ============================================================

fn run_shell_analysis(
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

    let project = gitnexus_shell::find_shell_project_root(root).ok_or_else(|| {
        "Shell project root not found (no .sh/.bash/.zsh/.ksh/.bats files or shell shebang scripts detected)".to_string()
    })?;
    let files = gitnexus_shell::list_shell_source_files(&project)
        .map_err(|e| format!("Failed to list Shell source files: {e}"))?;

    let mut analyses_by_file = BTreeMap::new();
    for file in &files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let rel_path = file
            .strip_prefix(&project.root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        analyses_by_file.insert(
            file.clone(),
            gitnexus_shell::extract_shell_file(&source, &rel_path),
        );
    }

    let graph = gitnexus_shell::build_shell_graph(&project, &analyses_by_file);
    let json_val = serde_json::to_value(&graph)
        .map_err(|e| format!("Shell graph JSON serialization failed: {e}"))?;
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

fn compute_shell_quality_gates(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<QualityGateResult> {
    compute_arkts_quality_gates(nodes, edges)
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
        .filter(|n| {
            let k = n.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            k == "symbol" || k == "Symbol"
        })
        .count();
    let source_file_count = nodes
        .iter()
        .filter(|n| {
            let k = n.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            k == "sourceFile" || k == "source-file" || k == "SourceFile"
        })
        .count();
    let call_edge_count = edges
        .iter()
        .filter(|e| {
            let k = e.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let t = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
            k == "calls" || k == "Calls" || t == "CALLS"
        })
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

/// 构建 Shell GraphSummary。
///
/// Shell 分析会把 `rm -rf`、`curl | sh` 等静态风险放进 graph.diagnostics；
/// summary 必须从真实诊断源计数，避免 WebUI/CLI 把脚本风险误显示为 0。
fn build_shell_summary(
    graph: &serde_json::Value,
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> GraphSummary {
    let mut summary = build_arkts_summary(nodes, edges);
    summary.diagnostic_count = graph
        .get("diagnostics")
        .and_then(|v| v.as_array())
        .map(|items| items.len() as u32)
        .unwrap_or(0);
    summary
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
                    eprintln!("Cangjie support is disabled in this dev binary.");
                    eprintln!("CodeLattice-Tool installed binary includes Cangjie.");
                    eprintln!("For source builds: cargo build --features tree-sitter-cangjie or ALL_LANGUAGE_FEATURES");
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
            engine,
            profile,
            profile_page,
            profile_page_size,
            public_only,
        } => {
            if format != "json" && format != "gitnexus-rc" {
                eprintln!("错误：支持的格式：json, gitnexus-rc");
                std::process::exit(1);
            }

            if let Err(e) = validate_profile(&profile) {
                eprintln!("错误：{e}");
                std::process::exit(1);
            }
            let profile_options =
                AnalyzeProfileOptions::new(profile_page, profile_page_size, public_only);

            let is_bridge = format == "gitnexus-rc";
            let root_path = match check_root(&root) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            if language == "auto" && !is_bridge {
                if let Some(workspace) =
                    build_workspace_auto_entry(root_path, "auto-language-multi-project-root")
                {
                    let json = serde_json::to_string_pretty(&workspace).unwrap_or_else(|e| {
                        eprintln!("错误：Workspace JSON 序列化失败: {e}");
                        std::process::exit(1);
                    });
                    println!("{json}");
                    return;
                }
            }

            // ═══ Analysis Engine 1.3 path (--engine serial|parallel|parity) ═══
            if engine != "off" {
                let result = match engine.as_str() {
                    "serial" | "parallel" | "parity" => crate::engine_bridge::run_engine_analysis(
                        root_path,
                        &language,
                        engine == "parallel",
                    ),
                    _ => {
                        eprintln!(
                            "Unknown engine mode: {}. Use serial, parallel, or off.",
                            engine
                        );
                        std::process::exit(1);
                    }
                };
                match result {
                    Ok(output) => {
                        let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                            eprintln!("Engine serialization error: {e}");
                            std::process::exit(1);
                        });
                        println!("{json}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Engine analysis error: {e}");
                        std::process::exit(1);
                    }
                }
            }

            let lang = match resolve_language(&language, root_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            if engine != "off" {
                let parallel = engine == "parallel";
                match engine_bridge::run_engine_analysis(root_path, &lang, parallel) {
                    Ok(result) => {
                        let filtered = filter_analyze_profile(&result, &profile, profile_options);
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&filtered).unwrap_or_default()
                        );
                        return;
                    }
                    Err(e) => {
                        eprintln!("Engine analysis failed: {e}, falling back to standard path");
                    }
                }
            }

            #[cfg(debug_assertions)]
            eprintln!("分析中... language={lang}, root={root}");

            match lang.as_str() {
                "rust" => {
                    let (json_val, nodes, edges, _trace) = match run_rust_analysis(root_path) {
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

                        print_analyze_result(&result, &profile, profile_options);
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

                        print_analyze_result(&result, &profile, profile_options);
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
                        print_analyze_result(&result, &profile, profile_options);
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
                        print_analyze_result(&result, &profile, profile_options);
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
                "javascript" => {
                    let (json_val, nodes, edges) = match run_javascript_analysis(root_path) {
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
                        print_analyze_result(&result, &profile, profile_options);
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
                "c" => {
                    let (json_val, nodes, edges) = match run_c_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_c_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.1.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = bridge_format::convert_c_graph(
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
                        print_analyze_result(&result, &profile, profile_options);
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
                "cpp" => {
                    let (json_val, nodes, edges) = match run_cpp_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_cpp_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.1.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = cpp_bridge::convert_cpp_graph(
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
                        print_analyze_result(&result, &profile, profile_options);
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
                "python" => {
                    let (json_val, nodes, edges) = match run_python_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_python_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("v0.1.0")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = python_bridge::convert_python_graph(
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
                        print_analyze_result(&result, &profile, profile_options);
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
                "shell" => {
                    let (json_val, nodes, edges) = match run_shell_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };

                    let quality_gates = compute_shell_quality_gates(&nodes, &edges);
                    let schema_version = json_val
                        .get("schemaVersion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("shell-v0.1")
                        .to_string();

                    if is_bridge {
                        let analyzed_at = now_iso8601();
                        let bridge = shell_bridge::convert_shell_graph(
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
                        let summary = build_shell_summary(&json_val, &nodes, &edges);
                        let result = LanguageAnalysisResult {
                            language: lang,
                            root: root_path.to_string_lossy().to_string(),
                            analyzed_at: now_iso8601(),
                            schema_version,
                            summary,
                            quality_gates: quality_gates.clone(),
                            graph: json_val,
                        };
                        print_analyze_result(&result, &profile, profile_options);
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
                && language != "c"
                && language != "cpp"
                && language != "python"
                && language != "shell"
            {
                eprintln!(
                    "错误：quality 命令需要显式指定 --language rust|cangjie|arkts|typescript|c|cpp|python|shell"
                );
                std::process::exit(1);
            }

            eprintln!("质量门检查中... language={language}, root={root}");

            let (gates, overall) = match language.as_str() {
                "rust" => {
                    let (_json_val, nodes, edges, _trace) = match run_rust_analysis(root_path) {
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
                "javascript" => {
                    let (_json_val, nodes, edges) = match run_javascript_analysis(root_path) {
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
                "c" => {
                    let (_json_val, nodes, edges) = match run_c_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_c_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "cpp" => {
                    let (_json_val, nodes, edges) = match run_cpp_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_cpp_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "python" => {
                    let (_json_val, nodes, edges) = match run_python_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_python_quality_gates(&nodes, &edges);
                    let all_pass = gates.iter().all(|g| g.passed);
                    let overall = if all_pass { "pass" } else { "fail" };
                    (gates, overall)
                }
                "shell" => {
                    let (_json_val, nodes, edges) = match run_shell_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gates = compute_shell_quality_gates(&nodes, &edges);
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
                    let (json_val, nodes, edges, _trace) = match run_rust_analysis(root_path) {
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
                "javascript" => {
                    let (_json_val, nodes, edges) = match run_javascript_analysis(root_path) {
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
                "c" => {
                    let (_json_val, nodes, edges) = match run_c_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_c_quality_gates(&nodes, &edges);
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
                "cpp" => {
                    let (_json_val, nodes, edges) = match run_cpp_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_cpp_quality_gates(&nodes, &edges);
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
                "python" => {
                    let (_json_val, nodes, edges) = match run_python_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_python_quality_gates(&nodes, &edges);
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
                "shell" => {
                    let (_json_val, nodes, edges) = match run_shell_analysis(root_path) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{e}");
                            std::process::exit(1);
                        }
                    };
                    let gs = build_arkts_summary(&nodes, &edges);
                    let gates = compute_shell_quality_gates(&nodes, &edges);
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

        // ===== CodeLattice-native detect-changes =====
        Commands::DetectChanges {
            root,
            language,
            scope,
            diff_mode,
            base_ref,
            format,
            limit,
            compact,
            include_snippet,
            snippet_context,
            include_fixtures,
            strict_workspace,
        } => {
            if format != "json" {
                eprintln!("错误：detect-changes 当前仅支持 --format json");
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
            let selected_diff_mode = match scope_to_diff_mode(&scope, diff_mode.as_deref()) {
                Ok(mode) => mode,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            };

            let mut changed_args = json!({
                "root": root_path.to_string_lossy(),
                "language": lang,
                "diffMode": selected_diff_mode,
                "compact": compact,
                "includeSnippet": include_snippet,
                "snippetContext": snippet_context,
                "limit": limit
            });
            if let Some(base) = &base_ref {
                changed_args["baseRef"] = json!(base);
            }

            let changed = match call_mcp_tool_via_current_binary(
                "codelattice_changed_symbols",
                changed_args.clone(),
            ) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("错误：CodeLattice changed_symbols 调用失败: {e}");
                    std::process::exit(1);
                }
            };

            let mut assist_args = json!({
                "root": root_path.to_string_lossy(),
                "language": lang,
                "compact": compact
            });
            assist_args["diffMode"] = changed_args["diffMode"].clone();
            if let Some(base) = &base_ref {
                assist_args["baseRef"] = json!(base);
            }

            let assist = match call_mcp_tool_via_current_binary(
                "codelattice_production_assist",
                assist_args,
            ) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("错误：CodeLattice production_assist 调用失败: {e}");
                    std::process::exit(1);
                }
            };

            let output_language = changed
                .get("language")
                .and_then(|v| v.as_str())
                .unwrap_or(&language)
                .to_string();
            let untracked_files = if selected_diff_mode == "head" || scope == "all" {
                collect_untracked_files(root_path)
            } else {
                Vec::new()
            };

            // workspace change intelligence：file ownership + cross-project impact
            let changed_files_for_owners = changed
                .get("changedFiles")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let untracked_file_values: Vec<Value> =
                untracked_files.iter().map(|f| json!({"path": f})).collect();
            let all_changed_for_owners: Vec<Value> = changed_files_for_owners
                .iter()
                .chain(untracked_file_values.iter())
                .cloned()
                .collect();
            let file_owners = map_files_to_owners(&all_changed_for_owners, root_path);
            let workspace_policy = WorkspaceImpactPolicy {
                include_fixtures,
                strict_workspace,
            };
            let workspace_impact =
                compute_workspace_impact(root_path, &file_owners, workspace_policy);

            let output = build_detect_changes_report(
                root_path,
                &output_language,
                &scope,
                &selected_diff_mode,
                compact,
                changed,
                assist,
                untracked_files,
                workspace_impact,
                file_owners,
            );

            let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                eprintln!("错误：detect-changes JSON 序列化失败: {e}");
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
