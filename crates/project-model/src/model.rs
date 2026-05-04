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
    /// 提取的 import/use 列表，--include imports 时填充
    pub imports: Vec<ImportUse>,
    /// import/use 提取相关 diagnostics，--include imports 时填充
    pub import_diagnostics: Vec<ImportUseDiagnostic>,
    /// 提取的 call site 列表，--include calls 时填充
    pub calls: Vec<CallSite>,
    /// call 提取相关 diagnostics，--include calls 时填充
    pub call_diagnostics: Vec<CallDiagnostic>,
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
    /// 依赖 crate 名称（不含 version/feature），含隐式依赖 std/core/alloc
    pub dependency_names: Vec<String>,
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
    /// import/use 提取计数，--include imports 时填充
    pub import_count: u32,
    /// call site 提取计数，--include calls 时填充
    pub call_count: u32,
    /// external crate call 总数
    pub call_external_crate_total: u32,
    /// external crate call 中已分类（known_crate 非空）
    pub call_external_crate_classified: u32,
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

// ============================================================
// Import/Use Resolution 数据模型
// 第一刀：Intermediate output，不直接进入 graph emitter。
// 只解析到 module/file 层，不做 item-level symbol resolution。
// ============================================================

/// use 声明路径类型
#[derive(Debug, Clone, Serialize)]
pub enum ImportUseKind {
    Crate,
    SelfPath,
    Super,
    External,
    Unknown,
}

impl ImportUseKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportUseKind::Crate => "crate",
            ImportUseKind::SelfPath => "self",
            ImportUseKind::Super => "super",
            ImportUseKind::External => "external",
            ImportUseKind::Unknown => "unknown",
        }
    }
}

/// import/use 解析原因
#[derive(Debug, Clone, Serialize)]
pub enum ImportUseResolutionReason {
    UseCrateResolved,
    UseSelfResolved,
    UseSuperResolved,
    UseGroupExpanded,
    UseAliasResolved,
    UseReexportResolved,
    UseExternalSkipped,
    UseGlobUnsupported,
    UseTargetUnresolved,
    UseTargetAmbiguous,
    UseSuperAtCrateRoot,
    UseCfgGatedUnknown,
    UseParseError,
    UseResolutionSkipped,
    UseMacroImportUnsupported,
    UseSymbolResolved,
    UseSymbolAliasResolved,
    UseSymbolReexportResolved,
    UseSymbolSelfResolved,
    UseSymbolSuperResolved,
    UseSymbolUnresolved,
    UseSymbolAmbiguous,
    UseSymbolSkipped,
}

impl ImportUseResolutionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportUseResolutionReason::UseCrateResolved => "use-crate-resolved",
            ImportUseResolutionReason::UseSelfResolved => "use-self-resolved",
            ImportUseResolutionReason::UseSuperResolved => "use-super-resolved",
            ImportUseResolutionReason::UseGroupExpanded => "use-group-expanded",
            ImportUseResolutionReason::UseAliasResolved => "use-alias-resolved",
            ImportUseResolutionReason::UseReexportResolved => "use-reexport-resolved",
            ImportUseResolutionReason::UseExternalSkipped => "use-external-skipped",
            ImportUseResolutionReason::UseGlobUnsupported => "use-glob-unsupported",
            ImportUseResolutionReason::UseTargetUnresolved => "use-target-unresolved",
            ImportUseResolutionReason::UseTargetAmbiguous => "use-target-ambiguous",
            ImportUseResolutionReason::UseSuperAtCrateRoot => "use-super-at-crate-root",
            ImportUseResolutionReason::UseCfgGatedUnknown => "use-cfg-gated-unknown",
            ImportUseResolutionReason::UseParseError => "use-parse-error",
            ImportUseResolutionReason::UseResolutionSkipped => "use-resolution-skipped",
            ImportUseResolutionReason::UseMacroImportUnsupported => "use-macro-import-unsupported",
            ImportUseResolutionReason::UseSymbolResolved => "use-symbol-resolved",
            ImportUseResolutionReason::UseSymbolAliasResolved => "use-symbol-alias-resolved",
            ImportUseResolutionReason::UseSymbolReexportResolved => "use-symbol-reexport-resolved",
            ImportUseResolutionReason::UseSymbolSelfResolved => "use-symbol-self-resolved",
            ImportUseResolutionReason::UseSymbolSuperResolved => "use-symbol-super-resolved",
            ImportUseResolutionReason::UseSymbolUnresolved => "use-symbol-unresolved",
            ImportUseResolutionReason::UseSymbolAmbiguous => "use-symbol-ambiguous",
            ImportUseResolutionReason::UseSymbolSkipped => "use-symbol-skipped",
        }
    }
}

/// import/use 解析目标
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportUseTarget {
    pub resolved_path: Option<String>,
    pub resolved_kind: Option<String>,
    pub target_module_path: Option<String>,
    pub target_file_path: Option<String>,
    pub resolved_symbol_id: Option<String>,
    pub resolved_symbol_kind: Option<String>,
    pub resolved_symbol_name: Option<String>,
    pub resolved_symbol_source_path: Option<String>,
}

/// import/use diagnostic
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportUseDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub target_name: Option<String>,
}

/// sourcePath → modulePath 映射条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulePathEntry {
    pub source_path: String,
    pub module_path: String,
    pub confidence: f32,
    pub reason: String,
}

/// modulePath 计算诊断
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulePathDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source_path: String,
}

/// 单条 import/use 提取结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportUse {
    /// 稳定身份：{sourcePath}::use::{lineStart}::{targetIndex}
    pub id: String,
    /// .rs 文件相对路径
    pub source_path: String,
    /// 当前文件的 module path（如 crate::foo）
    pub module_path: Option<String>,
    /// use 声明原文
    pub raw_text: String,
    pub line_start: u32,
    pub line_end: u32,
    /// 可见性（public / private / pub-crate 等）
    pub visibility: String,
    /// 路径类型（crate / self / super / external / unknown）
    pub path_kind: String,
    /// 原始路径（如 crate::foo::Bar）
    pub original_path: String,
    /// 展开后路径（grouped import 展开后可能不同）
    pub expanded_path: Option<String>,
    /// as 后的别名
    pub alias: Option<String>,
    /// 是否 pub use（re-export）
    pub is_re_export: bool,
    /// 实际引入的名称（alias 优先，否则路径末段）
    pub target_name: String,
    /// 解析结果（null = 解析失败或 skipped）
    pub resolved_to: Option<ImportUseTarget>,
    /// 解析置信度
    pub confidence: f32,
    /// 解析原因 code
    pub reason: String,
    /// 附带 diagnostics
    pub diagnostics: Vec<ImportUseDiagnostic>,
    /// 解析层级：module | symbol | unresolved | skipped
    pub resolution_level: String,
}

// ============================================================
// CALLS Intermediate Output 数据模型
// 第一刀：Intermediate output，不直接进入 graph emitter。
// 只解析明确可静态解析的调用形式。
// ============================================================

/// 调用类型，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum CallKind {
    FreeFunction,
    QualifiedPath,
    SelfPath,
    SuperPath,
    AssociatedFunction,
    MethodCall,
    /// external crate 调用（如 std::vec::Vec::new），不解析 crate 内 symbol
    ExternalCrate,
    Unknown,
}

impl CallKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallKind::FreeFunction => "free-function",
            CallKind::QualifiedPath => "qualified-path",
            CallKind::SelfPath => "self-path",
            CallKind::SuperPath => "super-path",
            CallKind::AssociatedFunction => "associated-function",
            CallKind::MethodCall => "method-call",
            CallKind::ExternalCrate => "external-crate",
            CallKind::Unknown => "unknown",
        }
    }
}

/// call site 位置
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSpan {
    pub line_start: u32,
    pub line_end: u32,
    pub byte_start: usize,
    pub byte_end: usize,
}

/// call 解析原因，有限集合枚举
#[derive(Debug, Clone, Serialize)]
pub enum CallResolutionReason {
    CallSameModuleResolved,
    CallImportResolved,
    CallSameFileUniqueName,
    CallCratePathResolved,
    CallSelfPathResolved,
    CallSuperPathResolved,
    CallAssociatedFnResolved,
    CallModulePathResolved,
    CallTargetUnresolved,
    CallTargetAmbiguous,
    CallMethodDispatchUnsupported,
    /// blind method name resolution：唯一 method name in crate，不验证 receiver type
    /// confidence 0.65，低于所有现有 resolution path
    CallMethodNameResolved,
    CallEnumConstructor,
    /// external crate call classified to known crate name（不解析 crate 内 symbol）
    /// confidence 0.60：crate name known from [dependencies]，低于 method-name-resolved(0.65)
    CallExternalCrateClassified,
    /// external crate call resolved by direct path construction（std/core/alloc only）
    /// callee_path → resolved_symbol_id，不验证 symbol 存在性（rustc 编译提供隐含保证）
    /// confidence 0.80：高于 classified(0.60)，低于 same-module(0.90) / import(0.85)
    CallExternalCratePathResolved,
}

impl CallResolutionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallResolutionReason::CallSameModuleResolved => "call-same-module-resolved",
            CallResolutionReason::CallImportResolved => "call-import-resolved",
            // same-file unique-name heuristic fallback：confidence 0.70，低于 same-module(0.90) 和 import(0.85)
            CallResolutionReason::CallSameFileUniqueName => "call-same-file-unique-name",
            CallResolutionReason::CallCratePathResolved => "call-crate-path-resolved",
            CallResolutionReason::CallSelfPathResolved => "call-self-path-resolved",
            CallResolutionReason::CallSuperPathResolved => "call-super-path-resolved",
            CallResolutionReason::CallAssociatedFnResolved => "call-associated-fn-resolved",
            // bare module path (如 root_resolution::func) — Rust 语义中是 crate-relative
            // confidence 0.85：低于 explicit crate:: (0.90)，高于 heuristic (0.70)
            CallResolutionReason::CallModulePathResolved => "call-module-path-resolved",
            CallResolutionReason::CallTargetUnresolved => "call-target-unresolved",
            CallResolutionReason::CallTargetAmbiguous => "call-target-ambiguous",
            CallResolutionReason::CallMethodDispatchUnsupported => {
                "call-method-dispatch-unsupported"
            }
            // blind method name resolution：唯一 method name in crate，不验证 receiver type
            // confidence 0.65，低于 same-module(0.90) / import(0.85) / bare-path(0.85) / heuristic(0.70)
            CallResolutionReason::CallMethodNameResolved => "call-method-name-resolved",
            // Rust enum variant constructor（Some/Ok/Err）不是函数调用，直接标记
            CallResolutionReason::CallEnumConstructor => "call-enum-constructor",
            // external crate call classified to known crate name
            // confidence 0.60：crate name known，symbol within crate 未解析
            CallResolutionReason::CallExternalCrateClassified => "call-external-crate-classified",
            // external crate call resolved by direct path construction（std/core/alloc only）
            // confidence 0.80：编译保证路径正确，无 symbol 级验证
            CallResolutionReason::CallExternalCratePathResolved => {
                "call-external-crate-path-resolved"
            }
        }
    }
}

/// call diagnostic
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub target_name: Option<String>,
}

/// 单条 call site 提取结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSite {
    /// 稳定身份：{sourcePath}::call::{lineStart}::{calleeName}
    pub id: String,
    /// enclosing function/method symbol id
    pub caller_symbol_id: Option<String>,
    /// enclosing function/method name
    pub caller_name: Option<String>,
    /// .rs 文件相对路径
    pub source_path: String,
    /// 当前文件的 modulePath
    pub module_path: Option<String>,
    /// call site 位置
    pub span: CallSpan,
    /// 调用原文
    pub raw_text: String,
    /// external crate 调用时填充 known crate name（null = intra-crate）
    pub known_crate: Option<String>,
    /// callee 路径原文
    /// callee 路径原文
    pub callee_path: String,
    /// callee 标识符
    pub callee_name: String,
    /// CallKind.as_str()
    pub call_kind: String,
    /// 命中的 symbol id（null = 未解析或 diagnostic only）
    pub resolved_symbol_id: Option<String>,
    /// 命中的 symbol kind
    pub resolved_symbol_kind: Option<String>,
    /// 解析置信度
    pub confidence: f32,
    /// 解析原因 code
    pub reason: String,
    /// 附带 diagnostics
    pub diagnostics: Vec<CallDiagnostic>,
}
