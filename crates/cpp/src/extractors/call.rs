//! C++ call extraction from tree-sitter-cpp parse trees.
//!
//! Phase A: Extract call expressions with conservative confidence scoring.
//! No template instantiation, no virtual dispatch, no full overload resolution.

/// A call extracted from a C++ source file.
#[derive(Debug, Clone, PartialEq)]
pub struct CppCall {
    /// Name of the function/method being called (as written in source).
    pub callee_name: String,
    /// Qualified name if determinable (e.g., "ns::Class::method").
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

/// Returns empty vec when tree-sitter-cpp feature is disabled.
#[cfg(not(feature = "tree-sitter-cpp"))]
pub fn extract_cpp_calls(
    _source: &str,
    _file_path: &str,
    _project_function_names: &[String],
) -> Vec<CppCall> {
    vec![]
}

#[cfg(feature = "tree-sitter-cpp")]
pub fn extract_cpp_calls(
    source: &str,
    file_path: &str,
    project_function_names: &[String],
) -> Vec<CppCall> {
    let mut parser = match super::try_init_cpp_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    extract_cpp_calls_from_root(&root, source, file_path, project_function_names)
}

#[cfg(feature = "tree-sitter-cpp")]
pub fn extract_cpp_calls_from_root(
    root: &tree_sitter::Node,
    source: &str,
    file_path: &str,
    project_function_names: &[String],
) -> Vec<CppCall> {
    let source_bytes = source.as_bytes();
    let mut calls = Vec::new();
    collect_calls(
        root,
        source_bytes,
        file_path,
        project_function_names,
        &mut calls,
    );
    calls
}

#[cfg(feature = "tree-sitter-cpp")]
fn collect_calls(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    project_fn_names: &[String],
    calls: &mut Vec<CppCall>,
) {
    if node.kind() == "call_expression" {
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

#[cfg(feature = "tree-sitter-cpp")]
fn extract_call_info(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    project_fn_names: &[String],
) -> Option<CppCall> {
    // The first child of a call_expression is the function/field expression
    let func_node = node.child(0)?;
    let line = node.start_position().row + 1;

    match func_node.kind() {
        "identifier" => {
            // Direct function call: func(args)
            let name = text_of_node(&func_node, source);
            let is_project_fn = project_fn_names.iter().any(|f| f == &name);

            let (confidence, reason) = if is_project_fn {
                (0.90, "direct-same-file-function-call".to_string())
            } else {
                // Check if it matches any project function name
                let matches_project = project_fn_names
                    .iter()
                    .any(|f| f.ends_with(&format!("::{name}")));
                if matches_project {
                    (0.60, "name-only-cross-file-candidate".to_string())
                } else {
                    // Unknown external or macro call
                    return None;
                }
            };

            Some(CppCall {
                callee_name: name,
                callee_qualified: None,
                receiver: None,
                caller_file: file_path.to_string(),
                line,
                confidence,
                reason,
            })
        }

        "field_expression" => {
            // Method call: obj.method() or obj->method()
            extract_field_call(&func_node, source, file_path, project_fn_names, line)
        }

        "qualified_identifier" => {
            // Qualified call: ns::func() or Class::method()
            let full_name = text_of_node(&func_node, source);
            let parts: Vec<&str> = full_name.split("::").collect();
            let method_name = parts.last().unwrap_or(&"").to_string();

            let matches_project = project_fn_names
                .iter()
                .any(|f| f == &full_name || f.ends_with(&format!("::{method_name}")));

            let (confidence, reason) = if matches_project {
                (0.80, "qualified-project-function-call".to_string())
            } else {
                (0.75, "class-static-method-name-match".to_string())
            };

            Some(CppCall {
                callee_name: method_name,
                callee_qualified: Some(full_name),
                receiver: None,
                caller_file: file_path.to_string(),
                line,
                confidence,
                reason,
            })
        }

        "template_function" | "template_method" => {
            // Template call — low confidence, just record the name
            let name = text_of_node(&func_node, source);
            Some(CppCall {
                callee_name: name,
                callee_qualified: None,
                receiver: None,
                caller_file: file_path.to_string(),
                line,
                confidence: 0.40,
                reason: "template-call-not-instantiated".to_string(),
            })
        }

        _ => None,
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_field_call(
    node: &tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    project_fn_names: &[String],
    line: usize,
) -> Option<CppCall> {
    // field_expression: object . name  or  object -> name
    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    let mut object_name = String::new();
    let mut method_name = String::new();

    for child in &children {
        match child.kind() {
            "identifier" | "field_identifier" => {
                method_name = text_of_node(child, source);
            }
            "this" => {
                object_name = "this".to_string();
            }
            "." | "->" => {
                // operator
            }
            _ => {
                // Could be nested field expression or type
                if object_name.is_empty() {
                    object_name = text_of_node(child, source);
                }
            }
        }
    }

    if method_name.is_empty() {
        return None;
    }

    let matches_project = project_fn_names
        .iter()
        .any(|f| f == &method_name || f.ends_with(&format!("::{method_name}")));

    let (confidence, reason) = if object_name == "this" && matches_project {
        (0.80, "this-method-call-project-match".to_string())
    } else if matches_project {
        (0.75, "receiver-method-project-match".to_string())
    } else {
        (0.45, "receiver-method-name-only".to_string())
    };

    Some(CppCall {
        callee_name: method_name,
        callee_qualified: None,
        receiver: if object_name.is_empty() {
            None
        } else {
            Some(object_name)
        },
        caller_file: file_path.to_string(),
        line,
        confidence,
        reason,
    })
}

#[cfg(feature = "tree-sitter-cpp")]
fn text_of_node(node: &tree_sitter::Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).to_string()
}
