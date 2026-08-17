//! Text-level call fallback — 无 AST 时的行级调用提取与解析。
//!
//! calls.rs 第三刀拆分（2026-08-17）：text fallback 从 calls.rs 整体迁出，
//! 行为等价。与 AST 路径共享 calls.rs 的 resolve_free_function /
//! resolve_associated_function（pub(crate)），与 stdlib_tables 共享
//! receiver-type / trait-method 查表，不复制逻辑。
//!
//! 已知限制（继承 stop-line）：
//! - 不做 type inference，method dispatch 不验证 receiver type
//! - 只扫描行级文本，不处理跨行调用与 macro 展开

use std::collections::HashSet;

use crate::calls::resolve_associated_function;
use crate::calls::resolve_free_function;
use crate::calls_index::*;
use crate::model::*;
use crate::stdlib_tables::*;

// ============================================================
// text-level fallback（入口）
// ============================================================

pub(crate) fn extract_calls_text_fallback(
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

    resolve_call_site_text(
        &mut call_site,
        symbol_index,
        import_bindings,
        source_text,
        None,
        None,
    );

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
            // crate:: 路径需区分 QualifiedPath（自由函数）和 AssociatedFunction（类型方法）
            // 如 crate::module::Type::method() 应有 >=4 段且倒数第二段首字母大写
            if segments.len() >= 4 {
                let second_last = segments[segments.len() - 2];
                if second_last
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    // Enum::Variant 模式：最后一段也大写时是 enum variant constructor
                    let is_enum_variant = name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);
                    if is_enum_variant {
                        CallKind::FreeFunction
                    } else {
                        CallKind::AssociatedFunction
                    }
                } else {
                    CallKind::QualifiedPath
                }
            } else {
                CallKind::QualifiedPath
            }
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
                // 区分 Type::method()（AssociatedFunction）和 Enum::Variant()（FreeFunction）
                let is_enum_variant = name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                if is_enum_variant {
                    CallKind::FreeFunction
                } else {
                    CallKind::AssociatedFunction
                }
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
    func_start_hint: Option<usize>,
    enclosing_impl_target: Option<&str>,
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
                    // Phase 2: receiver-type-aware resolution（优先于 stdlib trait fallback）
                    // 当 receiver type 可静态确定时，用更高 confidence (0.65)，
                    // 优于仅按 method name 匹配 trait 的 fallback (0.55)。
                    // 从 raw_text 提取 receiver variable name（e.g., "x.push(1)" → "x"）
                    // 扫描 same-function let 绑定/参数类型注解，查 STDLIB_TYPE_METHODS 表
                    if is_known_receiver_type_method(&call.callee_name) {
                        if let Some(dot_pos) = call.raw_text.find('.') {
                            let receiver = &call.raw_text[..dot_pos];
                            // 只处理简单 identifier receiver（不是 literal 或 path）
                            if receiver.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                if let Some(base_type) = scan_variable_type_annotation(
                                    source_text,
                                    call.span.byte_start,
                                    receiver,
                                    func_start_hint,
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
                    }
                    // Phase 1 extended: stdlib trait method fallback
                    // receiver type 无法静态确定时，按 method name 匹配唯一定义的 stdlib trait
                    // e.g., to_string() → std::string::ToString::to_string
                    // confidence 0.55：低于 receiver-type (0.65)，因为 receiver type 未验证
                    if let Some(trait_path) = lookup_stdlib_trait_method(&call.callee_name) {
                        call.resolved_symbol_id = Some(trait_path.to_string());
                        call.confidence = 0.55;
                        call.reason = CallResolutionReason::CallStdlibTraitMethodResolved
                            .as_str()
                            .to_string();
                        return;
                    }
                    call.reason = CallResolutionReason::CallTargetUnresolved
                        .as_str()
                        .to_string();
                }
                _multiple => {
                    // Phase 2d (B): self 方法解析。
                    // 当 receiver 是 self（self.foo() / &self.foo() / &mut self.foo()）
                    // 且已知 enclosing fn 的 impl_target 时，在多个同名 method 里
                    // 按 impl_target 过滤，若唯一匹配则解析。
                    // 这是静态可解的（impl 块上下文），不需 type inference。
                    if let Some(dot_pos) = call.raw_text.find('.') {
                        let receiver = call.raw_text[..dot_pos]
                            .trim_start_matches('&')
                            .trim_start_matches("mut ")
                            .trim();
                        if receiver == "self" {
                            if let Some(impl_target) = enclosing_impl_target {
                                let candidates: Vec<_> = methods
                                    .iter()
                                    .filter(|m| {
                                        m.impl_details
                                            .as_ref()
                                            .map(|d| d.impl_target == impl_target)
                                            .unwrap_or(false)
                                    })
                                    .collect();
                                if candidates.len() == 1 {
                                    let single = candidates[0];
                                    call.resolved_symbol_id = Some(single.id.clone());
                                    call.resolved_symbol_kind = Some(single.symbol_kind.clone());
                                    call.confidence = 0.70;
                                    call.reason = CallResolutionReason::CallSelfMethodResolved
                                        .as_str()
                                        .to_string();
                                    return;
                                }
                            }
                        }
                    }
                    // Phase 2c: 多个 crate 内同名 method 时也尝试 stdlib trait fallback。
                    // 常见 method（clone/len/push/to_string 等）在 crate 内有多个 impl，
                    // 但 method name 对应 known-unique stdlib trait 时仍可安全解析。
                    // 不验证 receiver type，confidence 0.55 保持。
                    if let Some(trait_path) = lookup_stdlib_trait_method(&call.callee_name) {
                        call.resolved_symbol_id = Some(trait_path.to_string());
                        call.confidence = 0.55;
                        call.reason = CallResolutionReason::CallStdlibTraitMethodResolved
                            .as_str()
                            .to_string();
                        return;
                    }
                    // Phase 2c: receiver type scan（与 [] 分支对称），
                    // 从 raw_text 提取 receiver variable name，扫描类型注解查 STDLIB_TYPE_METHODS 表
                    if is_known_receiver_type_method(&call.callee_name) {
                        if let Some(dot_pos) = call.raw_text.find('.') {
                            let receiver = &call.raw_text[..dot_pos];
                            if receiver.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                if let Some(base_type) = scan_variable_type_annotation(
                                    source_text,
                                    call.span.byte_start,
                                    receiver,
                                    func_start_hint,
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
                    }
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
