//! Tool Dispatcher —— 只读工具 schema、分级取证预算、结果截断（P0 §5.3）。
//!
//! 默认预算冻结（§5.3）：
//! - Level 1: project_summary / search_nodes，3 次 × 8 KiB
//! - Level 2: get_node_context / get_edge_evidence，5 次 × 16 KiB
//! - Level 3: get_call_chain / get_source_excerpt，4 次 × 32 KiB / 50 行
//! - 会话总计 12 次调用，maxEvidenceTokens=16,384
//! 预算由 Gateway 强制执行；模型不能自行放宽。超限结构化截断并标记 truncated。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolLevel {
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolBudget {
    pub level: ToolLevel,
    pub max_calls: u32,
    pub max_return_bytes: u64,
    pub max_source_lines: Option<u32>,
}

pub const DEFAULT_BUDGETS: [ToolBudget; 3] = [
    ToolBudget {
        level: ToolLevel::Level1,
        max_calls: 3,
        max_return_bytes: 8 * 1024,
        max_source_lines: None,
    },
    ToolBudget {
        level: ToolLevel::Level2,
        max_calls: 5,
        max_return_bytes: 16 * 1024,
        max_source_lines: None,
    },
    ToolBudget {
        level: ToolLevel::Level3,
        max_calls: 4,
        max_return_bytes: 32 * 1024,
        max_source_lines: Some(50),
    },
];

pub const SESSION_MAX_CALLS: u32 = 12;
pub const SESSION_MAX_EVIDENCE_TOKENS: u64 = 16_384;

/// 只读工具白名单（§6.4）：模型只能通过这里访问事实。
pub const READ_ONLY_TOOLS: &[&str] = &[
    "project_summary",
    "search_nodes",
    "get_node_context",
    "get_edge_evidence",
    "get_call_chain",
    "get_source_excerpt",
    "get_static_limitations",
    "get_change_impact", // 必须携带明确 what-if
];

fn level_of(tool: &str) -> Option<ToolLevel> {
    match tool {
        "project_summary" | "search_nodes" => Some(ToolLevel::Level1),
        "get_node_context" | "get_edge_evidence" => Some(ToolLevel::Level2),
        "get_call_chain" | "get_source_excerpt" => Some(ToolLevel::Level3),
        "get_static_limitations" | "get_change_impact" => Some(ToolLevel::Level2),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    UnknownTool,
    LevelBudgetExhausted,
    SessionBudgetExhausted,
    NotReadOnly,
}

/// 逐级取证预算（每次回答会话内跟踪）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BudgetTracker {
    pub level1_calls: u32,
    pub level2_calls: u32,
    pub level3_calls: u32,
    pub session_calls: u32,
    pub evidence_tokens: u64,
}

impl BudgetTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 校验并登记一次工具调用；失败时返回 BudgetError，不消耗配额。
    pub fn consume(&mut self, tool: &str) -> Result<(), BudgetError> {
        if !READ_ONLY_TOOLS.contains(&tool) {
            return Err(BudgetError::NotReadOnly);
        }
        let level = level_of(tool).ok_or(BudgetError::UnknownTool)?;
        if self.session_calls >= SESSION_MAX_CALLS {
            return Err(BudgetError::SessionBudgetExhausted);
        }
        let budget = DEFAULT_BUDGETS
            .iter()
            .find(|b| b.level == level)
            .expect("budget table complete");
        let used = match level {
            ToolLevel::Level1 => &mut self.level1_calls,
            ToolLevel::Level2 => &mut self.level2_calls,
            ToolLevel::Level3 => &mut self.level3_calls,
        };
        if *used >= budget.max_calls {
            return Err(BudgetError::LevelBudgetExhausted);
        }
        *used += 1;
        self.session_calls += 1;
        Ok(())
    }

    pub fn remaining_calls(&self) -> u32 {
        SESSION_MAX_CALLS.saturating_sub(self.session_calls)
    }

    /// 追加证据 token 并检查总预算；超限返回 true（调用方应停止取证）。
    pub fn add_evidence_tokens(&mut self, tokens: u64) -> bool {
        self.evidence_tokens = self.evidence_tokens.saturating_add(tokens);
        self.evidence_tokens > SESSION_MAX_EVIDENCE_TOKENS
    }

    pub fn exhausted(&self) -> bool {
        self.session_calls >= SESSION_MAX_CALLS
            || self.evidence_tokens > SESSION_MAX_EVIDENCE_TOKENS
    }
}

/// 结构化截断：超限结果按字节截断并标记 truncated=true，不静默丢弃。
pub fn truncate_bytes(input: &[u8], max: u64) -> (Vec<u8>, bool) {
    if input.len() as u64 <= max {
        return (input.to_vec(), false);
    }
    let keep = max as usize;
    let mut out = input[..keep].to_vec();
    out.extend_from_slice(b"\n... [truncated]");
    (out, true)
}

/// 源码摘录按行截断（L3 maxSourceLines=50）。
pub fn truncate_lines(text: &str, max_lines: u32) -> (String, bool) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() as u32 <= max_lines {
        return (text.to_string(), false);
    }
    let kept = lines[..max_lines as usize].join("\n");
    (
        format!("{kept}\n... [truncated at {max_lines} lines]"),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_frozen_values() {
        // §5.3 冻结：L1 3×8KiB、L2 5×16KiB、L3 4×32KiB/50 行、总 12、16K tokens
        assert_eq!(DEFAULT_BUDGETS[0].max_calls, 3);
        assert_eq!(DEFAULT_BUDGETS[0].max_return_bytes, 8 * 1024);
        assert_eq!(DEFAULT_BUDGETS[1].max_calls, 5);
        assert_eq!(DEFAULT_BUDGETS[1].max_return_bytes, 16 * 1024);
        assert_eq!(DEFAULT_BUDGETS[2].max_calls, 4);
        assert_eq!(DEFAULT_BUDGETS[2].max_return_bytes, 32 * 1024);
        assert_eq!(DEFAULT_BUDGETS[2].max_source_lines, Some(50));
        assert_eq!(SESSION_MAX_CALLS, 12);
        assert_eq!(SESSION_MAX_EVIDENCE_TOKENS, 16_384);
    }

    #[test]
    fn level_budget_exhausts_and_recovers() {
        let mut t = BudgetTracker::new();
        for i in 0..3 {
            assert!(t.consume("project_summary").is_ok(), "call {i} ok");
        }
        assert_eq!(
            t.consume("project_summary"),
            Err(BudgetError::LevelBudgetExhausted)
        );
        // L2 独立配额不受 L1 影响
        assert!(t.consume("get_node_context").is_ok());
    }

    #[test]
    fn session_total_budget_is_enforced() {
        let mut t = BudgetTracker::new();
        // 12 次 L1 调用不可能（L1 上限 3）；交替层级耗尽会话总数
        // 每层正好用满：3 L1 + 5 L2 + 4 L3 = 12 次会话总调用
        let tools = [
            "project_summary",
            "search_nodes",
            "project_summary",
            "get_node_context",
            "get_edge_evidence",
            "get_node_context",
            "get_edge_evidence",
            "get_static_limitations",
            "get_call_chain",
            "get_source_excerpt",
            "get_call_chain",
            "get_source_excerpt",
        ];
        for (i, tool) in tools.iter().enumerate() {
            assert!(t.consume(tool).is_ok(), "call {i} ({tool}) ok");
        }
        assert_eq!(
            t.consume("project_summary"),
            Err(BudgetError::SessionBudgetExhausted)
        );
        assert_eq!(t.remaining_calls(), 0);
    }

    #[test]
    fn non_readonly_tool_is_rejected() {
        let mut t = BudgetTracker::new();
        assert_eq!(t.consume("shell_exec"), Err(BudgetError::NotReadOnly));
        assert_eq!(t.session_calls, 0);
    }

    #[test]
    fn evidence_token_budget_halts_further_mining() {
        let mut t = BudgetTracker::new();
        let over = t.add_evidence_tokens(10_000);
        assert!(!over);
        let over = t.add_evidence_tokens(10_000);
        assert!(over, "超过 16,384 后必须停止取证");
        assert!(t.exhausted());
    }

    #[test]
    fn truncate_marks_truncated_and_never_silently_drops() {
        let (out, truncated) = truncate_bytes(b"1234567890", 5);
        assert!(truncated);
        assert!(out.len() <= 5 + 20);
        assert!(out.ends_with(b"[truncated]"));

        let (lines, truncated) = truncate_lines("a\nb\nc\nd", 3);
        assert!(truncated);
        assert!(lines.contains("[truncated at 3 lines]"));
        assert!(!lines.contains("d\n"));
    }
}
