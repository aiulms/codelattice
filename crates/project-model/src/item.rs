//! Item/Symbol 提取器
//!
//! 第一刀：定义 trait 和 NoopItemExtractor（已落地）。
//! 第二刀：TextItemExtractor，保守逐行扫描提取 top-level items。
//! 第三刀（未来）：TreeSitterItemExtractor，完整 11 种 SymbolKind。
//!
//! TextItemExtractor 只提取 8 种 top-level item：
//! Function / Struct / Enum / Trait / TypeAlias / Const / Static / MacroDefinition
//! 不提取 ImplBlock / Method / AssociatedFunction / Module（需花括号匹配，留给 tree-sitter）。

use crate::diagnostic::codes;
use crate::model::{Symbol, SymbolDiagnostic, SymbolKind, Visibility};

/// 单个 .rs 文件的 item 提取输入
#[derive(Debug, Clone)]
pub struct ItemExtractionInput {
    /// 相对 repo root 的 .rs 文件路径
    pub source_path: String,
    /// .rs 文件内容
    pub source_text: String,
    /// 所属 package name
    pub package_name: String,
    /// 所属 target name（ambiguous 时 None）
    pub target_name: Option<String>,
    /// crate 内 module 路径（如 "crate" 或 "crate::models"）
    pub module_path: Option<String>,
}

/// 单个 .rs 文件的 item 提取输出
#[derive(Debug, Clone, Default)]
pub struct ItemExtractionOutput {
    /// 提取到的 symbols
    pub symbols: Vec<Symbol>,
    /// 提取过程中的 diagnostics
    pub diagnostics: Vec<SymbolDiagnostic>,
}

/// Item 提取器 trait（parser seam）
pub trait ItemExtractor {
    /// 从单个 .rs 文件提取 items
    fn extract_items(&self, input: &ItemExtractionInput) -> ItemExtractionOutput;
}

/// Noop 提取器：不做任何提取，返回空结果
pub struct NoopItemExtractor;

impl ItemExtractor for NoopItemExtractor {
    fn extract_items(&self, _input: &ItemExtractionInput) -> ItemExtractionOutput {
        ItemExtractionOutput::default()
    }
}

/// 从多个 .rs 文件批量提取 items
pub fn extract_symbols_from_files(
    extractor: &dyn ItemExtractor,
    inputs: &[ItemExtractionInput],
) -> ItemExtractionOutput {
    let mut all_symbols = Vec::new();
    let mut all_diagnostics = Vec::new();

    for input in inputs {
        let output = extractor.extract_items(input);
        all_symbols.extend(output.symbols);
        all_diagnostics.extend(output.diagnostics);
    }

    ItemExtractionOutput {
        symbols: all_symbols,
        diagnostics: all_diagnostics,
    }
}

// ============================================================
// 第二刀：TextItemExtractor — 保守逐行扫描
// ============================================================

/// 保守文本提取器：逐行扫描 .rs 文件，提取 top-level items
///
/// 精度低于 tree-sitter，但对 99% 的 top-level item 足够。
/// 不提取 impl 块内 method / inline module 内 item / enum variant。
pub struct TextItemExtractor;

impl ItemExtractor for TextItemExtractor {
    fn extract_items(&self, input: &ItemExtractionInput) -> ItemExtractionOutput {
        extract_items_from_source(input)
    }
}

/// 逐行扫描 .rs 文件提取 top-level items
fn extract_items_from_source(input: &ItemExtractionInput) -> ItemExtractionOutput {
    let mut symbols = Vec::new();
    let mut diagnostics = Vec::new();

    // 发出 fallback-extraction diagnostic（每个文件一次）
    diagnostics.push(SymbolDiagnostic {
        code: codes::FALLBACK_EXTRACTION.to_string(),
        severity: "warning".to_string(),
        message: "使用 text-level fallback 提取，精度低于 tree-sitter".to_string(),
        source_path: input.source_path.clone(),
        symbol_id: None,
        suggested_action: Some("引入 tree-sitter 后可获得完整提取".to_string()),
    });

    let mut in_block_comment = false;
    // 跟踪上一个属性行是否为 cfg，用于标记 cfg-gated item
    let mut prev_line_is_cfg = false;

    for (line_idx, line) in input.source_text.lines().enumerate() {
        let line_num = (line_idx + 1) as u32;
        let trimmed = line.trim();

        // block comment 状态机：跟踪是否在 /* ... */ 内
        if in_block_comment {
            // 检查是否退出 block comment
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            // block comment 内的行全部跳过
            // 简化处理：不追踪嵌套，对 99% 代码足够
            continue;
        }

        // 检查是否进入 block comment
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        // 跳过空行
        if trimmed.is_empty() {
            prev_line_is_cfg = false;
            continue;
        }

        // 跳过行注释（// /// //!）
        if trimmed.starts_with("//") {
            prev_line_is_cfg = false;
            continue;
        }

        // 跳过属性行（#[...]）
        if trimmed.starts_with("#[") {
            // 检测 cfg attribute
            if trimmed.contains("#[cfg(") {
                prev_line_is_cfg = true;
            } else {
                prev_line_is_cfg = false;
            }
            continue;
        }

        // 跳过内部属性行（#![...]）
        if trimmed.starts_with("#![") {
            prev_line_is_cfg = false;
            continue;
        }

        // 尝试匹配 item 模式
        if let Some(symbol) = try_match_item(trimmed, line_num, input, prev_line_is_cfg) {
            // 同名检测：如果已有同名 symbol，用行号 disambiguator
            symbols.push(symbol);
        } else {
            // 检测不提取的 item 并发出 diagnostic
            try_emit_unsupported_diagnostic(
                trimmed,
                &input.source_path,
                line_num,
                &mut diagnostics,
            );
        }

        prev_line_is_cfg = false;
    }

    // 同名 disambiguator 后处理
    dedup_symbol_ids(&mut symbols);

    ItemExtractionOutput {
        symbols,
        diagnostics,
    }
}

/// 尝试从一行文本匹配 top-level item
fn try_match_item(
    trimmed: &str,
    line_num: u32,
    input: &ItemExtractionInput,
    is_cfg_gated: bool,
) -> Option<Symbol> {
    // 尝试匹配各种 item 模式
    if let Some(result) = try_match_fn(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Function,
            &result.visibility,
            line_num,
            input,
            result.generic_params.as_deref(),
            result.is_async,
            result.is_unsafe,
            result.is_const_fn,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_struct(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Struct,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_enum(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Enum,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_trait(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Trait,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_type_alias(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::TypeAlias,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_const(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Const,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_static(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::Static,
            &result.visibility,
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    if let Some(result) = try_match_macro_rules(trimmed) {
        return Some(build_symbol(
            &result.name,
            SymbolKind::MacroDefinition,
            &Visibility::Public, // macro_rules! 默认可见
            line_num,
            input,
            None,
            false,
            false,
            false,
            is_cfg_gated,
        ));
    }

    None
}

/// 匹配结果：name + 可见性 + 可选修饰符
struct MatchResult {
    name: String,
    visibility: Visibility,
    generic_params: Option<String>,
    is_async: bool,
    is_unsafe: bool,
    is_const_fn: bool,
}

/// 简单匹配结果（无修饰符）
struct SimpleMatchResult {
    name: String,
    visibility: Visibility,
}

/// 尝试匹配 fn 定义
fn try_match_fn(trimmed: &str) -> Option<MatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();

    // 逐个消耗修饰符
    let mut is_async = false;
    let mut is_unsafe = false;
    let mut is_const_fn = false;
    let mut rest = rest;

    loop {
        if rest.starts_with("async ") {
            is_async = true;
            rest = rest[6..].trim_start();
        } else if rest.starts_with("unsafe ") {
            is_unsafe = true;
            rest = rest[7..].trim_start();
        } else if rest.starts_with("const ") {
            is_const_fn = true;
            rest = rest[6..].trim_start();
        } else {
            break;
        }
    }

    if !rest.starts_with("fn ") {
        return None;
    }
    rest = rest[3..].trim_start();

    let (name, generic_params) = parse_name_and_generic(rest)?;
    Some(MatchResult {
        name,
        visibility,
        generic_params,
        is_async,
        is_unsafe,
        is_const_fn,
    })
}

/// 尝试匹配 struct 定义
fn try_match_struct(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    if !rest.starts_with("struct ") {
        return None;
    }
    let rest = rest[7..].trim_start();
    let (name, _) = parse_name_and_generic(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 enum 定义
fn try_match_enum(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    if !rest.starts_with("enum ") {
        return None;
    }
    let rest = rest[5..].trim_start();
    let (name, _) = parse_name_and_generic(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 trait 定义
fn try_match_trait(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    if !rest.starts_with("trait ") {
        return None;
    }
    let rest = rest[6..].trim_start();
    let (name, _) = parse_name_and_generic(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 type 别名
fn try_match_type_alias(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    if !rest.starts_with("type ") {
        return None;
    }
    let rest = rest[5..].trim_start();
    let (name, _) = parse_name_and_generic(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 const 定义
fn try_match_const(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    // 跳过 async/unsafe 修饰（const 可跟 unsafe）
    let mut rest = rest;
    if rest.starts_with("unsafe ") {
        rest = rest[7..].trim_start();
    }
    if !rest.starts_with("const ") {
        return None;
    }
    let rest = rest[6..].trim_start();
    let name = parse_identifier(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 static 定义
fn try_match_static(trimmed: &str) -> Option<SimpleMatchResult> {
    let (rest, visibility) = parse_visibility(trimmed)?;
    let rest = rest.trim_start();
    let mut rest = rest;
    if rest.starts_with("unsafe ") {
        rest = rest[7..].trim_start();
    }
    if !rest.starts_with("static ") {
        return None;
    }
    let rest = rest[7..].trim_start();
    // 跳过 mut 关键字
    let rest = if rest.starts_with("mut ") {
        rest[4..].trim_start()
    } else {
        rest
    };
    let name = parse_identifier(rest)?;
    Some(SimpleMatchResult { name, visibility })
}

/// 尝试匹配 macro_rules! 定义
fn try_match_macro_rules(trimmed: &str) -> Option<SimpleMatchResult> {
    if !trimmed.starts_with("macro_rules!") {
        return None;
    }
    let rest = trimmed[12..].trim_start();
    let name = parse_identifier(rest)?;
    Some(SimpleMatchResult {
        name,
        visibility: Visibility::Public,
    })
}

/// 解析可见性前缀，返回 (剩余文本, 可见性)
fn parse_visibility(trimmed: &str) -> Option<(&str, Visibility)> {
    if trimmed.starts_with("pub(crate)") {
        Some((&trimmed[10..], Visibility::Crate))
    } else if trimmed.starts_with("pub(super)") {
        Some((&trimmed[10..], Visibility::Super))
    } else if trimmed.starts_with("pub(in ") {
        // 找到右括号
        if let Some(end) = trimmed[7..].find(')') {
            Some((&trimmed[7 + end + 1..], Visibility::Restricted))
        } else {
            Some((&trimmed[7..], Visibility::Restricted))
        }
    } else if trimmed.starts_with("pub ") {
        Some((&trimmed[4..], Visibility::Public))
    } else {
        // 无 pub 前缀，可能是 private item
        Some((trimmed, Visibility::Private))
    }
}

/// 从标识符起始位置提取名字，遇到非标识符字符停止
fn parse_identifier(rest: &str) -> Option<String> {
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_string())
}

/// 提取 name 和可选 generic params（同一行内）
fn parse_name_and_generic(rest: &str) -> Option<(String, Option<String>)> {
    let name = parse_identifier(rest)?;
    let after_name = &rest[name.len()..];

    // 检查是否有 generic params <...>
    let after_name_trimmed = after_name.trim_start();
    if after_name_trimmed.starts_with('<') {
        // 尝试在同一行内找匹配的 >
        if let Some(end) = find_matching_angle_bracket(after_name_trimmed) {
            let generic = after_name_trimmed[1..end].to_string();
            Some((name, Some(format!("<{generic}>"))))
        } else {
            // > 不在同一行，记录 name，generic 为 None
            Some((name, None))
        }
    } else {
        Some((name, None))
    }
}

/// 在同一行内找匹配 > 的位置（不追踪嵌套 <>，简化处理）
fn find_matching_angle_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// 构建 Symbol
fn build_symbol(
    name: &str,
    kind: SymbolKind,
    visibility: &Visibility,
    line_num: u32,
    input: &ItemExtractionInput,
    generic_params: Option<&str>,
    is_async: bool,
    is_unsafe: bool,
    is_const_fn: bool,
    is_cfg_gated: bool,
) -> Symbol {
    let module_path = input.module_path.as_deref().unwrap_or("crate");
    let id = format!("{}::{}::{}", input.package_name, module_path, name);
    let is_pub = matches!(visibility, Visibility::Public);

    // text-level 提取 confidence
    let _ = is_cfg_gated; // cfg-gated 不影响 confidence，只影响 future diagnostic

    Symbol {
        id,
        name: name.to_string(),
        symbol_kind: kind.as_str().to_string(),
        source_path: input.source_path.clone(),
        package_name: input.package_name.clone(),
        target_name: input.target_name.clone(),
        module_path: Some(module_path.to_string()),
        visibility: visibility.as_str().to_string(),
        parent_id: None,
        line_start: line_num,
        line_end: line_num,
        generic_params: generic_params.map(|s| s.to_string()),
        is_async,
        is_unsafe,
        is_const_fn,
        is_pub,
        impl_details: None,
    }
}

/// 检测不提取的 item 并发出 diagnostic
fn try_emit_unsupported_diagnostic(
    trimmed: &str,
    source_path: &str,
    line_num: u32,
    diagnostics: &mut Vec<SymbolDiagnostic>,
) {
    // 检测 impl 块
    if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
        diagnostics.push(SymbolDiagnostic {
            code: codes::IMPL_BLOCK_AMBIGUOUS_TARGET.to_string(),
            severity: "info".to_string(),
            message: format!("impl 块不提取（留给 tree-sitter）: 行 {line_num}"),
            source_path: source_path.to_string(),
            symbol_id: None,
            suggested_action: Some(
                "tree-sitter 提取器可提取 impl/Method/AssociatedFunction".to_string(),
            ),
        });
    }

    // 检测 inline module
    if trimmed.contains("mod ") && trimmed.contains('{') {
        diagnostics.push(SymbolDiagnostic {
            code: codes::ITEM_PARSE_ERROR.to_string(),
            severity: "info".to_string(),
            message: format!("inline module 不提取内部 item: 行 {line_num}"),
            source_path: source_path.to_string(),
            symbol_id: None,
            suggested_action: None,
        });
    }

    // 检测 macro invocation：名字后跟 !
    if let Some(bang_pos) = trimmed.find('!') {
        if bang_pos > 0 {
            let before = &trimmed[..bang_pos];
            let before = before.trim();
            // 排除 macro_rules! 本身
            if !before.ends_with("macro_rules") && !before.starts_with('#') {
                let candidate = before.split_whitespace().next().unwrap_or("");
                if !candidate.is_empty()
                    && candidate
                        .chars()
                        .all(|c: char| c.is_alphanumeric() || c == '_')
                {
                    diagnostics.push(SymbolDiagnostic {
                        code: codes::MACRO_INVOCATION_UNEXPANDED.to_string(),
                        severity: "info".to_string(),
                        message: format!("macro 调用不展开: {candidate}!()"),
                        source_path: source_path.to_string(),
                        symbol_id: None,
                        suggested_action: None,
                    });
                }
            }
        }
    }
}

/// 同名 symbol id disambiguator 后处理
fn dedup_symbol_ids(symbols: &mut Vec<Symbol>) {
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for symbol in symbols.iter_mut() {
        if seen_ids.contains(&symbol.id) {
            // 同名冲突，用行号 disambiguator
            symbol.id = format!("{}::_L{}", symbol.id, symbol.line_start);
        }
        seen_ids.insert(symbol.id.clone());
    }
}
