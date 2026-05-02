//! Item/Symbol 提取器 parser seam
//!
//! 这是 parser seam：第一刀只定义 trait 和 NoopItemExtractor，
//! 不做真实 Rust AST 解析，避免在数据契约未稳定前引入 parser 依赖（tree-sitter）。
//! 后续第二刀实现 TextItemExtractor（正则顶层提取），
//! 第三刀实现 TreeSitterItemExtractor（完整 11 种 SymbolKind）。

use crate::model::{Symbol, SymbolDiagnostic};

/// 单个 .rs 文件的 item 提取输入
#[derive(Debug, Clone)]
pub struct ItemExtractionInput {
    /// 相对 repo root 的 .rs 文件路径
    pub source_path: String,
    /// .rs 文件内容
    pub source_text: String,
    /// 所属 package name
    pub package_name: String,
    /// 所属 target name（ambiguous 时 None）
    pub target_name: Option<String>,
    /// crate 内 module 路径（如 "crate" 或 "crate::models"）
    pub module_path: Option<String>,
}

/// 单个 .rs 文件的 item 提取输出
#[derive(Debug, Clone, Default)]
pub struct ItemExtractionOutput {
    /// 提取到的 symbols
    pub symbols: Vec<Symbol>,
    /// 提取过程中的 diagnostics
    pub diagnostics: Vec<SymbolDiagnostic>,
}

/// Item 提取器 trait（parser seam）
///
/// 为什么用 trait 而不是直接函数：
/// - 允许 NoopItemExtractor（第一刀不做真实 extraction）
/// - 允许 TextItemExtractor（第二刀正则顶层提取）
/// - 允许 TreeSitterItemExtractor（第三刀完整提取）
/// - 不在数据契约未稳定前引入 parser 依赖
pub trait ItemExtractor {
    /// 从单个 .rs 文件提取 items
    fn extract_items(&self, input: &ItemExtractionInput) -> ItemExtractionOutput;
}

/// Noop 提取器：不做任何提取，返回空结果
///
/// 第一刀默认使用，保持输出兼容（symbols: [], symbolDiagnostics: []）。
/// 当 --include symbols 传入但无真实 extractor 时使用。
pub struct NoopItemExtractor;

impl ItemExtractor for NoopItemExtractor {
    fn extract_items(&self, _input: &ItemExtractionInput) -> ItemExtractionOutput {
        // 不做任何提取，返回空结果
        ItemExtractionOutput::default()
    }
}

/// 从多个 .rs 文件批量提取 items
pub fn extract_symbols_from_files(
    extractor: &dyn ItemExtractor,
    inputs: &[ItemExtractionInput],
) -> ItemExtractionOutput {
    let mut all_symbols = Vec::new();
    let mut all_diagnostics = Vec::new();

    for input in inputs {
        let output = extractor.extract_items(input);
        all_symbols.extend(output.symbols);
        all_diagnostics.extend(output.diagnostics);
    }

    ItemExtractionOutput {
        symbols: all_symbols,
        diagnostics: all_diagnostics,
    }
}
