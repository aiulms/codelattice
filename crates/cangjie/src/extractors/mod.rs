//! Tree-sitter Cangjie parser integration and AST symbol extraction.
//!
//! Available only when the `tree-sitter-cangjie` feature is enabled.

pub mod symbol;

#[cfg(feature = "tree-sitter-cangjie")]
pub use symbol::{extract_cangjie_symbols, extract_cangjie_symbols_from_tree};
pub use symbol::{CangjieSymbol, CangjieSymbolKind};

/// Check whether the tree-sitter-cangjie parser is available at runtime.
///
/// Returns `true` when the `tree-sitter-cangjie` feature was enabled at build time.
pub fn is_cangjie_parser_available() -> bool {
    cfg!(feature = "tree-sitter-cangjie")
}

/// Error returned when a Cangjie source parse fails.
#[derive(Debug)]
pub enum CangjieParseError {
    /// The tree-sitter-cangjie feature is not enabled.
    NotAvailable,
    /// The source could not be parsed (tree-sitter returned an error).
    ParseFailed(String),
    /// The resulting tree contains ERROR nodes.
    HasErrorNodes,
}

impl std::fmt::Display for CangjieParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "tree-sitter-cangjie feature is not enabled"),
            Self::ParseFailed(msg) => write!(f, "parse failed: {msg}"),
            Self::HasErrorNodes => write!(f, "parse produced ERROR nodes"),
        }
    }
}

/// Initialize a tree-sitter-cangjie parser.
///
/// Returns `None` if the language cannot be loaded (e.g. ABI mismatch).
#[cfg(feature = "tree-sitter-cangjie")]
pub fn try_init_cangjie_parser() -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    extern "C" {
        fn tree_sitter_cangjie() -> tree_sitter::Language;
    }
    let language = unsafe { tree_sitter_cangjie() };
    if parser.set_language(&language).is_ok() {
        Some(parser)
    } else {
        None
    }
}

/// Parse Cangjie source code and return the concrete syntax tree.
///
/// Returns `CangjieParseError::HasErrorNodes` if the tree contains ERROR
/// nodes (syntax errors), but the tree itself is still returned for
/// inspection.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn parse_cangjie_source(source: &str) -> Result<tree_sitter::Tree, CangjieParseError> {
    let mut parser = try_init_cangjie_parser().ok_or(CangjieParseError::NotAvailable)?;
    parser
        .parse(source, None)
        .ok_or_else(|| CangjieParseError::ParseFailed("tree-sitter returned None".into()))
}

/// Check whether a parsed tree contains ERROR or MISSING nodes.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn tree_has_error_nodes(tree: &tree_sitter::Tree) -> bool {
    has_error_node(tree.root_node())
}

#[cfg(feature = "tree-sitter-cangjie")]
fn has_error_node(node: tree_sitter::Node) -> bool {
    if node.kind() == "ERROR" || node.kind() == "MISSING" {
        return true;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i.try_into().unwrap()) {
            if has_error_node(child) {
                return true;
            }
        }
    }
    false
}
