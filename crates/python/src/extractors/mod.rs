pub mod call;
pub mod import;
pub mod symbol;

#[cfg(feature = "tree-sitter-python")]
pub use call::extract_python_calls;
pub use call::PythonCall;
#[cfg(feature = "tree-sitter-python")]
pub use import::extract_python_imports;
pub use import::{PythonImport, PythonImportKind};
#[cfg(feature = "tree-sitter-python")]
pub use symbol::extract_python_symbols;
pub use symbol::{PythonSymbolKind, PythonVisibility};

/// Re-export PythonSymbol type.
pub use symbol::PythonSymbol;

/// Check whether the tree-sitter-python parser is available at runtime.
pub fn is_python_parser_available() -> bool {
    cfg!(feature = "tree-sitter-python")
}

/// Error returned when a Python source parse fails.
#[derive(Debug)]
pub enum PythonParseError {
    /// The tree-sitter-python feature is not enabled.
    NotAvailable,
    /// The source could not be parsed.
    ParseFailed(String),
}

impl std::fmt::Display for PythonParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-python feature is not enabled"),
            Self::ParseFailed(msg) => write!(f, "parse failed: {msg}"),
        }
    }
}

/// Initialize a tree-sitter parser for Python.
#[cfg(feature = "tree-sitter-python")]
pub fn try_init_python_parser() -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_ok()
    {
        Some(parser)
    } else {
        None
    }
}

/// Parse Python source code and return the syntax tree.
#[cfg(feature = "tree-sitter-python")]
pub fn parse_python_source(
    parser: &mut tree_sitter::Parser,
    source: &str,
) -> Result<tree_sitter::Tree, PythonParseError> {
    parser
        .parse(source, None)
        .ok_or_else(|| PythonParseError::ParseFailed("tree-sitter returned None".to_string()))
}
