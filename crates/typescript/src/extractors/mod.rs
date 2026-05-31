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

/// Parse-once extraction result for a single TypeScript/TSX source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsExtraction {
    pub symbols: Vec<TsSymbol>,
    pub imports: Vec<TsImport>,
    pub references: Vec<TsReference>,
}

/// Extract symbols, imports, and references from one parse tree.
#[cfg(feature = "tree-sitter-typescript")]
pub fn extract_ts_file(source: &str, lang: TsLanguage) -> TsExtraction {
    let mut parser = match try_init_ts_parser(lang) {
        Some(p) => p,
        None => {
            return TsExtraction {
                symbols: vec![],
                imports: vec![],
                references: vec![],
            };
        }
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            return TsExtraction {
                symbols: vec![],
                imports: vec![],
                references: vec![],
            };
        }
    };
    let root = tree.root_node();
    extract_ts_file_from_root(&root, source)
}

/// Extract symbols, imports, and references from an existing parse tree.
#[cfg(feature = "tree-sitter-typescript")]
pub fn extract_ts_file_from_root(root: &tree_sitter::Node, source: &str) -> TsExtraction {
    TsExtraction {
        symbols: symbol::extract_ts_symbols_from_root(root, source),
        imports: imports::extract_ts_imports_from_root(root, source),
        references: references::extract_ts_references_from_root(root, source),
    }
}

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
    if parser.set_language(&language.into()).is_ok() {
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

#[cfg(all(test, feature = "tree-sitter-typescript"))]
mod tests {
    use super::*;

    #[test]
    fn parse_once_extraction_matches_separate_extractors() {
        let source = r#"
            import { makeUser } from "./user";
            interface User { id: string }
            export function run(user: User) {
                return makeUser(user.id);
            }
        "#;

        let combined = extract_ts_file(source, TsLanguage::TypeScript);

        assert_eq!(
            combined.symbols,
            extract_ts_symbols(source, TsLanguage::TypeScript)
        );
        assert_eq!(
            combined.imports,
            extract_ts_imports(source, TsLanguage::TypeScript)
        );
        assert_eq!(
            combined.references,
            extract_ts_references(source, TsLanguage::TypeScript)
        );
    }

    #[test]
    fn root_extraction_matches_parse_once_extraction() {
        let source = r#"
            import { makeUser } from "./user";
            export class UserRunner {
                run(id: string) {
                    return makeUser(id);
                }
            }
        "#;
        let mut parser =
            try_init_ts_parser(TsLanguage::TypeScript).expect("parser should initialize");
        let tree = parser.parse(source, None).expect("fixture should parse");
        let root = tree.root_node();

        assert_eq!(
            extract_ts_file_from_root(&root, source),
            extract_ts_file(source, TsLanguage::TypeScript)
        );
    }
}
