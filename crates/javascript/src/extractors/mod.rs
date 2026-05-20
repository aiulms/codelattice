pub mod imports;
pub mod references;
pub mod symbol;

#[cfg(feature = "tree-sitter-javascript")]
pub use imports::extract_js_imports;
pub use imports::{JsImport, JsImportKind};
#[cfg(feature = "tree-sitter-javascript")]
pub use references::extract_js_references;
pub use references::{JsReference, JsReferenceKind};
#[cfg(feature = "tree-sitter-javascript")]
pub use symbol::extract_js_symbols;
pub use symbol::{JsSymbol, JsSymbolKind};

/// JavaScript 解析器是否可用。
pub fn is_js_parser_available() -> bool {
    cfg!(feature = "tree-sitter-javascript")
}

/// JavaScript 解析错误。
#[derive(Debug)]
pub enum JsParseError {
    /// tree-sitter-javascript feature 未启用。
    NotAvailable,
    /// 解析失败。
    ParseFailed(String),
}

impl std::fmt::Display for JsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-javascript feature 未启用"),
            Self::ParseFailed(msg) => write!(f, "解析失败: {msg}"),
        }
    }
}

/// JavaScript 语言变体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsLanguage {
    /// 标准 JavaScript (.js, .mjs, .cjs)
    JavaScript,
    /// JSX (.jsx)
    Jsx,
}

/// 初始化 tree-sitter parser。
/// 内部委托给 gitnexus-typescript 的 parser（TypeScript grammar 向下兼容 JavaScript）。
#[cfg(feature = "tree-sitter-javascript")]
pub fn try_init_js_parser(lang: JsLanguage) -> Option<tree_sitter::Parser> {
    let ts_lang = match lang {
        JsLanguage::JavaScript => gitnexus_typescript::extractors::TsLanguage::TypeScript,
        JsLanguage::Jsx => gitnexus_typescript::extractors::TsLanguage::Tsx,
    };
    gitnexus_typescript::extractors::try_init_ts_parser(ts_lang)
}
