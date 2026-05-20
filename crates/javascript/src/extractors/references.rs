//! JavaScript 引用/调用抽取。
//!
//! 提取函数调用、成员访问等引用信息。

/// 引用类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsReferenceKind {
    Call,
    MemberAccess,
}

impl std::fmt::Display for JsReferenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call => write!(f, "call"),
            Self::MemberAccess => write!(f, "memberAccess"),
        }
    }
}

/// 从 JavaScript 源文件提取的引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsReference {
    pub name: String,
    pub kind: JsReferenceKind,
    pub line: usize,
}

/// 从 JavaScript 源码提取所有引用。
#[cfg(feature = "tree-sitter-javascript")]
pub fn extract_js_references(source: &str, lang: super::JsLanguage) -> Vec<JsReference> {
    let mut parser = match super::try_init_js_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut refs = Vec::new();
    collect_refs(&root, source, &mut refs);
    refs
}

#[cfg(feature = "tree-sitter-javascript")]
fn collect_refs(node: &tree_sitter::Node, source: &str, refs: &mut Vec<JsReference>) {
    if node.kind() == "call_expression" {
        if let Some(callee) = node.child(0) {
            let name = callee_name(&callee, source);
            if !name.is_empty() {
                refs.push(JsReference {
                    name,
                    kind: JsReferenceKind::Call,
                    line: node.start_position().row + 1,
                });
            }
        }
    }
    for i in 0..node.child_count() {
        collect_refs(&node.child(i as u32).unwrap(), source, refs);
    }
}

#[cfg(feature = "tree-sitter-javascript")]
fn callee_name(node: &tree_sitter::Node, source: &str) -> String {
    match node.kind() {
        "identifier" => source[node.byte_range()].to_string(),
        "member_expression" => {
            let obj = node
                .child(0)
                .map(|n| source[n.byte_range()].to_string())
                .unwrap_or_default();
            let prop = node
                .child(2)
                .map(|n| source[n.byte_range()].to_string())
                .unwrap_or_default();
            if obj.is_empty() {
                prop
            } else {
                format!("{obj}.{prop}")
            }
        }
        _ => String::new(),
    }
}
