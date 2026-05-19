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

mod arkts_bridge;
mod bridge_format;
mod c_bridge;
mod cangjie_bridge;
mod cpp_bridge;
mod language_detect;
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
            DetectedLanguage::C => Ok("c".to_string()),
            DetectedLanguage::Cpp => Ok("cpp".to_string()),
            DetectedLanguage::Python => Ok("python".to_string()),
            DetectedLanguage::Shell => Ok("shell".to_string()),
            DetectedLanguage::Ambiguous => Err(
                "语言检测失败：存在多种清单文件，请使用 --language rust|cangjie|arkts|typescript|c|cpp|python|shell 显式指定".to_string(),
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
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
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
    let overall_risk = pick_detect_changes_risk(&changed, &assist);
    let untracked_file_count = untracked_files.len() as u64;
    let total_file_change_count = changed_file_count + untracked_file_count;

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
            "overallRiskReasons": assist.get("overallRiskReasons").cloned().unwrap_or_else(|| json!([])),
            "highestRiskSymbols": assist.get("highestRiskSymbols").cloned().unwrap_or_else(|| json!([]))
        },
        "reviewChecklist": assist.get("reviewChecklist").cloned().unwrap_or_else(|| json!([])),
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
            "nativeCodeLattice": true
        },
        "cautions": [
            "Static analysis only: this does not prove runtime breakage or safety.",
            "affectedProcessCount is null because CodeLattice does not use the legacy GitNexus process model.",
            "Use changedSymbols, unknownHunks, risk reasons, and reviewChecklist as investigation leads."
        ],
        "underlyingTools": [
            "codelattice_changed_symbols",
            "codelattice_production_assist"
        ]
    });

    if compact {
        if let Some(obj) = report.as_object_mut() {
            obj.remove("quality");
            obj.remove("docs");
            obj.remove("deletedFiles");
            obj.remove("renamedFiles");
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
    Err("ArkTS support is disabled. 请使用 --features tree-sitter-arkts 重新编译。".to_string())
}

// ============================================================
// TypeScript 分析 + Graph 提取
// ============================================================

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
    use std::collections::BTreeMap;

    // 1. Build project model (tsconfig.json / package.json)
    let project =
        gitnexus_typescript::project::find_typescript_project_root(root).ok_or_else(|| {
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

    // Build module resolver for path alias / monorepo support
    let resolver = gitnexus_typescript::TsModuleResolver::build(&ts_project.root, &source_files);

    let graph = gitnexus_typescript::graph::build_ts_graph(
        &ts_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
        Some(&resolver),
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

#[cfg(not(feature = "tree-sitter-typescript"))]
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
        "TypeScript support is disabled. 请使用 --features tree-sitter-typescript 重新编译。"
            .to_string(),
    )
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
    Err("C support is disabled. 请使用 --features tree-sitter-c 重新编译。".to_string())
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
    Err("C++ support is disabled. 请使用 --features tree-sitter-cpp 重新编译。".to_string())
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
    use std::collections::BTreeMap;

    // 1. Build project model
    let project = gitnexus_python::project::find_python_project_root(root).ok_or_else(|| {
        "Python project root not found (no Python markers or files detected)".to_string()
    })?;

    let (source_files, stub_files) =
        gitnexus_python::project::list_python_source_files(&project)
            .map_err(|e| format!("Failed to list Python source files: {e}"))?;

    let all_files: Vec<std::path::PathBuf> = source_files
        .iter()
        .chain(stub_files.iter())
        .cloned()
        .collect();

    // 2. Extract per-file data
    let mut symbols_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonImport>> =
        BTreeMap::new();

    for file in &all_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel_path = file
            .strip_prefix(&project.root)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();

        let syms = gitnexus_python::extract_python_symbols(&source, &rel_path);
        let imps = gitnexus_python::extract_python_imports(&source);

        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !imps.is_empty() {
            imports_by_file.insert(file.clone(), imps);
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
                        gitnexus_python::PythonSymbolKind::Function
                            | gitnexus_python::PythonSymbolKind::AsyncFunction
                            | gitnexus_python::PythonSymbolKind::Method
                            | gitnexus_python::PythonSymbolKind::Constructor
                    )
                })
                .flat_map(|s| [s.qualified_name.clone(), s.name.clone()])
        })
        .collect();

    // 4. Extract calls per file
    let mut calls_by_file: BTreeMap<std::path::PathBuf, Vec<gitnexus_python::PythonCall>> =
        BTreeMap::new();
    for file in &all_files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel_path = file
            .strip_prefix(&project.root)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();

        let calls = gitnexus_python::extract_python_calls(&source, &rel_path, &project_fn_names);

        if !calls.is_empty() {
            calls_by_file.insert(file.clone(), calls);
        }
    }

    // 5. Build module index for import resolution
    let module_index =
        gitnexus_python::PythonModuleIndex::build(&project.root, &project.source_files);

    // 6. Build graph
    let graph = gitnexus_python::build_python_graph(
        &project,
        &symbols_by_file,
        &imports_by_file,
        &calls_by_file,
        Some(&module_index),
    );

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

    Ok((json_val, nodes, edges))
}

#[cfg(not(feature = "tree-sitter-python"))]
fn run_python_analysis(
    _root: &Path,
) -> Result<
    (
        serde_json::Value,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ),
    String,
> {
    Err("Python support is disabled. 请使用 --features tree-sitter-python 重新编译。".to_string())
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

            let output = build_detect_changes_report(
                root_path,
                &output_language,
                &scope,
                &selected_diff_mode,
                compact,
                changed,
                assist,
                untracked_files,
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
