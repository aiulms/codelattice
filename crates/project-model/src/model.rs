//! ProjectModel 核心数据模型
//!
//! 与 CLI/output contract preflight 冻结的 JSON 字段一一对应。
//! Rust struct 使用 snake_case，serde rename 为 camelCase 以匹配 JSON 契约。

use serde::Serialize;

/// package 发现方式，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum DiscoveryReason {
    RootManifest,
    SubdirectoryScan,
    WorkspaceExplicit,
    WorkspaceGlob,
}

impl DiscoveryReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryReason::RootManifest => "root-manifest",
            DiscoveryReason::SubdirectoryScan => "subdirectory-scan",
            DiscoveryReason::WorkspaceExplicit => "workspace-explicit",
            DiscoveryReason::WorkspaceGlob => "workspace-glob",
        }
    }
}

/// target 类型，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum TargetKind {
    Lib,
    Bin,
    Test,
    Bench,
    Example,
    CustomBuild,
    Unknown,
}

impl TargetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetKind::Lib => "lib",
            TargetKind::Bin => "bin",
            TargetKind::Test => "test",
            TargetKind::Bench => "bench",
            TargetKind::Example => "example",
            TargetKind::CustomBuild => "custom-build",
            TargetKind::Unknown => "unknown",
        }
    }
}

/// 顶层输出，覆盖 CLI/output contract 的 14 个字段
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelOutput {
    /// 输出格式版本，runtime-only
    pub version: String,
    /// 命令名，runtime-only
    pub command: String,
    /// repo 根目录（相对路径），可比较字段
    pub repo_root: String,
    /// ISO 8601 时间戳，runtime-only
    pub generated_at: String,
    /// 顶层 ProjectModel 统计
    pub project_model: ProjectModelSummary,
    /// Package 列表
    pub packages: Vec<PackageModel>,
    /// Workspace 列表
    pub workspaces: Vec<WorkspaceModel>,
    /// Target 列表
    pub targets: Vec<TargetModel>,
    /// Source 文件归属
    pub source_ownership: Vec<SourceOwnership>,
    /// crate:: 解析结果
    pub root_resolution: Vec<RootResolution>,
    /// Diagnostics 列表
    pub diagnostics: Vec<crate::diagnostic::Diagnostic>,
    /// 是否 partial indexing，runtime-only
    pub partial: bool,
    /// 非致命警告，runtime-only
    pub warnings: Vec<Warning>,
    /// 运行统计，runtime-only
    pub stats: Stats,
}

/// 顶层 ProjectModel 统计
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModelSummary {
    pub manifest_count: u32,
    pub package_count: u32,
    pub workspace_count: u32,
    pub diagnostics_count: u32,
}

/// Package 模型，对应 expected.json 的 expectedPackages
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageModel {
    pub name: String,
    /// 相对 repo root 的 Cargo.toml 路径
    pub manifest_path: String,
    /// 相对 repo root 的 package 根目录
    pub package_root: String,
    pub target_count: u32,
    pub feature_names: Vec<String>,
    pub is_workspace_member: bool,
    /// 有限集合枚举，不是自然语言
    pub discovery_reason: String,
}

/// Workspace 模型
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceModel {
    pub manifest_path: String,
    pub workspace_root: String,
    /// manifest 中的原始 members 声明
    pub raw_members: Vec<String>,
    /// glob 展开后的成员路径
    pub expanded_members: Vec<String>,
}

/// Target 模型
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetModel {
    pub package_name: String,
    pub name: String,
    /// Lib / Bin / Test / Bench / Example / CustomBuild / Unknown
    pub kind: String,
    pub crate_root_file: String,
    pub source_root_dir: String,
}

/// Source 文件归属，对应 expected.json 的 expectedSourceOwnership
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOwnership {
    pub source_path: String,
    /// null 表示无所属 package
    pub package: Option<String>,
    /// null 表示无所属 target
    pub target: Option<String>,
    pub ownership_reason: String,
    pub confidence: f32,
}

/// crate:: 解析结果，对应 expected.json 的 expectedRootResolution
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootResolution {
    pub source_path: String,
    pub query_path: String,
    /// null 表示解析失败（no-edge 策略阻止）
    pub resolved_path: Option<String>,
    /// null 对应解析失败
    pub target_kind: Option<String>,
    pub root_reason: String,
    pub confidence: f32,
}

/// 非致命警告，runtime-only
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

/// 运行统计，runtime-only
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub source_file_count: u32,
    pub owned_file_count: u32,
    pub unowned_file_count: u32,
    pub resolution_success_count: u32,
    pub resolution_fail_count: u32,
}
