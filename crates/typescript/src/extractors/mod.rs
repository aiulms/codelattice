pub mod imports;
pub mod references;
pub mod symbol;

#[cfg(feature = "tree-sitter-typescript")]
pub use imports::extract_ts_imports;
pub use imports::TsImport;
#[cfg(feature = "tree-sitter-typescript")]
pub use references::extract_ts_references;
pub use references::{TsReference, TsReferenceKind};
#[cfg(feature = "tree-sitter-typescript")]
pub use symbol::extract_ts_symbols;
pub use symbol::{TsSymbol, TsSymbolKind};

/// Check whether the tree-sitter-typescript parser is available at runtime.
pub fn is_ts_parser_available() -> bool {
    cfg!(feature = "tree-sitter-typescript")
}

/// Error returned when a TypeScript source parse fails.
#[derive(Debug)]
pub enum TsParseError {
    /// The tree-sitter-typescript feature is not enabled.
    NotAvailable,
    /// The source could not be parsed.
    ParseFailed(String),
}

impl std::fmt::Display for TsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-typescript feature is not enabled"),
            Self::ParseFailed(msg) => write!(f, "parse failed: {msg}"),
        }
    }
}

/// Source language variant for selecting the correct tree-sitter grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsLanguage {
    /// Standard TypeScript (.ts, .ets).
    TypeScript,
    /// TSX / JSX (.tsx).
    Tsx,
}

/// Initialize a tree-sitter parser for the given language variant.
#[cfg(feature = "tree-sitter-typescript")]
pub fn try_init_ts_parser(lang: TsLanguage) -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    let language = match lang {
        TsLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        TsLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
    };
    if parser
        .set_language(&language.into())
        .is_ok()
    {
        Some(parser)
    } else {
        None
    }
}

/// Parse TypeScript source code and return the syntax tree.
#[cfg(feature = "tree-sitter-typescript")]
pub fn parse_ts_source(
    parser: &mut tree_sitter::Parser,
    source: &str,
) -> Result<tree_sitter::Tree, TsParseError> {
    parser
        .parse(source, None)
        .ok_or_else(|| TsParseError::ParseFailed("tree-sitter returned None".to_string()))
}
