//! Risk scoring & priority calibration helpers
//!
//! 来源：mcp_server.rs 原 lines 738-921（2026-07-26 行为等价提取，Wave 1 第三刀）。
//! 纯 `f64`/`usize`/`&Value` 函数，不依赖 GraphView / McpCache 等枢纽类型。
//!
//! 职责：把原始静态风险分数校准成人可读的风险等级与优先级标签。包含两层校准：
//! - 原始分数 → rawRiskLevel（high/medium/low，阈值 8.0/3.0）
//! - rank 校准 → rankAdjustedRiskLevel（结合 priorityRank 与总分占比，抑制尾部高原始分
//!   被误判为高风险；这是静态分析的相对风险，不是 runtime proof）
//!
//! 内部强内聚：enrich_risk_item 是主入口，组合 project_risk_level_from_score /
//! rank_adjusted_risk_level / relative_priority / risk_driver_tags /
//! risk_score_interpretation / calibrated_priority_label / calibrated_percentile_band /
//! risk_tie_breaker。

use serde_json::{json, Value};

pub(crate) fn project_risk_level_from_score(score: f64) -> &'static str {
    if score >= 8.0 {
        "high"
    } else if score >= 3.0 {
        "medium"
    } else {
        "low"
    }
}

pub(crate) fn relative_priority(rank: usize, score: f64, top_score: f64) -> &'static str {
    if rank == 1 {
        "top"
    } else if rank <= 3 && top_score > 0.0 && score >= top_score * 0.8 {
        "peer-high"
    } else if score >= 3.0 {
        "elevated"
    } else {
        "baseline"
    }
}

pub(crate) fn calibrated_priority_label(rank: usize, total: usize) -> &'static str {
    if rank == 1 {
        "primary"
    } else if rank <= 3 || (total > 0 && rank * 5 <= total) {
        "secondary"
    } else if rank <= 8 || (total > 0 && rank * 2 <= total) {
        "watch"
    } else {
        "baseline"
    }
}

pub(crate) fn calibrated_percentile_band(rank: usize, total: usize) -> &'static str {
    if total <= 1 || rank == 1 {
        "top"
    } else if rank * 10 <= total {
        "top_10_percent"
    } else if rank * 5 <= total {
        "top_20_percent"
    } else if rank * 2 <= total {
        "top_half"
    } else {
        "baseline_half"
    }
}

pub(crate) fn rank_adjusted_risk_level(
    score: f64,
    rank: usize,
    top_score: f64,
    total: usize,
) -> &'static str {
    if score <= 0.0 {
        return "low";
    }
    let raw = project_risk_level_from_score(score);
    match calibrated_priority_label(rank, total) {
        "primary" => raw,
        "secondary" if raw == "high" && top_score > 0.0 && score >= top_score * 0.65 => "medium",
        "secondary" if raw == "medium" => "medium",
        "watch" if raw == "high" => "medium",
        "watch" if raw == "medium" => "low",
        _ => "low",
    }
}

pub(crate) fn risk_tie_breaker(score: f64, rank: usize, top_score: f64) -> String {
    if rank == 1 {
        "highest ranked static signal in this result set".to_string()
    } else if top_score > 0.0 && (top_score - score).abs() < f64::EPSILON {
        "raw score ties the top item; priorityRank breaks the tie deterministically by graph/file/symbol ordering".to_string()
    } else if top_score > 0.0 && score >= top_score * 0.8 {
        "raw score is close to the top item; inspect after lower priorityRank items".to_string()
    } else {
        "lower raw score or weaker drivers than earlier priorityRank items".to_string()
    }
}

pub(crate) fn risk_driver_tags(reasons: &[String], score: f64, kind: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for reason in reasons {
        let lower = reason.to_lowercase();
        let tag = if lower.contains("fan-in") {
            "fan_in"
        } else if lower.contains("fan-out") {
            "fan_out"
        } else if lower.contains("cross-file") {
            "cross_file_impact"
        } else if lower.contains("low-confidence") {
            "low_confidence"
        } else if lower.contains("diagnostic") {
            "diagnostics"
        } else if lower.contains("generated") || lower.contains("vendor") {
            "generated_or_vendor"
        } else {
            "static_metric"
        };
        if !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_string());
        }
    }
    if tags.is_empty() {
        tags.push(if score > 0.0 {
            "weak_static_signal".to_string()
        } else {
            "navigation_baseline".to_string()
        });
    }
    if !tags.iter().any(|t| t == kind) {
        tags.push(kind.to_string());
    }
    tags
}

pub(crate) fn risk_score_interpretation(
    score: f64,
    rank: usize,
    top_score: f64,
    total: usize,
) -> String {
    let level = rank_adjusted_risk_level(score, rank, top_score, total);
    let relative = relative_priority(rank, score, top_score);
    format!(
        "{level} rank-adjusted static risk, relativePriority={relative}. Compare priorityRank and riskCalibration before comparing equal-looking raw scores; this is not runtime proof."
    )
}

pub(crate) fn enrich_risk_item(
    item: &Value,
    rank: usize,
    top_score: f64,
    total_items: usize,
) -> Value {
    let score = item["riskScore"].as_f64().unwrap_or(0.0);
    let kind = item["kind"].as_str().unwrap_or("item");
    let reasons: Vec<String> = item["reasons"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let raw_level = project_risk_level_from_score(score);
    let rank_adjusted_level = rank_adjusted_risk_level(score, rank, top_score, total_items);
    let mut enriched = item.clone();
    if let Some(obj) = enriched.as_object_mut() {
        obj.insert("rawRiskScore".to_string(), json!(score));
        obj.insert("rawRiskLevel".to_string(), json!(raw_level));
        obj.insert("priorityRank".to_string(), json!(rank));
        obj.insert(
            "relativePriority".to_string(),
            json!(relative_priority(rank, score, top_score)),
        );
        obj.insert("riskLevel".to_string(), json!(rank_adjusted_level));
        obj.insert(
            "rankAdjustedRiskLevel".to_string(),
            json!(rank_adjusted_level),
        );
        obj.insert(
            "riskDrivers".to_string(),
            json!(risk_driver_tags(&reasons, score, kind)),
        );
        obj.insert(
            "riskScoreInterpretation".to_string(),
            json!(risk_score_interpretation(
                score,
                rank,
                top_score,
                total_items
            )),
        );
        obj.insert(
            "riskCalibration".to_string(),
            json!({
                "rawRiskScore": score,
                "rawRiskLevel": raw_level,
                "rankAdjustedRiskLevel": rank_adjusted_level,
                "calibratedRiskLevel": rank_adjusted_level,
                "calibratedPriorityBand": calibrated_priority_label(rank, total_items),
                "percentileBand": calibrated_percentile_band(rank, total_items),
                "tieBreaker": risk_tie_breaker(score, rank, top_score),
                "rankGuidance": "Use priorityRank first, then riskDrivers/rawRiskScore. Equal raw scores are intentionally separated by rank."
            }),
        );
        obj.insert(
            "whyTopRisk".to_string(),
            json!(if reasons.is_empty() {
                "Ranked by the strongest available static signal in this result set.".to_string()
            } else {
                reasons.join("; ")
            }),
        );
    }
    enriched
}
