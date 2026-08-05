//! Output Validator —— claim schema、evidenceRef、navigationAction 白名单
//! 与 identifier 词表检查（P0 §6.3 / 验收 11–13）。
//!
//! 结构性校验只能证明引用 ID 存在；自然语言与证据的语义蕴含不在 P0 范围。
//! 规则：
//! - 有有效 evidenceRef 的模型断言 → grounded_interpretation
//! - 无引用或引用不完整 → 强制 hypothesis
//! - 模型明确无法回答 → unknown
//! - 声明的代码 identifier 不在证据词表 → 降级为 hypothesis（验收 13）

use crate::dto::{Claim, ClaimClassification, NavigationAction};

#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// 允许的 evidence ref 前缀（如 "rel:", "src:", "limit:", "coverage:"）
    pub allowed_ref_prefixes: Vec<&'static str>,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            allowed_ref_prefixes: vec!["rel:", "src:", "limit:", "coverage:", "occ:"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    UnknownEvidenceRef(String),
    UnknownNavigationTarget(String),
    MissingEvidenceForGrounded(String),
    DowngradedToHypothesis(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
    pub downgraded_claims: Vec<String>,
}

/// 词表检查：模型以反引号 / symbol chip 声称存在的标识必须出现在证据词表。
/// 普通自然语言不做强词法约束（避免同义表达误判为幻觉）。
/// 只提取成对反引号之间的内容；无反引号的文本返回空（不误判中文）。
pub fn extract_backticked_identifiers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !text.contains('`') {
        return out;
    }
    let parts: Vec<&str> = text.split('`').collect();
    // 反引号成对出现：下标 1, 3, 5... 是括号内内容
    for chunk in parts.iter().skip(1).step_by(2) {
        let trimmed = chunk.trim();
        if trimmed.len() >= 2 && !trimmed.contains(' ') {
            out.push(trimmed.to_string());
        }
    }
    out
}

pub struct OutputValidator {
    config: ValidatorConfig,
}

impl OutputValidator {
    pub fn new(config: ValidatorConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(ValidatorConfig::default())
    }

    /// 校验证据引用集合；返回可解析的 ref 与问题列表。
    fn resolve_refs<'a>(
        &self,
        refs: &'a [String],
        valid_refs: &'a [String],
    ) -> (Vec<&'a str>, Vec<ValidationIssue>) {
        let mut ok = Vec::new();
        let mut issues = Vec::new();
        for r in refs {
            if valid_refs.contains(r) {
                ok.push(r.as_str());
            } else if self
                .config
                .allowed_ref_prefixes
                .iter()
                .any(|p| r.starts_with(p))
            {
                // 前缀合法但不在当前证据包中 → 不完整引用
                issues.push(ValidationIssue::UnknownEvidenceRef(r.clone()));
            } else {
                issues.push(ValidationIssue::UnknownEvidenceRef(r.clone()));
            }
        }
        (ok, issues)
    }

    /// 校验并归一化 claim（验收 12/13）。
    pub fn validate_claim(
        &self,
        claim: Claim,
        valid_refs: &[String],
        identifier_vocabulary: &[String],
    ) -> (Claim, ValidationReport) {
        let mut report = ValidationReport::default();
        let mut claim = claim;

        let (_ok, issues) = self.resolve_refs(&claim.evidence_refs, valid_refs);
        report.issues.extend(issues);

        // 分类规则（验收 11）：
        match claim.classification {
            ClaimClassification::GroundedInterpretation => {
                if claim.evidence_refs.is_empty() {
                    report
                        .issues
                        .push(ValidationIssue::MissingEvidenceForGrounded(
                            claim.id.clone(),
                        ));
                    claim.classification = ClaimClassification::Hypothesis;
                    report.downgraded_claims.push(claim.id.clone());
                }
            }
            ClaimClassification::Hypothesis | ClaimClassification::Unknown => {}
        }

        // identifier 词表检查（验收 13）
        let identifiers = extract_backticked_identifiers(&claim.text);
        for id in identifiers {
            if !identifier_vocabulary.contains(&id) {
                if claim.classification == ClaimClassification::GroundedInterpretation {
                    claim.classification = ClaimClassification::Hypothesis;
                    report.downgraded_claims.push(claim.id.clone());
                }
                report
                    .issues
                    .push(ValidationIssue::DowngradedToHypothesis(id));
            }
        }

        (claim, report)
    }

    /// navigationAction 白名单校验（验收 10/12）：目标必须可解析到证据词表。
    pub fn validate_navigation(
        &self,
        action: &NavigationAction,
        valid_node_ids: &[String],
        valid_relation_keys: &[String],
    ) -> Result<(), ValidationIssue> {
        match action {
            NavigationAction::FocusNode { node_id, .. } => {
                if valid_node_ids.contains(node_id) {
                    Ok(())
                } else {
                    Err(ValidationIssue::UnknownNavigationTarget(node_id.clone()))
                }
            }
            NavigationAction::FocusRelation { relation_key, .. } => {
                if valid_relation_keys.contains(relation_key) {
                    Ok(())
                } else {
                    Err(ValidationIssue::UnknownNavigationTarget(
                        relation_key.clone(),
                    ))
                }
            }
            NavigationAction::FocusSource { .. } => Ok(()), // sourceRef 由 Inspector 单独处理
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn grounded_claim_without_evidence_is_downgraded() {
        let v = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:1".into(),
            text: "该函数处理输入".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: vec![],
            coverage_caveat_refs: vec![],
        };
        let (out, report) = v.validate_claim(claim, &[], &[]);
        assert_eq!(out.classification, ClaimClassification::Hypothesis);
        assert_eq!(report.downgraded_claims, vec!["claim:1"]);
    }

    #[test]
    fn grounded_claim_with_valid_refs_stays_grounded() {
        let v = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:2".into(),
            text: "调用链证据".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: refs(&["rel:abc"]),
            coverage_caveat_refs: vec![],
        };
        let (out, report) = v.validate_claim(claim, &refs(&["rel:abc"]), &[]);
        assert_eq!(
            out.classification,
            ClaimClassification::GroundedInterpretation
        );
        assert!(report.downgraded_claims.is_empty());
    }

    #[test]
    fn unknown_evidence_ref_is_reported() {
        let v = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:3".into(),
            text: "x".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: refs(&["rel:ghost"]),
            coverage_caveat_refs: vec![],
        };
        let (_out, report) = v.validate_claim(claim, &refs(&["rel:abc"]), &[]);
        assert!(report
            .issues
            .iter()
            .any(|i| matches!(i, ValidationIssue::UnknownEvidenceRef(r) if r == "rel:ghost")));
    }

    #[test]
    fn backticked_identifier_not_in_vocabulary_downgrades_claim() {
        let v = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:4".into(),
            text: "调用了 `Calculator::new` 完成初始化".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: refs(&["rel:abc"]),
            coverage_caveat_refs: vec![],
        };
        let (out, report) =
            v.validate_claim(claim, &refs(&["rel:abc"]), &["Calculator".to_string()]);
        assert_eq!(
            out.classification,
            ClaimClassification::Hypothesis,
            "标识不在词表必须降级"
        );
        assert!(report.downgraded_claims.contains(&"claim:4".to_string()));
    }

    #[test]
    fn identifier_in_vocabulary_keeps_classification() {
        let v = OutputValidator::with_defaults();
        let claim = Claim {
            id: "claim:5".into(),
            text: "调用了 `Calculator::new`".into(),
            classification: ClaimClassification::GroundedInterpretation,
            evidence_refs: refs(&["rel:abc"]),
            coverage_caveat_refs: vec![],
        };
        let (out, _) =
            v.validate_claim(claim, &refs(&["rel:abc"]), &["Calculator::new".to_string()]);
        assert_eq!(
            out.classification,
            ClaimClassification::GroundedInterpretation
        );
    }

    #[test]
    fn navigation_to_unknown_target_is_rejected() {
        let v = OutputValidator::with_defaults();
        let action = NavigationAction::FocusNode {
            node_id: "n:ghost".into(),
            snapshot_id: "snap:1".into(),
        };
        assert!(v
            .validate_navigation(&action, &["n:a".to_string()], &[])
            .is_err());
        let action2 = NavigationAction::FocusRelation {
            relation_key: "rel:abc".into(),
            snapshot_id: "snap:1".into(),
            occurrence_key: None,
        };
        assert!(v
            .validate_navigation(&action2, &[], &["rel:abc".to_string()])
            .is_ok());
    }
}
