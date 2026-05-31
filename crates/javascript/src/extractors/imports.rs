//! JavaScript import/require 提取。
//!
//! 覆盖：
//! - ESM `import ... from "x"`, `import { ... } from "x"`, `import * as x from "x"`
//! - ESM `export { ... } from "x"`
//! - CommonJS `const x = require("x")`, `require("x")`
//! - CommonJS `module.exports = ...`, `exports.foo = ...`
//! - Dynamic `import("x")` → 记录为 diagnostic

/// JavaScript import 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsImportKind {
    EsmImport,
    EsmDynamicImport,
    CommonJsRequire,
    CommonJsModuleExports,
    CommonJsExportsAccess,
}

impl std::fmt::Display for JsImportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::EsmImport => "esm-import",
            Self::EsmDynamicImport => "esm-dynamic-import",
            Self::CommonJsRequire => "commonjs-require",
            Self::CommonJsModuleExports => "commonjs-module-exports",
            Self::CommonJsExportsAccess => "commonjs-exports-access",
        };
        write!(f, "{s}")
    }
}

/// 从 JavaScript 源文件提取的 import 语句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsImport {
    pub module_path: String,
    pub imported_names: Vec<String>,
    pub is_default: bool,
    pub is_namespace: bool,
    pub namespace_alias: Option<String>,
    pub line: usize,
    pub is_relative: bool,
    pub kind: JsImportKind,
}

/// 从 JavaScript 源码提取所有 import/require。
#[cfg(feature = "tree-sitter-javascript")]
pub fn extract_js_imports(source: &str, lang: super::JsLanguage) -> Vec<JsImport> {
    let mut parser = match super::try_init_js_parser(lang) {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    extract_js_imports_from_root(&root, source)
}

#[cfg(feature = "tree-sitter-javascript")]
pub(super) fn extract_js_imports_from_root(
    root: &tree_sitter::Node,
    source: &str,
) -> Vec<JsImport> {
    let mut imports = Vec::new();
    collect_imports(root, source, &mut imports);
    imports
}

#[cfg(feature = "tree-sitter-javascript")]
fn collect_imports(node: &tree_sitter::Node, source: &str, imports: &mut Vec<JsImport>) {
    match node.kind() {
        "import_statement" => {
            if let Some(imp) = parse_import_statement(node, source) {
                imports.push(imp);
            }
        }
        "call_expression" => {
            if let Some(imp) = parse_require_call(node, source) {
                imports.push(imp);
            } else if let Some(imp) = parse_dynamic_import(node, source) {
                imports.push(imp);
            }
        }
        "assignment_expression" => {
            if let Some(imp) = parse_module_exports(node, source) {
                imports.push(imp);
            } else if let Some(imp) = parse_exports_access(node, source) {
                imports.push(imp);
            }
        }
        _ => {}
    }
    for i in 0..node.child_count() {
        collect_imports(&node.child(i as u32).unwrap(), source, imports);
    }
}

#[cfg(feature = "tree-sitter-javascript")]
fn parse_import_statement(node: &tree_sitter::Node, source: &str) -> Option<JsImport> {
    let line = node.start_position().row + 1;
    let module_path = find_string_in_node(node, source)?;
    let is_relative = module_path.starts_with('.');
    let mut imported_names = Vec::new();
    let mut is_default = false;
    let mut is_namespace = false;
    let mut namespace_alias = None;

    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "import_clause" {
            for j in 0..child.child_count() {
                let c = child.child(j as u32).unwrap();
                match c.kind() {
                    "identifier" => {
                        is_default = true;
                        imported_names.push(source[c.byte_range()].to_string());
                    }
                    "named_imports" => {
                        extract_named_imports_from(&c, source, &mut imported_names);
                    }
                    "namespace_import" => {
                        is_namespace = true;
                        for k in 0..c.child_count() {
                            let nc = c.child(k as u32).unwrap();
                            if nc.kind() == "identifier" {
                                namespace_alias = Some(source[nc.byte_range()].to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Some(JsImport {
        module_path,
        imported_names,
        is_default,
        is_namespace,
        namespace_alias,
        line,
        is_relative,
        kind: JsImportKind::EsmImport,
    })
}

#[cfg(feature = "tree-sitter-javascript")]
fn parse_require_call(node: &tree_sitter::Node, source: &str) -> Option<JsImport> {
    let callee = node.child(0)?;
    let callee_text = &source[callee.byte_range()];
    if callee.kind() != "identifier" || callee_text != "require" {
        return None;
    }
    let args = node.child(1)?;
    if args.kind() != "arguments" {
        return None;
    }
    let module_path = find_string_in_node(&args, source)?;
    let is_relative = module_path.starts_with('.');
    Some(JsImport {
        module_path,
        imported_names: vec![],
        is_default: false,
        is_namespace: false,
        namespace_alias: None,
        line: node.start_position().row + 1,
        is_relative,
        kind: JsImportKind::CommonJsRequire,
    })
}

#[cfg(feature = "tree-sitter-javascript")]
fn parse_dynamic_import(node: &tree_sitter::Node, source: &str) -> Option<JsImport> {
    let callee = node.child(0)?;
    if callee.kind() != "import" {
        return None;
    }
    let args = node.child(1)?;
    if args.kind() != "arguments" {
        return None;
    }
    let module_path = find_string_in_node(&args, source)?;
    let is_relative = module_path.starts_with('.');
    Some(JsImport {
        module_path,
        imported_names: vec![],
        is_default: false,
        is_namespace: false,
        namespace_alias: None,
        line: node.start_position().row + 1,
        is_relative,
        kind: JsImportKind::EsmDynamicImport,
    })
}

#[cfg(feature = "tree-sitter-javascript")]
fn parse_module_exports(node: &tree_sitter::Node, source: &str) -> Option<JsImport> {
    let left = node.child(0)?;
    if &source[left.byte_range()] != "module.exports" {
        return None;
    }
    Some(JsImport {
        module_path: "module.exports".to_string(),
        imported_names: vec![],
        is_default: true,
        is_namespace: false,
        namespace_alias: None,
        line: node.start_position().row + 1,
        is_relative: false,
        kind: JsImportKind::CommonJsModuleExports,
    })
}

#[cfg(feature = "tree-sitter-javascript")]
fn parse_exports_access(node: &tree_sitter::Node, source: &str) -> Option<JsImport> {
    let left = node.child(0)?;
    let lt = source[left.byte_range()].to_string();
    if !lt.starts_with("exports.") {
        return None;
    }
    Some(JsImport {
        module_path: lt,
        imported_names: vec![],
        is_default: false,
        is_namespace: false,
        namespace_alias: None,
        line: node.start_position().row + 1,
        is_relative: false,
        kind: JsImportKind::CommonJsExportsAccess,
    })
}

#[cfg(feature = "tree-sitter-javascript")]
fn find_string_in_node(node: &tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "string" {
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

#[cfg(feature = "tree-sitter-javascript")]
fn extract_named_imports_from(node: &tree_sitter::Node, source: &str, names: &mut Vec<String>) {
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
