//! Diagnose term extraction helpers
//!
//! 来源：mcp_server.rs 原 lines 921-994（2026-07-26 行为等价提取，Wave 1 第二刀）。
//! 纯文本处理，不依赖 GraphView / McpCache 等枢纽类型。
//!
//! 职责：从用户输入文本（symptom / errorText / query 等）中抽取用于 diagnose
//! 检索的关键词，过滤停用词与过短 token。

use serde_json::Value;

/// 停用词判定：diagnose 检索时忽略的高频无信息量词汇。
pub(crate) fn diagnose_stop_word(term: &str) -> bool {
    matches!(
        term,
        "the"
            | "and"
            | "for"
            | "with"
            | "from"
            | "that"
            | "this"
            | "when"
            | "where"
            | "before"
            | "after"
            | "into"
            | "returns"
            | "wrong"
            | "result"
            | "error"
            | "failed"
            | "fails"
            | "failure"
            | "issue"
            | "problem"
            | "bug"
            | "panic"
            | "null"
            | "none"
            | "undefined"
    )
}

/// 从文本中抽取候选检索词：按非字母数字字符切分，小写化，过滤停用词与长度 <3 的 token，去重。
pub(crate) fn diagnose_terms_from_text(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            if current.len() >= 3 && !diagnose_stop_word(&current) && !terms.contains(&current) {
                terms.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty()
        && current.len() >= 3
        && !diagnose_stop_word(&current)
        && !terms.contains(&current)
    {
        terms.push(current);
    }

    terms
}

/// 从 MCP 工具参数中提取 diagnose 输入文本：按优先级依次尝试多个参数键，
/// 拼接非空值。键的顺序即优先级（symptom 最高，issue 最低）。
pub(crate) fn diagnose_input_text(params: &Value) -> String {
    [
        "symptom",
        "errorText",
        "query",
        "changedPath",
        "symbol",
        "name",
        "issue",
        "observedError",
    ]
    .iter()
    .filter_map(|key| params[*key].as_str())
    .filter(|s| !s.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}
