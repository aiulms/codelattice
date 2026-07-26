//! Call 解析查询索引 — CalleeIndex / ImportBindingTable / CallerIndex
//!
//! 来源：calls.rs 原 lines 190-567（2026-07-26 行为等价提取，第二刀）。
//! 与 stdlib_tables 提取（2026-05-04 第一刀）遵循相同 playbook：纯搬运，
//! 保留原 doc 注释与中文语义注释，所有项可见性统一为 `pub(crate)`，
//! 供 calls.rs 内部消费，不对外暴露。
//!
//! 三个索引的职责：
//! - `CalleeIndex`：按 (module_path, name) / id / method-name / source-file /
//!   crate-wide 组织 Symbol，供 call site callee 解析查询。与 imports.rs
//!   SymbolIndex 对称。
//! - `ImportBindingTable`：从已解析 ImportUse 构建绑定表，支持按
//!   (module_path, target_name) 查 import binding 与 external type 原路径。
//! - `CallerIndex`：按 source_path 组织 function/method/associated-function
//!   Symbol，用于推断 call site 的 enclosing function（最小 enclosing scope）。

use std::collections::{HashMap, HashSet};

use crate::model::*;

// ============================================================
// CalleeIndex — 与 imports.rs SymbolIndex 对称
// ============================================================

#[derive(Clone)]
pub(crate) struct CalleeMatch {
    pub(crate) id: String,
    pub(crate) symbol_kind: String,
    #[allow(dead_code)]
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) source_path: String,
    pub(crate) module_path: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) impl_details: Option<ImplBlockDetail>,
}

pub(crate) struct CalleeIndex {
    by_module_and_name: HashMap<(String, String), Vec<CalleeMatch>>,
    by_id: HashMap<String, CalleeMatch>,
    methods_by_name: HashMap<String, Vec<CalleeMatch>>,
    by_source_name_kind: HashMap<(String, String, String), Vec<CalleeMatch>>,
    functions_by_package_name: HashMap<(String, String), Vec<CalleeMatch>>,
    types_by_package_name: HashMap<(String, String), Vec<CalleeMatch>>,
    /// source_path → package_name 映射，用于跨文件 same-crate 搜索
    source_to_package: HashMap<String, String>,
    /// caller source_path → wildcard-imported module original_path 集合
    /// 用于 crate-wide search 多 match 时的源模块感知消歧
    wildcard_modules: HashMap<String, HashSet<String>>,
}

pub(crate) fn build_callee_index(
    symbols: &[Symbol],
    source_ownership: &[SourceOwnership],
) -> CalleeIndex {
    let mut index: HashMap<(String, String), Vec<CalleeMatch>> = HashMap::new();
    let mut by_id: HashMap<String, CalleeMatch> = HashMap::new();
    let mut methods_by_name: HashMap<String, Vec<CalleeMatch>> = HashMap::new();
    let mut by_source_name_kind: HashMap<(String, String, String), Vec<CalleeMatch>> =
        HashMap::new();
    let mut functions_by_package_name: HashMap<(String, String), Vec<CalleeMatch>> = HashMap::new();
    let mut types_by_package_name: HashMap<(String, String), Vec<CalleeMatch>> = HashMap::new();

    // 构建 source_path → package_name 映射（用于跨文件 same-crate 搜索）
    let source_to_package: HashMap<String, String> = source_ownership
        .iter()
        .filter_map(|so| {
            so.package
                .as_ref()
                .map(|pkg| (so.source_path.clone(), pkg.clone()))
        })
        .collect();

    for sym in symbols {
        match sym.symbol_kind.as_str() {
            "module" => continue,
            _ => {}
        }

        let mp = sym.module_path.as_deref().unwrap_or("crate").to_string();
        let key = (mp.clone(), sym.name.clone());

        let callee = CalleeMatch {
            id: sym.id.clone(),
            symbol_kind: sym.symbol_kind.clone(),
            name: sym.name.clone(),
            source_path: sym.source_path.clone(),
            module_path: mp,
            parent_id: sym.parent_id.clone(),
            impl_details: sym.impl_details.clone(),
        };

        by_id.insert(callee.id.clone(), callee.clone());
        if callee.symbol_kind == "method" {
            methods_by_name
                .entry(callee.name.clone())
                .or_default()
                .push(callee.clone());
        }
        by_source_name_kind
            .entry((
                callee.source_path.clone(),
                callee.name.clone(),
                callee.symbol_kind.clone(),
            ))
            .or_default()
            .push(callee.clone());
        if let Some(package) = source_to_package.get(&callee.source_path) {
            if callee.symbol_kind == "function" {
                functions_by_package_name
                    .entry((package.clone(), callee.name.clone()))
                    .or_default()
                    .push(callee.clone());
            } else if callee.symbol_kind == "struct" || callee.symbol_kind == "enum" {
                types_by_package_name
                    .entry((package.clone(), callee.name.clone()))
                    .or_default()
                    .push(callee.clone());
            }
        }
        index.entry(key).or_default().push(callee);
    }

    CalleeIndex {
        by_module_and_name: index,
        by_id,
        methods_by_name,
        by_source_name_kind,
        functions_by_package_name,
        types_by_package_name,
        source_to_package,
        wildcard_modules: HashMap::new(),
    }
}

impl CalleeIndex {
    pub(crate) fn lookup(&self, module_path: &str, name: &str) -> &[CalleeMatch] {
        self.by_module_and_name
            .get(&(module_path.to_string(), name.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn lookup_by_id(&self, symbol_id: &str) -> Option<&CalleeMatch> {
        self.by_id.get(symbol_id)
    }

    /// 按 name-only 查找所有 method symbol（不验证 receiver type）
    /// blind method name resolution：唯一匹配时解析，confidence 0.65
    /// 不要求 receiver type 匹配 — 这是 type-inference stop-line 的 heuristic bridge
    pub(crate) fn lookup_method_by_name(&self, name: &str) -> Vec<&CalleeMatch> {
        self.methods_by_name
            .get(name)
            .map(|matches| matches.iter().collect())
            .unwrap_or_default()
    }

    /// 同文件 fallback 查找：按 source_path + name + symbol_kind 过滤
    /// 只在 same-module 和 import-binding 都失败后调用（fallback，线性扫描但单文件 <100 symbols）
    /// 限制 symbol_kind == kind 以避免匹配 Method / Trait 等非函数 symbol
    pub(crate) fn lookup_by_source_file(
        &self,
        source_path: &str,
        name: &str,
        kind: &str,
    ) -> Vec<&CalleeMatch> {
        self.by_source_name_kind
            .get(&(source_path.to_string(), name.to_string(), kind.to_string()))
            .map(|matches| matches.iter().collect())
            .unwrap_or_default()
    }

    /// 跨文件 same-crate function 搜索
    /// 查找与 caller 同 package 的其他 source file 中匹配的 function symbol
    /// 用于 same-module + import binding 都失败后的跨文件解析
    pub(crate) fn lookup_crate_wide_function(
        &self,
        caller_source_path: &str,
        name: &str,
    ) -> Vec<&CalleeMatch> {
        let caller_package = match self.source_to_package.get(caller_source_path) {
            Some(pkg) => pkg,
            None => return vec![],
        };

        self.functions_by_package_name
            .get(&(caller_package.clone(), name.to_string()))
            .map(|matches| {
                matches
                    .iter()
                    .filter(|m| m.source_path != caller_source_path)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 跨文件 same-crate type 搜索（用于 associated function 的 type 查找）
    /// 查找与 caller 同 package 的其他 source file 中匹配的 struct/enum type symbol
    pub(crate) fn lookup_crate_wide_type(
        &self,
        caller_source_path: &str,
        name: &str,
    ) -> Vec<&CalleeMatch> {
        let caller_package = match self.source_to_package.get(caller_source_path) {
            Some(pkg) => pkg,
            None => return vec![],
        };

        self.types_by_package_name
            .get(&(caller_package.clone(), name.to_string()))
            .map(|matches| {
                matches
                    .iter()
                    .filter(|m| m.source_path != caller_source_path)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 写入 wildcard import 源模块映射（封装入口，避免字段直接暴露）
    /// 由 extract_and_resolve_calls 在构建完 callee index 后调用：
    /// caller source_path → wildcard-imported module paths，用于 crate-wide
    /// search 多 match 时的源模块感知消歧。
    pub(crate) fn set_wildcard_modules(&mut self, map: HashMap<String, HashSet<String>>) {
        self.wildcard_modules = map;
    }

    /// 读取某 caller source_path 的 wildcard-imported module 集合（封装入口）
    /// 用于 crate-wide search 多 match 时的源模块感知消歧。
    pub(crate) fn wildcard_modules_for(&self, source_path: &str) -> Option<&HashSet<String>> {
        self.wildcard_modules.get(source_path)
    }
}

// ============================================================
// ImportBindingTable — 从已解析 ImportUse 构建绑定表
// ============================================================

pub(crate) struct ImportBinding {
    #[allow(dead_code)]
    pub(crate) target_name: String,
    pub(crate) resolved_symbol_id: Option<String>,
    pub(crate) resolved_symbol_kind: Option<String>,
    /// External crate original path (e.g., std::collections::HashMap)
    /// Used for resolving associated-function calls on imported external types
    pub(crate) original_path: Option<String>,
    /// Import path kind: "crate", "self", "super", "external", "unknown"
    pub(crate) path_kind: String,
    #[allow(dead_code)]
    pub(crate) source_path: String,
}

pub(crate) struct ImportBindingTable {
    bindings: HashMap<(String, String), Vec<ImportBinding>>,
}

pub(crate) fn build_import_binding_table(imports: &[ImportUse]) -> ImportBindingTable {
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

/// 构建 wildcard import 源模块映射
///
/// 从已解析 ImportUse 中提取 wildcard/glob import（original_path 以 "::*" 结尾），
/// 构建 caller source_path → wildcard-imported module 路径的映射。
///
/// 用途：crate-wide search 返回多个 match 时，利用 wildcard import 的源模块信息消歧——优先匹配
/// 来自 wildcard-imported 模块的 symbol。
///
/// 检测方式：original_path 以 "::*" 结尾的 import 为 wildcard/glob import。
/// 提取 module 路径：去掉末尾 "::*"，将 original_path 规范化为与 CalleeMatch.module_path
/// 可比较的绝对模块路径。
///
/// 规范化策略：
/// - 含 "::" 的路径（如 "crate::stdlib_tables::*"）→ 直接去掉 "::*" → "crate::stdlib_tables"
/// - 裸名称（如 "calculations::*"）→ 基于 caller 的 module_path 构建：
///   caller module_path = "crate" → "crate::calculations"
pub(crate) fn build_wildcard_module_map(
    imports: &[ImportUse],
) -> std::collections::HashMap<String, HashSet<String>> {
    let mut map: std::collections::HashMap<String, HashSet<String>> = HashMap::new();
    for imp in imports {
        if let Some(stripped) = imp.original_path.strip_suffix("::*") {
            // 规范化为绝对模块路径（对齐 CalleeMatch.module_path）
            let normalized = if stripped.contains("::") {
                // 已有完整路径：crate::stdlib_tables, self::foo
                stripped.to_string()
            } else {
                // 裸名称：基于 caller 的 module_path 构建
                let caller_module = imp.module_path.as_deref().unwrap_or("crate");
                format!("{}::{}", caller_module, stripped)
            };
            let entry = map.entry(imp.source_path.clone()).or_default();
            entry.insert(normalized);
        }
    }
    map
}

impl ImportBindingTable {
    pub(crate) fn lookup(&self, module_path: &str, name: &str) -> &[ImportBinding] {
        self.bindings
            .get(&(module_path.to_string(), name.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Look up an external crate type binding by name in the given module.
    /// Returns the original_path (e.g., "std::collections::HashMap") if the
    /// name was imported from an external crate.
    pub(crate) fn lookup_external_type(&self, module_path: &str, name: &str) -> Option<&str> {
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

    /// 构造空绑定表（封装入口，避免字段直接暴露）
    /// 用于无 import 上下文的场景（如 extract_delta_calls）。
    pub(crate) fn empty() -> Self {
        ImportBindingTable {
            bindings: HashMap::new(),
        }
    }
}

// ============================================================
// CallerIndex — 用于推断 enclosing function
// ============================================================

pub(crate) struct CallerInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) source_path: String,
    pub(crate) line_start: u32,
    pub(crate) line_end: u32,
}

pub(crate) struct CallerIndex {
    by_file: HashMap<String, Vec<CallerInfo>>,
}

pub(crate) fn build_caller_index(symbols: &[Symbol]) -> CallerIndex {
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
    for callers in index.values_mut() {
        callers.sort_by(|a, b| {
            a.line_start
                .cmp(&b.line_start)
                .then(a.line_end.cmp(&b.line_end))
                .then(a.name.cmp(&b.name))
        });
    }

    CallerIndex { by_file: index }
}

impl CallerIndex {
    pub(crate) fn find_enclosing(&self, source_path: &str, line: u32) -> Option<&CallerInfo> {
        let callers = self.by_file.get(source_path)?;
        let mut best: Option<&CallerInfo> = None;
        let mut best_span = u32::MAX;

        let upper = callers.partition_point(|caller| caller.line_start <= line);
        for caller in callers[..upper].iter().rev() {
            if line >= caller.line_start && line <= caller.line_end {
                let span = caller.line_end - caller.line_start;
                if span < best_span {
                    best_span = span;
                    best = Some(caller);
                }
                // 从最近的 start_line 往前扫，普通 Rust 函数不嵌套；第一个命中通常就是最窄 scope。
                if span == 0 || caller.line_start < line {
                    break;
                }
            }
        }

        best
    }
}
