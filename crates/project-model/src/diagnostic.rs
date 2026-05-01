//! Diagnostic 类型
//!
//! 覆盖 CLI/output contract 冻结的 diagnostic codes。
//! Diagnostics 是 no-edge / absence 策略的核心表达方式：
//! - 当无法确定 ownership/resolution 时，通过 diagnostic 解释原因
//! - 当已知局限触发时，通过 diagnostic 记录
//! - 不允许默默跳过或产出 fake edges

use serde::Serialize;

/// Diagnostic 条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// enum-like string，有限集合，不是自然语言
    pub code: String,
    /// "error" / "warning" / "info"
    pub severity: String,
    /// 人类可读描述
    pub message: String,
    /// 受影响路径
    pub path: String,
    /// 关联置信度（如有）
    pub confidence: Option<f32>,
    /// reason code（如有）
    pub reason: Option<String>,
    /// 相关路径
    pub related_paths: Vec<String>,
    /// 修复建议
    pub suggested_action: Option<String>,
}

/// diagnostic codes 冻结集合
pub mod codes {
    pub const CARGO_ROOT_MISSING: &str = "cargo-root-missing";
    pub const CARGO_ROOT_AMBIGUOUS: &str = "cargo-root-ambiguous";
    pub const WORKSPACE_MEMBER_AMBIGUOUS: &str = "workspace-member-ambiguous";
    pub const VIRTUAL_WORKSPACE_ROOT_NOT_CRATE_ROOT: &str = "virtual-workspace-root-not-crate-root";
    pub const NESTED_PACKAGE_INSIDE_WORKSPACE_MEMBER: &str =
        "nested-package-inside-workspace-member";
    pub const NONSTANDARD_BIN_PATH_UNSUPPORTED: &str = "nonstandard-bin-path-unsupported";
    pub const COMPLEX_GLOB_UNSUPPORTED: &str = "complex-glob-unsupported";
    pub const PARTIAL_INDEXING: &str = "partial-indexing";
    pub const SCAN_NOT_IMPLEMENTED: &str = "project-model-scan-not-implemented";

    // 第一刀 manifest scanner 新增 diagnostic codes
    pub const CARGO_TOML_MISSING: &str = "cargo-toml-missing";
    pub const CARGO_TOML_PARSE_ERROR: &str = "cargo-toml-parse-error";
    pub const PACKAGE_NAME_MISSING: &str = "package-name-missing";
    pub const WORKSPACE_MEMBERS_INVALID: &str = "workspace-members-invalid";
    pub const WORKSPACE_MEMBER_PATH_MISSING: &str = "workspace-member-path-missing";
    pub const TARGET_ROOT_MISSING: &str = "target-root-missing";

    // 第二刀 source ownership 新增 diagnostic codes
    pub const SOURCE_OUTSIDE_PACKAGE: &str = "source-outside-package";
    pub const SOURCE_TARGET_AMBIGUOUS: &str = "source-target-ambiguous";
    pub const SOURCE_TARGET_MISSING: &str = "source-target-missing";
    pub const SOURCE_SCAN_SKIPPED: &str = "source-scan-skipped";

    // 第三刀 rootResolution 新增 diagnostic codes
    pub const CRATE_ROOT_MISSING: &str = "crate-root-missing";
    pub const CRATE_PATH_UNRESOLVED: &str = "crate-path-unresolved";
    pub const CRATE_PATH_AMBIGUOUS: &str = "crate-path-ambiguous";
    pub const MODULE_NOT_DECLARED: &str = "module-not-declared";
    pub const MODULE_FILE_MISSING: &str = "module-file-missing";
    pub const CFG_GATED_MODULE_UNKNOWN: &str = "cfg-gated-module-unknown";
    pub const PATH_ATTRIBUTE_UNSUPPORTED: &str = "path-attribute-unsupported";
    pub const ROOT_RESOLUTION_SKIPPED: &str = "root-resolution-skipped";
}
