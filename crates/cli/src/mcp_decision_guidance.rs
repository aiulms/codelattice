//! Facade decision guidance & mode semantics
//!
//! 来源：mcp_server.rs 原 lines 577-736（2026-07-26 行为等价提取，Wave 1 第三刀）。
//! 纯 `&Value`/`&str` 函数，不依赖 GraphView / McpCache 等枢纽类型。
//!
//! 职责：为 AI facade 响应构造 decisionGuidance 字段——根据 tool / mode / root kind
//! 给出工具角色、模式语义、下一步推荐工具、compact 策略说明，帮助 AI 在 6 个 facade
//! 入口之间正确路由。内部强内聚：decision_guidance 调用 recommended_next_tool /
//! tool_role / mode_semantics / compact_semantics。

use serde_json::{json, Value};

/// 按 (tool, mode) 返回该组合的 does / doesNot / stopWhen / goDeeperWith 语义说明。
pub(crate) fn mode_semantics(tool: &str, mode: &str) -> Value {
    match (tool, mode) {
        ("codelattice_project", "quick") => json!({
            "mode": "quick",
            "does": "Fast static orientation: entry candidates, read-first files, top components, and ranked risk hints.",
            "doesNot": "Does not return full graph/file metrics, execute target code, run tests, or prove coverage.",
            "stopWhen": "You only need to know where to start reading or which project area is probably risky.",
            "goDeeperWith": "codelattice_project mode=standard"
        }),
        ("codelattice_project", "standard") => json!({
            "mode": "standard",
            "does": "Balanced project map with component risk, review-first areas, and enough static detail for planning edits.",
            "doesNot": "Does not include every detailed metric or runtime/coverage proof.",
            "stopWhen": "You can identify the module and symbols to inspect before editing.",
            "goDeeperWith": "codelattice_project mode=deep"
        }),
        ("codelattice_project", "deep") => json!({
            "mode": "deep",
            "does": "Detailed static evidence for high-risk edits and manual review.",
            "doesNot": "Still does not execute the target project or prove runtime behavior.",
            "stopWhen": "You have enough static evidence and should switch to targeted tests or source edits.",
            "goDeeperWith": "codelattice_change_review mode=impact for a concrete target"
        }),
        ("codelattice_project", "overview") => json!({
            "mode": "overview",
            "does": "Basic project/root overview and root diagnosis.",
            "doesNot": "Does not provide the progressive read-first/risk map from quick/standard/deep.",
            "goDeeperWith": "codelattice_project mode=quick"
        }),
        ("codelattice_change_review", "impact") => json!({
            "mode": "impact",
            "does": "Concrete pre-edit impact review for a known symbol or change target.",
            "doesNot": "Does not replace project orientation; use workflow/project first when the target is unclear.",
            "goDeeperWith": "codelattice_symbol mode=context or callers/callees"
        }),
        ("codelattice_workflow", "before_edit") => json!({
            "mode": "before_edit",
            "does": "Intent-level router for pre-edit safety when an AI is unsure which concrete review calls to make.",
            "doesNot": "Does not replace codelattice_change_review once the target is concrete.",
            "goDeeperWith": "codelattice_change_review mode=impact"
        }),
        _ => json!({
            "mode": mode,
            "does": "Runs the requested CodeLattice facade mode using static analysis only.",
            "doesNot": "Does not execute target code, run tests, or provide coverage proof.",
            "goDeeperWith": "Use nextActions or recommendedNextCalls from this response."
        }),
    }
}

pub(crate) fn tool_role(tool: &str) -> &'static str {
    match tool {
        "codelattice_workflow" => "intent router and workflow orchestration",
        "codelattice_project" => "single-project structure and risk map",
        "codelattice_symbol" => "symbol search, context, and call relationships",
        "codelattice_change_review" => "concrete pre/post edit risk review",
        "codelattice_workspace" => "workspace and cross-project boundary analysis",
        "codelattice_cache" => "optional cache status and management",
        _ => "CodeLattice static analysis facade",
    }
}

pub(crate) fn recommended_next_tool(tool: &str, mode: &str, root_kind: &str) -> &'static str {
    if root_kind == "workspace" || root_kind == "protected_live_workspace" {
        return "codelattice_workspace";
    }
    match (tool, mode) {
        ("codelattice_workflow", _) => "follow returned recommendedNextCalls",
        ("codelattice_project", "quick") => "codelattice_project mode=standard",
        ("codelattice_project", "standard") => "codelattice_symbol or codelattice_change_review",
        ("codelattice_project", "deep") => "codelattice_change_review",
        ("codelattice_project", _) => "codelattice_project mode=quick",
        ("codelattice_symbol", _) => "codelattice_change_review",
        ("codelattice_change_review", _) => "targeted tests or source review",
        ("codelattice_workspace", _) => "codelattice_project",
        _ => "codelattice_workflow",
    }
}

pub(crate) fn compact_semantics(compact: bool) -> Value {
    if compact {
        json!({
            "enabled": true,
            "kept": ["summary", "bounded result evidence", "rootDiagnosis or rootDiagnosisSummary/rootDiagnosisRef", "decisionGuidance", "nextActions", "analysisSemantics"],
            "omitted": ["full rootDiagnosis in high-frequency facades", "large arrays", "full sourceOnlyEntries", "sourceSnippet unless includeSnippet=true"],
            "lossPolicy": "Compact keeps decision-critical routing/risk hints and bounded evidence; repeated root/project diagnostics and bulky snippets are omitted.",
            "detailAvailableVia": "Re-run with compact=false, a deeper mode, or job_detail when the response provides a jobId."
        })
    } else {
        json!({
            "enabled": false,
            "kept": ["summary", "result", "rootDiagnosis", "decisionGuidance", "nextActions", "analysisSemantics"],
            "omitted": [],
            "lossPolicy": "Full mode keeps detailed static evidence but can be large."
        })
    }
}

pub(crate) fn compact_root_diagnosis_summary(root_diagnosis: &Value) -> Value {
    json!({
        "kind": root_diagnosis.get("kind").cloned().unwrap_or_else(|| json!("unknown")),
        "canonicalRoot": root_diagnosis.get("canonicalRoot").cloned().unwrap_or(Value::Null),
        "recommendedTool": root_diagnosis.get("recommendedTool").cloned().unwrap_or(Value::Null),
        "counts": root_diagnosis.get("counts").cloned().unwrap_or_else(|| json!({})),
        "detectedProjectSummary": root_diagnosis.get("detectedProjectSummary").cloned().unwrap_or(Value::Null),
        "sourceOnlySummary": root_diagnosis.get("sourceOnlySummary").cloned().unwrap_or(Value::Null),
        "detailHint": "Full rootDiagnosis is omitted in compact high-frequency facade responses. Re-run with compact=false or codelattice_project/workspace if you need project-root diagnostics."
    })
}

pub(crate) fn compact_should_omit_full_root_diagnosis(tool: &str) -> bool {
    matches!(
        tool,
        "codelattice_symbol"
            | "codelattice_change_review"
            | "codelattice_workflow"
            | "codelattice_cleanup"
    )
}

pub(crate) fn compact_should_omit_full_root_diagnosis_for(tool: &str, mode: &str) -> bool {
    compact_should_omit_full_root_diagnosis(tool)
        || (tool == "codelattice_project" && mode == "diagnose")
}

pub(crate) fn decision_guidance(
    tool: &str,
    mode: &str,
    root_diagnosis: &Value,
    compact: bool,
) -> Value {
    let root_kind = root_diagnosis["kind"].as_str().unwrap_or("not_applicable");
    let recommended_next = recommended_next_tool(tool, mode, root_kind);
    let tool_boundary = if compact {
        json!([
            "workflow=router",
            "project=orientation/risk",
            "symbol=name/calls",
            "change_review=concrete edit",
            "workspace=monorepo"
        ])
    } else {
        json!({
            "workflow": "Use codelattice_workflow when intent is unclear or you want orchestration.",
            "project": "Use codelattice_project for project structure, entry points, components, and risk orientation.",
            "symbol": "Use codelattice_symbol when you know a symbol/name and need context or call relationships.",
            "changeReview": "Use codelattice_change_review when you have a concrete edit target or changed symbols.",
            "workspace": "Use codelattice_workspace for monorepos, project boundaries, and cross-project impact."
        })
    };
    json!({
        "toolRole": tool_role(tool),
        "modeSemantics": mode_semantics(tool, mode),
        "rootKind": root_kind,
        "rootUseGuidance": match root_kind {
            "workspace" | "protected_live_workspace" => "Use workspace-level tools first, then choose a manifest-backed project root before symbol/project review.",
            "single_project" => "Safe to use project, symbol, and change_review facades against this root.",
            "unsupported_or_mixed_workspace" => "Use workspace graph and pick supported project roots; unsupported entries are reported but not analyzed.",
            _ => "Root classification is uncertain; prefer codelattice_workflow mode=explore or codelattice_workspace mode=graph."
        },
        "recommendedNextTool": recommended_next,
        "toolBoundary": tool_boundary,
        "compactSemantics": compact_semantics(compact)
    })
}
