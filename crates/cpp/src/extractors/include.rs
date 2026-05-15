//! C++ include extraction from tree-sitter-cpp parse trees.

/// Kind of include directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CppIncludeKind {
    /// `#include "file.hpp"` — local/project include.
    Local,
    /// `#include <iostream>` — system/external include.
    System,
}

/// An include directive extracted from a C++ source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppInclude {
    pub path: String,
    pub kind: CppIncludeKind,
    pub line: usize,
}

/// Placeholder — returns empty vec when feature disabled.
#[cfg(not(feature = "tree-sitter-cpp"))]
pub fn extract_cpp_includes(_source: &str) -> Vec<CppInclude> {
    vec![]
}

#[cfg(feature = "tree-sitter-cpp")]
pub fn extract_cpp_includes(source: &str) -> Vec<CppInclude> {
    let mut parser = match super::try_init_cpp_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut includes = Vec::new();
    collect_includes(&root, source, &mut includes);
    includes
}

#[cfg(feature = "tree-sitter-cpp")]
fn collect_includes(node: &tree_sitter::Node, source: &str, includes: &mut Vec<CppInclude>) {
    if node.kind() == "preproc_include" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "string_literal" => {
                    let raw = &source[child.byte_range()];
                    let path = raw.trim_matches(|c| c == '"');
                    includes.push(CppInclude {
                        path: path.to_string(),
                        kind: CppIncludeKind::Local,
                        line: node.start_position().row + 1,
                    });
                }
                "system_lib_string" => {
                    let raw = &source[child.byte_range()];
                    let path = raw.trim_start_matches('<').trim_end_matches('>');
                    includes.push(CppInclude {
                        path: path.to_string(),
                        kind: CppIncludeKind::System,
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
