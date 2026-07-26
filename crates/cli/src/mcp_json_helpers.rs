//! MCP JSON output / error helpers
//!
//! 来源：mcp_server.rs 原 lines 1334-1491（2026-07-26 行为等价提取，Wave 1 第一刀）。
//! 与 stdlib_tables / calls_index 提取遵循相同 playbook：纯搬运，保留原 doc 注释，
//! 所有项可见性统一为 `pub(crate)`，供 mcp_server.rs 内部消费，不对外暴露。
//!
//! 本模块收录 MCP tool 结果包装与错误构造的纯函数：
//! - `mcp_error` / `mcp_error_detail` / `mcp_error_with_hint`：统一错误 JSON 结构。
//! - `tool_result` / `tool_result_cached` / `tool_error`：把 data 包装成 MCP
//!   `content` envelope，可选注入 cache hit/miss 信号。
//! - `inject_cache_meta` / `merge_cache_and_result`：向 tool result 注入缓存元数据。
//! - `read_source_snippet`：按符号行号区间读取源码片段（带上下文窗口与 50 行上限）。
//!
//! 这些函数仅依赖 `serde_json::Value` / `json!` 宏与文件系统，不依赖 GraphView /
//! McpCache 等枢纽类型。

use serde_json::{json, Value};

// ============================================================
// Error helpers
// ============================================================

/// Unified error structure with code, message, details, and hint.
pub(crate) fn mcp_error(code: &str, message: &str) -> Value {
    json!({
        "error": code,
        "message": message
    })
}

pub(crate) fn mcp_error_detail(code: &str, message: &str, details: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details
    })
}

pub(crate) fn mcp_error_with_hint(code: &str, message: &str, details: &str, hint: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details,
        "hint": hint
    })
}

#[allow(dead_code)]
pub(crate) fn tool_error(code: &str, message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&mcp_error(code, message)).unwrap_or_default() }],
        "isError": true
    })
}

pub(crate) fn tool_result(data: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(data).unwrap_or_default() }]
    })
}

/// Like tool_result but injects cache hit/miss signal.
#[allow(dead_code)]
pub(crate) fn tool_result_cached(data: &Value, cache_hit: bool, duration_ms: u64) -> Value {
    let mut enriched = data.clone();
    inject_cache_meta(&mut enriched, cache_hit, duration_ms);
    tool_result(&enriched)
}

/// Helper: inject cache hit/miss signal into a tool result Value.
pub(crate) fn inject_cache_meta(data: &mut Value, cache_hit: bool, duration_ms: u64) {
    if let Some(obj) = data.as_object_mut() {
        obj.insert("cacheHit".to_string(), json!(cache_hit));
        if !cache_hit {
            obj.insert("analysisDurationMs".to_string(), json!(duration_ms));
        }
    }
}

/// Merge cache_meta into a json output and wrap in tool_result.
pub(crate) fn merge_cache_and_result(data: &Value, cache_meta: &Value) -> Value {
    let mut enriched = data.clone();
    if let (Some(obj), Some(meta)) = (enriched.as_object_mut(), cache_meta.as_object()) {
        for (k, v) in meta {
            obj.insert(k.clone(), v.clone());
        }
    }
    tool_result(&enriched)
}

/// Read a source code snippet from a file relative to root.
/// Returns a JSON object with `lines`, `startLine`, `endLine`, and optional `warning`.
/// Context lines: number of lines before/after the symbol.
/// Max snippet size: 50 lines to avoid huge outputs.
pub(crate) fn read_source_snippet(
    root: &str,
    relative_path: &str,
    symbol_start: u64,
    symbol_end: u64,
    context_lines: usize,
) -> Value {
    let max_lines = 50usize;
    let ctx = context_lines.min(10); // cap context at 10 lines each side

    let full_path = std::path::Path::new(root).join(relative_path);

    if !full_path.exists() {
        return json!({
            "warning": format!("File not found: {}", relative_path),
            "lines": Value::Null,
            "startLine": Value::Null,
            "endLine": Value::Null
        });
    }

    let content = match std::fs::read_to_string(&full_path) {
        Ok(s) => s,
        Err(e) => {
            return json!({
                "warning": format!("Cannot read file {}: {}", relative_path, e),
                "lines": Value::Null,
                "startLine": Value::Null,
                "endLine": Value::Null
            });
        }
    };

    let file_lines: Vec<&str> = content.lines().collect();
    let total_lines = file_lines.len();

    if total_lines == 0 {
        return json!({
            "warning": "Empty file",
            "lines": "",
            "startLine": 1,
            "endLine": 1
        });
    }

    // Convert 1-based to 0-based, with bounds checking
    let sym_start = if symbol_start > 0 {
        (symbol_start as usize).saturating_sub(1)
    } else {
        0
    };
    let sym_end = if symbol_end > 0 {
        (symbol_end as usize).saturating_sub(1)
    } else {
        sym_start
    };

    // Add context, clamped to file bounds
    let snippet_start = sym_start.saturating_sub(ctx);
    let snippet_end = (sym_end + ctx + 1).min(total_lines); // +1 because end is inclusive

    // Enforce max_lines
    let snippet_end = if snippet_end - snippet_start > max_lines {
        snippet_start + max_lines
    } else {
        snippet_end
    };
    let snippet_end = snippet_end.min(total_lines);

    let snippet_lines: Vec<&str> = file_lines[snippet_start..snippet_end].to_vec();

    json!({
        "lines": snippet_lines.join("\n"),
        "startLine": snippet_start + 1, // back to 1-based
        "endLine": snippet_end,
        "totalLines": total_lines
    })
}
