//! C include extraction from tree-sitter-c parse trees.

/// Kind of include directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CIncludeKind {
    /// `#include "file.h"` — local/project include.
    Local,
    /// `#include <stdio.h>` — system/external include.
    System,
}

/// An include directive extracted from a C source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CInclude {
    pub path: String,
    pub kind: CIncludeKind,
    pub line: usize,
}

/// Placeholder — returns empty vec when feature disabled.
#[cfg(not(feature = "tree-sitter-c"))]
pub fn extract_c_includes(_source: &str) -> Vec<CInclude> {
    vec![]
}

#[cfg(feature = "tree-sitter-c")]
pub fn extract_c_includes(source: &str) -> Vec<CInclude> {
    let mut parser = match super::try_init_c_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    extract_c_includes_from_root(&root, source)
}

#[cfg(feature = "tree-sitter-c")]
pub fn extract_c_includes_from_root(root: &tree_sitter::Node, source: &str) -> Vec<CInclude> {
    let mut includes = Vec::new();
    collect_includes(root, source, &mut includes);
    includes
}

#[cfg(feature = "tree-sitter-c")]
fn collect_includes(node: &tree_sitter::Node, source: &str, includes: &mut Vec<CInclude>) {
    if node.kind() == "preproc_include" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "string_literal" => {
                    let raw = &source[child.byte_range()];
                    // Strip surrounding quotes
                    let path = raw.trim_matches(|c| c == '"');
                    includes.push(CInclude {
                        path: path.to_string(),
                        kind: CIncludeKind::Local,
                        line: node.start_position().row + 1,
                    });
                }
                "system_lib_string" => {
                    let raw = &source[child.byte_range()];
                    // Strip < >
                    let path = raw.trim_start_matches('<').trim_end_matches('>');
                    includes.push(CInclude {
                        path: path.to_string(),
                        kind: CIncludeKind::System,
                        line: node.start_position().row + 1,
                    });
                }
                _ => {}
            }
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_includes(&child, source, includes);
    }
}
