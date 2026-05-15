//! Python call extraction from tree-sitter-python parse trees.
//!
//! Phase A: Extract call expressions with conservative confidence scoring.
//! No dynamic type inference, no eval/getattr/importlib resolution.

/// A call extracted from a Python source file.
#[derive(Debug, Clone, PartialEq)]
pub struct PythonCall {
    /// Name of the function/method being called (as written in source).
    pub callee_name: String,
    /// Qualified name if determinable (e.g., "UserService.run").
    pub callee_qualified: Option<String>,
    /// Receiver object name if method call (e.g., "obj" in obj.method()).
    pub receiver: Option<String>,
    /// File where the call occurs.
    pub caller_file: String,
    /// 1-based line of the call.
    pub line: usize,
    /// Confidence score (0.0-1.0).
    pub confidence: f64,
    /// Reason for the confidence level.
    pub reason: String,
}

/// Returns empty vec when tree-sitter-python feature is disabled.
#[cfg(not(feature = "tree-sitter-python"))]
pub fn extract_python_calls(
    _source: &str,
    _file_path: &str,
    _project_function_names: &[String],
) -> Vec<PythonCall> {
    vec![]
}

#[cfg(feature = "tree-sitter-python")]
pub fn extract_python_calls(
    source: &str,
    file_path: &str,
    project_function_names: &[String],
) -> Vec<PythonCall> {
    let mut parser = match super::try_init_python_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let source_bytes = source.as_bytes();
    let mut calls = Vec::new();
    collect_calls(
        &root,
        source_bytes,
        file_path,
        project_function_names,
        &mut calls,
    );
    calls
}

#[cfg(feature = "tree-sitter-python")]
fn collect_calls(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    project_fn_names: &[String],
    calls: &mut Vec<PythonCall>,
) {
    if node.kind() == "call" {
        if let Some(call_info) = extract_call_info(node, source, file_path, project_fn_names) {
            calls.push(call_info);
        }
    }

    // Recurse
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(&child, source, file_path, project_fn_names, calls);
    }
}

#[cfg(feature = "tree-sitter-python")]
fn extract_call_info(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    project_fn_names: &[String],
) -> Option<PythonCall> {
    // In tree-sitter-python, a "call" node has the function as first child
    // e.g., `func(args)` → call → [identifier "func", argument_list]
    // e.g., `obj.method(args)` → call → [attribute, argument_list]
    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    let func_node = children.first()?;

    match func_node.kind() {
        "identifier" => {
            let name = std::str::from_utf8(&source[func_node.byte_range()])
                .unwrap_or("")
                .to_string();

            let line = node.start_position().row + 1;

            // Check if this is a project function
            let is_project_fn = project_fn_names.contains(&name);

            let (confidence, reason) = if is_project_fn {
                (
                    0.90,
                    "direct-same-file-or-imported-function-call".to_string(),
                )
            } else {
                (0.60, "name-only-candidate-not-in-project-index".to_string())
            };

            Some(PythonCall {
                callee_name: name.clone(),
                callee_qualified: Some(name),
                receiver: None,
                caller_file: file_path.to_string(),
                line,
                confidence,
                reason,
            })
        }
        "attribute" => {
            // obj.method() or module.func()
            let mut ac = func_node.walk();
            let parts: Vec<tree_sitter::Node> = func_node.children(&mut ac).collect();

            let receiver = parts.first().map(|p| {
                std::str::from_utf8(&source[p.byte_range()])
                    .unwrap_or("")
                    .to_string()
            });
            let method_name = parts.iter().find(|p| p.kind() == "identifier").map(|p| {
                std::str::from_utf8(&source[p.byte_range()])
                    .unwrap_or("")
                    .to_string()
            });

            let method = method_name.unwrap_or_default();
            let recv = receiver.unwrap_or_default();
            let line = node.start_position().row + 1;

            let qualified = if recv.is_empty() {
                method.clone()
            } else {
                format!("{recv}.{method}")
            };

            // Check if qualified name is in project index
            let is_project_fn = project_fn_names
                .iter()
                .any(|n| n == &qualified || n == &method || n.ends_with(&format!(".{method}")));

            let (confidence, reason) = if is_project_fn {
                (0.80, "module-qualified-project-call".to_string())
            } else {
                (0.45, "receiver-method-name-only".to_string())
            };

            Some(PythonCall {
                callee_name: method,
                callee_qualified: Some(qualified),
                receiver: Some(recv),
                caller_file: file_path.to_string(),
                line,
                confidence,
                reason,
            })
        }
        _ => None,
    }
}
