//! Python import extraction from tree-sitter-python parse trees.

/// Kind of import statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PythonImportKind {
    /// `import module`
    Import,
    /// `import module as alias`
    ImportAs,
    /// `from module import name`
    FromImport,
    /// `from module import name as alias`
    FromImportAs,
    /// `from .module import name` (relative)
    RelativeImport,
    /// `from module import *` (star import — low confidence)
    StarImport,
}

/// An import extracted from a Python source file.
#[derive(Debug, Clone, PartialEq)]
pub struct PythonImport {
    /// Module path being imported.
    pub module_path: String,
    /// Specific name imported (for from-import).
    pub imported_name: Option<String>,
    /// Alias name (for as-import).
    pub alias: Option<String>,
    /// Kind of import.
    pub kind: PythonImportKind,
    /// Relative import level (number of dots, 0 = absolute).
    pub level: usize,
    /// 1-based line number.
    pub line: usize,
}

/// Returns empty vec when tree-sitter-python feature is disabled.
#[cfg(not(feature = "tree-sitter-python"))]
pub fn extract_python_imports(_source: &str) -> Vec<PythonImport> {
    vec![]
}

#[cfg(feature = "tree-sitter-python")]
pub fn extract_python_imports(source: &str) -> Vec<PythonImport> {
    let mut parser = match super::try_init_python_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut imports = Vec::new();
    collect_imports(&root, source, &mut imports);
    imports
}

#[cfg(feature = "tree-sitter-python")]
fn collect_imports(node: &tree_sitter::Node, source: &str, imports: &mut Vec<PythonImport>) {
    match node.kind() {
        "import_statement" => {
            extract_import_statement(node, source, imports);
        }
        "import_from_statement" => {
            extract_from_import_statement(node, source, imports);
        }
        _ => {}
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_imports(&child, source, imports);
    }
}

#[cfg(feature = "tree-sitter-python")]
fn extract_import_statement(
    node: &tree_sitter::Node,
    source: &str,
    imports: &mut Vec<PythonImport>,
) {
    // import module [as alias]
    // import module1, module2
    let line = node.start_position().row + 1;
    let mut cursor = node.walk();

    // Collect all (name, optional alias) pairs
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();
    let mut i = 0;
    while i < children.len() {
        let child = children[i];
        if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
            if child.kind() == "aliased_import" {
                // aliased_import contains: dotted_name, "as", identifier
                let mut ac = child.walk();
                let parts: Vec<tree_sitter::Node> = child.children(&mut ac).collect();
                let module = parts
                    .iter()
                    .find(|p| p.kind() == "dotted_name")
                    .map(|p| source[p.byte_range()].to_string())
                    .unwrap_or_default();
                let alias = parts
                    .iter()
                    .find(|p| p.kind() == "identifier")
                    .map(|p| source[p.byte_range()].to_string());

                imports.push(PythonImport {
                    module_path: module,
                    imported_name: None,
                    alias,
                    kind: PythonImportKind::ImportAs,
                    level: 0,
                    line,
                });
            } else {
                let module = source[child.byte_range()].to_string();
                // Check if next sibling is "as"
                if i + 2 < children.len()
                    && children[i + 1].kind() == "as"
                    && children[i + 2].kind() == "identifier"
                {
                    let alias = source[children[i + 2].byte_range()].to_string();
                    imports.push(PythonImport {
                        module_path: module,
                        imported_name: None,
                        alias: Some(alias),
                        kind: PythonImportKind::ImportAs,
                        level: 0,
                        line,
                    });
                    i += 3;
                    continue;
                }
                imports.push(PythonImport {
                    module_path: module,
                    imported_name: None,
                    alias: None,
                    kind: PythonImportKind::Import,
                    level: 0,
                    line,
                });
            }
        }
        i += 1;
    }
}

#[cfg(feature = "tree-sitter-python")]
fn extract_from_import_statement(
    node: &tree_sitter::Node,
    source: &str,
    imports: &mut Vec<PythonImport>,
) {
    // from [.]module import name [as alias]
    // from [.]module import name1, name2
    // from [.]module import *
    let line = node.start_position().row + 1;
    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    // Count dots for relative level
    let level = children.iter().filter(|c| c.kind() == ".").count();

    // Get module name (dotted_name)
    let module = children
        .iter()
        .find(|c| c.kind() == "dotted_name")
        .map(|c| source[c.byte_range()].to_string())
        .unwrap_or_default();

    // Check for star import
    if children.iter().any(|c| c.kind() == "wildcard_import") {
        imports.push(PythonImport {
            module_path: module,
            imported_name: Some("*".to_string()),
            alias: None,
            kind: PythonImportKind::StarImport,
            level,
            line,
        });
        return;
    }

    // Collect imported names (identifier or aliased_import after "import" keyword)
    let after_import = children.iter().position(|c| c.kind() == "import");
    if let Some(start_idx) = after_import {
        let mut i = start_idx + 1;
        while i < children.len() {
            let child = children[i];
            match child.kind() {
                "identifier" => {
                    let name = source[child.byte_range()].to_string();
                    // Check for "as" alias
                    if i + 2 < children.len()
                        && children[i + 1].kind() == "as"
                        && children[i + 2].kind() == "identifier"
                    {
                        let alias = source[children[i + 2].byte_range()].to_string();
                        imports.push(PythonImport {
                            module_path: module.clone(),
                            imported_name: Some(name),
                            alias: Some(alias),
                            kind: if level > 0 {
                                PythonImportKind::RelativeImport
                            } else {
                                PythonImportKind::FromImportAs
                            },
                            level,
                            line,
                        });
                        i += 3;
                        continue;
                    }
                    imports.push(PythonImport {
                        module_path: module.clone(),
                        imported_name: Some(name),
                        alias: None,
                        kind: if level > 0 {
                            PythonImportKind::RelativeImport
                        } else {
                            PythonImportKind::FromImport
                        },
                        level,
                        line,
                    });
                }
                "aliased_import" => {
                    // name as alias
                    let mut ac = child.walk();
                    let parts: Vec<tree_sitter::Node> = child.children(&mut ac).collect();
                    let name = parts
                        .iter()
                        .find(|p| p.kind() == "identifier")
                        .map(|p| source[p.byte_range()].to_string())
                        .unwrap_or_default();
                    // Second identifier after "as" keyword
                    let alias = parts
                        .iter()
                        .filter(|p| p.kind() == "identifier")
                        .nth(1)
                        .map(|p| source[p.byte_range()].to_string());
                    imports.push(PythonImport {
                        module_path: module.clone(),
                        imported_name: Some(name),
                        alias,
                        kind: if level > 0 {
                            PythonImportKind::RelativeImport
                        } else {
                            PythonImportKind::FromImportAs
                        },
                        level,
                        line,
                    });
                }
                _ => {}
            }
            i += 1;
        }
    }
}
