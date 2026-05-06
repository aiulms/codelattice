//! Same-file reference extraction from tree-sitter-cangjie parse trees.
//!
//! Ports the TS adapter `extractReferences()` AST walk pattern:
//! - typeStack: tracks enclosing type (class/struct/interface/enum)
//! - funcStack: tracks enclosing function/method/constructor
//! - Same-file symbol index for target resolution (no import/cross-file lookup)
//!
//! Produces [`CangjieReference`] values that map to USES/ACCESSES/MODIFIES graph edges.
//!
//! Available only when the `tree-sitter-cangjie` feature is enabled.

use std::collections::HashMap;

use super::symbol::{CangjieSymbol, CangjieSymbolKind};

// ---------------------------------------------------------------------------
// Reference types
// ---------------------------------------------------------------------------

/// Kind of a reference extracted from Cangjie AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// Type annotation reference (variable/parameter/return/generic arg).
    Uses,
    /// Field read access (obj.field without call suffix).
    Accesses,
    /// Write/mutation (assignment, compound assignment, field write).
    Modifies,
}

/// A reference from one symbol (source) to another symbol (target).
#[derive(Debug, Clone, PartialEq)]
pub struct CangjieReference {
    pub kind: ReferenceKind,
    /// Method/Constructor/Function node ID where this reference occurs.
    pub source_id: String,
    /// Name of the referenced symbol.
    pub target_name: String,
    /// Expected target symbol kinds (for same-file index lookup).
    pub target_kinds: Vec<CangjieSymbolKind>,
    /// File path where this reference was found.
    pub file_path: String,
    /// File path of the target symbol's definition (cross-file resolution).
    /// `None` means unresolved or same-file (emitter falls back to `file_path`).
    pub target_file: Option<String>,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Reason code for the edge (e.g. "cangjie-type-annotation").
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Builtin type filtering
// ---------------------------------------------------------------------------

/// Cangjie builtin type names — these do NOT produce USES edges.
const BUILTIN_TYPES: &[&str] = &[
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "IntNative",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "UIntNative",
    "Float16",
    "Float32",
    "Float64",
    "String",
    "Rune",
    "Bool",
    "Nothing",
    "Unit",
    "Thistype",
    "Array",
    "Range",
    "Option",
    "VArray",
    "CPointer",
    "CString",
];

fn is_builtin_type(name: &str) -> bool {
    BUILTIN_TYPES.contains(&name)
}

// ---------------------------------------------------------------------------
// AST node type constants (tree-sitter-cangjie)
// ---------------------------------------------------------------------------

/// Type declaration node kinds that introduce an enclosing type scope.
const TYPE_DECLARATION_KINDS: &[&str] = &[
    "classDefinition",
    "structDefinition",
    "interfaceDefinition",
    "enumDefinition",
];

/// Type name node kinds for each type declaration.
fn type_name_kind(parent_kind: &str) -> Option<&'static str> {
    match parent_kind {
        "classDefinition" => Some("className"),
        "structDefinition" => Some("structName"),
        "interfaceDefinition" => Some("interfaceName"),
        "enumDefinition" => Some("enumName"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Enclosing function/method context
// ---------------------------------------------------------------------------

/// Context for the current enclosing function/method/constructor during AST walk.
struct FuncContext {
    /// Function name (funcName for functionDefinition, "init" for init, "main" for mainDefinition).
    func_name: String,
    /// Owner type name for methods (class/struct/interface/enum), None for top-level functions.
    owner_name: Option<String>,
    /// Whether this is an init constructor.
    is_init: bool,
    /// Number of parameters (for Method/Constructor arity suffix in sourceId).
    arity: usize,
}

// ---------------------------------------------------------------------------
// Same-file symbol index
// ---------------------------------------------------------------------------

/// Index of symbols within a single file, keyed by name.
struct SameFileIndex<'a> {
    by_name: HashMap<&'a str, Vec<&'a CangjieSymbol>>,
}

impl<'a> SameFileIndex<'a> {
    fn build(symbols: &'a [CangjieSymbol]) -> Self {
        let mut by_name: HashMap<&str, Vec<&CangjieSymbol>> = HashMap::new();
        for sym in symbols {
            by_name.entry(&sym.name).or_default().push(sym);
        }
        Self { by_name }
    }

    /// Look up a symbol by name, filtering by allowed target kinds.
    ///
    /// Returns `Some(&CangjieSymbol)` if exactly one match is found;
    /// `None` if zero or multiple matches (ambiguous).
    fn resolve(&self, name: &str, target_kinds: &[CangjieSymbolKind]) -> Option<&'a CangjieSymbol> {
        let candidates = self.by_name.get(name)?;
        let filtered: Vec<&&CangjieSymbol> = candidates
            .iter()
            .filter(|s| target_kinds.contains(&s.kind))
            .collect();

        if filtered.len() == 1 {
            Some(filtered[0])
        } else {
            None // ambiguous or no match → no-edge
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-file symbol index (project-wide)
// ---------------------------------------------------------------------------

/// Project-wide symbol index for cross-file reference resolution.
///
/// Unlike [`SameFileIndex`] which is scoped to a single file, this index
/// covers all source files in the project workspace.
#[cfg(feature = "tree-sitter-cangjie")]
struct CrossFileSymbolIndex {
    /// Key: (file_path_string, symbol_name) → all symbols matching that name in that file.
    by_file_and_name: HashMap<(String, String), Vec<CangjieSymbol>>,
}

#[cfg(feature = "tree-sitter-cangjie")]
impl CrossFileSymbolIndex {
    fn build(
        symbols_by_file: &std::collections::BTreeMap<std::path::PathBuf, Vec<CangjieSymbol>>,
    ) -> Self {
        let mut by_file_and_name: HashMap<(String, String), Vec<CangjieSymbol>> = HashMap::new();
        for (file, symbols) in symbols_by_file {
            let file_key = file.to_string_lossy().to_string();
            for sym in symbols {
                let key = (file_key.clone(), sym.name.clone());
                by_file_and_name.entry(key).or_default().push(sym.clone());
            }
        }
        Self { by_file_and_name }
    }

    /// Look up a symbol in a specific file by name and kind filter.
    /// Returns `Some` only on unique match (same semantics as SameFileIndex).
    #[allow(dead_code)] // reserved for direct cross-file lookup; current path uses find_files_with_symbol_in_dir
    fn resolve(
        &self,
        file: &str,
        name: &str,
        kinds: &[CangjieSymbolKind],
    ) -> Option<&CangjieSymbol> {
        let key = (file.to_string(), name.to_string());
        let candidates = self.by_file_and_name.get(&key)?;
        let filtered: Vec<&CangjieSymbol> = candidates
            .iter()
            .filter(|s| kinds.contains(&s.kind))
            .collect();
        if filtered.len() == 1 {
            Some(filtered[0])
        } else {
            None
        }
    }

    /// Find files under `dir` that define a symbol with the given name and matching kind.
    fn find_files_with_symbol_in_dir(
        &self,
        dir: &std::path::Path,
        name: &str,
        kinds: &[CangjieSymbolKind],
    ) -> Vec<String> {
        let dir_str = dir.to_string_lossy().to_string();
        self.by_file_and_name
            .iter()
            .filter_map(|((file, sym_name), syms)| {
                if file.starts_with(&dir_str) && sym_name == name {
                    let matching = syms.iter().any(|s| kinds.contains(&s.kind));
                    if matching {
                        Some(file.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Import binding table (cross-file reference resolution)
// ---------------------------------------------------------------------------

/// A resolved import binding for cross-file reference lookup.
#[cfg(feature = "tree-sitter-cangjie")]
#[derive(Debug, Clone)]
pub struct ImportBinding {
    /// The file containing the target symbol definition.
    pub target_file: String,
    /// The symbol name as defined in the target file.
    pub target_name: String,
}

/// Maps (source_file, local_name) → candidate import bindings.
///
/// Built from the project's import graph and symbol index.
/// Used by reference extraction to resolve cross-file targets.
#[cfg(feature = "tree-sitter-cangjie")]
pub struct ImportBindingTable {
    bindings: HashMap<(String, String), Vec<ImportBinding>>,
}

#[cfg(feature = "tree-sitter-cangjie")]
impl ImportBindingTable {
    /// Build the import binding table from the project's imports and symbol index.
    ///
    /// For each named import in each source file:
    /// 1. Resolve the import target to a package directory.
    /// 2. Find source files under that directory that define the imported symbol.
    /// 3. Record the binding if exactly one candidate file is found.
    pub fn build(
        symbols_by_file: &std::collections::BTreeMap<std::path::PathBuf, Vec<CangjieSymbol>>,
        imports_by_file: &std::collections::BTreeMap<
            std::path::PathBuf,
            Vec<super::imports::CangjieImport>,
        >,
        project: &crate::project::CangjieProject,
    ) -> Self {
        use super::imports::{parse_named_import_candidates, resolve_import_target};

        let cross_index = CrossFileSymbolIndex::build(symbols_by_file);
        let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

        let all_kinds = &[
            CangjieSymbolKind::Function,
            CangjieSymbolKind::Class,
            CangjieSymbolKind::Struct,
            CangjieSymbolKind::Enum,
            CangjieSymbolKind::Interface,
            CangjieSymbolKind::TypeAlias,
            CangjieSymbolKind::Macro,
        ];

        for (file_path, imports) in imports_by_file {
            let file_key = file_path.to_string_lossy().to_string();

            for import in imports {
                // MVP: skip wildcard imports
                if import.is_wildcard {
                    continue;
                }

                let candidates = parse_named_import_candidates(&import.raw_path);
                for candidate in candidates {
                    if let Some(resolved) = resolve_import_target(&candidate, project) {
                        if let Some(ref target_dir) = resolved.target_dir {
                            let candidate_files = cross_index.find_files_with_symbol_in_dir(
                                target_dir,
                                &candidate.exported_name,
                                all_kinds,
                            );

                            if candidate_files.len() == 1 {
                                bindings
                                    .entry((file_key.clone(), candidate.local_name.clone()))
                                    .or_default()
                                    .push(ImportBinding {
                                        target_file: candidate_files[0].clone(),
                                        target_name: candidate.exported_name.clone(),
                                    });
                            }
                            // else: ambiguous or no match → no binding (no fake edge)
                        }
                    }
                }
            }
        }

        Self { bindings }
    }

    /// Resolve a local name in a source file to a cross-file import binding.
    ///
    /// Returns `Some` only on unique match (exactly one candidate binding).
    /// Zero or multiple matches → `None` (no edge emitted).
    pub fn resolve(&self, source_file: &str, name: &str) -> Option<&ImportBinding> {
        let key = (source_file.to_string(), name.to_string());
        let candidates = self.bindings.get(&key)?;
        if candidates.len() == 1 {
            Some(&candidates[0])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// AST walk: reference extraction
// ---------------------------------------------------------------------------

#[cfg(feature = "tree-sitter-cangjie")]
use super::CangjieParseError;
/// Extract references from a Cangjie source file using same-file symbol resolution.
///
/// Walks the tree-sitter AST with typeStack + funcStack tracking.
/// For each discovered reference (type annotation, field read, write), looks up
/// the target in the same-file symbol index. Only unique matches produce references.
#[cfg(feature = "tree-sitter-cangjie")]
use std::path::Path;

/// Helper: convert a usize index to u32 for tree-sitter APIs.
#[cfg(feature = "tree-sitter-cangjie")]
fn idx(i: usize) -> u32 {
    i.try_into().unwrap()
}

#[cfg(feature = "tree-sitter-cangjie")]
pub fn extract_cangjie_references(
    source: &str,
    file_path: &Path,
    symbols: &[CangjieSymbol],
    tree: &tree_sitter::Tree,
    import_bindings: Option<&ImportBindingTable>,
) -> Result<Vec<CangjieReference>, CangjieParseError> {
    let index = SameFileIndex::build(symbols);
    let file_path_str = file_path.to_string_lossy().to_string();
    let root = tree.root_node();

    let mut references: Vec<CangjieReference> = Vec::new();

    // Enclosing type stack — type names only
    let mut type_stack: Vec<String> = Vec::new();
    // Enclosing function/method context stack
    let mut func_stack: Vec<FuncContext> = Vec::new();

    /// Build a sourceId (Method/Constructor/Function node ID) from context.
    fn build_source_id(func_ctx: Option<&FuncContext>, file_path: &str) -> Option<String> {
        let ctx = func_ctx?;

        if ctx.is_init {
            // init → Constructor:<filePath>:<Owner>.init#<arity>
            let owner = ctx.owner_name.as_ref()?; // orphan init: skip
            let qualified = format!("{}.init", owner);
            Some(format!(
                "Constructor:{}:{}#{}",
                file_path, qualified, ctx.arity
            ))
        } else if ctx.owner_name.is_some() {
            // method in type body → Method:<filePath>:<Owner>.<funcName>#<arity>
            let owner = ctx.owner_name.as_ref().unwrap();
            let qualified = format!("{}.{}", owner, ctx.func_name);
            Some(format!("Method:{}:{}#{}", file_path, qualified, ctx.arity))
        } else {
            // top-level function or main → Function:<filePath>:<funcName>
            Some(format!("Function:{}:{}", file_path, ctx.func_name))
        }
    }

    /// Extract name from a type declaration node (classDefinition/structDefinition/...).
    fn extract_type_name(node: tree_sitter::Node, source: &str) -> Option<String> {
        let name_kind = type_name_kind(node.kind())?;
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(idx(i)) {
                if child.kind() == name_kind {
                    return child
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|t| t.to_string());
                }
            }
        }
        None
    }

    /// Count parameters in a functionDefinition/init node.
    fn count_params(func_node: tree_sitter::Node) -> usize {
        for i in 0..func_node.named_child_count() {
            if let Some(child) = func_node.named_child(idx(i)) {
                if child.kind() == "parameterList" {
                    let mut count = 0;
                    for j in 0..child.named_child_count() {
                        if let Some(p) = child.named_child(idx(j)) {
                            if p.kind() == "parameter" {
                                count += 1;
                            }
                        }
                    }
                    return count;
                }
            }
        }
        0
    }

    /// Extract user-defined type name from a userType node.
    /// Returns None for builtin types.
    fn extract_user_type_name(type_node: tree_sitter::Node, source: &str) -> Option<String> {
        if type_node.kind() != "userType" {
            return None;
        }
        for i in 0..type_node.child_count() {
            let child = type_node.child(idx(i))?;
            match child.kind() {
                "identifier" => {
                    let name = child.utf8_text(source.as_bytes()).ok()?;
                    if is_builtin_type(&name) {
                        return None;
                    }
                    return Some(name.to_string());
                }
                "scoped_identifier" => {
                    // Take the last identifier (package.TypeName → TypeName)
                    let mut last_name: Option<String> = None;
                    for j in 0..child.child_count() {
                        if let Some(part) = child.child(idx(j)) {
                            if part.kind() == "identifier" {
                                last_name = part
                                    .utf8_text(source.as_bytes())
                                    .ok()
                                    .map(|t| t.to_string());
                            }
                        }
                    }
                    if let Some(name) = last_name {
                        if is_builtin_type(&name) {
                            return None;
                        }
                        return Some(name);
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }

    /// Find first named child with a specific kind.
    fn find_named_child_by_kind<'a>(
        parent: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        for i in 0..parent.named_child_count() {
            if let Some(child) = parent.named_child(idx(i)) {
                if child.kind() == kind {
                    return Some(child);
                }
            }
        }
        None
    }

    /// Find last named child with a specific kind.
    fn find_last_named_child_by_kind<'a>(
        parent: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut result = None;
        for i in 0..parent.child_count() {
            if let Some(child) = parent.child(idx(i)) {
                if child.is_named() && child.kind() == kind {
                    result = Some(child);
                }
            }
        }
        result
    }

    /// Collect named children of a node.
    fn named_children<'a>(parent: tree_sitter::Node<'a>) -> Vec<tree_sitter::Node<'a>> {
        let mut result = Vec::new();
        for i in 0..parent.child_count() {
            if let Some(child) = parent.child(idx(i)) {
                if child.is_named() {
                    result.push(child);
                }
            }
        }
        result
    }

    /// Check if a postfixExpression node has a callSuffix (i.e., is a function call).
    fn has_call_suffix(node: tree_sitter::Node) -> bool {
        let children = named_children(node);
        children.last().map_or(false, |c| c.kind() == "callSuffix")
    }

    /// Extract the callee name from a postfixExpression that has a callSuffix.
    ///
    /// Handles two call forms:
    /// - Simple call: `func(args)` or `Type(args)` → last named child before callSuffix
    /// - Qualified call: `pkg.func(args)` → last fieldAccess segment
    ///
    /// Returns `None` for method calls (`obj.method(args)`) which require type inference.
    fn extract_callee_name(node: tree_sitter::Node, source: &str) -> Option<String> {
        let children = named_children(node);
        if children.is_empty() {
            return None;
        }

        // Case 1: Simple call — func(args) or Type(args)
        //   postfixExpression → [atomicVariable, callSuffix]
        if children[0].kind() == "atomicVariable" {
            let var = find_named_child_by_kind(children[0], "varBindingPattern");
            return var.and_then(|v| v.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()));
        }

        // Case 2: Qualified call or method call
        //   postfixExpression → [postfixExpression(...), callSuffix]
        if children[0].kind() == "postfixExpression" {
            let inner_children = named_children(children[0]);

            // Method call detection: inner postfixExpression ends with fieldAccess
            // and does NOT itself have callSuffix → obj.method(), skip
            if let Some(last) = inner_children.last() {
                if last.kind() == "fieldAccess"
                    && (inner_children.len() < 2
                        || inner_children[inner_children.len() - 2].kind() != "callSuffix")
                {
                    // obj.method() — method call, requires type inference
                    return None;
                }
            }

            // Qualified call: pkg.func(args) → extract func from fieldAccess
            let field = find_last_named_child_by_kind(children[0], "fieldAccess");
            if let Some(fa) = field {
                let av = find_named_child_by_kind(fa, "atomicVariable");
                if let Some(av_node) = av {
                    let vb = find_named_child_by_kind(av_node, "varBindingPattern");
                    return vb
                        .and_then(|v| v.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()));
                }
            }
        }

        None
    }

    // Push a reference if the target resolves uniquely.
    // Resolution order: same-file index → import binding table (cross-file).
    fn push_reference(
        references: &mut Vec<CangjieReference>,
        kind: ReferenceKind,
        source_id: Option<String>,
        target_name: &str,
        target_kinds: Vec<CangjieSymbolKind>,
        file_path: &str,
        index: &SameFileIndex,
        import_bindings: Option<&ImportBindingTable>,
        confidence: f64,
        reason: &str,
    ) {
        let source_id = match source_id {
            Some(id) => id,
            None => return, // orphan init, skip
        };

        // Step 1: same-file resolution
        if index.resolve(target_name, &target_kinds).is_some() {
            references.push(CangjieReference {
                kind,
                source_id,
                target_name: target_name.to_string(),
                target_kinds,
                file_path: file_path.to_string(),
                target_file: None, // same-file: emitter uses file_path
                confidence,
                reason: reason.to_string(),
            });
            return;
        }

        // Step 2: cross-file resolution via import bindings
        if let Some(bindings) = import_bindings {
            if let Some(binding) = bindings.resolve(file_path, target_name) {
                let cross_reason = format!("{} (cross-file via import)", reason);
                references.push(CangjieReference {
                    kind,
                    source_id,
                    target_name: binding.target_name.clone(),
                    target_kinds,
                    file_path: file_path.to_string(),
                    target_file: Some(binding.target_file.clone()),
                    confidence: 0.85, // cross-file explicit import: confidence 0.85
                    reason: cross_reason,
                });
            }
            // else: ambiguous or no match → no edge (no fake edge)
        }
    }

    /// Recursive AST walk.
    fn walk(
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        type_stack: &mut Vec<String>,
        func_stack: &mut Vec<FuncContext>,
        index: &SameFileIndex,
        import_bindings: Option<&ImportBindingTable>,
        references: &mut Vec<CangjieReference>,
    ) {
        let kind = node.kind();

        // ── Track enclosing type context ──
        if TYPE_DECLARATION_KINDS.contains(&kind) {
            let type_name = extract_type_name(node, source);
            if let Some(ref name) = type_name {
                type_stack.push(name.clone());
            }

            // Walk children (recursive, but type tracking already handled at this level)
            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    walk(
                        child,
                        source,
                        file_path,
                        type_stack,
                        func_stack,
                        index,
                        import_bindings,
                        references,
                    );
                }
            }

            if type_name.is_some() {
                type_stack.pop();
            }
            return;
        }

        // ── Track enclosing function/method/constructor ──
        if kind == "functionDefinition" {
            let func_name = find_named_child_by_kind(node, "funcName")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("anonymous")
                .to_string();
            let owner = type_stack.last().cloned();
            let arity = count_params(node);
            func_stack.push(FuncContext {
                func_name,
                owner_name: owner,
                is_init: false,
                arity,
            });

            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    walk(
                        child,
                        source,
                        file_path,
                        type_stack,
                        func_stack,
                        index,
                        import_bindings,
                        references,
                    );
                }
            }

            func_stack.pop();
            return;
        }

        if kind == "init" {
            let owner = type_stack.last().cloned();
            let arity = count_params(node);
            func_stack.push(FuncContext {
                func_name: "init".to_string(),
                owner_name: owner,
                is_init: true,
                arity,
            });

            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    walk(
                        child,
                        source,
                        file_path,
                        type_stack,
                        func_stack,
                        index,
                        import_bindings,
                        references,
                    );
                }
            }

            func_stack.pop();
            return;
        }

        if kind == "mainDefinition" {
            func_stack.push(FuncContext {
                func_name: "main".to_string(),
                owner_name: None,
                is_init: false,
                arity: 0,
            });

            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    walk(
                        child,
                        source,
                        file_path,
                        type_stack,
                        func_stack,
                        index,
                        import_bindings,
                        references,
                    );
                }
            }

            func_stack.pop();
            return;
        }

        // ── Field read access: postfixExpression → last named child is fieldAccess ──
        if kind == "postfixExpression" {
            let children = named_children(node);
            if let Some(last_child) = children.last() {
                if last_child.kind() == "fieldAccess" {
                    // Check that the second-to-last named child is NOT callSuffix
                    let is_field_read = if children.len() >= 2 {
                        children[children.len() - 2].kind() != "callSuffix"
                    } else {
                        true // single named child = fieldAccess = field read
                    };

                    if is_field_read {
                        let atomic_var = find_named_child_by_kind(*last_child, "atomicVariable");
                        if let Some(av) = atomic_var {
                            let var_binding = find_named_child_by_kind(av, "varBindingPattern");
                            if let Some(vb) = var_binding {
                                if let Ok(field_name) = vb.utf8_text(source.as_bytes()) {
                                    let source_id = build_source_id(func_stack.last(), file_path);
                                    push_reference(
                                        references,
                                        ReferenceKind::Accesses,
                                        source_id,
                                        &field_name,
                                        vec![
                                            CangjieSymbolKind::Class,
                                            CangjieSymbolKind::Struct,
                                            CangjieSymbolKind::Enum,
                                        ],
                                        file_path,
                                        index,
                                        import_bindings,
                                        0.65,
                                        "cangjie-field-read",
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Function call: postfixExpression with callSuffix ──
        // 简单函数调用 func(args) / Type(args) / pkg.func(args) → USES edge。
        // 不处理 method call（obj.method(args)）— 需要 receiver type inference（stop-line）。
        if kind == "postfixExpression" && has_call_suffix(node) {
            if let Some(callee_name) = extract_callee_name(node, source) {
                // 过滤 builtin type 的构造函数调用（如 Array(10)）
                if !is_builtin_type(&callee_name) {
                    let source_id = build_source_id(func_stack.last(), file_path);
                    push_reference(
                        references,
                        ReferenceKind::Uses,
                        source_id,
                        &callee_name,
                        vec![
                            CangjieSymbolKind::Function,
                            CangjieSymbolKind::Class, // 构造函数调用如 Point(1,2)
                            CangjieSymbolKind::Struct, // struct 字面量调用
                        ],
                        file_path,
                        index,
                        import_bindings,
                        0.80, // 同文件函数调用：confidence 0.80
                        "cangjie-function-call",
                    );
                }
            }
            // 不 return — 继续递归 walk children（callee 表达式内可能有嵌套调用如 foo(bar())）
        }

        // ── Type annotation: variableDeclaration ──
        if kind == "variableDeclaration" {
            if let Some(type_node) = node.child_by_field_name("type") {
                if let Some(type_name) = extract_user_type_name(type_node, source) {
                    let source_id = build_source_id(func_stack.last(), file_path);
                    push_reference(
                        references,
                        ReferenceKind::Uses,
                        source_id,
                        &type_name,
                        vec![
                            CangjieSymbolKind::Class,
                            CangjieSymbolKind::Struct,
                            CangjieSymbolKind::Enum,
                            CangjieSymbolKind::Interface,
                            CangjieSymbolKind::TypeAlias,
                        ],
                        file_path,
                        index,
                        import_bindings,
                        0.60,
                        "cangjie-type-annotation",
                    );
                }
            }
        }

        // ── Type annotation: parameter ──
        if kind == "parameter" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    if child.kind() == "userType" {
                        if let Some(type_name) = extract_user_type_name(child, source) {
                            let source_id = build_source_id(func_stack.last(), file_path);
                            push_reference(
                                references,
                                ReferenceKind::Uses,
                                source_id,
                                &type_name,
                                vec![
                                    CangjieSymbolKind::Class,
                                    CangjieSymbolKind::Struct,
                                    CangjieSymbolKind::Enum,
                                    CangjieSymbolKind::Interface,
                                    CangjieSymbolKind::TypeAlias,
                                ],
                                file_path,
                                index,
                                import_bindings,
                                0.60,
                                "cangjie-type-annotation",
                            );
                        }
                    }
                }
            }
        }

        // ── Type annotation: returnType ──
        if kind == "returnType" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    if child.kind() == "userType" {
                        if let Some(type_name) = extract_user_type_name(child, source) {
                            let source_id = build_source_id(func_stack.last(), file_path);
                            push_reference(
                                references,
                                ReferenceKind::Uses,
                                source_id,
                                &type_name,
                                vec![
                                    CangjieSymbolKind::Class,
                                    CangjieSymbolKind::Struct,
                                    CangjieSymbolKind::Enum,
                                    CangjieSymbolKind::Interface,
                                    CangjieSymbolKind::TypeAlias,
                                ],
                                file_path,
                                index,
                                import_bindings,
                                0.60,
                                "cangjie-type-annotation",
                            );
                        }
                    }
                }
            }
        }

        // ── Type annotation: typeArguments (generic) ──
        if kind == "typeArguments" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(idx(i)) {
                    if child.kind() == "userType" {
                        if let Some(type_name) = extract_user_type_name(child, source) {
                            let source_id = build_source_id(func_stack.last(), file_path);
                            push_reference(
                                references,
                                ReferenceKind::Uses,
                                source_id,
                                &type_name,
                                vec![
                                    CangjieSymbolKind::Class,
                                    CangjieSymbolKind::Struct,
                                    CangjieSymbolKind::Enum,
                                    CangjieSymbolKind::Interface,
                                    CangjieSymbolKind::TypeAlias,
                                ],
                                file_path,
                                index,
                                import_bindings,
                                0.60,
                                "cangjie-type-annotation",
                            );
                        }
                    }
                }
            }
        }

        // ── Write/mutation: assignmentExpression ──
        if kind == "assignmentExpression" {
            let children = named_children(node);
            if children.len() >= 2 {
                let lhs = children[0];

                // Detect compound assignment from source text
                let node_text = node.utf8_text(source.as_bytes()).unwrap_or_default();
                let is_compound = node_text.contains("+=")
                    || node_text.contains("-=")
                    || node_text.contains("*=")
                    || node_text.contains("/=");

                // Simple variable write: x = val, x += val
                if lhs.kind() == "atomicVariable" {
                    let var_binding = find_named_child_by_kind(lhs, "varBindingPattern");
                    if let Some(vb) = var_binding {
                        if let Ok(target_name) = vb.utf8_text(source.as_bytes()) {
                            let source_id = build_source_id(func_stack.last(), file_path);
                            let (confidence, reason) = if is_compound {
                                (0.85, "cangjie-modifies-compound")
                            } else {
                                (0.85, "cangjie-modifies-assignment")
                            };

                            if let Some(sid) = source_id {
                                // Local variable write: lookup in same-file index
                                if index
                                    .resolve(
                                        &target_name,
                                        &[
                                            CangjieSymbolKind::Class,
                                            CangjieSymbolKind::Struct,
                                            CangjieSymbolKind::Enum,
                                        ],
                                    )
                                    .is_some()
                                {
                                    references.push(CangjieReference {
                                        kind: ReferenceKind::Modifies,
                                        source_id: sid,
                                        target_name: target_name.to_string(),
                                        target_kinds: vec![
                                            CangjieSymbolKind::Class,
                                            CangjieSymbolKind::Struct,
                                            CangjieSymbolKind::Enum,
                                        ],
                                        file_path: file_path.to_string(),
                                        target_file: None,
                                        confidence,
                                        reason: reason.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
                // Field write: obj.field = val, obj.field += val
                else if lhs.kind() == "postfixExpression" {
                    let field_access = find_last_named_child_by_kind(lhs, "fieldAccess");
                    if let Some(fa) = field_access {
                        let atomic_var = find_named_child_by_kind(fa, "atomicVariable");
                        if let Some(av) = atomic_var {
                            let var_binding = find_named_child_by_kind(av, "varBindingPattern");
                            if let Some(vb) = var_binding {
                                if let Ok(target_name) = vb.utf8_text(source.as_bytes()) {
                                    let source_id = build_source_id(func_stack.last(), file_path);
                                    let (confidence, reason) = if is_compound {
                                        (0.80, "cangjie-modifies-field-compound")
                                    } else {
                                        (0.80, "cangjie-modifies-field-write")
                                    };

                                    if let Some(sid) = source_id {
                                        if index
                                            .resolve(
                                                &target_name,
                                                &[
                                                    CangjieSymbolKind::Class,
                                                    CangjieSymbolKind::Struct,
                                                    CangjieSymbolKind::Enum,
                                                ],
                                            )
                                            .is_some()
                                        {
                                            references.push(CangjieReference {
                                                kind: ReferenceKind::Modifies,
                                                source_id: sid,
                                                target_name: target_name.to_string(),
                                                target_kinds: vec![
                                                    CangjieSymbolKind::Class,
                                                    CangjieSymbolKind::Struct,
                                                    CangjieSymbolKind::Enum,
                                                ],
                                                file_path: file_path.to_string(),
                                                target_file: None,
                                                confidence,
                                                reason: reason.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Continue recursion for non-special nodes ──
        for i in 0..node.child_count() {
            if let Some(child) = node.child(idx(i)) {
                walk(
                    child,
                    source,
                    file_path,
                    type_stack,
                    func_stack,
                    index,
                    import_bindings,
                    references,
                );
            }
        }
    }

    walk(
        root,
        source,
        &file_path_str,
        &mut type_stack,
        &mut func_stack,
        &index,
        import_bindings,
        &mut references,
    );

    Ok(references)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify BUILTIN_TYPES covers the full set from the TS adapter.
    #[test]
    fn builtin_types_coverage() {
        assert!(is_builtin_type("Int64"));
        assert!(is_builtin_type("Float64"));
        assert!(is_builtin_type("String"));
        assert!(is_builtin_type("Bool"));
        assert!(is_builtin_type("Unit"));
        assert!(is_builtin_type("Nothing"));
        assert!(is_builtin_type("Array"));
        assert!(is_builtin_type("Option"));
        assert!(is_builtin_type("Range"));
        // Not builtin
        assert!(!is_builtin_type("Point"));
        assert!(!is_builtin_type("Size"));
        assert!(!is_builtin_type("MyClass"));
    }

    #[test]
    fn same_file_index_unique_match() {
        let symbols = vec![
            CangjieSymbol {
                kind: CangjieSymbolKind::Class,
                name: "Point".to_string(),
                start_line: 1,
                end_line: 5,
            },
            CangjieSymbol {
                kind: CangjieSymbolKind::Struct,
                name: "Size".to_string(),
                start_line: 7,
                end_line: 10,
            },
        ];
        let index = SameFileIndex::build(&symbols);

        let result = index.resolve(
            "Point",
            &[
                CangjieSymbolKind::Class,
                CangjieSymbolKind::Struct,
                CangjieSymbolKind::Enum,
                CangjieSymbolKind::Interface,
            ],
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "Point");
        assert_eq!(result.unwrap().kind, CangjieSymbolKind::Class);
    }

    #[test]
    fn same_file_index_no_match() {
        let symbols = vec![CangjieSymbol {
            kind: CangjieSymbolKind::Class,
            name: "Point".to_string(),
            start_line: 1,
            end_line: 5,
        }];
        let index = SameFileIndex::build(&symbols);

        // name not found
        assert!(index
            .resolve("Unknown", &[CangjieSymbolKind::Class])
            .is_none());
    }

    #[test]
    fn same_file_index_ambiguous_multiple_matches() {
        let symbols = vec![
            CangjieSymbol {
                kind: CangjieSymbolKind::Class,
                name: "Point".to_string(),
                start_line: 1,
                end_line: 5,
            },
            CangjieSymbol {
                kind: CangjieSymbolKind::Struct,
                name: "Point".to_string(),
                start_line: 7,
                end_line: 10,
            },
        ];
        let index = SameFileIndex::build(&symbols);

        // Two matches with same name and kind filter includes both → ambiguous
        let result = index.resolve(
            "Point",
            &[CangjieSymbolKind::Class, CangjieSymbolKind::Struct],
        );
        assert!(result.is_none());
    }

    #[test]
    fn same_file_index_kind_filter_excludes() {
        let symbols = vec![
            CangjieSymbol {
                kind: CangjieSymbolKind::Class,
                name: "Point".to_string(),
                start_line: 1,
                end_line: 5,
            },
            CangjieSymbol {
                kind: CangjieSymbolKind::Function,
                name: "Point".to_string(),
                start_line: 7,
                end_line: 10,
            },
        ];
        let index = SameFileIndex::build(&symbols);

        // Only Class matches because Function is not in target_kinds
        let result = index.resolve(
            "Point",
            &[CangjieSymbolKind::Class, CangjieSymbolKind::Struct],
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().kind, CangjieSymbolKind::Class);
    }
}
