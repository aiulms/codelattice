//! Python symbol extraction from tree-sitter-python parse trees.
//!
//! Available only when the `tree-sitter-python` feature is enabled.
//!
//! Extracts: module, function, async function, class, method, constructor,
//! variable/constant, decorator reference.

/// Kinds of symbols extractable from Python source files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PythonSymbolKind {
    Module,
    Function,
    AsyncFunction,
    Class,
    Method,
    Constructor,
    Variable,
    Constant,
    TestFunction,
    Decorator,
}

impl std::fmt::Display for PythonSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Module => write!(f, "module"),
            Self::Function => write!(f, "function"),
            Self::AsyncFunction => write!(f, "asyncFunction"),
            Self::Class => write!(f, "class"),
            Self::Method => write!(f, "method"),
            Self::Constructor => write!(f, "constructor"),
            Self::Variable => write!(f, "variable"),
            Self::Constant => write!(f, "constant"),
            Self::TestFunction => write!(f, "testFunction"),
            Self::Decorator => write!(f, "decorator"),
        }
    }
}

/// Visibility level for a Python symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PythonVisibility {
    /// Public (default, no leading underscore).
    Public,
    /// Private (single leading underscore).
    Private,
    /// Dunder (double leading/trailing underscore).
    Dunder,
}

impl std::fmt::Display for PythonVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Private => write!(f, "private"),
            Self::Dunder => write!(f, "dunder"),
        }
    }
}

/// A symbol extracted from a Python source file.
#[derive(Debug, Clone, PartialEq)]
pub struct PythonSymbol {
    /// Unique ID (deterministic, based on qualified name + file + line).
    pub id: String,
    /// Simple name.
    pub name: String,
    /// Qualified name (e.g., "sample_app.service.UserService.run").
    pub qualified_name: String,
    /// Symbol kind.
    pub kind: PythonSymbolKind,
    /// Source file path (relative or absolute).
    pub source_path: String,
    /// 1-based start line.
    pub line_start: usize,
    /// 1-based end line.
    pub line_end: usize,
    /// Visibility.
    pub visibility: PythonVisibility,
    /// Whether this is an async function/method.
    pub is_async: bool,
    /// Whether this is a test function (starts with "test_").
    pub is_test: bool,
    /// Decorators applied to this symbol.
    pub decorators: Vec<String>,
}

/// Returns empty vec when tree-sitter-python feature is disabled.
#[cfg(not(feature = "tree-sitter-python"))]
pub fn extract_python_symbols(_source: &str, _file_path: &str) -> Vec<PythonSymbol> {
    vec![]
}

#[cfg(feature = "tree-sitter-python")]
pub fn extract_python_symbols(source: &str, file_path: &str) -> Vec<PythonSymbol> {
    let mut parser = match super::try_init_python_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut symbols = Vec::new();
    let mut class_stack: Vec<String> = Vec::new();
    collect_symbols(&root, source, file_path, &mut class_stack, &mut symbols);
    symbols
}

#[cfg(feature = "tree-sitter-python")]
fn collect_symbols(
    node: &tree_sitter::Node,
    source: &str,
    file_path: &str,
    class_stack: &mut Vec<String>,
    symbols: &mut Vec<PythonSymbol>,
) {
    match node.kind() {
        "function_definition" | "decorated_definition" => {
            // Handle decorated definitions — the actual function/class is a child
            let mut target_node = *node;
            let mut decorators = Vec::new();

            if node.kind() == "decorated_definition" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "decorator" => {
                            let text = &source[child.byte_range()];
                            // Extract decorator name (strip @)
                            let dec_text = text.trim().trim_start_matches('@');
                            let dec_name = dec_text.split('(').next().unwrap_or(dec_text).trim();
                            decorators.push(dec_name.to_string());
                        }
                        "function_definition" | "class_definition" => {
                            target_node = child;
                        }
                        _ => {}
                    }
                }
            }

            if target_node.kind() == "function_definition" {
                extract_function_symbol(
                    &target_node,
                    source,
                    file_path,
                    class_stack,
                    &decorators,
                    symbols,
                );
            } else if target_node.kind() == "class_definition" {
                extract_class_symbol(
                    &target_node,
                    source,
                    file_path,
                    class_stack,
                    &decorators,
                    symbols,
                );

                // Recurse into class body for methods
                let class_name = node_text(&target_node, source, "name")
                    .unwrap_or_else(|| "Unknown".to_string());
                class_stack.push(class_name);
                let mut cursor = target_node.walk();
                for child in target_node.children(&mut cursor) {
                    if child.kind() == "block" {
                        let mut block_cursor = child.walk();
                        for block_child in child.children(&mut block_cursor) {
                            collect_symbols(&block_child, source, file_path, class_stack, symbols);
                        }
                    }
                }
                class_stack.pop();
                return; // Already recursed
            }
        }
        "class_definition" => {
            extract_class_symbol(node, source, file_path, class_stack, &[], symbols);

            let class_name =
                node_text(node, source, "name").unwrap_or_else(|| "Unknown".to_string());
            class_stack.push(class_name);
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "block" {
                    let mut block_cursor = child.walk();
                    for block_child in child.children(&mut block_cursor) {
                        collect_symbols(&block_child, source, file_path, class_stack, symbols);
                    }
                }
            }
            class_stack.pop();
            return; // Already recursed
        }
        "expression_statement" => {
            // Top-level assignments — variable/constant
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "assignment" {
                    extract_assignment_symbol(child, source, file_path, symbols);
                }
            }
        }
        _ => {}
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(&child, source, file_path, class_stack, symbols);
    }
}

#[cfg(feature = "tree-sitter-python")]
fn extract_function_symbol(
    node: &tree_sitter::Node,
    source: &str,
    file_path: &str,
    class_stack: &[String],
    decorators: &[String],
    symbols: &mut Vec<PythonSymbol>,
) {
    let name = match node_text(node, source, "name") {
        Some(n) => n,
        None => return,
    };

    let is_async = node.children(&mut node.walk()).any(|c| c.kind() == "async");

    let is_test = name.starts_with("test_");
    let is_constructor = name == "__init__";

    let in_class = !class_stack.is_empty();

    let kind = if is_constructor {
        PythonSymbolKind::Constructor
    } else if in_class {
        if is_test {
            PythonSymbolKind::TestFunction
        } else {
            PythonSymbolKind::Method
        }
    } else if is_async {
        PythonSymbolKind::AsyncFunction
    } else if is_test {
        PythonSymbolKind::TestFunction
    } else {
        PythonSymbolKind::Function
    };

    let visibility = classify_visibility(&name);
    let qualified_name = build_qualified_name(class_stack, &name);

    let id = format!(
        "py:sym:{file_path}:{}:{}",
        qualified_name,
        node.start_position().row + 1
    );

    symbols.push(PythonSymbol {
        id,
        name,
        qualified_name,
        kind,
        source_path: file_path.to_string(),
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        visibility,
        is_async,
        is_test,
        decorators: decorators.to_vec(),
    });
}

#[cfg(feature = "tree-sitter-python")]
fn extract_class_symbol(
    node: &tree_sitter::Node,
    source: &str,
    file_path: &str,
    class_stack: &[String],
    decorators: &[String],
    symbols: &mut Vec<PythonSymbol>,
) {
    let name = match node_text(node, source, "name") {
        Some(n) => n,
        None => return,
    };

    let visibility = classify_visibility(&name);
    let qualified_name = build_qualified_name(class_stack, &name);

    let id = format!(
        "py:sym:{file_path}:{}:{}",
        qualified_name,
        node.start_position().row + 1
    );

    symbols.push(PythonSymbol {
        id,
        name,
        qualified_name,
        kind: PythonSymbolKind::Class,
        source_path: file_path.to_string(),
        line_start: node.start_position().row + 1,
        line_end: node.end_position().row + 1,
        visibility,
        is_async: false,
        is_test: false,
        decorators: decorators.to_vec(),
    });
}

#[cfg(feature = "tree-sitter-python")]
fn extract_assignment_symbol(
    node: tree_sitter::Node,
    source: &str,
    file_path: &str,
    symbols: &mut Vec<PythonSymbol>,
) {
    let mut cursor = node.walk();
    let left = node.children(&mut cursor).next();
    let name = match left {
        Some(n) if n.kind() == "identifier" => &source[n.byte_range()],
        _ => return,
    };

    // Skip if it looks like a dunder assignment (__all__, __version__, etc.)
    let name_str = name.to_string();
    if name_str.starts_with('_') && !name_str.starts_with("__") {
        return; // Skip private assignments
    }

    // Heuristic: uppercase name → constant, otherwise → variable
    let kind = if name_str.chars().all(|c| c.is_uppercase() || c == '_') && !name_str.is_empty() {
        PythonSymbolKind::Constant
    } else {
        PythonSymbolKind::Variable
    };

    let visibility = classify_visibility(&name_str);
    let line = node.start_position().row + 1;
    let id = format!("py:sym:{file_path}:{name_str}:{line}");

    symbols.push(PythonSymbol {
        id,
        name: name_str.clone(),
        qualified_name: name_str,
        kind,
        source_path: file_path.to_string(),
        line_start: line,
        line_end: node.end_position().row + 1,
        visibility,
        is_async: false,
        is_test: false,
        decorators: vec![],
    });
}

#[cfg(feature = "tree-sitter-python")]
fn node_text<'a>(node: &tree_sitter::Node, source: &'a str, field: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(source[child.byte_range()].to_string())
}

#[cfg(feature = "tree-sitter-python")]
fn classify_visibility(name: &str) -> PythonVisibility {
    if name.starts_with("__") && name.ends_with("__") {
        PythonVisibility::Dunder
    } else if name.starts_with('_') {
        PythonVisibility::Private
    } else {
        PythonVisibility::Public
    }
}

#[cfg(feature = "tree-sitter-python")]
fn build_qualified_name(class_stack: &[String], name: &str) -> String {
    if class_stack.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", class_stack.join("."), name)
    }
}
