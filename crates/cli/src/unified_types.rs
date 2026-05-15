//! 统一输出类型定义 — Rust + Cangjie 共用最小 output contract
//!
//! 本文件定义 LanguageAnalysisResult / GraphSummary / QualityGateResult 等类型。
//! 仅作为 CLI 层的包装/聚合，不被 project-model 或 cangjie crate 内部消费。
//! 对齐 docs/architecture/unified-output-contract.md

use serde::Serialize;

// ============================================================
// 图概要统计（语言无关）
// ============================================================

/// 图概要统计 — 从 Rust GraphStats 或 Cangjie node/edge counts 聚合而来
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSummary {
    pub node_count: u32,
    pub edge_count: u32,
    pub symbol_count: u32,
    pub source_file_count: u32,
    pub package_count: u32,
    pub diagnostic_count: u32,
    pub call_edge_count: u32,
}

// ============================================================
// 质量门结果（语言无关）
// ============================================================

/// 单个质量门结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateResult {
    /// 门名称（snake_case identifier），如 "duplicate_nodes" / "dangling_source"
    pub gate_name: String,
    /// 是否通过
    pub passed: bool,
    /// 人类可读的详细说明
    pub detail: String,
}

// ============================================================
// 统一分析结果（顶层 wrapper）
// ============================================================

/// 统一分析结果 — analyze 命令的输出
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageAnalysisResult {
    /// 分析语言标识："rust" 或 "cangjie"
    pub language: String,
    /// 项目根目录绝对路径
    pub root: String,
    /// 分析时间戳 (ISO 8601)
    pub analyzed_at: String,
    /// schema 版本字符串
    pub schema_version: String,
    /// 图概要统计
    pub summary: GraphSummary,
    /// 质量门结果列表
    pub quality_gates: Vec<QualityGateResult>,
    /// 完整图输出（语言相关具体结构）
    pub graph: serde_json::Value,
}

// ============================================================
// 质量门命令输出
// ============================================================

/// quality 命令的输出
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityCommandOutput {
    pub language: String,
    pub root: String,
    /// "pass" | "fail" | "unsupported"
    pub overall: String,
    pub gates: Vec<QualityGateResult>,
}

// ============================================================
// 概要命令输出
// ============================================================

/// quality 概要
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualitySummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
}

/// summary 命令的输出（精简版，不含完整 graph）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryCommandOutput {
    pub language: String,
    pub root: String,
    pub analyzed_at: String,
    pub graph_summary: GraphSummary,
    pub quality_summary: QualitySummary,
}

// ============================================================
// 语言检测结果
// ============================================================

/// 语言检测结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedLanguage {
    Rust,
    Cangjie,
    /// ArkTS project (oh-package.json5 detected)
    ArkTS,
    /// TypeScript project (tsconfig.json / package.json with .ts/.tsx files)
    TypeScript,
    /// C project (CMakeLists.txt / Makefile / .c/.h files, no C++ files)
    C,
    /// C++ project (CMakeLists.txt / .cpp/.hpp files; may include .h)
    Cpp,
    /// Python project (pyproject.toml / setup.py / .py files)
    Python,
    /// 多种清单存在，需要用户显式指定
    Ambiguous,
    /// 没有可识别的清单文件
    Unknown,
}

impl DetectedLanguage {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            DetectedLanguage::Rust => "rust",
            DetectedLanguage::Cangjie => "cangjie",
            DetectedLanguage::ArkTS => "arkts",
            DetectedLanguage::TypeScript => "typescript",
            DetectedLanguage::C => "c",
            DetectedLanguage::Cpp => "cpp",
            DetectedLanguage::Python => "python",
            DetectedLanguage::Ambiguous => "ambiguous",
            DetectedLanguage::Unknown => "unknown",
        }
    }
}
