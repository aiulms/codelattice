//! Graph node display helpers
//!
//! 来源：mcp_server.rs 原 lines 996-1035（2026-07-26 行为等价提取，Wave 1 第二刀）。
//! 纯 `&Value -> T` 读取函数，不依赖 GraphView / McpCache 等枢纽类型，
//! 仅从 graph node JSON 中按多个字段 fallback 读取显示属性。
//!
//! 这些函数被 ~30 处 handler 调用，用于把 graph node 转成人可读的 name / kind /
//! sourcePath / lineStart，统一处理不同来源 node 的字段命名差异。

use serde_json::Value;

/// 节点显示名：按 properties.name > label > id 末段 fallback。
pub(crate) fn node_display_name(node: &Value) -> String {
    node["properties"]["name"]
        .as_str()
        .or_else(|| node["label"].as_str())
        .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
        .unwrap_or("unknown")
        .to_string()
}

/// 节点 symbol kind：按 properties.symbolKind > properties.kind > kind > label fallback。
pub(crate) fn node_symbol_kind(node: &Value) -> String {
    node["properties"]["symbolKind"]
        .as_str()
        .or_else(|| node["properties"]["kind"].as_str())
        .or_else(|| node["kind"].as_str())
        .or_else(|| node["label"].as_str())
        .unwrap_or("symbol")
        .to_string()
}

/// 节点 source path：按 properties.sourcePath > properties.file fallback。
pub(crate) fn node_source_path(node: &Value) -> String {
    node["properties"]["sourcePath"]
        .as_str()
        .or_else(|| node["properties"]["file"].as_str())
        .unwrap_or("")
        .to_string()
}

/// 节点起始行号：按 properties.lineStart > startLine > line > id 末段数字 fallback。
pub(crate) fn node_line_start(node: &Value) -> u64 {
    node["properties"]["lineStart"]
        .as_u64()
        .or_else(|| node["properties"]["startLine"].as_u64())
        .or_else(|| node["properties"]["line"].as_u64())
        .or_else(|| {
            node["id"]
                .as_str()
                .and_then(|id| id.rsplit(':').next())
                .and_then(|part| part.parse::<u64>().ok())
        })
        .unwrap_or(0)
}
