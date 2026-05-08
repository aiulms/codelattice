//! Stdlib tables and helpers — 从 calls.rs 提取的静态数据和查找辅助函数
//!
//! 来源：calls.rs 原 lines 1852–2150（2026-05-04 行为等价提取）
//! 包含：prelude type 映射、stdlib trait method 映射、receiver type → method 表、
//! variable type annotation 扫描、泛型参数清除等辅助函数。
//!
//! 所有函数为 pub(crate)，供 calls.rs 内部使用。

// ============================================================
// helpers
// ============================================================

pub(crate) fn split_last_segment(path: &str) -> (String, String) {
    match path.rfind("::") {
        Some(pos) => (path[..pos].to_string(), path[pos + 2..].to_string()),
        None => (String::new(), path.to_string()),
    }
}

/// Map common Rust prelude/stdlib type names to their canonical paths.
/// These types are implicitly available without explicit `use` imports.
pub(crate) fn lookup_prelude_type_path(type_name: &str) -> Option<&'static str> {
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
pub(crate) fn lookup_stdlib_trait_method(method_name: &str) -> Option<&'static str> {
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
pub(crate) struct StdlibTypeMethodEntry {
    /// resolved path 中使用的 type path prefix（e.g., "std::vec::Vec"）
    pub(crate) type_path: &'static str,
    /// 类型注解匹配模式（e.g., ["Vec<", "Vec "]，匹配 "Vec<i32>" 和 "Vec "）
    pub(crate) patterns: &'static [&'static str],
    /// (method_name, method_path_suffix) — resolved_symbol_id = type_path + "::" + method_path_suffix
    pub(crate) methods: &'static [(&'static str, &'static str)],
}

/// 已知 stdlib type → methods 映射表
/// 仅包含最常见的 stdlib 类型和最高频的 method names
pub(crate) static STDLIB_TYPE_METHODS: &[StdlibTypeMethodEntry] = &[
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
    // HashSet<T>
    StdlibTypeMethodEntry {
        type_path: "std::collections::HashSet",
        patterns: &["HashSet"],
        methods: &[
            ("len", "len"),
            ("is_empty", "is_empty"),
            ("contains", "contains"),
            ("insert", "insert"),
            ("remove", "remove"),
            ("clear", "clear"),
            ("get", "get"),
        ],
    },
    // BTreeMap<K,V>
    StdlibTypeMethodEntry {
        type_path: "std::collections::BTreeMap",
        patterns: &["BTreeMap"],
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
    // PathBuf
    StdlibTypeMethodEntry {
        type_path: "std::path::PathBuf",
        patterns: &["PathBuf"],
        methods: &[
            ("as_path", "as_path"),
            ("push", "push"),
            ("pop", "pop"),
            ("exists", "exists"),
            ("is_dir", "is_dir"),
            ("is_file", "is_file"),
            ("to_str", "to_str"),
            ("display", "display"),
            ("join", "join"),
            ("parent", "parent"),
            ("file_name", "file_name"),
            ("extension", "extension"),
            ("strip_prefix", "strip_prefix"),
        ],
    },
];

/// 从 let 绑定中扫描变量类型注解。
/// 在 call site 之前查找 `let var_name: Type = ...` 或 `let mut var_name: Type = ...`
/// 提取 Type 的 base name（去掉 &/mut/泛型参数）。
pub(crate) fn scan_variable_type_annotation(
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

    // Phase 2d: let-binding 链已知构造函数推断 receiver type
    // 处理 let v = Vec::new(); v.push(1) 之类无类型注解的声明
    // 通过 RHS 中的已知构造函数推断变量类型
    let let_patterns_no_type: &[String] = &[
        format!("let {} = ", var_name),
        format!("let mut {} = ", var_name),
    ];
    for pattern in let_patterns_no_type {
        if let Some(pos) = func_scope.rfind(pattern.as_str()) {
            let rhs_start = pos + pattern.len();
            let rest = &func_scope[rhs_start..];
            let rhs_end = rest.find(';').unwrap_or(rest.len());
            let rhs = rest[..rhs_end].trim();
            if let Some(paren_pos) = rhs.find('(') {
                let constructor_path = rhs[..paren_pos].trim();
                // 简单校验：构造路径只含字母、数字、_、:
                if constructor_path
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
                {
                    if let Some(base_type) = lookup_constructor_type(constructor_path) {
                        return Some(base_type.to_string());
                    }
                }
            }
            break;
        }
    }

    None
}

/// 根据 receiver type 和 method name 查找 stdlib method 路径
pub(crate) fn lookup_receiver_type_method(base_type: &str, method_name: &str) -> Option<String> {
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

// ============================================================
// 已知构造函数 → 基础类型 映射表
// 用于在 let v = Vec::new() 之类无类型注解的声明中推断 receiver type
// 不涉及类型推断 — 仅利用已解析的 known crate path
// ============================================================

/// 已知构造函数 → (base type name, canonical type path)
/// base type name 用于 STDLIB_TYPE_METHODS lookup
const KNOWN_CONSTRUCTORS: &[(&str, &str)] = &[
    ("Vec::new", "Vec"),
    ("Vec::with_capacity", "Vec"),
    ("String::new", "String"),
    ("String::from", "String"),
    ("HashMap::new", "HashMap"),
    ("HashMap::with_capacity", "HashMap"),
    ("HashSet::new", "HashSet"),
    ("PathBuf::new", "PathBuf"),
    ("PathBuf::from", "PathBuf"),
    ("BTreeMap::new", "BTreeMap"),
];

/// 根据构造函数调用路径查找对应的基础类型名称
pub(crate) fn lookup_constructor_type(constructor_path: &str) -> Option<&'static str> {
    KNOWN_CONSTRUCTORS
        .iter()
        .find(|(cpath, _)| *cpath == constructor_path)
        .map(|(_, btype)| *btype)
}

/// Strip generic parameters from a path for use as resolved_symbol_id.
/// "std::collections::HashMap::<&str, i32>::new" → "std::collections::HashMap::new"
/// Splits by "::", removes segments that are entirely generic args (start with `<`),
/// and strips generic suffix from segments like "HashMap<K,V>".
pub(crate) fn strip_generics(path: &str) -> String {
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
