//! MCP v0.1 Practical AI Layer for CodeLattice CLI
//!
//! Implements a MCP JSON-RPC server over stdin/stdout.
//! Provides 8 tools:
//!   v0:  codelattice_analyze, codelattice_quality, codelattice_summary, codelattice_smoke
//!   v0.1: codelattice_graph_overview, codelattice_unresolved_report,
//!         codelattice_symbol_search, codelattice_export_bridge
//!
//! Transport: newline-delimited JSON-RPC.
//! Approach: subprocess — spawns the CLI binary for analyze/quality/summary,
//!           and the smoke script for smoke.
//! Safety: path deny list, output path restrictions (/tmp only for export).

use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ============================================================
// Path Safety
// ============================================================

/// Paths that are explicitly denied for MCP access (live repos).
const DENIED_PATHS: &[&str] = &["/Users/jiangxuanyang/Desktop/cangjie"];

/// Validate that an output path is within /tmp (or a system temp dir).
fn validate_output_path(output_path: &str) -> Result<PathBuf, Value> {
    let path = PathBuf::from(output_path);

    // Check by string prefix: must start with /tmp/ or /private/tmp/
    // We use string comparison rather than canonicalize because the file
    // may not exist yet, and /tmp may not canonicalize consistently.
    let path_str = path.to_string_lossy();
    if !path_str.starts_with("/tmp/") && !path_str.starts_with("/private/tmp/") {
        return Err(mcp_error_with_hint(
            "output_path_denied",
            &format!("Output path must be under /tmp, got: {output_path}"),
            "export_bridge only writes to /tmp for safety",
            "Use outputPath like /tmp/codelattice-bridge-<name>.json or omit to auto-generate",
        ));
    }

    Ok(path)
}

fn validate_root_path(root: &str) -> Result<PathBuf, Value> {
    let path = PathBuf::from(root);

    // Canonicalize for comparison (resolves symlinks, trailing slashes, etc.)
    let canonical = path
        .canonicalize()
        .map_err(|_| mcp_error("path_not_found", &format!("Path does not exist: {root}")))?;

    if !canonical.is_dir() {
        return Err(mcp_error(
            "path_not_directory",
            &format!("Path is not a directory: {root}"),
        ));
    }

    // Check deny list
    for denied in DENIED_PATHS {
        let denied_canonical = PathBuf::from(denied).canonicalize().ok();
        if let Some(dc) = denied_canonical {
            if canonical == dc {
                return Err(mcp_error(
                    "path_denied",
                    &format!("Path is on deny list (live repo): {denied}"),
                ));
            }
        }
        // Also check string prefix as fallback
        if canonical.to_string_lossy().starts_with(denied) {
            return Err(mcp_error(
                "path_denied",
                &format!("Path is under denied directory: {denied}"),
            ));
        }
    }

    Ok(canonical)
}

// ============================================================
// Error helpers
// ============================================================

/// Unified error structure with code, message, details, and hint.
fn mcp_error(code: &str, message: &str) -> Value {
    json!({
        "error": code,
        "message": message
    })
}

fn mcp_error_detail(code: &str, message: &str, details: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details
    })
}

fn mcp_error_with_hint(code: &str, message: &str, details: &str, hint: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details,
        "hint": hint
    })
}

#[allow(dead_code)]
fn tool_error(code: &str, message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&mcp_error(code, message)).unwrap_or_default() }],
        "isError": true
    })
}

fn tool_result(data: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(data).unwrap_or_default() }]
    })
}

// ============================================================
// Subprocess helpers
// ============================================================

fn get_cli_binary() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gitnexus-rust-core-cli"))
}

fn run_subcommand_with_timeout(args: &[&str], timeout: Duration) -> Result<Value, Value> {
    let binary = get_cli_binary();
    let start = Instant::now();

    let mut child = Command::new(&binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            mcp_error(
                "command_failed",
                &format!("Failed to spawn {}: {}", binary.display(), e),
            )
        })?;

    // Poll for completion with timeout
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                let _stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                if !status.success() {
                    return Err(mcp_error(
                        "command_failed",
                        &format!(
                            "Command exited with code {:?}: {}",
                            status.code(),
                            stdout.trim().chars().take(200).collect::<String>()
                        ),
                    ));
                }

                // Parse stdout as JSON
                let trimmed = stdout.trim();
                if trimmed.is_empty() {
                    return Err(mcp_error(
                        "json_parse_failed",
                        "Command produced empty output",
                    ));
                }

                return serde_json::from_str(trimmed).map_err(|e| {
                    mcp_error(
                        "json_parse_failed",
                        &format!(
                            "Failed to parse JSON: {}. Output: {}",
                            e,
                            &trimmed[..trimmed.len().min(200)]
                        ),
                    )
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(mcp_error(
                        "timeout",
                        &format!("Command timed out after {:?}", timeout),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(mcp_error(
                    "command_failed",
                    &format!("Failed to check process status: {}", e),
                ));
            }
        }
    }
}

fn run_script_with_timeout(
    script: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, Value> {
    let start = Instant::now();

    let mut child = Command::new("bash")
        .arg(script)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| mcp_error("command_failed", &format!("Failed to run script: {}", e)))?;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                let _stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();

                if !status.success() {
                    return Err(mcp_error(
                        "smoke_failed",
                        &format!("Smoke script exited with code {:?}", status.code()),
                    ));
                }

                return Ok(stdout);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(mcp_error("timeout", "Smoke script timed out"));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(mcp_error(
                    "command_failed",
                    &format!("Failed to check script status: {}", e),
                ));
            }
        }
    }
}

// ============================================================
// Language helpers
// ============================================================

/// Check if cangjie language is requested but feature is not compiled.
/// Returns Err if cangjie requested without feature, Ok(()) otherwise.
fn check_cangjie_feature(language: &str) -> Result<(), Value> {
    if language == "cangjie" {
        #[cfg(not(feature = "tree-sitter-cangjie"))]
        {
            return Err(mcp_error_with_hint(
                "cangjie_disabled",
                "Cangjie support not compiled",
                "Cangjie language was requested but tree-sitter-cangjie feature is not enabled",
                "Rebuild with --features tree-sitter-cangjie",
            ));
        }
    }
    Ok(())
}

/// Run the CLI analyze subcommand and return parsed JSON.
/// Used by multiple tools that need the full analyze output.
fn run_analyze_subprocess(
    root: &Path,
    language: &str,
    format: &str,
    strict: bool,
) -> Result<Value, Value> {
    let root_str = root.to_string_lossy().to_string();
    let mut args = vec![
        "analyze".to_string(),
        "--root".to_string(),
        root_str,
        "--language".to_string(),
        language.to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    if strict {
        args.push("--strict".to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_subcommand_with_timeout(&arg_refs, Duration::from_secs(60))
}

// ============================================================
// Tool Handlers
// ============================================================

fn handle_analyze(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_cangjie_feature(language)?;

    let strict = params["strict"].as_bool().unwrap_or(true);
    let include_graph = params["includeGraph"].as_bool().unwrap_or(false);

    let result = run_analyze_subprocess(&validated, language, "json", strict)?;

    // Compact output: strip graph unless includeGraph=true
    if !include_graph {
        if let Some(obj) = result.as_object() {
            let mut filtered = obj.clone();
            // Remove the full graph, keep summary/stats
            filtered.remove("graph");
            return Ok(tool_result(&Value::Object(filtered)));
        }
    }

    Ok(tool_result(&result))
}

fn handle_quality(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let root_str = validated.to_string_lossy().to_string();
    let language = params["language"].as_str().unwrap_or("auto");
    check_cangjie_feature(language)?;

    let args = vec![
        "quality",
        "--root",
        &root_str,
        "--language",
        language,
        "--format",
        "json",
    ];

    let result = run_subcommand_with_timeout(&args, Duration::from_secs(60))?;

    // Shape output: put failed gates first for AI readability
    if let Some(obj) = result.as_object() {
        let mut shaped = obj.clone();
        if let Some(gates) = shaped.get("gates").and_then(|g| g.as_array()).cloned() {
            let mut sorted = gates;
            sorted.sort_by(|a, b| {
                let a_pass = a["passed"].as_bool().unwrap_or(true);
                let b_pass = b["passed"].as_bool().unwrap_or(true);
                b_pass.cmp(&a_pass) // false (failed) sorts before true (passed)
            });
            shaped.insert("gates".to_string(), Value::Array(sorted));
        }
        return Ok(tool_result(&Value::Object(shaped)));
    }

    Ok(tool_result(&result))
}

fn handle_summary(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let root_str = validated.to_string_lossy().to_string();
    let language = params["language"].as_str().unwrap_or("auto");
    check_cangjie_feature(language)?;

    let args = vec![
        "summary",
        "--root",
        &root_str,
        "--language",
        language,
        "--format",
        "json",
    ];

    let result = run_subcommand_with_timeout(&args, Duration::from_secs(60))?;
    Ok(tool_result(&result))
}

fn handle_smoke(params: &Value) -> Result<Value, Value> {
    let mode = params["mode"].as_str().unwrap_or("full");

    let script = {
        // Find the smoke script relative to workspace
        let exe = std::env::current_exe().unwrap_or_default();
        let workspace = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        let script_path = workspace.join("scripts").join("alpha-trial-smoke.sh");
        if script_path.exists() {
            script_path.to_string_lossy().to_string()
        } else {
            // Fallback: try relative to current dir
            "scripts/alpha-trial-smoke.sh".to_string()
        }
    };

    let mode_arg = match mode {
        "rust-only" => "--rust-only",
        "cangjie-only" => "--cangjie-only",
        _ => "",
    };

    let args = if mode_arg.is_empty() {
        vec![]
    } else {
        vec![mode_arg]
    };

    let output = run_script_with_timeout(&script, &args, Duration::from_secs(120))?;

    // Parse the output to extract pass/fail/skip counts
    let mut pass_count = 0u32;
    let mut fail_count = 0u32;
    let mut skip_count = 0u32;

    for line in output.lines() {
        if line.contains("PASS:") {
            // Try to extract number after "PASS:"
            if let Some(rest) = line.split("PASS:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    pass_count = n;
                }
            }
        }
        if line.contains("FAIL:") {
            if let Some(rest) = line.split("FAIL:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    fail_count = n;
                }
            }
        }
        if line.contains("SKIP:") {
            if let Some(rest) = line.split("SKIP:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    skip_count = n;
                }
            }
        }
    }

    let passed = fail_count == 0;
    let tail_lines: Vec<&str> = output
        .lines()
        .rev()
        .take(15)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut result = serde_json::Map::new();
    result.insert("mode".to_string(), json!(mode));
    result.insert("passed".to_string(), json!(passed));
    result.insert("passCount".to_string(), json!(pass_count));
    result.insert("failCount".to_string(), json!(fail_count));
    result.insert("skipCount".to_string(), json!(skip_count));
    result.insert("tailOutput".to_string(), json!(tail_lines.join("\n")));
    if !passed {
        result.insert("hint".to_string(), json!("Check tailOutput for failure details. Run scripts/alpha-trial-smoke.sh locally to reproduce."));
    }

    Ok(tool_result(&Value::Object(result)))
}

// ============================================================
// v0.1 Tool Handlers
// ============================================================

fn handle_graph_overview(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_cangjie_feature(language)?;

    // Run analyze with json format to get full graph, then extract overview
    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    let summary = &result["summary"];
    let quality_gates = result["qualityGates"].as_array();

    // Count nodes by kind
    let mut node_kind_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut edge_kind_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    if let Some(graph) = result.get("graph") {
        if let Some(nodes) = graph["nodes"].as_array() {
            for node in nodes {
                let kind = node["label"].as_str().unwrap_or("unknown");
                *node_kind_counts.entry(kind.to_string()).or_insert(0) += 1;
            }
        }
        if let Some(edges) = graph["edges"].as_array() {
            for edge in edges {
                let kind = edge["type"].as_str().unwrap_or("unknown");
                *edge_kind_counts.entry(kind.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Quality summary
    let quality_summary = if let Some(gates) = quality_gates {
        let passed = gates
            .iter()
            .filter(|g| g["passed"].as_bool().unwrap_or(false))
            .count();
        let failed = gates.len() - passed;
        json!({
            "total": gates.len(),
            "passed": passed,
            "failed": failed
        })
    } else {
        json!({"total": 0, "passed": 0, "failed": 0})
    };

    // Diagnostics summary
    let diag_summary = if let Some(graph) = result.get("graph") {
        if let Some(diagnostics) = graph["diagnostics"].as_array() {
            let mut by_severity: std::collections::HashMap<String, u64> =
                std::collections::HashMap::new();
            for d in diagnostics {
                let sev = d["properties"]["severity"].as_str().unwrap_or("unknown");
                *by_severity.entry(sev.to_string()).or_insert(0) += 1;
            }
            let sev_map: serde_json::Map<String, Value> = by_severity
                .into_iter()
                .map(|(k, v)| (k, json!(v)))
                .collect();
            json!({
                "total": diagnostics.len(),
                "bySeverity": Value::Object(sev_map)
            })
        } else {
            json!({"total": 0})
        }
    } else {
        json!({"total": 0})
    };

    let node_kind_map: serde_json::Map<String, Value> = node_kind_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    let edge_kind_map: serde_json::Map<String, Value> = edge_kind_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();

    Ok(tool_result(&json!({
        "language": result["language"],
        "root": result["root"],
        "nodeCount": summary["nodeCount"],
        "edgeCount": summary["edgeCount"],
        "symbolCount": summary["symbolCount"],
        "packageCount": summary["packageCount"],
        "sourceFileCount": summary["sourceFileCount"],
        "nodeKindCounts": Value::Object(node_kind_map),
        "edgeKindCounts": Value::Object(edge_kind_map),
        "qualitySummary": quality_summary,
        "diagnosticsSummary": diag_summary
    })))
}

fn handle_unresolved_report(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let limit = params["limit"].as_u64().unwrap_or(20) as usize;
    check_cangjie_feature(language)?;

    // Run analyze with json format to get graph
    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    // Check if language is cangjie — no unresolved call concept
    let detected_lang = result["language"].as_str().unwrap_or(language);
    if detected_lang == "cangjie" {
        return Ok(tool_result(&json!({
            "language": detected_lang,
            "supported": false,
            "reason": "Cangjie does not track unresolved calls in v0.1 (no CALLS edge confidence/reason classification)",
            "total": 0,
            "items": []
        })));
    }

    // For Rust: find CALLS edges with low confidence or unresolved reason
    let mut unresolved_items = Vec::new();
    let mut reason_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    if let Some(graph) = result.get("graph") {
        if let Some(edges) = graph["edges"].as_array() {
            for edge in edges {
                if edge["type"].as_str() != Some("CALLS") {
                    continue;
                }

                let confidence = edge["properties"]["confidence"].as_f64().unwrap_or(1.0);
                let reason = edge["properties"]["reason"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                // Consider unresolved if confidence < 1.0 or reason contains "unresolved"
                let is_unresolved = confidence < 1.0 || reason.contains("unresolved");

                if is_unresolved {
                    // Count by reason
                    *reason_counts.entry(reason.clone()).or_insert(0) += 1;

                    if unresolved_items.len() < limit {
                        unresolved_items.push(json!({
                            "source": edge["source"],
                            "target": edge["target"],
                            "confidence": confidence,
                            "reason": reason,
                            "callKind": edge["properties"]["callKind"]
                        }));
                    }
                }
            }
        }
    }

    // Also check diagnostics for unresolved-related codes
    let mut diag_unresolved = Vec::new();
    if let Some(graph) = result.get("graph") {
        if let Some(diagnostics) = graph["diagnostics"].as_array() {
            for d in diagnostics {
                let code = d["properties"]["code"].as_str().unwrap_or("");
                if code.contains("unresolved") || code.contains("stop-line") {
                    diag_unresolved.push(json!({
                        "code": code,
                        "message": d["properties"]["message"],
                        "severity": d["properties"]["severity"],
                        "path": d["properties"]["path"]
                    }));
                }
            }
        }
    }

    let reason_map: serde_json::Map<String, Value> = reason_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();

    Ok(tool_result(&json!({
        "language": detected_lang,
        "supported": true,
        "total": unresolved_items.len() + diag_unresolved.len(),
        "unresolvedEdges": unresolved_items.len(),
        "unresolvedDiagnostics": diag_unresolved.len(),
        "reasonBreakdown": Value::Object(reason_map),
        "topItems": unresolved_items,
        "diagnosticItems": diag_unresolved,
        "stopLineNote": "Items near Rust stop-line (no rust-analyzer, no macro expansion, no full cfg evaluator) will appear as unresolved"
    })))
}

fn handle_symbol_search(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let query = params["query"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: query"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let kind_filter = params["kind"].as_str();
    let limit = params["limit"].as_u64().unwrap_or(20) as usize;
    let limit = limit.min(100); // max 100
    check_cangjie_feature(language)?;

    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    if let Some(graph) = result.get("graph") {
        if let Some(nodes) = graph["nodes"].as_array() {
            for node in nodes {
                // Only search symbol and package nodes
                let label = node["label"].as_str().unwrap_or("");
                if label != "symbol" && label != "package" && label != "source-file" {
                    continue;
                }

                // Kind filter
                if let Some(filter) = kind_filter {
                    let node_kind = node["properties"]["symbolKind"]
                        .as_str()
                        .or_else(|| node["properties"]["kind"].as_str())
                        .unwrap_or(label);
                    if node_kind.to_lowercase() != filter.to_lowercase() {
                        continue;
                    }
                }

                let name = node["properties"]["name"]
                    .as_str()
                    .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
                    .unwrap_or("");

                // Case-insensitive contains match
                if name.to_lowercase().contains(&query_lower) {
                    if matches.len() < limit {
                        let file_val = node["properties"]["sourcePath"]
                            .as_str()
                            .map(|s| json!(s))
                            .unwrap_or_else(|| {
                                node["properties"]["manifestPath"]
                                    .as_str()
                                    .map(|s| json!(s))
                                    .unwrap_or(Value::Null)
                            });
                        matches.push(json!({
                            "id": node["id"],
                            "name": name,
                            "kind": node["properties"]["symbolKind"].as_str().or_else(|| node["properties"]["kind"].as_str()).unwrap_or(label),
                            "file": file_val,
                            "line": node["properties"]["lineStart"],
                            "label": label
                        }));
                    }
                }
            }
        }
    }

    Ok(tool_result(&json!({
        "language": result["language"],
        "query": query,
        "matchCount": matches.len(),
        "matches": matches
    })))
}

fn handle_export_bridge(params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let language = params["language"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: language"))?;

    let validated = validate_root_path(root)?;
    check_cangjie_feature(language)?;

    // Determine output path
    let output_path = if let Some(op) = params["outputPath"].as_str() {
        validate_output_path(op)?
    } else {
        // Auto-generate in /tmp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        PathBuf::from(format!("/tmp/codelattice-bridge-{}.json", timestamp))
    };

    // Run analyze with gitnexus-rc format
    let result = run_analyze_subprocess(&validated, language, "gitnexus-rc", false)?;

    // Write to file
    let json_str = serde_json::to_string_pretty(&result).map_err(|e| {
        mcp_error_detail(
            "json_serialize_failed",
            "Failed to serialize bridge JSON",
            &e.to_string(),
        )
    })?;

    std::fs::write(&output_path, &json_str).map_err(|e| {
        mcp_error_detail(
            "write_failed",
            &format!("Failed to write bridge JSON to {}", output_path.display()),
            &e.to_string(),
        )
    })?;

    let bytes = json_str.len();

    // Extract counts from the bridge output
    let _stats = &result["stats"];
    let packages = result["packages"].as_array().map(|a| a.len()).unwrap_or(0);
    let source_files = result["sourceFiles"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let symbols = result["symbols"].as_array().map(|a| a.len()).unwrap_or(0);
    let relationships = result["edges"].as_array().map(|a| a.len()).unwrap_or(0);

    Ok(tool_result(&json!({
        "outputPath": output_path.to_string_lossy(),
        "bytes": bytes,
        "schemaVersion": result["schemaVersion"],
        "language": result["language"],
        "packages": packages,
        "files": source_files,
        "symbols": symbols,
        "relationships": relationships,
        "stdoutPurity": true
    })))
}

// ============================================================
// Tools List
// ============================================================

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "codelattice_analyze",
                "description": "Analyze a Rust or Cangjie project. Returns graph summary, quality gates, and optionally the full graph. Compact by default (graph excluded unless includeGraph=true).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to analyze" },
                        "strict": { "type": "boolean", "default": true, "description": "Mark quality gate failures as errors" },
                        "includeGraph": { "type": "boolean", "default": false, "description": "Include full graph in output (large, default off)" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_quality",
                "description": "Run quality gate checks on a project. Returns pass/fail for each gate, with failed gates listed first for quick triage.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to check" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_summary",
                "description": "Get a compact summary of project graph stats and quality gates without full graph output.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to summarize" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_smoke",
                "description": "Run end-to-end smoke tests (bridge JSON generation + Tool import). Validates Rust and/or Cangjie analysis pipeline. Includes tail output and failure hints.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["rust-only", "cangjie-only", "full"], "default": "full", "description": "Which smoke mode to run" }
                    }
                }
            },
            {
                "name": "codelattice_graph_overview",
                "description": "Get a compact overview of the graph: node/edge/symbol/package counts, kind breakdowns, quality and diagnostics summaries. No full graph data. Ideal for AI agents to quickly assess a project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to analyze" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_unresolved_report",
                "description": "Report unresolved calls and diagnostics. For Rust: shows CALLS edges with low confidence or unresolved reasons, grouped by reason. For Cangjie: returns supported=false (no unresolved concept in v0.1). Includes stop-line classification note.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to analyze" },
                        "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100, "description": "Max unresolved items to return" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_symbol_search",
                "description": "Search for symbols by name (case-insensitive contains match). Returns matching symbols with name, kind, file, and line. Optionally filter by symbol kind (function, struct, class, etc).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto", "description": "Language to search" },
                        "query": { "type": "string", "description": "Search query (case-insensitive substring match)" },
                        "kind": { "type": "string", "description": "Filter by symbol kind (function, struct, class, enum, interface, etc)" },
                        "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100, "description": "Max results to return" }
                    },
                    "required": ["root", "query"]
                }
            },
            {
                "name": "codelattice_export_bridge",
                "description": "Export project analysis as GitNexus-RC bridge JSON to /tmp. Returns file path, byte count, and schema/counts summary. Output path must be under /tmp. No Tool import — export only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie"], "description": "Language (must be explicit, not auto)" },
                        "outputPath": { "type": "string", "description": "Output file path (must be under /tmp). Default: auto-generated in /tmp" }
                    },
                    "required": ["root", "language"]
                }
            }
        ]
    })
}

// ============================================================
// JSON-RPC Dispatch
// ============================================================

fn make_response(id: &Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn make_error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn handle_request(request: &Value) -> Option<Value> {
    let method = request["method"].as_str().unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = request.get("params").cloned().unwrap_or(json!({}));

    // Notifications (no id or id is null) don't get responses
    if id.is_null() && !method.starts_with("tools/") {
        match method {
            "notifications/initialized" => {
                eprintln!("[mcp] client initialized");
                return None;
            }
            _ => {
                eprintln!("[mcp] ignoring notification: {}", method);
                return None;
            }
        }
    }

    match method {
        "initialize" => Some(make_response(
            &id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "codelattice", "version": "0.1.0" }
            }),
        )),

        "tools/list" => Some(make_response(&id, tools_list())),

        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");

            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = match tool_name {
                "codelattice_analyze" => handle_analyze(&arguments),
                "codelattice_quality" => handle_quality(&arguments),
                "codelattice_summary" => handle_summary(&arguments),
                "codelattice_smoke" => handle_smoke(&arguments),
                "codelattice_graph_overview" => handle_graph_overview(&arguments),
                "codelattice_unresolved_report" => handle_unresolved_report(&arguments),
                "codelattice_symbol_search" => handle_symbol_search(&arguments),
                "codelattice_export_bridge" => handle_export_bridge(&arguments),
                _ => Err(mcp_error(
                    "unknown_tool",
                    &format!("Unknown tool: {tool_name}"),
                )),
            };

            match result {
                Ok(r) => Some(make_response(&id, r)),
                Err(e) => Some(make_response(
                    &id,
                    json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&e).unwrap_or_default() }],
                        "isError": true
                    }),
                )),
            }
        }

        "shutdown" | "exit" => {
            eprintln!("[mcp] shutdown requested");
            None
        }

        _ => Some(make_error_response(
            &id,
            -32601,
            &format!("Method not found: {method}"),
        )),
    }
}

// ============================================================
// Server Main Loop
// ============================================================

pub fn run_mcp_server() -> Result<(), String> {
    eprintln!("[mcp] CodeLattice MCP v0 server starting on stdin/stdout");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = stdin
            .lock()
            .read_line(&mut line)
            .map_err(|e| format!("stdin read error: {e}"))?;

        if bytes_read == 0 {
            // EOF — client disconnected
            eprintln!("[mcp] stdin EOF, shutting down");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[mcp] JSON parse error: {e}");
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&error_response).unwrap_or_default()
                );
                let _ = stdout.flush();
                continue;
            }
        };

        eprintln!("[mcp] -> {}", request["method"].as_str().unwrap_or("?"));

        if let Some(response) = handle_request(&request) {
            let response_str = serde_json::to_string(&response).unwrap_or_default();
            let _ = writeln!(stdout, "{}", response_str);
            let _ = stdout.flush();
        }

        // Check for shutdown
        if request["method"].as_str() == Some("shutdown")
            || request["method"].as_str() == Some("exit")
        {
            break;
        }
    }

    eprintln!("[mcp] server stopped");
    Ok(())
}
