//! Call Site 提取与解析 — tree-sitter extractor + resolver
//!
//! 第五刀 intermediate output，不直接进入 graph emitter。
//! 从 .rs 文件中提取 call_expression 节点，分类为 8 种 CallKind，
//! 并使用 SymbolIndex + ImportBindingTable 进行 callee 解析。
//!
//! 核心策略：
//! - tree-sitter 优先提取 call_expression 节点
//! - 分类：free-function / qualified-path / self-path / super-path /
//!   associated-function / method-call / external-crate / unknown
//! - 解析策略：same-module → import-binding → crate-path → self/super → associated-fn
//! - method-call：blind method name resolution（confidence 0.65）
//! - external-crate：crate name classification only，不解析 crate 内 symbol（confidence 0.60）
//! - caller context 从 Symbol span overlap 推断（最小 enclosing function）
//!
//! 已知限制：
//! - 不做 type inference，method dispatch 无法验证 receiver type（stop-line）
//! - 不索引 external crate API symbol（stop-line）
//! - 不处理 closure / function pointer / macro expansion（stop-line）

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::model::*;
use crate::root_resolution::{self, ModuleResolveResult};

/// call site 提取结果
pub struct CallExtractionResult {
    pub calls: Vec<CallSite>,
    pub diagnostics: Vec<CallDiagnostic>,
}

/// 从多个 .rs 文件提取并解析 call site
pub fn extract_and_resolve_calls(
    repo_root: &Path,
    source_ownership: &[SourceOwnership],
    targets: &[TargetModel],
    packages: &[PackageModel],
    module_path_map: &crate::module_path::ModulePathMap,
    symbols: &[Symbol],
    imports: &[ImportUse],
) -> CallExtractionResult {
    let symbol_index = build_callee_index(symbols);
    let import_bindings = build_import_binding_table(imports);
    let caller_index = build_caller_index(symbols);

    // 构建已知外部 crate 名称集合（来自所有 package 的 [dependencies]）
    let dependency_names: HashSet<String> = packages
        .iter()
        .flat_map(|p| p.dependency_names.iter().cloned())
        .collect();

    let mut all_calls = Vec::new();
    let all_diagnostics = Vec::new();

    for so in source_ownership {
        if so.package.is_none() {
            continue;
        }

        let abs_path = repo_root.join(&so.source_path);
        let source_text = match std::fs::read_to_string(&abs_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let target_name = match &so.target {
            Some(t) => t.clone(),
            None => continue,
        };
        let target = match targets.iter().find(|t| t.name == target_name) {
            Some(t) => t,
            None => continue,
        };
        let crate_root_rel = &target.crate_root_file;
        let crate_root_abs = repo_root.join(crate_root_rel);

        let module_path = module_path_map.get(&so.source_path).to_string();

        let calls = extract_calls_from_file(
            &source_text,
            &so.source_path,
            &module_path,
            &crate_root_abs,
            repo_root,
            &symbol_index,
            &import_bindings,
            &caller_index,
            &dependency_names,
        );

        all_calls.extend(calls);
    }

    all_calls.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then(a.span.line_start.cmp(&b.span.line_start))
            .then(a.callee_name.cmp(&b.callee_name))
    });

    CallExtractionResult {
        calls: all_calls,
        diagnostics: all_diagnostics,
    }
}

// ============================================================
// CalleeIndex — 与 imports.rs SymbolIndex 对称
// ============================================================

struct CalleeMatch {
    id: String,
    symbol_kind: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    source_path: String,
    module_path: String,
    parent_id: Option<String>,
    impl_details: Option<ImplBlockDetail>,
}

struct CalleeIndex {
    by_module_and_name: HashMap<(String, String), Vec<CalleeMatch>>,
}

fn build_callee_index(symbols: &[Symbol]) -> CalleeIndex {
    let mut index: HashMap<(String, String), Vec<CalleeMatch>> = HashMap::new();

    for sym in symbols {
        match sym.symbol_kind.as_str() {
            "module" => continue,
            _ => {}
        }

        let mp = sym.module_path.as_deref().unwrap_or("crate").to_string();
        let key = (mp.clone(), sym.name.clone());

        index.entry(key).or_default().push(CalleeMatch {
            id: sym.id.clone(),
            symbol_kind: sym.symbol_kind.clone(),
            name: sym.name.clone(),
            source_path: sym.source_path.clone(),
            module_path: mp,
            parent_id: sym.parent_id.clone(),
            impl_details: sym.impl_details.clone(),
        });
    }

    CalleeIndex {
        by_module_and_name: index,
    }
}

impl CalleeIndex {
    fn lookup(&self, module_path: &str, name: &str) -> &[CalleeMatch] {
        self.by_module_and_name
            .get(&(module_path.to_string(), name.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    fn lookup_by_id(&self, symbol_id: &str) -> Option<&CalleeMatch> {
        for matches in self.by_module_and_name.values() {
            for m in matches {
                if m.id == symbol_id {
                    return Some(m);
                }
            }
        }
        None
    }

    /// 按 name-only 查找所有 method symbol（不验证 receiver type）
    /// blind method name resolution：唯一匹配时解析，confidence 0.65
    /// 不要求 receiver type 匹配 — 这是 type-inference stop-line 的 heuristic bridge
    fn lookup_method_by_name(&self, name: &str) -> Vec<&CalleeMatch> {
        self.by_module_and_name
            .values()
            .flatten()
            .filter(|m| m.symbol_kind == "method" && m.name == name)
            .collect()
    }

    /// 同文件 fallback 查找：按 source_path + name + symbol_kind 过滤
    /// 只在 same-module 和 import-binding 都失败后调用（fallback，线性扫描但单文件 <100 symbols）
    /// 限制 symbol_kind == kind 以避免匹配 Method / Trait 等非函数 symbol
    fn lookup_by_source_file(
        &self,
        source_path: &str,
        name: &str,
        kind: &str,
    ) -> Vec<&CalleeMatch> {
        self.by_module_and_name
            .values()
            .flatten()
            .filter(|m| m.source_path == source_path && m.name == name && m.symbol_kind == kind)
            .collect()
    }
}

// ============================================================
// ImportBindingTable — 从已解析 ImportUse 构建绑定表
// ============================================================

struct ImportBinding {
    #[allow(dead_code)]
    target_name: String,
    resolved_symbol_id: Option<String>,
    resolved_symbol_kind: Option<String>,
    /// External crate original path (e.g., std::collections::HashMap)
    /// Used for resolving associated-function calls on imported external types
    original_path: Option<String>,
    /// Import path kind: "crate", "self", "super", "external", "unknown"
    path_kind: String,
    #[allow(dead_code)]
    source_path: String,
}

struct ImportBindingTable {
    bindings: HashMap<(String, String), Vec<ImportBinding>>,
}

fn build_import_binding_table(imports: &[ImportUse]) -> ImportBindingTable {
    let mut table: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    for imp in imports {
        let mp = imp.module_path.as_deref().unwrap_or("crate").to_string();
        let key = (mp.clone(), imp.target_name.clone());

        let binding = ImportBinding {
            target_name: imp.target_name.clone(),
            resolved_symbol_id: imp
                .resolved_to
                .as_ref()
                .and_then(|t| t.resolved_symbol_id.clone()),
            resolved_symbol_kind: imp
                .resolved_to
                .as_ref()
                .and_then(|t| t.resolved_symbol_kind.clone()),
            original_path: Some(imp.original_path.clone()),
            path_kind: imp.path_kind.clone(),
            source_path: imp.source_path.clone(),
        };

        table.entry(key).or_default().push(binding);
    }

    ImportBindingTable { bindings: table }
}

impl ImportBindingTable {
    fn lookup(&self, module_path: &str, name: &str) -> &[ImportBinding] {
        self.bindings
            .get(&(module_path.to_string(), name.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Look up an external crate type binding by name in the given module.
    /// Returns the original_path (e.g., "std::collections::HashMap") if the
    /// name was imported from an external crate.
    fn lookup_external_type(&self, module_path: &str, name: &str) -> Option<&str> {
        self.bindings
            .get(&(module_path.to_string(), name.to_string()))
            .and_then(|bindings| {
                bindings.iter().find_map(|b| {
                    if b.path_kind == "external" {
                        b.original_path.as_deref()
                    } else {
                        None
                    }
                })
            })
    }
}

// ============================================================
// CallerIndex — 用于推断 enclosing function
// ============================================================

struct CallerInfo {
    id: String,
    name: String,
    #[allow(dead_code)]
    source_path: String,
    line_start: u32,
    line_end: u32,
}

struct CallerIndex {
    by_file: HashMap<String, Vec<CallerInfo>>,
}

fn build_caller_index(symbols: &[Symbol]) -> CallerIndex {
    let mut index: HashMap<String, Vec<CallerInfo>> = HashMap::new();

    for sym in symbols {
        let kind = sym.symbol_kind.as_str();
        if kind != "function" && kind != "method" && kind != "associated-function" {
            continue;
        }

        let entry = index.entry(sym.source_path.clone()).or_default();
        entry.push(CallerInfo {
            id: sym.id.clone(),
            name: sym.name.clone(),
            source_path: sym.source_path.clone(),
            line_start: sym.line_start,
            line_end: sym.line_end,
        });
    }

    CallerIndex { by_file: index }
}

impl CallerIndex {
    fn find_enclosing(&self, source_path: &str, line: u32) -> Option<&CallerInfo> {
        let callers = self.by_file.get(source_path)?;
        let mut best: Option<&CallerInfo> = None;
        let mut best_span = u32::MAX;

        for caller in callers {
            if line >= caller.line_start && line <= caller.line_end {
                let span = caller.line_end - caller.line_start;
                if span < best_span {
                    best_span = span;
                    best = Some(caller);
                }
            }
        }

        best
    }
}

// ============================================================
// tree-sitter 提取
// ============================================================

fn extract_calls_from_file(
    source_text: &str,
    source_path: &str,
    module_path: &str,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
) -> Vec<CallSite> {
    #[cfg(feature = "tree-sitter-extraction")]
    {
        if let Some(calls) = extract_calls_tree_sitter(
            source_text,
            source_path,
            module_path,
            crate_root_abs,
            repo_root,
            symbol_index,
            import_bindings,
            caller_index,
            dependency_names,
        ) {
            return calls;
        }
    }

    extract_calls_text_fallback(
        source_text,
        source_path,
        module_path,
        symbol_index,
        import_bindings,
        caller_index,
        dependency_names,
    )
}

#[cfg(feature = "tree-sitter-extraction")]
fn extract_calls_tree_sitter(
    source_text: &str,
    source_path: &str,
    module_path: &str,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
) -> Option<Vec<CallSite>> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).ok()?;

    let tree = parser.parse(source_text, None)?;
    let root = tree.root_node();
    let source_bytes = source_text.as_bytes();

    let mut calls = Vec::new();
    collect_call_expressions(
        &root,
        source_bytes,
        source_path,
        module_path,
        crate_root_abs,
        repo_root,
        symbol_index,
        import_bindings,
        caller_index,
        dependency_names,
        &mut calls,
    );

    Some(calls)
}

#[cfg(feature = "tree-sitter-extraction")]
fn collect_call_expressions<'a>(
    node: &tree_sitter::Node<'a>,
    source_bytes: &[u8],
    source_path: &str,
    module_path: &str,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
    calls: &mut Vec<CallSite>,
) {
    if node.kind() == "call_expression" {
        if let Some(call_site) = process_call_expression(
            node,
            source_bytes,
            source_path,
            module_path,
            crate_root_abs,
            repo_root,
            symbol_index,
            import_bindings,
            caller_index,
            dependency_names,
        ) {
            calls.push(call_site);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let inner_kind = child.kind();
            if inner_kind == "call_expression"
                || inner_kind == "method_call_expression"
                || inner_kind == "arguments"
            {
                collect_call_expressions(
                    &child,
                    source_bytes,
                    source_path,
                    module_path,
                    crate_root_abs,
                    repo_root,
                    symbol_index,
                    import_bindings,
                    caller_index,
                    dependency_names,
                    calls,
                );
            }
        }
        return;
    }

    if node.kind() == "method_call_expression" {
        if let Some(call_site) = process_method_call_expression(
            node,
            source_bytes,
            source_path,
            module_path,
            caller_index,
            crate_root_abs,
            repo_root,
            symbol_index,
            import_bindings,
        ) {
            calls.push(call_site);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let inner_kind = child.kind();
            if inner_kind == "call_expression"
                || inner_kind == "method_call_expression"
                || inner_kind == "arguments"
            {
                collect_call_expressions(
                    &child,
                    source_bytes,
                    source_path,
                    module_path,
                    crate_root_abs,
                    repo_root,
                    symbol_index,
                    import_bindings,
                    caller_index,
                    dependency_names,
                    calls,
                );
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_call_expressions(
            &child,
            source_bytes,
            source_path,
            module_path,
            crate_root_abs,
            repo_root,
            symbol_index,
            import_bindings,
            caller_index,
            dependency_names,
            calls,
        );
    }
}

#[cfg(feature = "tree-sitter-extraction")]
fn process_call_expression(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    source_path: &str,
    module_path: &str,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
) -> Option<CallSite> {
    let line_start = byte_to_line(source_bytes, node.start_byte());
    let line_end = byte_to_line(source_bytes, node.end_byte());
    let raw_text = node.utf8_text(source_bytes).unwrap_or("").to_string();

    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    let func_node = children.iter().find(|c| {
        let k = c.kind();
        k == "identifier"
            || k == "scoped_identifier"
            || k == "field_expression"
            || k == "generic_function"
            || k == "parenthesized_expression"
    })?;

    let func_node = if func_node.kind() == "generic_function" {
        let mut gc = func_node.walk();
        let children: Vec<tree_sitter::Node> = func_node.children(&mut gc).collect();
        children
            .into_iter()
            .find(|c| c.kind() == "identifier" || c.kind() == "scoped_identifier")?
    } else {
        *func_node
    };

    let (callee_path, callee_name, call_kind, known_crate) =
        classify_callee(&func_node, source_bytes, module_path, dependency_names);

    // Rust enum variant constructor 过滤：Some/Ok/Err/None 不是函数调用
    // tree-sitter 把 Some(x) 解析为 call_expression，但 Rust 语义中这些是 enum variant constructors
    const RUST_ENUM_CONSTRUCTORS: &[&str] = &["Some", "Ok", "Err", "None"];
    if RUST_ENUM_CONSTRUCTORS.contains(&callee_name.as_str()) {
        let caller_info = caller_index.find_enclosing(source_path, line_start);
        return Some(CallSite {
            id: format!("{}::call::{}::{}", source_path, line_start, callee_name),
            caller_symbol_id: caller_info.map(|c| c.id.clone()),
            caller_name: caller_info.map(|c| c.name.clone()),
            source_path: source_path.to_string(),
            module_path: Some(module_path.to_string()),
            span: CallSpan {
                line_start,
                line_end,
                byte_start: node.start_byte(),
                byte_end: node.end_byte(),
            },
            raw_text,
            known_crate: None,
            callee_path: callee_path.clone(),
            callee_name: callee_name.clone(),
            call_kind: call_kind.as_str().to_string(),
            resolved_symbol_id: None,
            resolved_symbol_kind: None,
            confidence: 0.0,
            reason: CallResolutionReason::CallEnumConstructor
                .as_str()
                .to_string(),
            diagnostics: vec![CallDiagnostic {
                code: "call-enum-constructor-filtered".to_string(),
                severity: "info".to_string(),
                message: format!(
                    "{} 是 Rust enum variant constructor，不是函数调用",
                    callee_name
                ),
                target_name: Some(callee_name.clone()),
            }],
        });
    }

    let caller_info = caller_index.find_enclosing(source_path, line_start);

    let mut call_site = CallSite {
        id: format!("{}::call::{}::{}", source_path, line_start, callee_name),
        caller_symbol_id: caller_info.map(|c| c.id.clone()),
        caller_name: caller_info.map(|c| c.name.clone()),
        source_path: source_path.to_string(),
        module_path: Some(module_path.to_string()),
        span: CallSpan {
            line_start,
            line_end,
            byte_start: node.start_byte(),
            byte_end: node.end_byte(),
        },
        raw_text,
        known_crate,
        callee_path: callee_path.clone(),
        callee_name: callee_name.clone(),
        call_kind: call_kind.as_str().to_string(),
        resolved_symbol_id: None,
        resolved_symbol_kind: None,
        confidence: 0.0,
        reason: String::new(),
        diagnostics: vec![],
    };

    let source_text = std::str::from_utf8(source_bytes).unwrap_or("");

    resolve_call_site(
        &mut call_site,
        crate_root_abs,
        repo_root,
        symbol_index,
        import_bindings,
        source_text,
    );

    Some(call_site)
}

#[cfg(feature = "tree-sitter-extraction")]
fn process_method_call_expression(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    source_path: &str,
    module_path: &str,
    caller_index: &CallerIndex,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
) -> Option<CallSite> {
    let line_start = byte_to_line(source_bytes, node.start_byte());
    let line_end = byte_to_line(source_bytes, node.end_byte());
    let raw_text = node.utf8_text(source_bytes).unwrap_or("").to_string();

    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    let method_name_node = children.iter().find(|c| c.kind() == "identifier")?;
    let method_name = method_name_node
        .utf8_text(source_bytes)
        .unwrap_or("")
        .to_string();

    let caller_info = caller_index.find_enclosing(source_path, line_start);

    let source_text = std::str::from_utf8(source_bytes).unwrap_or("");

    let mut call_site = CallSite {
        id: format!("{}::call::{}::{}", source_path, line_start, method_name),
        caller_symbol_id: caller_info.map(|c| c.id.clone()),
        caller_name: caller_info.map(|c| c.name.clone()),
        source_path: source_path.to_string(),
        module_path: Some(module_path.to_string()),
        span: CallSpan {
            line_start,
            line_end,
            byte_start: node.start_byte(),
            byte_end: node.end_byte(),
        },
        raw_text,
        known_crate: None,
        callee_path: method_name.clone(),
        callee_name: method_name,
        call_kind: CallKind::MethodCall.as_str().to_string(),
        resolved_symbol_id: None,
        resolved_symbol_kind: None,
        confidence: 0.0,
        reason: String::new(),
        diagnostics: vec![],
    };

    resolve_call_site(
        &mut call_site,
        crate_root_abs,
        repo_root,
        symbol_index,
        import_bindings,
        source_text,
    );

    Some(call_site)
}

#[cfg(feature = "tree-sitter-extraction")]
fn classify_callee(
    func_node: &tree_sitter::Node,
    source_bytes: &[u8],
    _module_path: &str,
    dependency_names: &HashSet<String>,
) -> (String, String, CallKind, Option<String>) {
    match func_node.kind() {
        "identifier" => {
            let name = func_node.utf8_text(source_bytes).unwrap_or("").to_string();
            (name.clone(), name, CallKind::FreeFunction, None)
        }
        "scoped_identifier" => {
            let path_text = func_node.utf8_text(source_bytes).unwrap_or("").to_string();
            let segments: Vec<&str> = path_text.split("::").collect();
            let name = segments.last().unwrap_or(&"").to_string();

            if segments.len() >= 2 {
                let first = segments[0];
                let second_last = segments[segments.len() - 2];

                // external crate 检测：第一个 segment 是已知 dependency name
                // 必须在 AssociatedFunction 检测之前，否则 std::vec::Vec::new() 会因 Vec 大写被误判为 AssociatedFunction
                // std / core / alloc 是隐式依赖（不在 Cargo.toml [dependencies] 中），已在 manifest.rs 硬编码补充
                if dependency_names.contains(first) {
                    return (
                        path_text.clone(),
                        name,
                        CallKind::ExternalCrate,
                        Some(first.to_string()),
                    );
                }

                if first == "crate" {
                    (path_text.clone(), name, CallKind::QualifiedPath, None)
                } else if first == "self" {
                    (path_text.clone(), name, CallKind::SelfPath, None)
                } else if first == "super" {
                    (path_text.clone(), name, CallKind::SuperPath, None)
                } else if second_last
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                    && segments.len() >= 3
                {
                    (path_text.clone(), name, CallKind::AssociatedFunction, None)
                } else if first
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                    && segments.len() == 2
                {
                    (path_text.clone(), name, CallKind::AssociatedFunction, None)
                } else {
                    (path_text.clone(), name, CallKind::QualifiedPath, None)
                }
            } else {
                (path_text.clone(), name, CallKind::Unknown, None)
            }
        }
        "field_expression" => {
            let text = func_node.utf8_text(source_bytes).unwrap_or("").to_string();
            let name = text.split('.').last().unwrap_or("").to_string();
            (text, name, CallKind::MethodCall, None)
        }
        _ => {
            let text = func_node.utf8_text(source_bytes).unwrap_or("").to_string();
            (text.clone(), text, CallKind::Unknown, None)
        }
    }
}

// ============================================================
// 解析逻辑
// ============================================================

fn resolve_call_site(
    call: &mut CallSite,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    source_text: &str,
) {
    match call.call_kind.as_str() {
        "free-function" => resolve_free_function(call, symbol_index, import_bindings),
        "qualified-path" => resolve_qualified_path(call, crate_root_abs, repo_root, symbol_index),
        "self-path" => resolve_self_path(call, crate_root_abs, repo_root, symbol_index),
        "super-path" => resolve_super_path(call, crate_root_abs, repo_root, symbol_index),
        "associated-function" => resolve_associated_function(call, symbol_index, import_bindings),
        "method-call" => {
            // blind method name resolution：查找 crate 内所有同名 method symbol
            // 不验证 receiver type（type inference stop-line），唯一匹配时才解析
            // confidence 0.65：低于所有现有 resolution path，因为 receiver type 未验证
            let methods = symbol_index.lookup_method_by_name(&call.callee_name);
            match methods.as_slice() {
                [single] => {
                    call.resolved_symbol_id = Some(single.id.clone());
                    call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                    call.confidence = 0.65;
                    call.reason = CallResolutionReason::CallMethodNameResolved
                        .as_str()
                        .to_string();
                }
                [] => {
                    // Phase 1 extended: check if method name is a known-unique stdlib trait method
                    // e.g., to_string() → std::string::ToString::to_string
                    if let Some(trait_path) = lookup_stdlib_trait_method(&call.callee_name) {
                        call.resolved_symbol_id = Some(trait_path.to_string());
                        call.confidence = 0.55;
                        call.reason = CallResolutionReason::CallStdlibTraitMethodResolved
                            .as_str()
                            .to_string();
                        return;
                    }
                    // Phase 2: receiver-type-aware resolution
                    // 从 raw_text 提取 receiver variable name（e.g., "x.push(1)" → "x"）
                    // 扫描 same-function let 绑定类型注解，查 STDLIB_TYPE_METHODS 表
                    if let Some(dot_pos) = call.raw_text.find('.') {
                        let receiver = &call.raw_text[..dot_pos];
                        // 只处理简单 identifier receiver（不是 literal 或 path）
                        if receiver.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            if let Some(base_type) = scan_variable_type_annotation(
                                source_text,
                                call.span.byte_start,
                                receiver,
                            ) {
                                if let Some(resolved_path) =
                                    lookup_receiver_type_method(&base_type, &call.callee_name)
                                {
                                    call.resolved_symbol_id = Some(resolved_path);
                                    call.confidence = 0.65;
                                    call.reason =
                                        CallResolutionReason::CallReceiverTypeMethodResolved
                                            .as_str()
                                            .to_string();
                                    return;
                                }
                            }
                        }
                    }
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                }
                _multiple => {
                    call.reason = CallResolutionReason::CallTargetAmbiguous
                        .as_str()
                        .to_string();
                }
            }
        }
        "external-crate" => {
            // Phase 1: direct path resolution for std/core/alloc
            // 代码已通过 rustc 编译 → 路径正确（compiler implied guarantee）
            // 不验证 symbol 存在性，直接构造 resolved_symbol_id
            // confidence 0.80：高于 classified(0.60)，低于 same-module(0.90) / import(0.85)
            if let Some(ref krate) = call.known_crate {
                if krate == "std" || krate == "core" || krate == "alloc" {
                    let clean_path = strip_generics(&call.callee_path);
                    call.resolved_symbol_id = Some(clean_path);
                    call.confidence = 0.80;
                    call.reason = CallResolutionReason::CallExternalCratePathResolved
                        .as_str()
                        .to_string();
                    return;
                }
            }
            // third-party crate：只分类 crate name，不解析 crate 内 symbol
            // confidence 0.60：crate name known (from [dependencies] 或隐式 std/core/alloc)，
            // 但 crate 内 symbol 未索引，低于 method-name-resolved (0.65)
            call.confidence = 0.60;
            call.reason = CallResolutionReason::CallExternalCrateClassified
                .as_str()
                .to_string();
        }
        _ => {
            if call.reason.is_empty() {
                call.reason = CallResolutionReason::CallTargetUnresolved
                    .as_str()
                    .to_string();
            }
        }
    }
}

/// 解析 free function：same-module → import binding
fn resolve_free_function(
    call: &mut CallSite,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
) {
    let module_path = call.module_path.as_deref().unwrap_or("crate");

    // 1. same-module lookup
    let same_module = symbol_index.lookup(module_path, &call.callee_name);
    match same_module {
        [single] => {
            call.resolved_symbol_id = Some(single.id.clone());
            call.resolved_symbol_kind = Some(single.symbol_kind.clone());
            call.confidence = 0.90;
            call.reason = CallResolutionReason::CallSameModuleResolved
                .as_str()
                .to_string();
            return;
        }
        multiple if !multiple.is_empty() => {
            call.reason = CallResolutionReason::CallTargetAmbiguous
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-ambiguous".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "multiple symbols match {}::{}",
                    module_path, call.callee_name
                ),
                target_name: Some(call.callee_name.clone()),
            });
            return;
        }
        _ => {}
    }

    // 2. import binding lookup
    let bindings = import_bindings.lookup(module_path, &call.callee_name);
    match bindings {
        [single] => {
            if let Some(ref sym_id) = single.resolved_symbol_id {
                call.resolved_symbol_id = Some(sym_id.clone());
                call.resolved_symbol_kind = single.resolved_symbol_kind.clone();
                call.confidence = 0.85;
                call.reason = CallResolutionReason::CallImportResolved
                    .as_str()
                    .to_string();
            } else {
                call.reason = CallResolutionReason::CallTargetUnresolved
                    .as_str()
                    .to_string();
                call.diagnostics.push(CallDiagnostic {
                    code: "call-target-unresolved".to_string(),
                    severity: "warning".to_string(),
                    message: format!("import {} 未解析到 symbol", call.callee_name),
                    target_name: Some(call.callee_name.clone()),
                });
            }
            return;
        }
        multiple if !multiple.is_empty() => {
            let resolved: Vec<_> = multiple
                .iter()
                .filter(|b| b.resolved_symbol_id.is_some())
                .collect();
            if resolved.len() == 1 {
                if let Some(ref sym_id) = resolved[0].resolved_symbol_id {
                    call.resolved_symbol_id = Some(sym_id.clone());
                    call.resolved_symbol_kind = resolved[0].resolved_symbol_kind.clone();
                    call.confidence = 0.85;
                    call.reason = CallResolutionReason::CallImportResolved
                        .as_str()
                        .to_string();
                }
            } else {
                call.reason = CallResolutionReason::CallTargetAmbiguous
                    .as_str()
                    .to_string();
                call.diagnostics.push(CallDiagnostic {
                    code: "call-target-ambiguous".to_string(),
                    severity: "warning".to_string(),
                    message: format!("multiple import bindings for {}", call.callee_name),
                    target_name: Some(call.callee_name.clone()),
                });
            }
            return;
        }
        _ => {}
    }

    // 3. same-file unique-name fallback
    // heuristic 只在 same-module 和 import-binding 都失败后触发
    // 查找同 source file 内唯一同名 Function symbol（限制 symbol_kind == "function"）
    // 不触发于 method-call / associated-function / qualified-path 等 call form
    let same_file_functions =
        symbol_index.lookup_by_source_file(&call.source_path, &call.callee_name, "function");
    match same_file_functions.as_slice() {
        [single] => {
            call.resolved_symbol_id = Some(single.id.clone());
            call.resolved_symbol_kind = Some(single.symbol_kind.clone());
            call.confidence = 0.70;
            call.reason = CallResolutionReason::CallSameFileUniqueName
                .as_str()
                .to_string();
            return;
        }
        multiple if !multiple.is_empty() => {
            // 同文件内多个同名 Function — ambiguous，不产 fake target（no-edge 策略）
            call.reason = CallResolutionReason::CallTargetAmbiguous
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-same-file-ambiguous".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "多个同名函数 {} 在 {} 中，无法唯一匹配",
                    call.callee_name, call.source_path
                ),
                target_name: Some(call.callee_name.clone()),
            });
            return;
        }
        _ => {
            // 0 matches — 落入 unresolved
        }
    }

    // 4. unresolved（原 step 3）
    call.reason = CallResolutionReason::CallTargetUnresolved
        .as_str()
        .to_string();
    call.diagnostics.push(CallDiagnostic {
        code: "call-target-unresolved".to_string(),
        severity: "info".to_string(),
        message: format!(
            "free function {} 未在当前 module 或 import 中找到",
            call.callee_name
        ),
        target_name: Some(call.callee_name.clone()),
    });
}

/// 解析 crate:: qualified path
fn resolve_qualified_path(
    call: &mut CallSite,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
) {
    let path = &call.callee_path;

    let (prefix, name) = split_last_segment(path);

    if prefix == "crate" {
        let matches = symbol_index.lookup("crate", &name);
        if let [single] = matches {
            call.resolved_symbol_id = Some(single.id.clone());
            call.resolved_symbol_kind = Some(single.symbol_kind.clone());
            call.confidence = 0.90;
            call.reason = CallResolutionReason::CallCratePathResolved
                .as_str()
                .to_string();
            return;
        }
    }

    let segments = root_resolution::parse_crate_path(&prefix);

    // Bare module name fallback：Rust 中 module::func() 是 crate-relative
    // parse_crate_path 仅处理 crate:: 前缀，bare name 返回空 segments
    // 此 fallback 在 segments 为空且 prefix 非空时触发，尝试 crate::{prefix} 查找
    if segments.is_empty() && !prefix.is_empty() {
        let target_mp = format!("crate::{}", prefix);
        let matches = symbol_index.lookup(&target_mp, &name);
        match matches {
            [single] => {
                call.resolved_symbol_id = Some(single.id.clone());
                call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                call.confidence = 0.85;
                call.reason = CallResolutionReason::CallModulePathResolved
                    .as_str()
                    .to_string();
                return;
            }
            _ => {
                // 未找到唯一匹配，fall through 到现有 file-based resolution
            }
        }
    }

    let result = root_resolution::resolve_module_chain(repo_root, crate_root_abs, &segments);

    match result {
        ModuleResolveResult::Resolved { .. } => {
            let target_mp = if segments.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", segments.join("::"))
            };

            let matches = symbol_index.lookup(&target_mp, &name);
            match matches {
                [single] => {
                    call.resolved_symbol_id = Some(single.id.clone());
                    call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                    call.confidence = 0.90;
                    call.reason = CallResolutionReason::CallCratePathResolved
                        .as_str()
                        .to_string();
                }
                [] => {
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                    call.diagnostics.push(CallDiagnostic {
                        code: "call-target-unresolved".to_string(),
                        severity: "warning".to_string(),
                        message: format!("symbol {} not found in {}", name, target_mp),
                        target_name: Some(name),
                    });
                }
                _ => {
                    call.reason = CallResolutionReason::CallTargetAmbiguous
                        .as_str()
                        .to_string();
                    call.diagnostics.push(CallDiagnostic {
                        code: "call-target-ambiguous".to_string(),
                        severity: "warning".to_string(),
                        message: format!("multiple symbols match {}::{}", target_mp, name),
                        target_name: Some(name),
                    });
                }
            }
        }
        _ => {
            call.reason = CallResolutionReason::CallTargetUnresolved
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!("crate path 无法解析: {}", prefix),
                target_name: Some(name),
            });
        }
    }
}

/// 解析 self:: 路径
fn resolve_self_path(
    call: &mut CallSite,
    crate_root_abs: &Path,
    repo_root: &Path,
    symbol_index: &CalleeIndex,
) {
    let module_path = call.module_path.as_deref().unwrap_or("crate");

    let self_rest = call
        .callee_path
        .strip_prefix("self::")
        .unwrap_or(&call.callee_path);

    let expanded = if module_path == "crate" {
        format!("crate::{}", self_rest)
    } else {
        format!("{}::{}", module_path, self_rest)
    };

    let (prefix, name) = split_last_segment(&expanded);

    let matches = symbol_index.lookup(&prefix, &name);
    match matches {
        [single] => {
            call.resolved_symbol_id = Some(single.id.clone());
            call.resolved_symbol_kind = Some(single.symbol_kind.clone());
            call.confidence = 0.80;
            call.reason = CallResolutionReason::CallSelfPathResolved
                .as_str()
                .to_string();
        }
        [] => {
            let segments = root_resolution::parse_crate_path(&expanded);
            let result =
                root_resolution::resolve_module_chain(repo_root, crate_root_abs, &segments);
            match result {
                ModuleResolveResult::Resolved { .. } => {
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                    call.diagnostics.push(CallDiagnostic {
                        code: "call-target-unresolved".to_string(),
                        severity: "info".to_string(),
                        message: format!(
                            "self path resolved to module but symbol {} not found",
                            name
                        ),
                        target_name: Some(name),
                    });
                }
                _ => {
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                    call.diagnostics.push(CallDiagnostic {
                        code: "call-target-unresolved".to_string(),
                        severity: "warning".to_string(),
                        message: format!("self path 无法解析: {} → {}", call.callee_path, expanded),
                        target_name: Some(name),
                    });
                }
            }
        }
        _ => {
            call.reason = CallResolutionReason::CallTargetAmbiguous
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-ambiguous".to_string(),
                severity: "warning".to_string(),
                message: format!("multiple symbols match {}::{}", prefix, name),
                target_name: Some(name),
            });
        }
    }
}

/// 解析 super:: 路径
fn resolve_super_path(
    call: &mut CallSite,
    _crate_root_abs: &Path,
    _repo_root: &Path,
    symbol_index: &CalleeIndex,
) {
    let module_path = call.module_path.as_deref().unwrap_or("crate");

    let mut super_count = 0usize;
    let mut rest = call.callee_path.as_str();
    while let Some(r) = rest.strip_prefix("super::") {
        super_count += 1;
        rest = r;
    }

    let mut base_parts: Vec<&str> = module_path.split("::").collect();
    for _ in 0..super_count {
        if base_parts.len() <= 1 {
            call.reason = CallResolutionReason::CallTargetUnresolved
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-unresolved".to_string(),
                severity: "info".to_string(),
                message: format!("super:: 在 crate root 使用: {}", call.callee_path),
                target_name: Some(call.callee_name.clone()),
            });
            return;
        }
        base_parts.pop();
    }

    let expanded = if rest.is_empty() {
        base_parts.join("::")
    } else {
        format!("{}::{}", base_parts.join("::"), rest)
    };

    let (prefix, name) = split_last_segment(&expanded);

    let matches = symbol_index.lookup(&prefix, &name);
    match matches {
        [single] => {
            call.resolved_symbol_id = Some(single.id.clone());
            call.resolved_symbol_kind = Some(single.symbol_kind.clone());
            call.confidence = 0.80;
            call.reason = CallResolutionReason::CallSuperPathResolved
                .as_str()
                .to_string();
        }
        [] => {
            call.reason = CallResolutionReason::CallTargetUnresolved
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!("super path symbol {} not found in {}", name, prefix),
                target_name: Some(name),
            });
        }
        _ => {
            call.reason = CallResolutionReason::CallTargetAmbiguous
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-target-ambiguous".to_string(),
                severity: "warning".to_string(),
                message: format!("multiple symbols match {}::{}", prefix, name),
                target_name: Some(name),
            });
        }
    }
}

/// 解析 associated function：Type::method / crate::module::Type::method
fn resolve_associated_function(
    call: &mut CallSite,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
) {
    let path = &call.callee_path;
    let module_path = call.module_path.as_deref().unwrap_or("crate");

    let (type_and_module, method_name) = split_last_segment(path);
    let (type_prefix, type_name) = split_last_segment(&type_and_module);

    let type_module = if type_prefix.is_empty() || type_prefix == "crate" {
        resolve_type_module(&type_name, module_path, import_bindings, symbol_index)
    } else if type_prefix.starts_with("crate::")
        || type_prefix.starts_with("self::")
        || type_prefix.starts_with("super::")
    {
        let clean_prefix = type_prefix
            .strip_prefix("crate::")
            .or_else(|| type_prefix.strip_prefix("self::"))
            .or_else(|| type_prefix.strip_prefix("super::"))
            .unwrap_or(&type_prefix);
        resolve_type_module(&type_name, clean_prefix, import_bindings, symbol_index)
    } else {
        resolve_type_module(&type_name, &type_prefix, import_bindings, symbol_index)
    };

    call.callee_name = method_name.clone();

    match type_module {
        Some(mp) => {
            let matches = symbol_index.lookup(&mp, &method_name);
            let impl_matches: Vec<_> = matches
                .iter()
                .filter(|m| {
                    m.parent_id.is_some()
                        || m.impl_details.is_some()
                        || m.symbol_kind == "method"
                        || m.symbol_kind == "associated-function"
                })
                .collect();

            match impl_matches.as_slice() {
                [single] => {
                    call.resolved_symbol_id = Some(single.id.clone());
                    call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                    call.confidence = 0.75;
                    call.reason = CallResolutionReason::CallAssociatedFnResolved
                        .as_str()
                        .to_string();
                }
                [] => {
                    let all_matches = symbol_index.lookup(&mp, &method_name);
                    if let [single] = all_matches {
                        call.resolved_symbol_id = Some(single.id.clone());
                        call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                        call.confidence = 0.70;
                        call.reason = CallResolutionReason::CallAssociatedFnResolved
                            .as_str()
                            .to_string();
                    } else {
                        call.reason = CallResolutionReason::CallTargetUnresolved
                            .as_str()
                            .to_string();
                        call.diagnostics.push(CallDiagnostic {
                            code: "call-target-unresolved".to_string(),
                            severity: "info".to_string(),
                            message: format!(
                                "associated fn {} not found on type {} in {}",
                                method_name, type_name, mp
                            ),
                            target_name: Some(method_name),
                        });
                    }
                }
                _ => {
                    call.reason = CallResolutionReason::CallTargetAmbiguous
                        .as_str()
                        .to_string();
                    call.diagnostics.push(CallDiagnostic {
                        code: "call-target-ambiguous".to_string(),
                        severity: "warning".to_string(),
                        message: format!("multiple impl methods match {}::{}", mp, method_name),
                        target_name: Some(method_name),
                    });
                }
            }
        }
        None => {
            // Phase 1 extended: type not found locally — check if imported from external crate
            // or is a known prelude/stdlib type
            if let Some(external_path) =
                import_bindings.lookup_external_type(module_path, &type_name)
            {
                let clean_path = strip_generics(&format!("{}::{}", external_path, method_name));
                call.resolved_symbol_id = Some(clean_path);
                call.confidence = 0.80;
                call.reason = CallResolutionReason::CallExternalCratePathResolved
                    .as_str()
                    .to_string();
                return;
            }
            // Also check prelude types (Vec, String, Box, Option, Result) —
            // these are implicitly available without explicit `use` imports
            if let Some(prelude_path) = lookup_prelude_type_path(&type_name) {
                let clean_path = strip_generics(&format!("{}::{}", prelude_path, method_name));
                call.resolved_symbol_id = Some(clean_path);
                call.confidence = 0.80;
                call.reason = CallResolutionReason::CallExternalCratePathResolved
                    .as_str()
                    .to_string();
                return;
            }
            call.reason = CallResolutionReason::CallTargetUnresolved
                .as_str()
                .to_string();
            call.diagnostics.push(CallDiagnostic {
                code: "call-associated-fn-unsupported".to_string(),
                severity: "info".to_string(),
                message: format!(
                    "associated fn type {} 未解析，无法查找 {}",
                    type_name, method_name
                ),
                target_name: Some(method_name),
            });
        }
    }
}

/// 尝试查找 type 所在的 module path
fn resolve_type_module(
    type_name: &str,
    current_module: &str,
    import_bindings: &ImportBindingTable,
    symbol_index: &CalleeIndex,
) -> Option<String> {
    // 1. same-module lookup
    let same_module = symbol_index.lookup(current_module, type_name);
    if !same_module.is_empty() {
        return Some(current_module.to_string());
    }

    // 2. import binding lookup
    let bindings = import_bindings.lookup(current_module, type_name);
    if let [single] = bindings {
        if single.resolved_symbol_id.is_some() {
            // 从 symbol id 反查 module path
            let sym = symbol_index.lookup_by_id(&single.resolved_symbol_id.as_ref().unwrap());
            if let Some(s) = sym {
                return Some(s.module_path.clone());
            }
        }
    }

    // 3. 全局搜索（低置信度 fallback）
    None
}

// ============================================================
// text-level fallback
// ============================================================

fn extract_calls_text_fallback(
    source_text: &str,
    source_path: &str,
    module_path: &str,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
) -> Vec<CallSite> {
    let mut calls = Vec::new();
    let mut in_block_comment = false;

    for (line_idx, line) in source_text.lines().enumerate() {
        let line_num = (line_idx + 1) as u32;
        let trimmed = line.trim();

        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }

        if let Some(call_site) = parse_text_call(
            trimmed,
            source_path,
            line_num,
            module_path,
            symbol_index,
            import_bindings,
            caller_index,
            dependency_names,
            source_text,
        ) {
            calls.push(call_site);
        }
    }

    calls
}

fn parse_text_call(
    trimmed: &str,
    source_path: &str,
    line_num: u32,
    module_path: &str,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    caller_index: &CallerIndex,
    dependency_names: &HashSet<String>,
    source_text: &str,
) -> Option<CallSite> {
    if trimmed.starts_with('#') || trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
        return None;
    }

    // 查找最外层函数调用
    let paren_pos = find_outermost_call(trimmed)?;
    let callee_part = trimmed[..paren_pos].trim_end();
    let (callee_path, callee_name, call_kind, known_crate) =
        classify_text_callee(callee_part, module_path, dependency_names);

    let caller_info = caller_index.find_enclosing(source_path, line_num);

    let mut call_site = CallSite {
        id: format!("{}::call::{}::{}", source_path, line_num, callee_name),
        caller_symbol_id: caller_info.map(|c| c.id.clone()),
        caller_name: caller_info.map(|c| c.name.clone()),
        source_path: source_path.to_string(),
        module_path: Some(module_path.to_string()),
        span: CallSpan {
            line_start: line_num,
            line_end: line_num,
            byte_start: 0,
            byte_end: trimmed.len(),
        },
        raw_text: trimmed.to_string(),
        known_crate,
        callee_path: callee_path.clone(),
        callee_name: callee_name.clone(),
        call_kind: call_kind.as_str().to_string(),
        resolved_symbol_id: None,
        resolved_symbol_kind: None,
        confidence: 0.0,
        reason: String::new(),
        diagnostics: vec![],
    };

    resolve_call_site_text(&mut call_site, symbol_index, import_bindings, source_text);

    Some(call_site)
}

fn find_outermost_call(text: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' => {
                if depth == 0 {
                    return Some(i);
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn classify_text_callee(
    callee_part: &str,
    _module_path: &str,
    dependency_names: &HashSet<String>,
) -> (String, String, CallKind, Option<String>) {
    // 去除 trailing dot expression（method call）
    if let Some(dot_pos) = callee_part.rfind('.') {
        let method_name = callee_part[dot_pos + 1..].to_string();
        if !method_name.is_empty() && !method_name.starts_with('|') {
            return (
                callee_part.to_string(),
                method_name,
                CallKind::MethodCall,
                None,
            );
        }
    }

    if callee_part.contains("::") {
        let segments: Vec<&str> = callee_part.split("::").collect();
        let name = segments.last().unwrap_or(&"").to_string();

        let first = segments.first().copied().unwrap_or("");

        // external crate 检测：第一个 segment 是已知 dependency name
        // 与 tree-sitter classify_callee 对应
        if dependency_names.contains(first) {
            return (
                callee_part.to_string(),
                name,
                CallKind::ExternalCrate,
                Some(first.to_string()),
            );
        }

        if first == "crate" || first == "self" || first == "super" {
            // classified by prefix
        } else if segments.len() >= 2 {
            // 可能是 Type::method 或 external::path
        }

        let call_kind = if first == "crate" {
            CallKind::QualifiedPath
        } else if first == "self" {
            CallKind::SelfPath
        } else if first == "super" {
            CallKind::SuperPath
        } else if segments.len() >= 2 {
            let second_last = segments[segments.len() - 2];
            if second_last
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                CallKind::AssociatedFunction
            } else {
                CallKind::QualifiedPath
            }
        } else {
            CallKind::Unknown
        };

        (callee_part.to_string(), name, call_kind, None)
    } else {
        (
            callee_part.to_string(),
            callee_part.to_string(),
            CallKind::FreeFunction,
            None,
        )
    }
}

fn resolve_call_site_text(
    call: &mut CallSite,
    symbol_index: &CalleeIndex,
    import_bindings: &ImportBindingTable,
    source_text: &str,
) {
    match call.call_kind.as_str() {
        "free-function" => resolve_free_function(call, symbol_index, import_bindings),
        "associated-function" => resolve_associated_function(call, symbol_index, import_bindings),
        "method-call" => {
            // blind method name resolution：查找 crate 内所有同名 method symbol
            // 不验证 receiver type（type inference stop-line），唯一匹配时才解析
            // confidence 0.65：低于所有现有 resolution path
            let methods = symbol_index.lookup_method_by_name(&call.callee_name);
            match methods.as_slice() {
                [single] => {
                    call.resolved_symbol_id = Some(single.id.clone());
                    call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                    call.confidence = 0.65;
                    call.reason = CallResolutionReason::CallMethodNameResolved
                        .as_str()
                        .to_string();
                }
                [] => {
                    // Phase 1 extended: check if method name is a known-unique stdlib trait method
                    // e.g., to_string() → std::string::ToString::to_string
                    if let Some(trait_path) = lookup_stdlib_trait_method(&call.callee_name) {
                        call.resolved_symbol_id = Some(trait_path.to_string());
                        call.confidence = 0.55;
                        call.reason = CallResolutionReason::CallStdlibTraitMethodResolved
                            .as_str()
                            .to_string();
                        return;
                    }
                    // Phase 2: receiver-type-aware resolution
                    // 从 raw_text 提取 receiver variable name（e.g., "x.push(1)" → "x"）
                    // 扫描 same-function let 绑定类型注解，查 STDLIB_TYPE_METHODS 表
                    if let Some(dot_pos) = call.raw_text.find('.') {
                        let receiver = &call.raw_text[..dot_pos];
                        // 只处理简单 identifier receiver（不是 literal 或 path）
                        if receiver.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            if let Some(base_type) = scan_variable_type_annotation(
                                source_text,
                                call.span.byte_start,
                                receiver,
                            ) {
                                if let Some(resolved_path) =
                                    lookup_receiver_type_method(&base_type, &call.callee_name)
                                {
                                    call.resolved_symbol_id = Some(resolved_path);
                                    call.confidence = 0.65;
                                    call.reason =
                                        CallResolutionReason::CallReceiverTypeMethodResolved
                                            .as_str()
                                            .to_string();
                                    return;
                                }
                            }
                        }
                    }
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                }
                _multiple => {
                    call.reason = CallResolutionReason::CallTargetAmbiguous
                        .as_str()
                        .to_string();
                }
            }
        }
        "external-crate" => {
            // Phase 1: direct path resolution for std/core/alloc
            // 代码已通过 rustc 编译 → 路径正确（compiler implied guarantee）
            // 不验证 symbol 存在性，直接构造 resolved_symbol_id
            // confidence 0.80：高于 classified(0.60)，低于 same-module(0.90) / import(0.85)
            if let Some(ref krate) = call.known_crate {
                if krate == "std" || krate == "core" || krate == "alloc" {
                    let clean_path = strip_generics(&call.callee_path);
                    call.resolved_symbol_id = Some(clean_path);
                    call.confidence = 0.80;
                    call.reason = CallResolutionReason::CallExternalCratePathResolved
                        .as_str()
                        .to_string();
                    return;
                }
            }
            // third-party crate：只分类 crate name，不解析 crate 内 symbol
            // confidence 0.60：crate name known，但 crate 内 symbol 未索引
            call.confidence = 0.60;
            call.reason = CallResolutionReason::CallExternalCrateClassified
                .as_str()
                .to_string();
        }
        _ => {
            call.reason = CallResolutionReason::CallTargetUnresolved
                .as_str()
                .to_string();
        }
    }
}

// ============================================================
// helpers
// ============================================================

fn split_last_segment(path: &str) -> (String, String) {
    match path.rfind("::") {
        Some(pos) => (path[..pos].to_string(), path[pos + 2..].to_string()),
        None => (String::new(), path.to_string()),
    }
}

/// Map common Rust prelude/stdlib type names to their canonical paths.
/// These types are implicitly available without explicit `use` imports.
fn lookup_prelude_type_path(type_name: &str) -> Option<&'static str> {
    match type_name {
        "Vec" => Some("std::vec::Vec"),
        "String" => Some("std::string::String"),
        "Box" => Some("std::boxed::Box"),
        "Option" => Some("std::option::Option"),
        "Result" => Some("std::result::Result"),
        _ => None,
    }
}

/// Map stdlib trait method names to their canonical trait method paths.
/// Only includes method names that are UNIQUE within stdlib — i.e., only one
/// trait defines this method in the standard library.
/// Confidence 0.55: trait path is correct, but concrete receiver type is unknown.
fn lookup_stdlib_trait_method(method_name: &str) -> Option<&'static str> {
    match method_name {
        "to_string" => Some("std::string::ToString::to_string"),
        "clone" => Some("std::clone::Clone::clone"),
        // collect() 在 std 中唯一定义在 Iterator trait 上
        "collect" => Some("std::iter::Iterator::collect"),
        _ => None,
    }
}

// ============================================================
// Phase 2: receiver-type-aware method resolution
// ============================================================

/// 已知 stdlib 类型的 method 映射表。
/// 每个 entry 包含：type path prefix、类型注解匹配 pattern、已知 methods 列表。
/// confidence 0.65：receiver type 从显式 let 绑定类型注解确定，method 集合从 stdlib docs 推导。
struct StdlibTypeMethodEntry {
    /// resolved path 中使用的 type path prefix（e.g., "std::vec::Vec"）
    type_path: &'static str,
    /// 类型注解匹配模式（e.g., ["Vec<", "Vec "]，匹配 "Vec<i32>" 和 "Vec "）
    patterns: &'static [&'static str],
    /// (method_name, method_path_suffix) — resolved_symbol_id = type_path + "::" + method_path_suffix
    methods: &'static [(&'static str, &'static str)],
}

/// 已知 stdlib type → methods 映射表
/// 仅包含最常见的 stdlib 类型和最高频的 method names
static STDLIB_TYPE_METHODS: &[StdlibTypeMethodEntry] = &[
    // Vec<T> — 最常见容器类型
    StdlibTypeMethodEntry {
        type_path: "std::vec::Vec",
        patterns: &["Vec"],
        methods: &[
            ("push", "push"),
            ("len", "len"),
            ("is_empty", "is_empty"),
            ("pop", "pop"),
            ("contains", "contains"),
            ("get", "get"),
            ("last", "last"),
            ("first", "first"),
            ("remove", "remove"),
            ("clear", "clear"),
            ("insert", "insert"),
        ],
    },
    // String — 字符串类型
    StdlibTypeMethodEntry {
        type_path: "std::string::String",
        patterns: &["String"],
        methods: &[
            ("len", "len"),
            ("is_empty", "is_empty"),
            ("push_str", "push_str"),
            ("push", "push"),
            ("remove", "remove"),
            ("contains", "contains"),
            ("replace", "replace"),
            ("as_str", "as_str"),
            ("trim", "trim"),
        ],
    },
    // str — 字符串切片（primitive type, 无 std:: prefix）
    StdlibTypeMethodEntry {
        type_path: "str",
        patterns: &["&str", "&'static str", "str "],
        methods: &[
            ("starts_with", "starts_with"),
            ("ends_with", "ends_with"),
            ("contains", "contains"),
            ("find", "find"),
            ("replace", "replace"),
            ("trim", "trim"),
            ("trim_start", "trim_start"),
            ("trim_end", "trim_end"),
            ("split", "split"),
            ("len", "len"),
            ("is_empty", "is_empty"),
        ],
    },
    // Option<T>
    StdlibTypeMethodEntry {
        type_path: "std::option::Option",
        patterns: &["Option"],
        methods: &[
            ("unwrap", "unwrap"),
            ("unwrap_or", "unwrap_or"),
            ("is_some", "is_some"),
            ("is_none", "is_none"),
            ("map", "map"),
            ("and_then", "and_then"),
        ],
    },
    // Result<T,E>
    StdlibTypeMethodEntry {
        type_path: "std::result::Result",
        patterns: &["Result"],
        methods: &[
            ("unwrap", "unwrap"),
            ("unwrap_or", "unwrap_or"),
            ("is_ok", "is_ok"),
            ("is_err", "is_err"),
            ("map", "map"),
            ("map_err", "map_err"),
        ],
    },
    // HashMap<K,V>
    StdlibTypeMethodEntry {
        type_path: "std::collections::HashMap",
        patterns: &["HashMap"],
        methods: &[
            ("len", "len"),
            ("is_empty", "is_empty"),
            ("contains_key", "contains_key"),
            ("get", "get"),
            ("insert", "insert"),
            ("remove", "remove"),
            ("clear", "clear"),
        ],
    },
];

/// 从 let 绑定中扫描变量类型注解。
/// 在 call site 之前查找 `let var_name: Type = ...` 或 `let mut var_name: Type = ...`
/// 提取 Type 的 base name（去掉 &/mut/泛型参数）。
fn scan_variable_type_annotation(
    source_text: &str,
    call_byte_start: usize,
    var_name: &str,
) -> Option<String> {
    let prefix = &source_text[..call_byte_start];

    // 寻找最近的 `fn` 关键字（确保不跨越函数边界）
    // 简单启发式：从 call site 往回找最近的 "fn "，作为函数体起点
    let fn_pos = prefix.rfind("fn ")?;
    let func_scope = &prefix[fn_pos..];

    // 在函数 scope 内寻找 `let var_name: Type =` 或 `let mut var_name: Type =`
    let patterns = [
        format!("let {}: ", var_name),
        format!("let mut {}: ", var_name),
    ];

    for pattern in &patterns {
        if let Some(pos) = func_scope.rfind(pattern.as_str()) {
            // 提取 type annotation：从 pattern 结束位置到 `=` 或 `;`
            let type_start = pos + pattern.len();
            let rest = &func_scope[type_start..];
            let type_end = rest
                .find(|c: char| c == '=' || c == ';')
                .unwrap_or(rest.len());
            let type_str = rest[..type_end].trim();

            if type_str.is_empty() {
                continue;
            }

            // 去掉引用前缀（&, &mut, &'a）
            let type_str = type_str
                .trim_start_matches("&'")
                .trim_start_matches("&mut ")
                .trim_start_matches("&");
            // 去掉 lifetime 参数后的剩余: "&'a " 形式已处理，
            // 但 "&'a Type" 需要额外处理
            let type_str = type_str.trim();

            // 提取 base type name（去掉泛型参数 <...>）
            let base_type = if let Some(generic_pos) = type_str.find('<') {
                &type_str[..generic_pos]
            } else {
                type_str
            };

            // 去掉可能的 whitespace 前缀
            let base_type = base_type.trim();

            if base_type.is_empty() {
                continue;
            }

            return Some(base_type.to_string());
        }
    }

    // Phase 2b: 扫描函数参数类型注解
    // 在函数签名中查找 `fn name(param: Type, ...)` 匹配 receiver name
    if let Some(paren_open) = func_scope.find('(') {
        if let Some(paren_close) = func_scope[paren_open..].find(')') {
            let paren_close = paren_open + paren_close;
            let params = &func_scope[paren_open + 1..paren_close];
            for param_part in params.split(',') {
                let param_part = param_part.trim();
                if let Some(colon_pos) = param_part.find(':') {
                    let param_name = param_part[..colon_pos].trim();
                    // 去掉 `mut` 前缀（`mut self` 等）
                    let param_name = param_name.trim_start_matches("mut ");
                    if param_name == var_name {
                        let param_type = param_part[colon_pos + 1..].trim();
                        // 去掉引用前缀（&, &mut, &'a）
                        let param_type = param_type
                            .trim_start_matches("&'")
                            .trim_start_matches("&mut ")
                            .trim_start_matches("&")
                            .trim();
                        let base_type = if let Some(generic_pos) = param_type.find('<') {
                            &param_type[..generic_pos]
                        } else {
                            param_type
                        };
                        let base_type = base_type.trim();
                        if !base_type.is_empty() {
                            return Some(base_type.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// 根据 receiver type 和 method name 查找 stdlib method 路径
fn lookup_receiver_type_method(base_type: &str, method_name: &str) -> Option<String> {
    for entry in STDLIB_TYPE_METHODS {
        // 检查 base_type 是否匹配该类型的 pattern
        let matches = entry
            .patterns
            .iter()
            .any(|p| base_type.starts_with(p) || base_type == p.trim());

        if !matches {
            continue;
        }

        // 查找 method
        for (meth, suffix) in entry.methods {
            if *meth == method_name {
                return Some(format!("{}::{}", entry.type_path, suffix));
            }
        }
    }
    None
}

/// Strip generic parameters from a path for use as resolved_symbol_id.
/// "std::collections::HashMap::<&str, i32>::new" → "std::collections::HashMap::new"
/// Splits by "::", removes segments that are entirely generic args (start with `<`),
/// and strips generic suffix from segments like "HashMap<K,V>".
fn strip_generics(path: &str) -> String {
    path.split("::")
        .filter_map(|seg| {
            if seg.is_empty() {
                return None;
            }
            // Entire segment is a generic arg: "::<&str, i32>" → segment is "<&str, i32>"
            if seg.starts_with('<') {
                return None;
            }
            // Strip generic suffix: "HashMap<K,V>" → "HashMap"
            if let Some(pos) = seg.find('<') {
                let cleaned = &seg[..pos];
                if cleaned.is_empty() {
                    None
                } else {
                    Some(cleaned.to_string())
                }
            } else {
                Some(seg.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(feature = "tree-sitter-extraction")]
fn byte_to_line(source_bytes: &[u8], byte_offset: usize) -> u32 {
    let mut line = 1u32;
    for &b in &source_bytes[..byte_offset.min(source_bytes.len())] {
        if b == b'\n' {
            line += 1;
        }
    }
    line
}
