pub mod include;
pub mod symbol;

#[cfg(feature = "tree-sitter-c")]
pub use include::{extract_c_includes, extract_c_includes_from_root};
pub use include::{CInclude, CIncludeKind};
#[cfg(feature = "tree-sitter-c")]
pub use symbol::{extract_c_symbols, extract_c_symbols_from_root};
pub use symbol::{CSymbol, CSymbolKind, CVisibility};

#[cfg(feature = "tree-sitter-c")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CExtraction {
    pub symbols: Vec<CSymbol>,
    pub includes: Vec<CInclude>,
}

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

#[cfg(feature = "tree-sitter-c")]
pub fn extract_c_file(source: &str) -> CExtraction {
    let mut parser = match try_init_c_parser() {
        Some(p) => p,
        None => {
            return CExtraction {
                symbols: vec![],
                includes: vec![],
            };
        }
    };
    let tree = match parse_c_source(&mut parser, source) {
        Ok(t) => t,
        Err(_) => {
            return CExtraction {
                symbols: vec![],
                includes: vec![],
            };
        }
    };
    let root = tree.root_node();
    CExtraction {
        symbols: extract_c_symbols_from_root(&root, source),
        includes: extract_c_includes_from_root(&root, source),
    }
}

#[cfg(all(test, feature = "tree-sitter-c"))]
mod tests {
    use super::*;

    const C_FIXTURE: &str = r#"
#include "local.h"
#include <stdio.h>

typedef struct User {
    int id;
} User;

static int helper(int id);

int main(void) {
    return helper(1);
}
"#;

    #[test]
    fn root_based_c_extraction_matches_source_extractors() {
        let mut parser = try_init_c_parser().expect("parser should initialize");
        let tree = parse_c_source(&mut parser, C_FIXTURE).expect("fixture should parse");
        let root = tree.root_node();

        assert_eq!(
            extract_c_symbols_from_root(&root, C_FIXTURE),
            extract_c_symbols(C_FIXTURE)
        );
        assert_eq!(
            extract_c_includes_from_root(&root, C_FIXTURE),
            extract_c_includes(C_FIXTURE)
        );
    }

    #[test]
    fn combined_c_extraction_matches_separate_extractors() {
        let extraction = extract_c_file(C_FIXTURE);

        assert_eq!(extraction.symbols, extract_c_symbols(C_FIXTURE));
        assert_eq!(extraction.includes, extract_c_includes(C_FIXTURE));
    }
}
