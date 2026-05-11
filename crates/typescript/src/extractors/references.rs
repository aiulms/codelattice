//! Reference/call extraction from tree-sitter-typescript parse trees.
//!
//! Extracts function calls, member accesses, and type references.

/// Kind of reference extracted from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TsReferenceKind {
    /// Function/method call.
    Call,
    /// Type reference (type annotation, generic argument).
    TypeUse,
    /// Property access (member_expression).
    MemberAccess,
    /// New expression (constructor call).
    NewExpression,
}

impl std::fmt::Display for TsReferenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call => write!(f, "call"),
            Self::TypeUse => write!(f, "typeUse"),
            Self::MemberAccess => write!(f, "memberAccess"),
            Self::NewExpression => write!(f, "newExpression"),
        }
    }
}

/// A reference extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsReference {
    pub kind: TsReferenceKind,
    /// The referenced name (function name, type name, etc.).
    pub name: String,
    /// 1-based line number.
    pub line: usize,
    /// Full text of the reference expression (for member chains like `this.vm.aboutToAppear()`).
    pub full_text: Option<String>,
}

/// Extract references from TypeScript source.
#[cfg(feature = "tree-sitter-typescript")]
pub fn extract_ts_references(
    source: &str,
    lang: super::TsLanguage,
) -> Vec<TsReference> {
    let mut parser = match super::try_init_ts_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut refs = Vec::new();
    collect_references(&root, source, &mut refs);
    refs
}

#[cfg(feature = "tree-sitter-typescript")]
fn collect_references(
    node: &tree_sitter::Node,
    source: &str,
    refs: &mut Vec<TsReference>,
) {
    match node.kind() {
        "call_expression" => {
            let line = node.start_position().row + 1;
            let full_text = Some(source[node.byte_range()].to_string());
            // Get the function name from the first child
            if let Some(func) = node.child(0) {
                let name = extract_call_name(&func, source);
                refs.push(TsReference {
                    kind: TsReferenceKind::Call,
                    name,
                    line,
                    full_text,
                });
            }
        }
        "new_expression" => {
            let line = node.start_position().row + 1;
            // The constructor name is the identifier after "new"
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.kind() == "identifier" || child.kind() == "member_expression" {
                    let name = source[child.byte_range()].to_string();
                    refs.push(TsReference {
                        kind: TsReferenceKind::NewExpression,
                        name,
                        line,
                        full_text: Some(source[node.byte_range()].to_string()),
                    });
                    break;
                }
            }
        }
        "type_identifier" => {
            let line = node.start_position().row + 1;
            let name = source[node.byte_range()].to_string();
            // Only track if it looks like a user-defined type (PascalCase heuristic)
            if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                refs.push(TsReference {
                    kind: TsReferenceKind::TypeUse,
                    name,
                    line,
                    full_text: None,
                });
            }
            return; // Don't recurse into type_identifier children
        }
        _ => {}
    }

    for i in 0..node.child_count() {
        collect_references(&node.child(i).unwrap(), source, refs);
    }
}

#[cfg(feature = "tree-sitter-typescript")]
fn extract_call_name(node: &tree_sitter::Node, source: &str) -> String {
    match node.kind() {
        "identifier" => source[node.byte_range()].to_string(),
        "member_expression" => source[node.byte_range()].to_string(),
        "call_expression" => source[node.byte_range()].to_string(),
        _ => source[node.byte_range()].to_string(),
    }
}
