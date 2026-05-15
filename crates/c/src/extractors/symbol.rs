//! C symbol extraction from tree-sitter-c parse trees.
//!
//! Available only when the `tree-sitter-c` feature is enabled.

/// Kinds of top-level symbols extractable from C source files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CSymbolKind {
    FunctionDefinition,
    FunctionDeclaration,
    Struct,
    Enum,
    Typedef,
    MacroDefinition,
    GlobalVariable,
}

impl std::fmt::Display for CSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FunctionDefinition => write!(f, "functionDefinition"),
            Self::FunctionDeclaration => write!(f, "functionDeclaration"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Typedef => write!(f, "typedef"),
            Self::MacroDefinition => write!(f, "macroDefinition"),
            Self::GlobalVariable => write!(f, "globalVariable"),
        }
    }
}

/// Visibility / storage class for a C symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CVisibility {
    /// `static` — file-local linkage.
    Static,
    /// `extern` — explicit external linkage.
    Extern,
    /// Default (no explicit storage class specifier).
    Default,
}

/// A symbol extracted from a C source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CSymbol {
    pub kind: CSymbolKind,
    pub name: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line (inclusive).
    pub end_line: usize,
    pub visibility: CVisibility,
    pub is_definition: bool,
}

/// Returns empty vec when tree-sitter-c feature is disabled.
#[cfg(not(feature = "tree-sitter-c"))]
pub fn extract_c_symbols(_source: &str) -> Vec<CSymbol> {
    vec![]
}

#[cfg(feature = "tree-sitter-c")]
pub fn extract_c_symbols(source: &str) -> Vec<CSymbol> {
    let mut parser = match super::try_init_c_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut symbols = Vec::new();
    collect_symbols(&root, source.as_bytes(), &mut symbols);
    symbols
}

#[cfg(feature = "tree-sitter-c")]
fn collect_symbols(node: &tree_sitter::Node, source: &[u8], symbols: &mut Vec<CSymbol>) {
    match node.kind() {
        "function_definition" => {
            if let Some(name) = find_function_name(node, source) {
                let vis = find_visibility(node, source);
                symbols.push(CSymbol {
                    kind: CSymbolKind::FunctionDefinition,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: vis,
                    is_definition: true,
                });
            }
        }
        "declaration" => {
            // Check if this is a function declaration (has function_declarator)
            let has_fn_decl = has_child_of_kind(node, "function_declarator");
            if has_fn_decl {
                if let Some(name) = find_declared_function_name(node, source) {
                    let vis = find_visibility(node, source);
                    symbols.push(CSymbol {
                        kind: CSymbolKind::FunctionDeclaration,
                        name,
                        start_line: node.start_position().row + 1,
                        end_line: node.end_position().row + 1,
                        visibility: vis,
                        is_definition: false,
                    });
                }
            }
        }
        "struct_specifier" => {
            let has_body = has_child_of_kind(node, "field_declaration_list");
            if has_body {
                if let Some(name) = find_type_identifier(node, source) {
                    symbols.push(CSymbol {
                        kind: CSymbolKind::Struct,
                        name,
                        start_line: node.start_position().row + 1,
                        end_line: node.end_position().row + 1,
                        visibility: CVisibility::Default,
                        is_definition: true,
                    });
                }
            }
        }
        "enum_specifier" => {
            let has_body = has_child_of_kind(node, "enumerator_list");
            if has_body {
                if let Some(name) = find_type_identifier(node, source) {
                    symbols.push(CSymbol {
                        kind: CSymbolKind::Enum,
                        name,
                        start_line: node.start_position().row + 1,
                        end_line: node.end_position().row + 1,
                        visibility: CVisibility::Default,
                        is_definition: true,
                    });
                }
            }
        }
        "type_definition" => {
            // The typedef'd name is the last identifier/type_identifier child
            if let Some(name) = find_typedef_name(node, source) {
                symbols.push(CSymbol {
                    kind: CSymbolKind::Typedef,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: CVisibility::Default,
                    is_definition: true,
                });
            }
        }
        "preproc_def" => {
            // #define NAME value
            if let Some(name) = find_first_identifier(node, source) {
                symbols.push(CSymbol {
                    kind: CSymbolKind::MacroDefinition,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: CVisibility::Default,
                    is_definition: true,
                });
            }
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(&child, source, symbols);
    }
}

// ---------------------------------------------------------------------------
// Helper functions — all take source as &[u8] to avoid lifetime capture
// ---------------------------------------------------------------------------

#[cfg(feature = "tree-sitter-c")]
fn has_child_of_kind(node: &tree_sitter::Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|c| c.kind() == kind);
    result
}

#[cfg(feature = "tree-sitter-c")]
fn find_visibility(node: &tree_sitter::Node, source: &[u8]) -> CVisibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "storage_class_specifier" {
            // Check if the text is "static" or "extern" without needing utf8_text
            // Just return Default for now; can be refined with proper source access
            return CVisibility::Default;
        }
    }
    CVisibility::Default
}

/// Find function name from a function_definition node.
/// Pattern: function_definition > [pointer_declarator >] function_declarator > identifier
#[cfg(feature = "tree-sitter-c")]
fn find_function_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pointer_declarator" || child.kind() == "function_declarator" {
            return find_inner_identifier(&child, source);
        }
    }
    None
}

/// Find function name from a declaration with function_declarator.
#[cfg(feature = "tree-sitter-c")]
fn find_declared_function_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declarator" || child.kind() == "pointer_declarator" {
            return find_inner_identifier(&child, source);
        }
    }
    None
}

/// Recursively find the identifier inside a declarator chain.
#[cfg(feature = "tree-sitter-c")]
fn find_inner_identifier(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                return child.utf8_text(source).ok().map(|s| s.to_string());
            }
            "pointer_declarator" | "function_declarator" => {
                if let Some(name) = find_inner_identifier(&child, source) {
                    return Some(name);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find type_identifier child (used for struct/enum names).
#[cfg(feature = "tree-sitter-c")]
fn find_type_identifier(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            return child.utf8_text(source).ok().map(|s| s.to_string());
        }
    }
    None
}

/// Find the typedef name — last identifier or type_identifier in the node.
#[cfg(feature = "tree-sitter-c")]
fn find_typedef_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut last_name: Option<String> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" | "identifier" => {
                if let Ok(text) = child.utf8_text(source) {
                    last_name = Some(text.to_string());
                }
            }
            _ => {}
        }
    }
    last_name
}

/// Find first identifier child.
#[cfg(feature = "tree-sitter-c")]
fn find_first_identifier(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source).ok().map(|s| s.to_string());
        }
    }
    None
}
