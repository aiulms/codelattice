pub mod include;
pub mod symbol;

#[cfg(feature = "tree-sitter-c")]
pub use include::extract_c_includes;
pub use include::{CInclude, CIncludeKind};
#[cfg(feature = "tree-sitter-c")]
pub use symbol::extract_c_symbols;
pub use symbol::{CSymbol, CSymbolKind, CVisibility};

/// Check whether the tree-sitter-c parser is available at runtime.
pub fn is_c_parser_available() -> bool {
    cfg!(feature = "tree-sitter-c")
}

/// Error returned when a C source parse fails.
#[derive(Debug)]
pub enum CParseError {
    /// The tree-sitter-c feature is not enabled.
    NotAvailable,
    /// The source could not be parsed.
    ParseFailed(String),
}

impl std::fmt::Display for CParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-c feature is not enabled"),
            Self::ParseFailed(msg) => write!(f, "parse failed: {msg}"),
        }
    }
}

/// Initialize a tree-sitter parser for C.
#[cfg(feature = "tree-sitter-c")]
pub fn try_init_c_parser() -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&tree_sitter_c::LANGUAGE.into()).is_ok() {
        Some(parser)
    } else {
        None
    }
}

/// Parse C source code and return the syntax tree.
#[cfg(feature = "tree-sitter-c")]
pub fn parse_c_source(
    parser: &mut tree_sitter::Parser,
    source: &str,
) -> Result<tree_sitter::Tree, CParseError> {
    parser
        .parse(source, None)
        .ok_or_else(|| CParseError::ParseFailed("tree-sitter returned None".to_string()))
}
