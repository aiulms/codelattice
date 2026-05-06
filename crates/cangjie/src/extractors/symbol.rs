//! AST-level symbol extraction from tree-sitter-cangjie parse trees.
//!
//! Available only when the `tree-sitter-cangjie` feature is enabled.

#[cfg(feature = "tree-sitter-cangjie")]
use super::CangjieParseError;

/// Kinds of top-level symbols extractable from Cangjie source files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CangjieSymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Interface,
    TypeAlias,
    Macro,
}

impl std::fmt::Display for CangjieSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Class => write!(f, "class"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Interface => write!(f, "interface"),
            Self::TypeAlias => write!(f, "typeAlias"),
            Self::Macro => write!(f, "macro"),
        }
    }
}

/// A top-level symbol extracted from a Cangjie source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CangjieSymbol {
    pub kind: CangjieSymbolKind,
    pub name: String,
    /// 1-based start line
    pub start_line: usize,
    /// 1-based end line (inclusive)
    pub end_line: usize,
}

/// Tree-sitter S-expression query for top-level Cangjie definitions.
///
/// Captures:
/// - `@name` — the identifier node of the definition
///
/// The symbol kind is determined from the parent node type.
#[cfg(feature = "tree-sitter-cangjie")]
const SYMBOL_QUERY: &str = r#"
(classDefinition (className) @name)
(interfaceDefinition (interfaceName) @name)
(functionDefinition (funcName) @name)
(mainDefinition "main" @name)
(macroDefinition (macroName) @name)
(structDefinition (structName) @name)
(typeAlias (typeAliasName) @name)
(enumDefinition (enumName) @name)
"#;

/// Map a tree-sitter node kind to the corresponding [`CangjieSymbolKind`].
#[cfg(feature = "tree-sitter-cangjie")]
fn classify_symbol(parent_kind: &str) -> Option<CangjieSymbolKind> {
    match parent_kind {
        "classDefinition" => Some(CangjieSymbolKind::Class),
        "interfaceDefinition" => Some(CangjieSymbolKind::Interface),
        "functionDefinition" => Some(CangjieSymbolKind::Function),
        "mainDefinition" => Some(CangjieSymbolKind::Function),
        "macroDefinition" => Some(CangjieSymbolKind::Macro),
        "structDefinition" => Some(CangjieSymbolKind::Struct),
        "typeAlias" => Some(CangjieSymbolKind::TypeAlias),
        "enumDefinition" => Some(CangjieSymbolKind::Enum),
        _ => None,
    }
}

/// Extract top-level symbols from a tree-sitter-cangjie parse tree.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn extract_cangjie_symbols_from_tree(
    source: &str,
    tree: &tree_sitter::Tree,
) -> Result<Vec<CangjieSymbol>, CangjieParseError> {
    use tree_sitter::StreamingIterator;

    extern "C" {
        fn tree_sitter_cangjie() -> tree_sitter::Language;
    }
    let language = unsafe { tree_sitter_cangjie() };

    let query = tree_sitter::Query::new(&language, SYMBOL_QUERY)
        .map_err(|e| CangjieParseError::ParseFailed(format!("query compile: {e}")))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let name_capture_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "name")
        .expect("query has @name capture") as u32;

    let mut symbols: Vec<CangjieSymbol> = Vec::new();

    matches.advance();
    while let Some(m) = matches.get() {
        for capture in m.captures {
            if capture.index == name_capture_idx {
                let name_node = capture.node;
                let name = name_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }

                // The parent of the name node is the definition node.
                if let Some(parent) = name_node.parent() {
                    let kind_str: &str = parent.kind();
                    if let Some(kind) = classify_symbol(kind_str) {
                        let start_line = name_node.start_position().row + 1;
                        let end_line = parent.end_position().row + 1;
                        symbols.push(CangjieSymbol {
                            kind,
                            name,
                            start_line,
                            end_line,
                        });
                    }
                }
            }
        }
        matches.advance();
    }

    Ok(symbols)
}

/// Extract top-level symbols from Cangjie source code.
///
/// Parses the source with tree-sitter-cangjie, then runs query-based
/// symbol extraction.  Returns [`CangjieParseError::HasErrorNodes`] if
/// the parse tree contains ERROR nodes (the symbols are still returned
/// in that case via `extract_cangjie_symbols_from_tree` — this function
/// chooses to be strict).
#[cfg(feature = "tree-sitter-cangjie")]
pub fn extract_cangjie_symbols(source: &str) -> Result<Vec<CangjieSymbol>, CangjieParseError> {
    let tree = super::parse_cangjie_source(source)?;
    extract_cangjie_symbols_from_tree(source, &tree)
}

#[cfg(test)]
mod tests {
    // These tests run only when the feature is enabled.
    #[cfg(feature = "tree-sitter-cangjie")]
    mod with_feature {
        use crate::extractors::symbol::{
            extract_cangjie_symbols, CangjieSymbol, CangjieSymbolKind,
        };

        fn extract(source: &str) -> Vec<CangjieSymbol> {
            extract_cangjie_symbols(source).expect("parse should succeed for test source")
        }

        #[test]
        fn empty_source_returns_no_symbols() {
            let syms = extract("");
            assert!(syms.is_empty());
        }

        #[test]
        fn function_definition() {
            let syms = extract(
                r#"
func hello(): Int64 {
    return 0
}
"#,
            );
            assert_eq!(syms.len(), 1);
            assert_eq!(syms[0].kind, CangjieSymbolKind::Function);
            assert_eq!(syms[0].name, "hello");
            assert!(syms[0].start_line >= 1);
        }

        #[test]
        fn class_definition() {
            let syms = extract(
                r#"
open class Foo {
    var x: Int64
    public init(x: Int64) {
        this.x = x
    }
}
"#,
            );
            let classes: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Class)
                .collect();
            assert_eq!(classes.len(), 1);
            assert_eq!(classes[0].name, "Foo");
        }

        #[test]
        fn struct_definition() {
            let syms = extract(
                r#"
struct Point {
    var x: Float64
    var y: Float64
}
"#,
            );
            let structs: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Struct)
                .collect();
            assert_eq!(structs.len(), 1);
            assert_eq!(structs[0].name, "Point");
        }

        #[test]
        fn enum_definition() {
            let syms = extract(
                r#"
enum Color {
    | Red, Green, Blue
}
"#,
            );
            let enums: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Enum)
                .collect();
            assert_eq!(enums.len(), 1);
            assert_eq!(enums[0].name, "Color");
        }

        #[test]
        fn interface_definition() {
            let syms = extract(
                r#"
interface Drawable {
    func draw(): Unit
}
"#,
            );
            let interfaces: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Interface)
                .collect();
            assert_eq!(interfaces.len(), 1);
            assert_eq!(interfaces[0].name, "Drawable");
        }

        #[test]
        fn type_alias() {
            let syms = extract(
                r#"
type MyInt = Int64
"#,
            );
            let aliases: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::TypeAlias)
                .collect();
            assert_eq!(aliases.len(), 1);
            assert_eq!(aliases[0].name, "MyInt");
        }

        #[test]
        fn macro_not_supported_by_grammar() {
            // The tree-sitter-cangjie grammar does not parse `macro name(...) { ... }`
            // as a macroDefinition — it produces an ERROR node.  This is a known
            // upstream limitation (the grammar only supports `macro package` declarations).
            // When the grammar is fixed upstream, re-run this test with the correct syntax.
            let source = "macro square(x: Int64): Int64 { return x * x }";
            // Expect either parse failure or no macro symbols from the query.
            let result = extract_cangjie_symbols(source);
            match result {
                Ok(syms) => {
                    let macros: Vec<_> = syms
                        .iter()
                        .filter(|s| s.kind == CangjieSymbolKind::Macro)
                        .collect();
                    // Currently 0 — update when grammar supports this syntax.
                    assert_eq!(
                        macros.len(),
                        0,
                        "grammar now supports macro definition syntax — update test"
                    );
                }
                Err(_) => {
                    // Parse may also fail if ERROR nodes are present.
                }
            }
        }

        #[test]
        fn multiple_definitions() {
            let syms = extract(
                r#"
func one(): Int64 { return 1 }
class A {}
struct B {}
func two(): Int64 { return 2 }
"#,
            );
            let funcs: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Function)
                .collect();
            let classes: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Class)
                .collect();
            let structs: Vec<_> = syms
                .iter()
                .filter(|s| s.kind == CangjieSymbolKind::Struct)
                .collect();
            assert_eq!(funcs.len(), 2);
            assert_eq!(classes.len(), 1);
            assert_eq!(structs.len(), 1);
            assert_eq!(syms.len(), 4);
        }

        #[test]
        fn fixture_cjpm_basic_main() {
            use std::path::PathBuf;
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop();
            path.pop();
            path.push("fixtures");
            path.push("cangjie");
            path.push("cjpm-basic");
            path.push("src");
            path.push("main.cj");
            let source = std::fs::read_to_string(&path).expect("read fixture");
            let syms = extract_cangjie_symbols(&source).expect("parse fixture");
            assert!(!syms.is_empty(), "expected at least one symbol (main)");
            let main = syms.iter().find(|s| s.name == "main");
            assert!(main.is_some(), "expected main function");
            assert_eq!(main.unwrap().kind, CangjieSymbolKind::Function);
        }
    }
}
