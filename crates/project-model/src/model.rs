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
    /// workspace member 子目录中未列入 members 的嵌套 package
    NestedInMember,
}

impl DiscoveryReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryReason::RootManifest => "root-manifest",
            DiscoveryReason::SubdirectoryScan => "subdirectory-scan",
            DiscoveryReason::WorkspaceExplicit => "workspace-explicit",
            DiscoveryReason::WorkspaceGlob => "workspace-glob",
            DiscoveryReason::NestedInMember => "nested-in-member",
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

/// 顶层输出，覆盖 CLI/output contract 的 14 个字段 + symbol 扩展
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
    /// 提取的 item/symbol 列表，--include symbols 时填充
    pub symbols: Vec<Symbol>,
    /// item 提取相关 diagnostics，--include symbols 时填充
    pub symbol_diagnostics: Vec<SymbolDiagnostic>,
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
    /// "module" / "file" / null（解析失败时）
    pub resolved_kind: Option<String>,
    /// 当前 source file 的 crate root 文件路径
    pub crate_root_file: Option<String>,
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
    /// item/symbol 提取计数，--include symbols 时填充
    pub symbol_count: u32,
}

// ============================================================
// Item/Symbol Model 数据模型
// 第一刀：只定义数据结构和 parser seam trait，不做真实 extraction。
// 避免在数据契约未稳定前引入 parser 依赖（tree-sitter）。
// ============================================================

/// item 种类，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum SymbolKind {
    Module,
    Function,
    Struct,
    Enum,
    Trait,
    ImplBlock,
    Method,
    AssociatedFunction,
    TypeAlias,
    Const,
    Static,
    MacroDefinition,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Module => "module",
            SymbolKind::Function => "function",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::ImplBlock => "impl-block",
            SymbolKind::Method => "method",
            SymbolKind::AssociatedFunction => "associated-function",
            SymbolKind::TypeAlias => "type-alias",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::MacroDefinition => "macro-definition",
        }
    }
}

/// item 可见性，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum Visibility {
    Public,
    Crate,
    Super,
    Restricted,
    Private,
    Unknown,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Crate => "pub-crate",
            Visibility::Super => "pub-super",
            Visibility::Restricted => "pub-restricted",
            Visibility::Private => "private",
            Visibility::Unknown => "unknown",
        }
    }
}

/// 提取的 item/symbol
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    /// 稳定身份：{packageName}::{modulePath}::{name}[::{disambiguator}]
    pub id: String,
    /// item 标识符
    pub name: String,
    /// SymbolKind.as_str()
    pub symbol_kind: String,
    /// 相对 repo root 的 .rs 文件路径
    pub source_path: String,
    /// 所属 package
    pub package_name: String,
    /// 所属 target（ambiguous 时 null）
    pub target_name: Option<String>,
    /// crate 内 module 路径，如 "crate::models"
    pub module_path: Option<String>,
    /// 可见性
    pub visibility: String,
    /// 父 item id（如 impl 块的 method）
    pub parent_id: Option<String>,
    /// 起始行号（1-indexed）
    pub line_start: u32,
    /// 结束行号
    pub line_end: u32,
    /// generic 参数原样记录，如 "<T>"；不进入 id
    pub generic_params: Option<String>,
    /// fn / method 是否 async
    pub is_async: bool,
    /// fn / method 是否 unsafe
    pub is_unsafe: bool,
    /// fn 是否 const
    pub is_const_fn: bool,
    /// 便捷：visibility == "public"
    pub is_pub: bool,
    /// impl 块详情（仅 ImplBlock / Method / AssociatedFunction 有值）
    pub impl_details: Option<ImplBlockDetail>,
}

/// impl 块扩展字段
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplBlockDetail {
    /// impl Target { ... } 中的 Target
    pub impl_target: String,
    /// impl Trait for Target 中的 Trait（trait impl 时有值）
    pub trait_name: Option<String>,
}

/// item 提取相关 diagnostic
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source_path: String,
    /// 关联 symbol id（如有）
    pub symbol_id: Option<String>,
    /// 修复建议
    pub suggested_action: Option<String>,
}
