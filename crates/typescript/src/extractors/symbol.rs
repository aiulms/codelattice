//! AST-level symbol extraction from tree-sitter-typescript parse trees.
//!
//! Available only when the `tree-sitter-typescript` feature is enabled.

/// Kinds of top-level symbols extractable from TypeScript/TSX source files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TsSymbolKind {
    Function,
    Class,
    Interface,
    Enum,
    TypeAlias,
    Variable,
    Namespace,
    Method,
    Property,
    /// ArkTS-specific: struct component decorated with @Component/@Entry.
    Component,
    /// ArkTS-specific: build() method inside a struct component.
    BuildMethod,
    /// ArkTS-specific: state-decorated property (@State, @Local, @Prop, etc.).
    StateProperty,
}

impl std::fmt::Display for TsSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Class => write!(f, "class"),
            Self::Interface => write!(f, "interface"),
            Self::Enum => write!(f, "enum"),
            Self::TypeAlias => write!(f, "typeAlias"),
            Self::Variable => write!(f, "variable"),
            Self::Namespace => write!(f, "namespace"),
            Self::Method => write!(f, "method"),
            Self::Property => write!(f, "property"),
            Self::Component => write!(f, "component"),
            Self::BuildMethod => write!(f, "buildMethod"),
            Self::StateProperty => write!(f, "stateProperty"),
        }
    }
}

/// A symbol extracted from a TypeScript source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsSymbol {
    pub kind: TsSymbolKind,
    pub name: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line (inclusive).
    pub end_line: usize,
    /// Owning type name (for methods/properties, e.g., "UserService").
    pub owner_name: Option<String>,
}

#[cfg(feature = "tree-sitter-typescript")]
use super::TsLanguage;

/// Extract all symbols from a TypeScript source string.
#[cfg(feature = "tree-sitter-typescript")]
pub fn extract_ts_symbols(source: &str, lang: TsLanguage) -> Vec<TsSymbol> {
    let mut parser = match super::try_init_ts_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut symbols = Vec::new();
    collect_symbols(&root, source, None, &mut symbols);
    symbols
}

#[cfg(feature = "tree-sitter-typescript")]
fn collect_symbols(
    node: &tree_sitter::Node,
    source: &str,
    owner: Option<&str>,
    symbols: &mut Vec<TsSymbol>,
) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = first_child_text(node, "identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Function,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "class_declaration" | "class" => {
            if let Some(name) = first_child_text(node, "type_identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Class,
                    name: name.clone(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
                // Recurse into class body for methods and properties
                if let Some(body) = node.child_by_field_name("body") {
                    collect_symbols(&body, source, Some(&name), symbols);
                }
                return; // Don't double-recurse
            }
        }
        "interface_declaration" => {
            if let Some(name) = first_child_text(node, "type_identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Interface,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "enum_declaration" => {
            if let Some(name) = first_child_text(node, "identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Enum,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = first_child_text(node, "type_identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::TypeAlias,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            // Only extract top-level variable declarations (not inside functions)
            if owner.is_none() {
                if let Some(name) = first_child_text(node, "identifier", source) {
                    symbols.push(TsSymbol {
                        kind: TsSymbolKind::Variable,
                        name,
                        start_line: node.start_position().row + 1,
                        end_line: node.end_position().row + 1,
                        owner_name: None,
                    });
                }
            }
        }
        "method_definition" => {
            if let Some(name) = first_child_text(node, "property_identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Method,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "public_field_definition" => {
            if let Some(name) = first_child_text(node, "property_identifier", source) {
                symbols.push(TsSymbol {
                    kind: TsSymbolKind::Property,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                });
            }
        }
        "namespace_import" | "import_namespace" => {
            // Not a definition symbol
        }
        _ => {}
    }

    // Recurse into children
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        collect_symbols(&child, source, owner, symbols);
    }
}

/// Get the text of the first child node matching a given kind.
#[cfg(feature = "tree-sitter-typescript")]
fn first_child_text<'a>(node: &tree_sitter::Node, kind: &str, source: &'a str) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == kind {
            return Some(source[child.byte_range()].to_string());
        }
    }
    None
}
