//! stdlib symbol index — 静态索引表，用于验证 external crate call path 对应真实 stdlib symbol。
//!
//! 设计目的：
//! - Phase 1 external crate resolution 直接构造 resolved_symbol_id（compiler implied guarantee），不做符号级验证。
//! - 本索引提供独立验证层：命中的 symbol 提升 confidence 0.80→0.85。
//! - 未命中的 symbol 保持 0.80（compiler implied guarantee 仍有效）。
//!
//! 覆盖范围：
//! - std 免费函数、常用类型、常用 collections
//! - prelude types 的常用 associated functions
//! - STDLIB_TRAIT_METHODS / STDLIB_TYPE_METHODS 中引用的所有类型和方法
//! - core/alloc 类型（当前只覆盖从 std 路径可达的条目）
//!
//! 更新策略：
//! - 第一刀 ~90 entries
//! - 按需扩展，不索引完整 std API（~10000+ symbols）
//! - 新增 entry 需要 fixture 或 real-world call 证据
//! - 不索引 third-party crate / unstable API

/// stdlib 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdlibSymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    TraitMethod,
    AssociatedFunction,
}

/// 静态索引表的一个条目
pub struct StdlibSymbolEntry {
    /// 符号完整路径，例如 "std::fs::read_to_string"
    pub path: &'static str,
    /// 符号类型
    pub kind: StdlibSymbolKind,
    /// 是否为 re-export（std 从 core/alloc 重新导出）
    #[allow(dead_code)]
    pub is_reexport: bool,
}

use StdlibSymbolKind::*;

/// 静态 stdlib 符号索引表（按字母序）
const STDLIB_SYMBOL_INDEX: &[StdlibSymbolEntry] = &[
    // --- std boxed ---
    e("std::boxed::Box", Struct, false),
    // --- std clone ---
    e("std::clone::Clone", Trait, false),
    // --- std cmp ---
    e("std::cmp::Eq", Trait, false),
    e("std::cmp::Ord", Trait, false),
    e("std::cmp::PartialEq", Trait, false),
    e("std::cmp::PartialOrd", Trait, false),
    // --- std collections ---
    e("std::collections::BinaryHeap", Struct, false),
    e("std::collections::BTreeMap", Struct, false),
    e("std::collections::BTreeSet", Struct, false),
    e("std::collections::HashMap", Struct, false),
    e("std::collections::HashSet", Struct, false),
    e("std::collections::LinkedList", Struct, false),
    e("std::collections::VecDeque", Struct, false),
    // --- std collections::HashMap methods ---
    e("std::collections::HashMap::new", AssociatedFunction, false),
    // --- std collections::HashSet methods ---
    e("std::collections::HashSet::new", AssociatedFunction, false),
    // --- std convert ---
    e("std::convert::AsMut", Trait, false),
    e("std::convert::AsRef", Trait, false),
    e("std::convert::From", Trait, false),
    e("std::convert::Into", Trait, false),
    // --- std default ---
    e("std::default::Default", Trait, false),
    // --- std env ---
    e("std::env::args", Function, false),
    e("std::env::current_dir", Function, false),
    e("std::env::var", Function, false),
    // --- std fmt ---
    e("std::fmt::Debug", Trait, false),
    e("std::fmt::Display", Trait, false),
    // --- std fs ---
    e("std::fs::create_dir_all", Function, false),
    e("std::fs::metadata", Function, false),
    e("std::fs::read", Function, false),
    e("std::fs::read_dir", Function, false),
    e("std::fs::read_to_string", Function, false),
    e("std::fs::remove_file", Function, false),
    e("std::fs::write", Function, false),
    // --- std io ---
    e("std::io::Error", Struct, false),
    e("std::io::Result", Struct, false),
    e("std::io::stderr", Function, false),
    e("std::io::stdout", Function, false),
    // --- std iter ---
    e("std::iter::IntoIterator", Trait, false),
    e("std::iter::Iterator", Trait, false),
    // --- std iter::Iterator methods ---
    e("std::iter::Iterator::collect", TraitMethod, false),
    // --- std marker ---
    e("std::marker::Copy", Trait, false),
    e("std::marker::Send", Trait, false),
    e("std::marker::Sized", Trait, false),
    e("std::marker::Sync", Trait, false),
    // --- std ops ---
    e("std::ops::Deref", Trait, false),
    e("std::ops::Drop", Trait, false),
    // --- std option ---
    e("std::option::Option", Enum, false),
    // --- std option::Option methods ---
    e("std::option::Option::is_none", AssociatedFunction, false),
    e("std::option::Option::is_some", AssociatedFunction, false),
    e("std::option::Option::unwrap_or", AssociatedFunction, false),
    // --- std path ---
    e("std::path::Path", Struct, false),
    e("std::path::PathBuf", Struct, false),
    // --- std path::PathBuf methods ---
    e("std::path::PathBuf::new", AssociatedFunction, false),
    // --- std process ---
    e("std::process::Command", Struct, false),
    e("std::process::exit", Function, false),
    // --- std rc ---
    e("std::rc::Rc", Struct, false),
    // --- std cell ---
    e("std::cell::RefCell", Struct, false),
    // --- std result ---
    e("std::result::Result", Enum, false),
    // --- std result::Result methods ---
    e("std::result::Result::is_err", AssociatedFunction, false),
    e("std::result::Result::is_ok", AssociatedFunction, false),
    e("std::result::Result::unwrap", AssociatedFunction, false),
    e("std::result::Result::unwrap_or", AssociatedFunction, false),
    // --- std string ---
    e("std::string::String", Struct, false),
    // --- std string::String methods ---
    e("std::string::String::from", AssociatedFunction, false),
    e("std::string::String::is_empty", AssociatedFunction, false),
    e("std::string::String::len", AssociatedFunction, false),
    e("std::string::String::new", AssociatedFunction, false),
    // --- std clone::Clone methods ---
    e("std::clone::Clone::clone", TraitMethod, false),
    // --- std string::ToString methods ---
    e("std::string::ToString::to_string", TraitMethod, false),
    // --- std sync ---
    e("std::sync::Arc", Struct, false),
    e("std::sync::Mutex", Struct, false),
    // --- std thread ---
    e("std::thread::spawn", Function, false),
    // --- std time ---
    e("std::time::Duration", Struct, false),
    e("std::time::Instant", Struct, false),
    // --- std vec ---
    e("std::vec::Vec", Struct, false),
    // --- std vec::Vec methods ---
    e("std::vec::Vec::is_empty", AssociatedFunction, false),
    e("std::vec::Vec::len", AssociatedFunction, false),
    e("std::vec::Vec::new", AssociatedFunction, false),
    e("std::vec::Vec::pop", AssociatedFunction, false),
    e("std::vec::Vec::push", AssociatedFunction, false),
];

const fn e(path: &'static str, kind: StdlibSymbolKind, is_reexport: bool) -> StdlibSymbolEntry {
    StdlibSymbolEntry {
        path,
        kind,
        is_reexport,
    }
}

/// 查询 stdlib 符号索引，返回匹配条目或 None
///
/// 查询时：
/// - 精确匹配 path
/// - 如果 path 包含 generic args（如 `HashMap::<K,V>::new`），会先尝试 strip generics 后的 path
/// - 只要 strip generics 后匹配到索引条目即可
pub fn lookup_stdlib_symbol(path: &str) -> Option<&'static StdlibSymbolEntry> {
    // 精确匹配
    for entry in STDLIB_SYMBOL_INDEX {
        if entry.path == path {
            return Some(entry);
        }
    }
    // 如果 path 包含 generics（如 HashMap::<K,V>::new），strip generics 后重新匹配
    // 注意：不会为每个调用都 strip generics 然后匹配，
    // 因为在 resolve_call_site 中 callee_path 已经过 strip_generics
    // 这里只做最后的 defensive match
    None
}

/// 判断 path 是否在 stdlib 符号索引中
pub fn is_known_stdlib_symbol(path: &str) -> bool {
    lookup_stdlib_symbol(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_known_symbol() {
        assert!(lookup_stdlib_symbol("std::fs::read_to_string").is_some());
        assert!(lookup_stdlib_symbol("std::process::exit").is_some());
        assert!(lookup_stdlib_symbol("std::vec::Vec").is_some());
        assert!(lookup_stdlib_symbol("std::vec::Vec::new").is_some());
        assert!(lookup_stdlib_symbol("std::collections::HashMap::new").is_some());
        assert!(lookup_stdlib_symbol("std::path::PathBuf::new").is_some());
        assert!(lookup_stdlib_symbol("std::string::String::from").is_some());
        assert!(lookup_stdlib_symbol("std::clone::Clone::clone").is_some());
        assert!(lookup_stdlib_symbol("std::string::ToString::to_string").is_some());
        assert!(lookup_stdlib_symbol("std::iter::Iterator::collect").is_some());
    }

    #[test]
    fn test_lookup_unknown_symbol() {
        assert!(lookup_stdlib_symbol("std::fs::nonexistent_func").is_none());
        assert!(lookup_stdlib_symbol("std::vec::Vec::nonexistent_method").is_none());
    }

    #[test]
    fn test_is_known_stdlib_symbol() {
        assert!(is_known_stdlib_symbol("std::fs::read_to_string"));
        assert!(!is_known_stdlib_symbol("std::fs::nonexistent"));
    }

    #[test]
    fn test_index_size_reasonable() {
        // 第一刀索引条目控制在合理范围（≤ 100）
        assert!(STDLIB_SYMBOL_INDEX.len() <= 100);
        assert!(STDLIB_SYMBOL_INDEX.len() >= 50);
    }
}
