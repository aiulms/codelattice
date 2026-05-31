pub mod call;
pub mod include;
pub mod symbol;

pub use call::CppCall;
#[cfg(feature = "tree-sitter-cpp")]
pub use call::{extract_cpp_calls, extract_cpp_calls_from_root};
#[cfg(feature = "tree-sitter-cpp")]
pub use include::{extract_cpp_includes, extract_cpp_includes_from_root};
pub use include::{CppInclude, CppIncludeKind};
#[cfg(feature = "tree-sitter-cpp")]
pub use symbol::{extract_cpp_symbols, extract_cpp_symbols_from_root};
pub use symbol::{CppSymbolKind, CppVisibility};

/// Re-export CSymbol-like type for C++ symbols.
pub use symbol::CppSymbol;

#[cfg(feature = "tree-sitter-cpp")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppBaseExtraction {
    pub symbols: Vec<CppSymbol>,
    pub includes: Vec<CppInclude>,
}

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

#[cfg(feature = "tree-sitter-cpp")]
pub fn extract_cpp_file_base(source: &str) -> CppBaseExtraction {
    let mut parser = match try_init_cpp_parser() {
        Some(p) => p,
        None => {
            return CppBaseExtraction {
                symbols: vec![],
                includes: vec![],
            };
        }
    };
    let tree = match parse_cpp_source(&mut parser, source) {
        Ok(t) => t,
        Err(_) => {
            return CppBaseExtraction {
                symbols: vec![],
                includes: vec![],
            };
        }
    };
    let root = tree.root_node();
    CppBaseExtraction {
        symbols: extract_cpp_symbols_from_root(&root, source),
        includes: extract_cpp_includes_from_root(&root, source),
    }
}

#[cfg(all(test, feature = "tree-sitter-cpp"))]
mod tests {
    use super::*;

    const CPP_FIXTURE: &str = r#"
#include "local.hpp"
#include <vector>

namespace demo {
class Runner {
public:
    void run();
};

void helper() {}

void Runner::run() {
    helper();
}
}
"#;

    #[test]
    fn root_based_cpp_extraction_matches_source_extractors() {
        let mut parser = try_init_cpp_parser().expect("parser should initialize");
        let tree = parse_cpp_source(&mut parser, CPP_FIXTURE).expect("fixture should parse");
        let root = tree.root_node();
        let project_fn_names = vec!["demo::helper".to_string(), "helper".to_string()];

        assert_eq!(
            extract_cpp_symbols_from_root(&root, CPP_FIXTURE),
            extract_cpp_symbols(CPP_FIXTURE)
        );
        assert_eq!(
            extract_cpp_includes_from_root(&root, CPP_FIXTURE),
            extract_cpp_includes(CPP_FIXTURE)
        );
        assert_eq!(
            extract_cpp_calls_from_root(&root, CPP_FIXTURE, "src/main.cpp", &project_fn_names),
            extract_cpp_calls(CPP_FIXTURE, "src/main.cpp", &project_fn_names)
        );
    }

    #[test]
    fn combined_cpp_base_extraction_matches_separate_extractors() {
        let extraction = extract_cpp_file_base(CPP_FIXTURE);

        assert_eq!(extraction.symbols, extract_cpp_symbols(CPP_FIXTURE));
        assert_eq!(extraction.includes, extract_cpp_includes(CPP_FIXTURE));
    }
}
