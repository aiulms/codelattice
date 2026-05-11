//! ArkTS component extraction.
//!
//! ArkTS uses `@Component` / `@Entry` decorators on `struct` declarations
//! to define UI components. In tree-sitter-typescript, these appear as
//! ERROR nodes (since TS doesn't have `struct`), so we recover them by
//! pattern-matching on the AST structure.

/// An ArkTS component extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkTsComponent {
    /// Component name (the struct identifier).
    pub name: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Decorators applied to the component (e.g., ["@Entry", "@Component"]).
    pub decorators: Vec<String>,
    /// Whether this is an entry component (has @Entry decorator).
    pub is_entry: bool,
    /// The build method, if present.
    pub build_method: Option<BuildMethodInfo>,
}

/// Information about the `build()` method in an ArkTS component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMethodInfo {
    pub start_line: usize,
    pub end_line: usize,
    /// UI calls inside build() (e.g., ["Column", "Row", "Text"]).
    pub ui_calls: Vec<String>,
}

/// Extract ArkTS components from source code.
///
/// Since tree-sitter-typescript parses `struct` as ERROR nodes, we walk
/// the AST looking for the pattern:
/// ```text
/// ERROR
///   decorator: @Entry / @Component / @ComponentV2
///   ERROR
///     identifier: "struct"
///     identifier: "ComponentName"
///     "{"
/// ```
#[cfg(feature = "tree-sitter-arkts")]
pub fn extract_arkts_components(source: &str) -> Vec<ArkTsComponent> {
    let mut parser = match gitnexus_typescript::extractors::try_init_ts_parser(
        gitnexus_typescript::extractors::TsLanguage::TypeScript,
    ) {
        Some(p) => p,
        None => return vec![],
    };

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let root = tree.root_node();
    let mut components = Vec::new();

    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        if child.kind() == "ERROR" {
            try_extract_component_from_error(&child, source, &mut components);
        }
    }

    components
}

#[cfg(feature = "tree-sitter-arkts")]
fn try_extract_component_from_error(
    error_node: &tree_sitter::Node,
    source: &str,
    components: &mut Vec<ArkTsComponent>,
) {
    // Collect decorators before the struct keyword
    let mut decorators = Vec::new();
    let mut found_struct = false;
    let mut struct_name = None;

    for i in 0..error_node.child_count() {
        let child = error_node.child(i).unwrap();

        if child.kind() == "decorator" {
            let text = source[child.byte_range()].to_string();
            decorators.push(text);
        }

        if child.kind() == "ERROR" {
            // Inner ERROR containing "struct Name {"
            let mut inner_iter = child.children(&mut child.walk());
            if let Some(first) = inner_iter.next() {
                if source[first.byte_range()] == "struct" {
                    found_struct = true;
                    if let Some(name_node) = inner_iter.next() {
                        if name_node.kind() == "identifier" {
                            struct_name = Some(source[name_node.byte_range()].to_string());
                        }
                    }
                }
            }
        }
    }

    if found_struct {
        if let Some(name) = struct_name {
            let is_entry = decorators.iter().any(|d| d == "@Entry");
            // Try to find build() method inside the struct body
            let build_method = find_build_method(error_node, source);
            components.push(ArkTsComponent {
                name,
                start_line: error_node.start_position().row + 1,
                end_line: error_node.end_position().row + 1,
                decorators,
                is_entry,
                build_method,
            });
        }
    }
}

#[cfg(feature = "tree-sitter-arkts")]
fn find_build_method(
    error_node: &tree_sitter::Node,
    source: &str,
) -> Option<BuildMethodInfo> {
    // Walk all descendants looking for "build" identifier followed by arguments
    let mut cursor = error_node.walk();
    loop {
        let node = cursor.node();
        if node.kind() == "call_expression" {
            if let Some(func) = node.child(0) {
                if source[func.byte_range()] == "build" {
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;
                    // Collect UI calls inside build
                    let mut ui_calls = Vec::new();
                    collect_ui_calls(&node, source, &mut ui_calls);
                    return Some(BuildMethodInfo {
                        start_line,
                        end_line,
                        ui_calls,
                    });
                }
            }
        }
        if !cursor.goto_first_child() {
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return None;
                }
            }
        }
    }
}

/// Known ArkUI declarative UI components.
#[cfg(feature = "tree-sitter-arkts")]
const ARKUI_COMPONENTS: &[&str] = &[
    "Column", "Row", "Stack", "Flex", "List", "Grid", "Scroll", "Tabs",
    "Text", "Image", "Button", "TextInput", "Toggle", "Slider", "Progress",
    "Divider", "Blank", "Navigation", "NavRouter", "NavDestination",
    "Swiper", "Video", "Web", "Canvas", "Clock", "Calendar",
    "AlphabetIndexer", "SideBarContainer", "Panel", "AlertDialog",
    "ActionSheet", "Toast", "LoadingProgress", "Marquee", "RichEditor",
    "Search", "Select", "Counter", "Rating", "Stepper", "TimePicker",
    "DatePicker", "TextPicker", "DataPanel", "Gauge", "QRCode",
    "Shape", "Path", "Circle", "Rect", "Ellipse", "Polyline", "Polygon",
    "Line", "Arc", "WaterFlow", "RelativeContainer", "GridRow", "GridCol",
];

#[cfg(feature = "tree-sitter-arkts")]
fn collect_ui_calls(
    node: &tree_sitter::Node,
    source: &str,
    ui_calls: &mut Vec<String>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "call_expression" {
            if let Some(func) = child.child(0) {
                let name = source[func.byte_range()].to_string();
                if ARKUI_COMPONENTS.contains(&name.as_str()) {
                    ui_calls.push(name);
                }
            }
        }
        collect_ui_calls(&child, source, ui_calls);
    }
}
