//! Import extraction from tree-sitter-typescript parse trees.
//!
//! Extracts import statements from TypeScript/TSX/ArkTS source files.


/// An import statement extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsImport {
    /// The imported module path (string literal).
    pub module_path: String,
    /// Imported names (empty for side-effect-only imports like `import "foo"`).
    pub imported_names: Vec<String>,
    /// Whether this is a default import.
    pub is_default: bool,
    /// Whether this is a namespace import (`import * as X`).
    pub is_namespace: bool,
    /// Local alias for namespace import.
    pub namespace_alias: Option<String>,
    /// 1-based line number.
    pub line: usize,
    /// Whether the module path is relative (starts with . or ..).
    pub is_relative: bool,
}

/// Extract all import statements from a TypeScript source string.
#[cfg(feature = "tree-sitter-typescript")]
pub fn extract_ts_imports(
    source: &str,
    lang: super::TsLanguage,
) -> Vec<TsImport> {
    let mut parser = match super::try_init_ts_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut imports = Vec::new();

    for i in 0..root.child_count() {
        let child = root.child(i as u32).unwrap();
        if child.kind() == "import_statement" {
            if let Some(imp) = parse_import_node(&child, source) {
                imports.push(imp);
            }
        }
    }
    imports
}

#[cfg(feature = "tree-sitter-typescript")]
fn parse_import_node(node: &tree_sitter::Node, source: &str) -> Option<TsImport> {
    let line = node.start_position().row + 1;

    // Find the module path (the string at the end)
    let module_path = find_module_path(node, source)?;

    let is_relative = module_path.starts_with('.') || module_path.starts_with('/');
    let mut imported_names = Vec::new();
    let mut is_default = false;
    let mut is_namespace = false;
    let mut namespace_alias = None;

    // Walk children to extract imported names
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        match child.kind() {
            "import_clause" => {
                for j in 0..child.child_count() {
                    let c = child.child(j as u32).unwrap();
                    match c.kind() {
                        "identifier" => {
                            // Default import: import X from "..."
                            is_default = true;
                            imported_names.push(source[c.byte_range()].to_string());
                        }
                        "named_imports" => {
                            // import { X, Y } from "..."
                            extract_named_imports(&c, source, &mut imported_names);
                        }
                        "namespace_import" => {
                            // import * as X from "..."
                            is_namespace = true;
                            // The alias is the identifier after "as"
                            for k in 0..c.child_count() {
                                let nc = c.child(k as u32).unwrap();
                                if nc.kind() == "identifier" {
                                    namespace_alias =
                                        Some(source[nc.byte_range()].to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Some(TsImport {
        module_path,
        imported_names,
        is_default,
        is_namespace,
        namespace_alias,
        line,
        is_relative,
    })
}

#[cfg(feature = "tree-sitter-typescript")]
fn find_module_path(node: &tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "string" {
            // The string content is inside string_fragment child
            for j in 0..child.child_count() {
                let c = child.child(j as u32).unwrap();
                if c.kind() == "string_fragment" {
                    return Some(source[c.byte_range()].to_string());
                }
            }
        }
    }
    None
}

#[cfg(feature = "tree-sitter-typescript")]
fn extract_named_imports(
    node: &tree_sitter::Node,
    source: &str,
    names: &mut Vec<String>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "import_specifier" {
            for j in 0..child.child_count() {
                let c = child.child(j as u32).unwrap();
                if c.kind() == "identifier" {
                    names.push(source[c.byte_range()].to_string());
                    break;
                }
            }
        }
    }
}
