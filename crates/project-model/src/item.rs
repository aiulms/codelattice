//! Item/Symbol 提取器
//!
//! 第一刀：定义 trait 和 NoopItemExtractor（已落地）。
//! 第二刀：TextItemExtractor，保守逐行扫描提取 top-level items。
//! 第三刀：TreeSitterItemExtractor，基于 tree-sitter CST 提取完整 12 种 SymbolKind。
//!
//! TextItemExtractor 只提取 8 种 top-level item：
//! Function / Struct / Enum / Trait / TypeAlias / Const / Static / MacroDefinition
//! 不提取 ImplBlock / Method / AssociatedFunction / Module（需花括号匹配，留给 tree-sitter）。
//!
//! TreeSitterItemExtractor 额外提取：
//! Module / ImplBlock / Method / AssociatedFunction + 更精确的 span 和 identity。

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
        type_annotations: vec![],
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

// ============================================================
// 第三刀：TreeSitterItemExtractor — 基于 tree-sitter CST
// ============================================================

#[cfg(feature = "tree-sitter-extraction")]
mod tree_sitter_impl {
    use crate::diagnostic::codes;
    use crate::item::{ItemExtractionInput, ItemExtractionOutput, ItemExtractor};
    use crate::model::{
        ImplBlockDetail, Symbol, SymbolDiagnostic, SymbolKind, TypeAnnotation, Visibility,
    };

    /// tree-sitter 提取器：基于 CST 提取 12 种 SymbolKind
    pub struct TreeSitterItemExtractor;

    impl ItemExtractor for TreeSitterItemExtractor {
        fn extract_items(&self, input: &ItemExtractionInput) -> ItemExtractionOutput {
            extract_with_tree_sitter(input)
        }
    }

    /// 尝试初始化 tree-sitter Rust grammar
    pub fn try_init_parser() -> Option<tree_sitter::Parser> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        if parser.set_language(&language.into()).is_ok() {
            Some(parser)
        } else {
            None
        }
    }

    /// 使用 tree-sitter 提取 items
    fn extract_with_tree_sitter(input: &ItemExtractionInput) -> ItemExtractionOutput {
        let mut symbols = Vec::new();
        let mut diagnostics = Vec::new();

        let mut parser = match try_init_parser() {
            Some(p) => p,
            None => {
                // grammar 初始化失败，返回空结果 + diagnostic
                diagnostics.push(SymbolDiagnostic {
                    code: codes::FALLBACK_TO_TEXT_EXTRACTOR.to_string(),
                    severity: "warning".to_string(),
                    message: "tree-sitter grammar 初始化失败".to_string(),
                    source_path: input.source_path.clone(),
                    symbol_id: None,
                    suggested_action: Some("将回退到 TextItemExtractor".to_string()),
                });
                return ItemExtractionOutput {
                    symbols,
                    diagnostics,
                };
            }
        };

        let tree = match parser.parse(&input.source_text, None) {
            Some(t) => t,
            None => {
                // 解析完全失败
                diagnostics.push(SymbolDiagnostic {
                    code: codes::TREE_SITTER_PARSE_ERROR.to_string(),
                    severity: "warning".to_string(),
                    message: "tree-sitter 解析完全失败".to_string(),
                    source_path: input.source_path.clone(),
                    symbol_id: None,
                    suggested_action: None,
                });
                return ItemExtractionOutput {
                    symbols,
                    diagnostics,
                };
            }
        };

        let root_node = tree.root_node();

        // 检测 parse error 节点
        let has_error = root_node.has_error();
        if has_error {
            diagnostics.push(SymbolDiagnostic {
                code: codes::TREE_SITTER_PARSE_ERROR.to_string(),
                severity: "warning".to_string(),
                message: "文件含语法错误，部分提取可能不完整".to_string(),
                source_path: input.source_path.clone(),
                symbol_id: None,
                suggested_action: None,
            });
        }

        // 提取当前文件的行数映射（用于 byte→line 转换）
        let source_bytes = input.source_text.as_bytes();

        // 递归遍历 CST
        let module_path = input.module_path.as_deref().unwrap_or("crate");
        walk_node(
            &root_node,
            source_bytes,
            input,
            module_path,
            None, // 顶层无 parentId
            None, // 顶层无 impl 上下文
            &mut symbols,
            &mut diagnostics,
        );

        // 如果有 error 且提取到部分 symbols，发 item-extraction-partial
        if has_error && !symbols.is_empty() {
            diagnostics.push(SymbolDiagnostic {
                code: codes::ITEM_EXTRACTION_PARTIAL.to_string(),
                severity: "info".to_string(),
                message: format!("部分提取 {} 个 symbol", symbols.len()),
                source_path: input.source_path.clone(),
                symbol_id: None,
                suggested_action: None,
            });
        }

        // 同名 disambiguator
        crate::item::dedup_symbol_ids_pub(&mut symbols);

        ItemExtractionOutput {
            symbols,
            diagnostics,
        }
    }

    /// 递归遍历 CST 节点
    ///
    /// `impl_context` 包含 (impl_target, impl_id)，当非 None 时表示当前在 impl 块内
    fn walk_node(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
        parent_id: Option<&str>,
        impl_context: Option<(&str, &str)>, // (impl_target, impl_id)
        symbols: &mut Vec<Symbol>,
        diagnostics: &mut Vec<SymbolDiagnostic>,
    ) {
        // 跳过 ERROR 和 MISSING 节点
        if node.is_error() || node.is_missing() {
            return;
        }

        let kind = node.kind();

        // 调试：记录所有顶层节点类型（仅当 module_path == "crate" 且无 parent 时）
        // 已确认节点名：mod_item / impl_item / function_item / struct_item / enum_item / trait_item / type_item / const_item / static_item / macro_definition / macro_invocation

        match kind {
            // 顶层 item 定义
            "function_item" => {
                if let Some((impl_target, impl_id)) = impl_context {
                    // impl 块内的 function_item → Method 或 AssociatedFunction
                    if let Some(fn_sym) = extract_impl_function(
                        node,
                        source_bytes,
                        input,
                        module_path,
                        impl_target,
                        impl_id,
                    ) {
                        symbols.push(fn_sym);
                    }
                } else if let Some(sym) =
                    extract_function(node, source_bytes, input, module_path, parent_id)
                {
                    symbols.push(sym);
                }
            }
            "struct_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::Struct,
                    "name",
                ) {
                    symbols.push(sym);
                }
            }
            "enum_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::Enum,
                    "name",
                ) {
                    symbols.push(sym);
                }
            }
            "trait_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::Trait,
                    "name",
                ) {
                    symbols.push(sym);
                }
                // trait 内 associated type/const 不提取，发 diagnostic
                emit_unsupported_associated_items(node, &input.source_path, diagnostics);
            }
            "type_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::TypeAlias,
                    "name",
                ) {
                    symbols.push(sym);
                }
            }
            "const_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::Const,
                    "name",
                ) {
                    symbols.push(sym);
                }
            }
            "static_item" => {
                if let Some(sym) = extract_simple_item(
                    node,
                    source_bytes,
                    input,
                    module_path,
                    parent_id,
                    SymbolKind::Static,
                    "name",
                ) {
                    symbols.push(sym);
                }
            }
            "macro_definition" => {
                // macro_rules! name { ... }
                if let Some(sym) = extract_macro_definition(node, source_bytes, input, module_path)
                {
                    symbols.push(sym);
                }
            }
            "meta_item" if node.kind() == "meta_item" => {
                // 属性节点，不提取
            }

            // Module 声明（tree-sitter 节点名：mod_item）
            "mod_item" => {
                if let Some(sym) =
                    extract_module_symbol(node, source_bytes, input, module_path, parent_id)
                {
                    // inline module 需要更新 modulePath 和 parentId
                    let has_body = node.child_by_field_name("body").is_some();
                    if has_body {
                        let name = sym.name.clone();
                        let module_id = sym.id.clone();
                        let nested_module_path = if module_path == "crate" {
                            format!("crate::{}", name)
                        } else {
                            format!("{}::{}", module_path, name)
                        };
                        symbols.push(sym);
                        // 递归 body 内子节点，用新的 modulePath 和 parentId
                        if let Some(body_node) = node.child_by_field_name("body") {
                            let mut cursor = body_node.walk();
                            for child in body_node.children(&mut cursor) {
                                walk_node(
                                    &child,
                                    source_bytes,
                                    input,
                                    &nested_module_path,
                                    Some(&module_id),
                                    None,
                                    symbols,
                                    diagnostics,
                                );
                            }
                        }
                        return;
                    }
                    symbols.push(sym);
                }
            }

            // Impl 块
            "impl_item" => {
                if let Some(sym) = extract_impl_block_symbol(node, source_bytes, input, module_path)
                {
                    let impl_id = sym.id.clone();
                    let (impl_target, _) = parse_impl_header(node, source_bytes);
                    symbols.push(sym);
                    // 递归 body 内子节点，传入 impl 上下文
                    if let Some(body_node) = node.child_by_field_name("body") {
                        let mut cursor = body_node.walk();
                        for child in body_node.children(&mut cursor) {
                            walk_node(
                                &child,
                                source_bytes,
                                input,
                                module_path,
                                Some(&impl_id),
                                Some((impl_target.as_str(), &impl_id)),
                                symbols,
                                diagnostics,
                            );
                        }
                    }
                    return;
                }
            }

            // macro invocation（非 macro_rules! 定义）
            "macro_invocation" => {
                diagnostics.push(SymbolDiagnostic {
                    code: codes::UNSUPPORTED_MACRO_EXPANSION.to_string(),
                    severity: "info".to_string(),
                    message: "宏调用不展开".to_string(),
                    source_path: input.source_path.clone(),
                    symbol_id: None,
                    suggested_action: None,
                });
            }

            _ => {}
        }

        // 继续递归子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_node(
                &child,
                source_bytes,
                input,
                module_path,
                parent_id,
                impl_context,
                symbols,
                diagnostics,
            );
        }
    }

    /// 提取 fn 定义（含 async/unsafe/const 修饰符）
    fn extract_function(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
        parent_id: Option<&str>,
    ) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source_bytes).ok()?;

        let visibility = extract_visibility(node, source_bytes);
        let generic_params = extract_generic_params(node, source_bytes);
        let (is_async, is_unsafe, is_const_fn) = extract_fn_modifiers(node, source_bytes);

        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        // v0.3: 提取函数签名的类型注解（return type / param types）
        let type_annotations = extract_type_annotations(node, source_bytes);

        Some(build_symbol_full(
            name,
            SymbolKind::Function,
            &visibility,
            line_start,
            line_end,
            input,
            module_path,
            parent_id,
            generic_params.as_deref(),
            is_async,
            is_unsafe,
            is_const_fn,
            None, // function 无 implDetails
            type_annotations,
        ))
    }

    /// 提取简单 item（struct/enum/trait/type/const/static）
    fn extract_simple_item(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
        parent_id: Option<&str>,
        kind: SymbolKind,
        name_field: &str,
    ) -> Option<Symbol> {
        let name_node = node.child_by_field_name(name_field)?;
        let name = name_node.utf8_text(source_bytes).ok()?;

        let visibility = extract_visibility(node, source_bytes);
        let generic_params = extract_generic_params(node, source_bytes);

        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        Some(build_symbol_full(
            name,
            kind,
            &visibility,
            line_start,
            line_end,
            input,
            module_path,
            parent_id,
            generic_params.as_deref(),
            false,
            false,
            false,
            None,
            vec![],
        ))
    }

    /// 提取 macro_rules! 定义
    fn extract_macro_definition(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
    ) -> Option<Symbol> {
        // macro_definition 的 name 在第一个 identifier 子节点
        let mut cursor = node.walk();
        let name = node
            .children(&mut cursor)
            .find(|c| c.kind() == "identifier")
            .and_then(|c| c.utf8_text(source_bytes).ok())?;

        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        Some(build_symbol_full(
            name,
            SymbolKind::MacroDefinition,
            &Visibility::Public,
            line_start,
            line_end,
            input,
            module_path,
            None,
            None,
            false,
            false,
            false,
            None,
            vec![],
        ))
    }

    /// 提取 module symbol（不含递归 body，由外层 walk_node 自动处理）
    fn extract_module_symbol(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
        parent_id: Option<&str>,
    ) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source_bytes).ok()?;

        let visibility = extract_visibility(node, source_bytes);
        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        Some(build_symbol_full(
            name,
            SymbolKind::Module,
            &visibility,
            line_start,
            line_end,
            input,
            module_path,
            parent_id,
            None,
            false,
            false,
            false,
            None,
            vec![],
        ))
    }

    /// 提取 impl block symbol（不含递归 body，由外层 walk_node 自动处理）
    fn extract_impl_block_symbol(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
    ) -> Option<Symbol> {
        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        let (impl_target, trait_name) = parse_impl_header(node, source_bytes);

        let impl_details = ImplBlockDetail {
            impl_target: impl_target.clone(),
            trait_name: trait_name.clone(),
        };

        Some(build_symbol_full(
            &format!(
                "_impl_{}{}",
                impl_target,
                trait_name
                    .as_ref()
                    .map(|t| format!("_for_{}", t))
                    .unwrap_or_default()
            ),
            SymbolKind::ImplBlock,
            &Visibility::Private,
            line_start,
            line_end,
            input,
            module_path,
            None,
            None,
            false,
            false,
            false,
            Some(&impl_details),
            vec![],
        ))
    }

    /// 解析 impl 头部：返回 (impl_target, trait_name)
    fn parse_impl_header(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
    ) -> (String, Option<String>) {
        let mut trait_name = None;
        let mut impl_target = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" => {
                    let text = child
                        .utf8_text(source_bytes)
                        .unwrap_or("Unknown")
                        .to_string();
                    if impl_target.is_none() {
                        // 第一个 type_identifier 可能是 trait 或 target
                        // 如果后面有 "for" 关键字，这是 trait impl
                        impl_target = Some(text);
                    } else {
                        // 第二个 type_identifier 是 trait impl 的 target
                        trait_name = impl_target.take();
                        impl_target = Some(text);
                    }
                }
                "for" => {
                    // "impl Trait for Type" — 之前读取的是 trait，之后的是 type
                    if let Some(first) = impl_target.take() {
                        trait_name = Some(first);
                    }
                }
                "generic_type" | "scoped_type_identifier" => {
                    // 递归查找子节点中的 type_identifier
                    // 处理 impl<'a> SameFileIndex<'a> — SameFileIndex<'a> 被 tree-sitter
                    // 解析为 generic_type 节点，其子节点含 type_identifier "SameFileIndex"
                    let mut inner_cursor = child.walk();
                    for inner_child in child.children(&mut inner_cursor) {
                        if inner_child.kind() == "type_identifier" {
                            let text = inner_child
                                .utf8_text(source_bytes)
                                .unwrap_or("Unknown")
                                .to_string();
                            if impl_target.is_none() {
                                impl_target = Some(text);
                            } else {
                                trait_name = impl_target.take();
                                impl_target = Some(text);
                            }
                        }
                    }
                }
                "where_clause" => {
                    // 跳过 where clause
                }
                _ => {}
            }
        }

        let target = impl_target.unwrap_or_else(|| "Unknown".to_string());
        (target, trait_name)
    }

    /// 提取 impl 块内的 function_item，判断是 Method 还是 AssociatedFunction
    fn extract_impl_function(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
        input: &ItemExtractionInput,
        module_path: &str,
        impl_target: &str,
        impl_id: &str,
    ) -> Option<Symbol> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source_bytes).ok()?;

        let visibility = extract_visibility(node, source_bytes);
        let generic_params = extract_generic_params(node, source_bytes);
        let (is_async, is_unsafe, is_const_fn) = extract_fn_modifiers(node, source_bytes);

        let line_start = byte_to_line(source_bytes, node.start_byte());
        let line_end = byte_to_line(source_bytes, node.end_byte());

        // v0.3: 提取 impl 内函数签名的类型注解
        let type_annotations = extract_type_annotations(node, source_bytes);

        // 判断 Method vs AssociatedFunction：检查首参数是否为 self receiver
        let is_method = has_self_receiver(node);

        let kind = if is_method {
            SymbolKind::Method
        } else {
            SymbolKind::AssociatedFunction
        };

        let impl_details = ImplBlockDetail {
            impl_target: impl_target.to_string(),
            trait_name: None, // Method/AssociatedFunction 不重复存 trait_name
        };

        Some(build_symbol_full(
            name,
            kind,
            &visibility,
            line_start,
            line_end,
            input,
            module_path,
            Some(impl_id),
            generic_params.as_deref(),
            is_async,
            is_unsafe,
            is_const_fn,
            Some(&impl_details),
            type_annotations,
        ))
    }

    /// 判断函数是否有 self receiver（Method vs AssociatedFunction）
    fn has_self_receiver(node: &tree_sitter::Node) -> bool {
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                let kind = child.kind();
                if kind == "self_parameter" {
                    return true;
                }
                // 第一个参数如果是 &self / &mut self / self，tree-sitter 标记为 self_parameter
                if kind == "parameter" {
                    // 检查参数内是否有 self
                    let mut inner = child.walk();
                    for sub in child.children(&mut inner) {
                        if sub.kind() == "self_parameter" {
                            return true;
                        }
                    }
                    // 第一个 parameter 不是 self，后续也不可能是
                    return false;
                }
            }
        }
        false
    }

    /// 从节点提取 visibility
    fn extract_visibility(node: &tree_sitter::Node, source_bytes: &[u8]) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let text = child.utf8_text(source_bytes).unwrap_or("");
                if text == "pub" {
                    return Visibility::Public;
                } else if text.starts_with("pub(crate)") {
                    return Visibility::Crate;
                } else if text.starts_with("pub(super)") {
                    return Visibility::Super;
                } else if text.starts_with("pub(in") {
                    return Visibility::Restricted;
                } else if text.starts_with("pub") {
                    // pub(in path::to::module) 等更复杂形式
                    return Visibility::Restricted;
                }
            }
        }
        Visibility::Private
    }

    /// 提取 generic params（如 <T>）
    fn extract_generic_params(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_arguments" || child.kind() == "constrained_type_parameter" {
                let text = child.utf8_text(source_bytes).ok()?;
                return Some(text.to_string());
            }
        }
        None
    }

    /// 提取 fn 修饰符：async / unsafe / const
    fn extract_fn_modifiers(node: &tree_sitter::Node, source_bytes: &[u8]) -> (bool, bool, bool) {
        let mut is_async = false;
        let mut is_unsafe = false;
        let mut is_const_fn = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let text = child.utf8_text(source_bytes).unwrap_or("");
            if text == "async" {
                is_async = true;
            } else if text == "unsafe" {
                is_unsafe = true;
            } else if text == "const" && child.kind() != "const_item" {
                is_const_fn = true;
            }
        }

        (is_async, is_unsafe, is_const_fn)
    }

    /// v0.3: 从 function_item 提取类型注解（return type / param types）
    ///
    /// 只提取显式类型注解，不做类型推断。解析出的类型名经过
    /// strip_generics + strip_reference + take_last_segment 处理。
    ///
    /// 不上函数体（不 walk block / let_declaration），只处理函数签名级别。
    fn extract_type_annotations(
        node: &tree_sitter::Node,
        source_bytes: &[u8],
    ) -> Vec<TypeAnnotation> {
        let mut annotations = Vec::new();

        // 提取 return type: `fn foo() -> ReturnType`
        if let Some(ret_node) = node.child_by_field_name("return_type") {
            let raw_text = ret_node.utf8_text(source_bytes).unwrap_or("").to_string();
            if let Some(type_name) = resolve_type_name(&raw_text) {
                annotations.push(TypeAnnotation {
                    type_name,
                    raw_text,
                    annotation_kind: "return-type".to_string(),
                });
            }
        }

        // 提取 param types: `fn foo(x: ParamType, y: AnotherType)`
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "parameter" {
                    // parameter 中找 type 子节点
                    if let Some(type_node) = child.child_by_field_name("type") {
                        let raw_text = type_node.utf8_text(source_bytes).unwrap_or("").to_string();
                        if let Some(type_name) = resolve_type_name(&raw_text) {
                            annotations.push(TypeAnnotation {
                                type_name,
                                raw_text,
                                annotation_kind: "param-type".to_string(),
                            });
                        }
                    }
                }
            }
        }

        annotations
    }

    /// 从类型注解文本中解析类型名。
    ///
    /// 处理顺序：
    /// 1. 去掉 reference: `&str` → `str`, `&mut Path` → `Path`
    /// 2. 去掉 generic args: `HashMap<K, V>` → `HashMap`
    /// 3. 取最后 path 段: `std::vec::Vec` → `Vec`
    ///
    /// 返回 None 当类型为 Rust primitive（如 `str`、`u8`），不产 ACCESSES edge。
    fn resolve_type_name(raw_text: &str) -> Option<String> {
        // 去掉 reference / mutable reference
        let s = raw_text.trim_start_matches('&');
        let s = s.trim_start_matches("mut ");
        let s = s.trim();
        // 去掉 generic args: 找到第一个 < 并截断
        let s = if let Some(lt_pos) = s.find('<') {
            &s[..lt_pos]
        } else {
            s
        };
        // 取最后 segment: "std::vec::Vec" → "Vec"
        let name = s.rsplit("::").next().unwrap_or(s);
        // 过滤 Rust primitive types（不产生 edge）
        let primitives = [
            "bool", "char", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
            "u128", "usize", "f32", "f64", "str", "Self",
        ];
        if primitives.contains(&name) || name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    /// byte offset → 行号（1-indexed）
    fn byte_to_line(source_bytes: &[u8], byte_offset: usize) -> u32 {
        let mut line = 1u32;
        for &b in &source_bytes[..byte_offset.min(source_bytes.len())] {
            if b == b'\n' {
                line += 1;
            }
        }
        line
    }

    /// 构建 Symbol（完整版，含 parentId / implDetails）
    fn build_symbol_full(
        name: &str,
        kind: SymbolKind,
        visibility: &Visibility,
        line_start: u32,
        line_end: u32,
        input: &ItemExtractionInput,
        module_path: &str,
        parent_id: Option<&str>,
        generic_params: Option<&str>,
        is_async: bool,
        is_unsafe: bool,
        is_const_fn: bool,
        impl_details: Option<&ImplBlockDetail>,
        type_annotations: Vec<TypeAnnotation>,
    ) -> Symbol {
        let id = format!("{}::{}::{}", input.package_name, module_path, name);
        let is_pub = matches!(visibility, Visibility::Public);

        Symbol {
            id,
            name: name.to_string(),
            symbol_kind: kind.as_str().to_string(),
            source_path: input.source_path.clone(),
            package_name: input.package_name.clone(),
            target_name: input.target_name.clone(),
            module_path: Some(module_path.to_string()),
            visibility: visibility.as_str().to_string(),
            parent_id: parent_id.map(|s| s.to_string()),
            line_start,
            line_end,
            generic_params: generic_params.map(|s| s.to_string()),
            is_async,
            is_unsafe,
            is_const_fn,
            is_pub,
            impl_details: impl_details.cloned(),
            type_annotations,
        }
    }

    /// trait 内 unsupported associated items diagnostic
    fn emit_unsupported_associated_items(
        node: &tree_sitter::Node,
        source_path: &str,
        diagnostics: &mut Vec<SymbolDiagnostic>,
    ) {
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                let kind = child.kind();
                if kind == "associated_type" || kind == "const_item" {
                    diagnostics.push(SymbolDiagnostic {
                        code: codes::UNSUPPORTED_ASSOCIATED_ITEM.to_string(),
                        severity: "info".to_string(),
                        message: format!(
                            "trait 内 {} 不提取为独立 symbol",
                            if kind == "associated_type" {
                                "associated type"
                            } else {
                                "associated const"
                            }
                        ),
                        source_path: source_path.to_string(),
                        symbol_id: None,
                        suggested_action: None,
                    });
                }
            }
        }
    }
}

/// 公开同名 disambiguator 后处理（供 tree-sitter impl 调用）
#[cfg(feature = "tree-sitter-extraction")]
fn dedup_symbol_ids_pub(symbols: &mut Vec<Symbol>) {
    dedup_symbol_ids(symbols);
}

/// 返回 tree-sitter 是否可用
#[cfg(feature = "tree-sitter-extraction")]
pub fn is_tree_sitter_available() -> bool {
    tree_sitter_impl::try_init_parser().is_some()
}

/// 返回 tree-sitter 是否可用（feature 禁用时始终 false）
#[cfg(not(feature = "tree-sitter-extraction"))]
pub fn is_tree_sitter_available() -> bool {
    false
}

/// 创建最佳可用 extractor（tree-sitter 优先，fallback 到 TextItemExtractor）
pub fn create_best_extractor() -> Box<dyn ItemExtractor> {
    if is_tree_sitter_available() {
        #[cfg(feature = "tree-sitter-extraction")]
        {
            Box::new(tree_sitter_impl::TreeSitterItemExtractor)
        }
        #[cfg(not(feature = "tree-sitter-extraction"))]
        {
            Box::new(TextItemExtractor)
        }
    } else {
        Box::new(TextItemExtractor)
    }
}
