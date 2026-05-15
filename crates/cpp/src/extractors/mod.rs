pub mod call;
pub mod include;
pub mod symbol;

#[cfg(feature = "tree-sitter-cpp")]
pub use call::extract_cpp_calls;
pub use call::CppCall;
#[cfg(feature = "tree-sitter-cpp")]
pub use include::extract_cpp_includes;
pub use include::{CppInclude, CppIncludeKind};
#[cfg(feature = "tree-sitter-cpp")]
pub use symbol::extract_cpp_symbols;
pub use symbol::{CppSymbolKind, CppVisibility};

/// Re-export CSymbol-like type for C++ symbols.
pub use symbol::CppSymbol;

/// Check whether the tree-sitter-cpp parser is available at runtime.
pub fn is_cpp_parser_available() -> bool {
    cfg!(feature = "tree-sitter-cpp")
}

/// Error returned when a C++ source parse fails.
#[derive(Debug)]
pub enum CppParseError {
    /// The tree-sitter-cpp feature is not enabled.
    NotAvailable,
    /// The source could not be parsed.
    ParseFailed(String),
}

impl std::fmt::Display for CppParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-cpp feature is not enabled"),
            Self::ParseFailed(msg) => write!(f, "parse failed: {msg}"),
        }
    }
}

/// Initialize a tree-sitter parser for C++.
#[cfg(feature = "tree-sitter-cpp")]
pub fn try_init_cpp_parser() -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .is_ok()
    {
        Some(parser)
    } else {
        None
    }
}

/// Parse C++ source code and return the syntax tree.
#[cfg(feature = "tree-sitter-cpp")]
pub fn parse_cpp_source(
    parser: &mut tree_sitter::Parser,
    source: &str,
) -> Result<tree_sitter::Tree, CppParseError> {
    parser
        .parse(source, None)
        .ok_or_else(|| CppParseError::ParseFailed("tree-sitter returned None".to_string()))
}
