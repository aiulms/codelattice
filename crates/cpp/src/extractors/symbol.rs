//! C++ symbol extraction from tree-sitter-cpp parse trees.
//!
//! Available only when the `tree-sitter-cpp` feature is enabled.
//!
//! Extracts: namespace, class, struct, method, constructor, destructor,
//! free function, enum/enum class, using alias, typedef, macro definition,
//! global variable.

/// Kinds of symbols extractable from C++ source files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CppSymbolKind {
    Namespace,
    Class,
    Struct,
    MethodDeclaration,
    MethodDefinition,
    ConstructorDeclaration,
    ConstructorDefinition,
    DestructorDeclaration,
    DestructorDefinition,
    FunctionDeclaration,
    FunctionDefinition,
    Enum,
    EnumClass,
    Typedef,
    UsingAlias,
    MacroDefinition,
    GlobalVariable,
}

impl std::fmt::Display for CppSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Namespace => write!(f, "namespace"),
            Self::Class => write!(f, "class"),
            Self::Struct => write!(f, "struct"),
            Self::MethodDeclaration => write!(f, "methodDeclaration"),
            Self::MethodDefinition => write!(f, "methodDefinition"),
            Self::ConstructorDeclaration => write!(f, "constructorDeclaration"),
            Self::ConstructorDefinition => write!(f, "constructorDefinition"),
            Self::DestructorDeclaration => write!(f, "destructorDeclaration"),
            Self::DestructorDefinition => write!(f, "destructorDefinition"),
            Self::FunctionDeclaration => write!(f, "functionDeclaration"),
            Self::FunctionDefinition => write!(f, "functionDefinition"),
            Self::Enum => write!(f, "enum"),
            Self::EnumClass => write!(f, "enumClass"),
            Self::Typedef => write!(f, "typedef"),
            Self::UsingAlias => write!(f, "usingAlias"),
            Self::MacroDefinition => write!(f, "macroDefinition"),
            Self::GlobalVariable => write!(f, "globalVariable"),
        }
    }
}

/// Visibility / access specifier for a C++ symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CppVisibility {
    Public,
    Protected,
    Private,
    /// Default (no explicit access specifier; depends on class vs struct default).
    Default,
}

impl std::fmt::Display for CppVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Protected => write!(f, "protected"),
            Self::Private => write!(f, "private"),
            Self::Default => write!(f, "default"),
        }
    }
}

/// Storage class for a C++ symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CppStorageClass {
    Static,
    Extern,
    Virtual,
    Default,
}

impl std::fmt::Display for CppStorageClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static => write!(f, "static"),
            Self::Extern => write!(f, "extern"),
            Self::Virtual => write!(f, "virtual"),
            Self::Default => write!(f, "default"),
        }
    }
}

/// A symbol extracted from a C++ source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppSymbol {
    pub kind: CppSymbolKind,
    pub name: String,
    /// Qualified name if determinable (e.g., "ns::MyClass::method").
    pub qualified_name: String,
    /// Parent qualified name (e.g., "ns::MyClass" for a method).
    pub parent_name: Option<String>,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line (inclusive).
    pub end_line: usize,
    pub visibility: CppVisibility,
    pub storage_class: CppStorageClass,
    pub is_definition: bool,
}

/// Returns empty vec when tree-sitter-cpp feature is disabled.
#[cfg(not(feature = "tree-sitter-cpp"))]
pub fn extract_cpp_symbols(_source: &str) -> Vec<CppSymbol> {
    vec![]
}

#[cfg(feature = "tree-sitter-cpp")]
pub fn extract_cpp_symbols(source: &str) -> Vec<CppSymbol> {
    let mut parser = match super::try_init_cpp_parser() {
        Some(p) => p,
        None => return vec![],
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let root = tree.root_node();
    let mut symbols = Vec::new();
    let mut namespace_stack: Vec<String> = Vec::new();
    let mut class_stack: Vec<ClassContext> = Vec::new();
    collect_symbols(
        &root,
        source.as_bytes(),
        &mut symbols,
        &mut namespace_stack,
        &mut class_stack,
    );
    symbols
}

/// Tracks current class context for method/constructor/destructor extraction.
#[derive(Debug, Clone)]
struct ClassContext {
    name: String,
    visibility: CppVisibility,
}

#[cfg(feature = "tree-sitter-cpp")]
fn collect_symbols(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &mut Vec<String>,
    class_stack: &mut Vec<ClassContext>,
) {
    match node.kind() {
        // --- Namespace ---
        "namespace_definition" => {
            if let Some(name) = node.child_by_field_name("name") {
                let ns_name = text_of(&name, source);
                namespace_stack.push(ns_name.clone());
                let qualified = qualified_name(namespace_stack, &[]);
                symbols.push(CppSymbol {
                    kind: CppSymbolKind::Namespace,
                    name: ns_name,
                    qualified_name: qualified.clone(),
                    parent_name: if namespace_stack.len() > 1 {
                        Some(qualified_name(
                            &namespace_stack[..namespace_stack.len() - 1],
                            &[],
                        ))
                    } else {
                        None
                    },
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: CppVisibility::Default,
                    storage_class: CppStorageClass::Default,
                    is_definition: true,
                });
            }

            // Recurse into namespace body
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "declaration_list" || child.kind() == "compound_statement" {
                    let mut body_cursor = child.walk();
                    for body_child in child.children(&mut body_cursor) {
                        collect_symbols(&body_child, source, symbols, namespace_stack, class_stack);
                    }
                }
            }

            if !namespace_stack.is_empty() {
                namespace_stack.pop();
            }
            return;
        }

        // --- Class / Struct ---
        "class_specifier" | "struct_specifier" => {
            // Find name: first identifier child that's a "name" field or the first identifier
            let name_node = node.child_by_field_name("name").or_else(|| {
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();
                children
                    .into_iter()
                    .find(|c| c.kind() == "type_identifier" || c.kind() == "identifier")
            });

            if let Some(name_node) = name_node {
                let class_name = text_of(&name_node, source);
                let is_class = node.kind() == "class_specifier";
                let default_vis = if is_class {
                    CppVisibility::Private
                } else {
                    CppVisibility::Public
                };
                class_stack.push(ClassContext {
                    name: class_name.clone(),
                    visibility: default_vis,
                });

                let class_name_ref = &class_name;
                let qualified = qualified_name(namespace_stack, &[class_name_ref]);
                symbols.push(CppSymbol {
                    kind: if is_class {
                        CppSymbolKind::Class
                    } else {
                        CppSymbolKind::Struct
                    },
                    name: class_name,
                    qualified_name: qualified.clone(),
                    parent_name: if !namespace_stack.is_empty() {
                        Some(qualified_name(namespace_stack, &[]))
                    } else {
                        None
                    },
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    visibility: CppVisibility::Default,
                    storage_class: CppStorageClass::Default,
                    is_definition: true,
                });
            }

            // Recurse into class body
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_symbols_in_class_body(
                    &child,
                    source,
                    symbols,
                    namespace_stack,
                    class_stack,
                );
            }

            if !class_stack.is_empty() {
                class_stack.pop();
            }
            return;
        }

        // --- Free function definition ---
        "function_definition" => {
            if class_stack.is_empty() {
                extract_free_function(node, source, symbols, namespace_stack, true);
            }
            // If inside class context, handled by class body recursion
            return;
        }

        // --- Free function declaration ---
        "declaration" => {
            // Check for function declaration (not inside class)
            if class_stack.is_empty() {
                extract_declaration(node, source, symbols, namespace_stack);
            }
            return;
        }

        // --- Enum / Enum class ---
        "enum_specifier" => {
            extract_enum(node, source, symbols, namespace_stack);
            return;
        }

        // --- Type definition ---
        "type_definition" => {
            extract_typedef(node, source, symbols, namespace_stack);
            return;
        }

        // --- Using alias ---
        "using_declaration" | "alias_declaration" => {
            extract_using(node, source, symbols, namespace_stack);
            return;
        }

        // --- Preprocessor ---
        "preproc_def" | "preproc_function_def" => {
            extract_macro(node, source, symbols);
            return;
        }

        // --- Linkage specification (extern "C" etc) ---
        "linkage_specification" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_symbols(&child, source, symbols, namespace_stack, class_stack);
            }
            return;
        }

        _ => {}
    }

    // Default: recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(&child, source, symbols, namespace_stack, class_stack);
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn collect_symbols_in_class_body(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &mut Vec<String>,
    class_stack: &mut Vec<ClassContext>,
) {
    // Handle access specifiers
    if node.kind() == "access_specifier" {
        if let Some(cls) = class_stack.last_mut() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "public" => cls.visibility = CppVisibility::Public,
                    "protected" => cls.visibility = CppVisibility::Protected,
                    "private" => cls.visibility = CppVisibility::Private,
                    _ => {}
                }
            }
        }
        return;
    }

    let current_vis = class_stack
        .last()
        .map(|c| c.visibility)
        .unwrap_or(CppVisibility::Default);

    match node.kind() {
        "function_definition" => {
            extract_method_or_function(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                current_vis,
                true,
            );
        }
        "declaration" => {
            // Could be method declaration, constructor declaration, etc.
            extract_class_declaration(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                current_vis,
            );
        }
        "constructor_definition" | "constructor_declaration" => {
            extract_constructor(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                current_vis,
            );
        }
        "destructor_definition" | "destructor_declaration" => {
            extract_destructor(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                current_vis,
            );
        }
        "class_specifier" | "struct_specifier" => {
            // Nested class/struct
            collect_symbols(node, source, symbols, namespace_stack, class_stack);
        }
        "enum_specifier" => {
            extract_enum(node, source, symbols, namespace_stack);
        }
        "field_declaration" => {
            // Could have static members — treat as global-ish
            extract_field_declaration(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                current_vis,
            );
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_symbols_in_class_body(
                    &child,
                    source,
                    symbols,
                    namespace_stack,
                    class_stack,
                );
            }
        }
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_free_function(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    is_definition: bool,
) {
    let name = find_function_name_node(node)
        .map(|n| text_of(&n, source))
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let vis = find_storage_class_visibility(node);
    let qualified = qualified_name(namespace_stack, &[&name]);
    symbols.push(CppSymbol {
        kind: if is_definition {
            CppSymbolKind::FunctionDefinition
        } else {
            CppSymbolKind::FunctionDeclaration
        },
        name,
        qualified_name: qualified.clone(),
        parent_name: if !namespace_stack.is_empty() {
            Some(qualified_name(namespace_stack, &[]))
        } else {
            None
        },
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: CppVisibility::Default,
        storage_class: vis,
        is_definition,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_method_or_function(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    class_stack: &[ClassContext],
    visibility: CppVisibility,
    is_definition: bool,
) {
    let name_node = find_function_name_node(node);
    let name = name_node.map(|n| text_of(&n, source)).unwrap_or_default();
    if name.is_empty() {
        return;
    }

    // Determine if this is a method (inside class body) or free function
    let class_name = class_stack.last().map(|c| c.name.clone());
    let storage = detect_storage_class(node);
    let qualified = if let Some(ref cn) = class_name {
        qualified_name(namespace_stack, &[cn, &name])
    } else {
        qualified_name(namespace_stack, &[&name])
    };

    symbols.push(CppSymbol {
        kind: if class_name.is_some() {
            if is_definition {
                CppSymbolKind::MethodDefinition
            } else {
                CppSymbolKind::MethodDeclaration
            }
        } else if is_definition {
            CppSymbolKind::FunctionDefinition
        } else {
            CppSymbolKind::FunctionDeclaration
        },
        name,
        qualified_name: qualified.clone(),
        parent_name: class_name.or_else(|| {
            if !namespace_stack.is_empty() {
                Some(qualified_name(namespace_stack, &[]))
            } else {
                None
            }
        }),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility,
        storage_class: storage,
        is_definition,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_constructor(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    class_stack: &[ClassContext],
    visibility: CppVisibility,
) {
    let class_name = class_stack.last().map(|c| c.name.clone());
    let name = class_name.clone().unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let qualified = qualified_name(namespace_stack, &[&name]);
    let is_def = node.kind() == "constructor_definition";

    symbols.push(CppSymbol {
        kind: if is_def {
            CppSymbolKind::ConstructorDefinition
        } else {
            CppSymbolKind::ConstructorDeclaration
        },
        name: format!("{name}()"),
        qualified_name: format!("{qualified}::{name}()"),
        parent_name: Some(qualified),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility,
        storage_class: CppStorageClass::Default,
        is_definition: is_def,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_destructor(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    class_stack: &[ClassContext],
    visibility: CppVisibility,
) {
    let class_name = class_stack.last().map(|c| c.name.clone());
    let name = class_name.clone().unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let qualified = qualified_name(namespace_stack, &[&name]);
    let is_def = node.kind() == "destructor_definition";

    symbols.push(CppSymbol {
        kind: if is_def {
            CppSymbolKind::DestructorDefinition
        } else {
            CppSymbolKind::DestructorDeclaration
        },
        name: format!("~{name}()"),
        qualified_name: format!("{qualified}::~{name}()"),
        parent_name: Some(qualified),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility,
        storage_class: CppStorageClass::Default,
        is_definition: is_def,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_class_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    class_stack: &[ClassContext],
    visibility: CppVisibility,
) {
    // Look for function declarator inside a declaration
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declarator" {
            // Method or function declaration
            extract_method_or_function(
                node,
                source,
                symbols,
                namespace_stack,
                class_stack,
                visibility,
                false,
            );
            return;
        }
    }

    // Check for constructor/destructor declaration
    let text = text_of(node, source);
    if let Some(cls) = class_stack.last() {
        if text.contains(&format!("{}(", cls.name)) || text.contains(&format!("~{}", cls.name)) {
            if text.contains('~') {
                extract_destructor(
                    node,
                    source,
                    symbols,
                    namespace_stack,
                    class_stack,
                    visibility,
                );
            } else {
                extract_constructor(
                    node,
                    source,
                    symbols,
                    namespace_stack,
                    class_stack,
                    visibility,
                );
            }
        }
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_field_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
    class_stack: &[ClassContext],
    visibility: CppVisibility,
) {
    // Only extract static fields as symbols
    let text = text_of(node, source);
    if !text.contains("static") {
        return;
    }

    // Try to find the identifier
    let name = node
        .child_by_field_name("declarator")
        .and_then(|d| find_leaf_identifier(&d, source))
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    let class_name = class_stack.last().map(|c| c.name.clone());
    let qualified = if let Some(ref cn) = class_name {
        qualified_name(namespace_stack, &[cn, &name])
    } else {
        qualified_name(namespace_stack, &[&name])
    };

    symbols.push(CppSymbol {
        kind: CppSymbolKind::GlobalVariable,
        name,
        qualified_name: qualified.clone(),
        parent_name: class_name,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility,
        storage_class: CppStorageClass::Static,
        is_definition: true,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declarator" {
            extract_free_function(node, source, symbols, namespace_stack, false);
            return;
        }
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_enum(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
) {
    let text = text_of(node, source);
    let is_enum_class = text.contains("enum class") || text.contains("enum struct");

    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(&n, source))
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    let qualified = qualified_name(namespace_stack, &[&name]);
    symbols.push(CppSymbol {
        kind: if is_enum_class {
            CppSymbolKind::EnumClass
        } else {
            CppSymbolKind::Enum
        },
        name,
        qualified_name: qualified.clone(),
        parent_name: if !namespace_stack.is_empty() {
            Some(qualified_name(namespace_stack, &[]))
        } else {
            None
        },
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: CppVisibility::Default,
        storage_class: CppStorageClass::Default,
        is_definition: true,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_typedef(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
) {
    let text = text_of(node, source);
    // typedef <type> <name>;
    // Simple heuristic: last identifier before the semicolon
    let name = node
        .child_by_field_name("declarator")
        .map(|d| text_of(&d, source))
        .or_else(|| {
            // Fallback: find last identifier by iterating children
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            children
                .into_iter()
                .rev()
                .find(|c| c.kind() == "type_identifier" || c.kind() == "identifier")
                .map(|n| text_of(&n, source))
        })
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    let qualified = qualified_name(namespace_stack, &[&name]);
    symbols.push(CppSymbol {
        kind: CppSymbolKind::Typedef,
        name,
        qualified_name: qualified.clone(),
        parent_name: if !namespace_stack.is_empty() {
            Some(qualified_name(namespace_stack, &[]))
        } else {
            None
        },
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: CppVisibility::Default,
        storage_class: CppStorageClass::Default,
        is_definition: true,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_using(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<CppSymbol>,
    namespace_stack: &[String],
) {
    // using <name> = <type>;
    // using namespace <ns>;
    let text = text_of(node, source);

    // Skip using namespace directives
    if text.starts_with("using namespace") {
        return;
    }

    let name = if node.kind() == "alias_declaration" {
        // alias_declaration has "name" field
        node.child_by_field_name("name")
            .map(|n| text_of(&n, source))
            .unwrap_or_default()
    } else {
        // using_declaration — skip for now (e.g., using Base::foo)
        return;
    };

    if name.is_empty() {
        return;
    }

    let qualified = qualified_name(namespace_stack, &[&name]);
    symbols.push(CppSymbol {
        kind: CppSymbolKind::UsingAlias,
        name,
        qualified_name: qualified.clone(),
        parent_name: if !namespace_stack.is_empty() {
            Some(qualified_name(namespace_stack, &[]))
        } else {
            None
        },
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: CppVisibility::Default,
        storage_class: CppStorageClass::Default,
        is_definition: true,
    });
}

#[cfg(feature = "tree-sitter-cpp")]
fn extract_macro(node: &tree_sitter::Node, source: &[u8], symbols: &mut Vec<CppSymbol>) {
    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(&n, source))
        .unwrap_or_default();

    if name.is_empty() {
        return;
    }

    symbols.push(CppSymbol {
        kind: CppSymbolKind::MacroDefinition,
        name: name.clone(),
        qualified_name: name,
        parent_name: None,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        visibility: CppVisibility::Default,
        storage_class: CppStorageClass::Default,
        is_definition: true,
    });
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

#[cfg(feature = "tree-sitter-cpp")]
fn text_of(node: &tree_sitter::Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).to_string()
}

#[cfg(feature = "tree-sitter-cpp")]
fn qualified_name(namespace_stack: &[String], extra: &[&String]) -> String {
    let mut parts: Vec<&str> = namespace_stack.iter().map(|s| s.as_str()).collect();
    for e in extra {
        parts.push(e.as_str());
    }
    parts.join("::")
}

#[cfg(feature = "tree-sitter-cpp")]
fn find_function_name_node<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    // First check for declarator child (function_definition has declarator field)
    if let Some(decl) = node.child_by_field_name("declarator") {
        return find_name_in_declarator(&decl);
    }
    None
}

#[cfg(feature = "tree-sitter-cpp")]
fn find_name_in_declarator<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    match node.kind() {
        "function_declarator" | "pointer_declarator" | "reference_declarator" => {
            // Recurse into children looking for the name
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "identifier" | "field_identifier" | "destructor_name" => {
                        return Some(child);
                    }
                    "qualified_identifier" => {
                        // Get the last part
                        let mut cursor2 = child.walk();
                        if let Some(last) = child.children(&mut cursor2).last() {
                            return Some(last.clone());
                        }
                        return Some(child);
                    }
                    "function_declarator"
                    | "pointer_declarator"
                    | "reference_declarator"
                    | "parenthesized_declarator" => {
                        if let Some(name) = find_name_in_declarator(&child) {
                            return Some(name);
                        }
                    }
                    _ => {}
                }
            }
        }
        "identifier" | "field_identifier" => {
            return Some(node.clone());
        }
        _ => {}
    }
    None
}

#[cfg(feature = "tree-sitter-cpp")]
fn find_leaf_identifier(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" => Some(text_of(node, source)),
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(name) = find_leaf_identifier(&child, source) {
                    return Some(name);
                }
            }
            None
        }
    }
}

#[cfg(feature = "tree-sitter-cpp")]
fn find_storage_class_visibility(node: &tree_sitter::Node) -> CppStorageClass {
    detect_storage_class(node)
}

#[cfg(feature = "tree-sitter-cpp")]
fn detect_storage_class(node: &tree_sitter::Node) -> CppStorageClass {
    // Check for storage class specifiers in the node's text
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "storage_class_specifier" => {
                // Could be static, extern, mutable, etc.
                return CppStorageClass::Static;
            }
            "virtual" => {
                return CppStorageClass::Virtual;
            }
            _ => {}
        }
    }
    CppStorageClass::Default
}
