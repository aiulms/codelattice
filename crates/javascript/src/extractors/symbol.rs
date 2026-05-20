//! JavaScript/JSX 符号抽取。
//!
//! 覆盖：function declaration、arrow function、class、method、
//! object method shorthand、exported symbol、default export、
//! CommonJS `module.exports` / `exports.foo`。

/// JavaScript 符号类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsSymbolKind {
    Function,
    ArrowFunction,
    Class,
    Method,
    Property,
    Variable,
    Component,
}

impl std::fmt::Display for JsSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Function => "function",
            Self::ArrowFunction => "arrowFunction",
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Variable => "variable",
            Self::Component => "component",
        };
        write!(f, "{s}")
    }
}

/// 从 JavaScript 源文件抽取的符号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsSymbol {
    pub kind: JsSymbolKind,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub owner_name: Option<String>,
    pub is_async: bool,
    pub is_generator: bool,
    pub is_export: bool,
    pub is_default_export: bool,
}

/// 从 JavaScript 源码提取所有符号。
#[cfg(feature = "tree-sitter-javascript")]
pub fn extract_js_symbols(source: &str, lang: super::JsLanguage) -> Vec<JsSymbol> {
    let mut parser = match super::try_init_js_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut symbols = Vec::new();
    let mut exports = std::collections::HashSet::new();
    let mut default_export_name = None;
    collect_export_info(&root, source, &mut exports, &mut default_export_name);
    collect_symbols_recursive(
        &root,
        source,
        None,
        &mut symbols,
        &exports,
        &default_export_name,
    );
    symbols
}

#[cfg(feature = "tree-sitter-javascript")]
fn collect_export_info(
    node: &tree_sitter::Node,
    source: &str,
    exports: &mut std::collections::HashSet<String>,
    default_name: &mut Option<String>,
) {
    if node.kind() == "export_statement" {
        for i in 0..node.child_count() {
            let child = node.child(i as u32).unwrap();
            match child.kind() {
                "function_declaration" | "class_declaration" | "generator_function_declaration" => {
                    if let Some(name) = first_child_by_kind(&child, "identifier") {
                        let n = text_of(&name, source);
                        exports.insert(n.clone());
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    for j in 0..child.child_count() {
                        let vc = child.child(j as u32).unwrap();
                        if vc.kind() == "variable_declarator" {
                            if let Some(id) = first_child_by_kind(&vc, "identifier") {
                                exports.insert(text_of(&id, source));
                            }
                        }
                    }
                }
                "default" => {
                    // export default function/class
                    for j in 0..node.child_count() {
                        let dc = node.child(j as u32).unwrap();
                        if dc.kind() == "function_declaration" || dc.kind() == "class_declaration" {
                            if let Some(id) = first_child_by_kind(&dc, "identifier") {
                                let n = text_of(&id, source);
                                exports.insert(n.clone());
                                *default_name = Some(n);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // module.exports = ...
    if node.kind() == "assignment_expression" {
        if let Some(left) = node.child(0) {
            let lt = text_of(&left, source);
            if lt == "module.exports" {
                if let Some(right) = node.child(2) {
                    if right.kind() == "identifier" {
                        *default_name = Some(text_of(&right, source));
                    } else if right.kind() == "function" || right.kind() == "class" {
                        if let Some(id) = first_child_by_kind(&right, "identifier") {
                            *default_name = Some(text_of(&id, source));
                        }
                    }
                }
            }
        }
    }
    // exports.foo = ...
    if node.kind() == "assignment_expression" {
        if let Some(left) = node.child(0) {
            let lt = text_of(&left, source);
            if lt.starts_with("exports.") {
                if let Some(name) = lt.strip_prefix("exports.") {
                    if !name.is_empty() {
                        exports.insert(name.to_string());
                    }
                }
            }
        }
    }
    for i in 0..node.child_count() {
        collect_export_info(
            &node.child(i as u32).unwrap(),
            source,
            exports,
            default_name,
        );
    }
}

#[cfg(feature = "tree-sitter-javascript")]
fn collect_symbols_recursive(
    node: &tree_sitter::Node,
    source: &str,
    owner: Option<&str>,
    symbols: &mut Vec<JsSymbol>,
    exports: &std::collections::HashSet<String>,
    default_export_name: &Option<String>,
) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(id) = first_child_by_kind(node, "identifier") {
                let name = text_of(&id, source);
                symbols.push(JsSymbol {
                    kind: JsSymbolKind::Function,
                    is_export: exports.contains(&name),
                    is_default_export: default_export_name.as_deref() == Some(&name),
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                    is_async: has_child(node, "async"),
                    is_generator: node.kind() == "generator_function_declaration",
                });
            }
        }
        "class_declaration" => {
            if let Some(id) = first_child_by_kind(node, "identifier") {
                let name = text_of(&id, source);
                symbols.push(JsSymbol {
                    kind: JsSymbolKind::Class,
                    is_export: exports.contains(&name),
                    is_default_export: default_export_name.as_deref() == Some(&name),
                    name: name.clone(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                    is_async: false,
                    is_generator: false,
                });
                // 递归 class body 提取 method
                if let Some(body) = node.child_by_field_name("body") {
                    collect_symbols_recursive(
                        &body,
                        source,
                        Some(&name),
                        symbols,
                        exports,
                        default_export_name,
                    );
                }
                return;
            }
        }
        "method_definition" => {
            if let Some(prop) = first_child_by_kind(node, "property_identifier") {
                let name = text_of(&prop, source);
                symbols.push(JsSymbol {
                    kind: JsSymbolKind::Method,
                    is_export: false,
                    is_default_export: false,
                    name,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    owner_name: owner.map(String::from),
                    is_async: has_child(node, "async"),
                    is_generator: has_child(node, "*"),
                });
            }
        }
        "lexical_declaration" | "variable_declaration" if owner.is_none() => {
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.kind() == "variable_declarator" {
                    if let Some(id) = first_child_by_kind(&child, "identifier") {
                        let name = text_of(&id, source);
                        let (is_arrow, is_async) = classify_declarator(&child);
                        let kind = if is_arrow {
                            JsSymbolKind::ArrowFunction
                        } else {
                            JsSymbolKind::Variable
                        };
                        symbols.push(JsSymbol {
                            kind,
                            is_export: exports.contains(&name),
                            is_default_export: false,
                            name,
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            owner_name: None,
                            is_async,
                            is_generator: false,
                        });
                    }
                }
            }
        }
        "pair" => {
            if let Some(prop) = first_child_by_kind(node, "property_identifier") {
                let has_fn = has_child(node, "function") || has_child(node, "arrow_function");
                if has_fn {
                    symbols.push(JsSymbol {
                        kind: JsSymbolKind::Method,
                        is_export: false,
                        is_default_export: false,
                        name: text_of(&prop, source),
                        start_line: node.start_position().row + 1,
                        end_line: node.end_position().row + 1,
                        owner_name: owner.map(String::from),
                        is_async: false,
                        is_generator: false,
                    });
                }
            }
        }
        _ => {}
    }
    for i in 0..node.child_count() {
        collect_symbols_recursive(
            &node.child(i as u32).unwrap(),
            source,
            owner,
            symbols,
            exports,
            default_export_name,
        );
    }
}

#[cfg(feature = "tree-sitter-javascript")]
fn classify_declarator(declarator: &tree_sitter::Node) -> (bool, bool) {
    for i in 0..declarator.child_count() {
        let child = declarator.child(i as u32).unwrap();
        match child.kind() {
            "arrow_function" => return (true, has_child(&child, "async")),
            "function" => return (false, has_child(&child, "async")),
            _ => {}
        }
    }
    (false, false)
}

#[cfg(feature = "tree-sitter-javascript")]
fn first_child_by_kind<'a>(
    node: &'a tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

#[cfg(feature = "tree-sitter-javascript")]
fn text_of<'a>(node: &tree_sitter::Node<'a>, source: &'a str) -> String {
    source[node.byte_range()].to_string()
}

#[cfg(feature = "tree-sitter-javascript")]
fn has_child(node: &tree_sitter::Node, kind: &str) -> bool {
    for i in 0..node.child_count() {
        if node.child(i as u32).unwrap().kind() == kind {
            return true;
        }
    }
    false
}
