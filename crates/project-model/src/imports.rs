//! Import/Use Resolution — tree-sitter extractor + resolver
//!
//! 第一刀 intermediate output，不直接进入 graph emitter。
//! 从 .rs 文件中提取 use 声明，展开 grouped import，解析 crate::/self::/super:: 路径。
//!
//! 核心策略：
//! - tree-sitter 优先提取 use_declaration / scoped_use_list / use_as_clause
//! - crate:: 路径复用 root_resolution::resolve_module_chain
//! - self:: 按当前 modulePath 展开，super:: 按 parent module 展开
//! - grouped import 每个叶节点产一条独立 ImportUse
//! - external crate / glob 只产 diagnostic，不解析/不展开
//! - unresolved 不产 fake resolvedTo，只产 diagnostic（no-edge 策略）
//!
//! 已知限制：
//! - modulePath 在当前 landed reality 中可能是 flat（如 "crate" 而非 "crate::config"），
//!   影响 self::/super:: 解析精度。记录为 known limitation，不产 fake resolvedTo。
//! - 不做 item-level symbol resolution，只解析到 module/file 层。

use std::path::Path;

use crate::model::*;
use crate::root_resolution::{self, ModuleResolveResult};

/// import/use resolution 扫描结果
pub struct ImportUseResult {
    pub imports: Vec<ImportUse>,
    pub diagnostics: Vec<ImportUseDiagnostic>,
    pub import_count: u32,
}

/// 从多个 .rs 文件提取并解析 use 声明
///
/// module_path_map 提供文件级 sourcePath → modulePath 映射，
/// 让 ImportUse.modulePath 使用真实模块路径而非 flat "crate"。
/// self:: / super:: 展开算法本身不变，输入改善后自动获得正确基准路径。
pub fn extract_and_resolve_imports(
    repo_root: &Path,
    source_ownership: &[SourceOwnership],
    targets: &[TargetModel],
    module_path_map: &crate::module_path::ModulePathMap,
) -> ImportUseResult {
    let mut all_imports = Vec::new();
    let all_diagnostics = Vec::new();

    for so in source_ownership {
        let pkg_name = match &so.package {
            Some(p) => p.clone(),
            None => continue,
        };

        let abs_path = repo_root.join(&so.source_path);
        let source_text = match std::fs::read_to_string(&abs_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        // 查找 crate root
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

        // 使用 ModulePathMap 查找文件级 modulePath，fallback 到 "crate"
        let module_path = Some(module_path_map.get(&so.source_path).to_string());

        let _ = pkg_name;

        // 提取 use 声明
        let mut imports = extract_use_declarations(&source_text, &so.source_path, &module_path);

        // 填充文件级 modulePath：extractor 产出的 ImportUse.modulePath 可能是 None，
        // 这里统一用 ModulePathMap 查找结果覆盖
        for import_use in &mut imports {
            if import_use.module_path.is_none() {
                import_use.module_path = module_path.clone();
            }
        }

        // 解析每条 use 声明
        for import_use in &mut imports {
            resolve_import(import_use, repo_root, &crate_root_abs, &module_path);
        }

        all_imports.extend(imports);
    }

    // 按 source_path + line_start + id 排序确保输出稳定
    all_imports.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then(a.line_start.cmp(&b.line_start))
            .then(a.id.cmp(&b.id))
    });

    let import_count = all_imports.len() as u32;

    ImportUseResult {
        imports: all_imports,
        diagnostics: all_diagnostics,
        import_count,
    }
}

/// 提取单个 .rs 文件中的所有 use 声明
fn extract_use_declarations(
    source_text: &str,
    source_path: &str,
    module_path: &Option<String>,
) -> Vec<ImportUse> {
    #[cfg(feature = "tree-sitter-extraction")]
    {
        if let Some(imports) = extract_with_tree_sitter(source_text, source_path, module_path) {
            return imports;
        }
    }

    // fallback: text-level 扫描
    extract_with_text(source_text, source_path, module_path)
}

// ============================================================
// text-level fallback
// ============================================================

/// text-level fallback：逐行扫描 use 声明
fn extract_with_text(
    source_text: &str,
    source_path: &str,
    _module_path: &Option<String>,
) -> Vec<ImportUse> {
    let mut imports = Vec::new();
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

        // 跳过 cfg 属性行
        if trimmed.starts_with("#[cfg(") {
            continue;
        }

        if let Some(decls) = parse_text_use_decl(trimmed, source_path, line_num) {
            imports.extend(decls);
        }
    }

    imports
}

/// 从文本行解析 use 声明，可能展开为多条（grouped import）
fn parse_text_use_decl(trimmed: &str, source_path: &str, line_num: u32) -> Option<Vec<ImportUse>> {
    if trimmed.starts_with('#') {
        return None;
    }

    let (rest, visibility, is_re_export) = parse_text_visibility(trimmed);
    let rest = rest.trim_start();
    if !rest.starts_with("use ") {
        return None;
    }
    let rest = rest[4..].trim_start();
    let path_part = rest.trim_end_matches(';').trim();

    if path_part.is_empty() {
        return None;
    }

    // 检查 glob
    if path_part.ends_with("::*") {
        let original_path = path_part.trim_end_matches("::*").to_string();
        let segments: Vec<&str> = original_path.split("::").collect();
        let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));

        return Some(vec![ImportUse {
            id: format!("{}::use::{}::0", source_path, line_num),
            source_path: source_path.to_string(),
            module_path: None,
            raw_text: trimmed.to_string(),
            line_start: line_num,
            line_end: line_num,
            visibility,
            path_kind: path_kind.as_str().to_string(),
            original_path,
            expanded_path: None,
            alias: None,
            is_re_export,
            target_name: "*".to_string(),
            resolved_to: None,
            confidence: 0.0,
            reason: ImportUseResolutionReason::UseGlobUnsupported
                .as_str()
                .to_string(),
            diagnostics: vec![ImportUseDiagnostic {
                code: "use-glob-unsupported".to_string(),
                severity: "info".to_string(),
                message: "glob import 不展开".to_string(),
                target_name: Some("*".to_string()),
            }],
        }]);
    }

    // 检查 grouped import: path::{...}
    if let Some(brace_start) = path_part.find("::{") {
        let base_path = &path_part[..brace_start];
        let group_content = &path_part[brace_start + 3..path_part.len().saturating_sub(1)];

        return Some(expand_grouped_import(
            base_path,
            group_content,
            source_path,
            line_num,
            trimmed,
            &visibility,
            is_re_export,
        ));
    }

    // 检查 alias: path as name
    if let Some(as_pos) = path_part.find(" as ") {
        let original_path = path_part[..as_pos].to_string();
        let alias_name = path_part[as_pos + 4..].trim().to_string();
        let segments: Vec<&str> = original_path.split("::").collect();
        let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));

        return Some(vec![ImportUse {
            id: format!("{}::use::{}::0", source_path, line_num),
            source_path: source_path.to_string(),
            module_path: None,
            raw_text: trimmed.to_string(),
            line_start: line_num,
            line_end: line_num,
            visibility,
            path_kind: path_kind.as_str().to_string(),
            original_path: original_path.clone(),
            expanded_path: Some(original_path),
            alias: Some(alias_name.clone()),
            is_re_export,
            target_name: alias_name,
            resolved_to: None,
            confidence: 0.0,
            reason: ImportUseResolutionReason::UseAliasResolved
                .as_str()
                .to_string(),
            diagnostics: vec![],
        }]);
    }

    // simple import: path::to::Item
    let segments: Vec<&str> = path_part.split("::").collect();
    let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));
    let target_name = segments.last().unwrap_or(&"").to_string();
    let original_path = path_part.to_string();

    Some(vec![ImportUse {
        id: format!("{}::use::{}::0", source_path, line_num),
        source_path: source_path.to_string(),
        module_path: None,
        raw_text: trimmed.to_string(),
        line_start: line_num,
        line_end: line_num,
        visibility,
        path_kind: path_kind.as_str().to_string(),
        original_path: original_path.clone(),
        expanded_path: Some(original_path),
        alias: None,
        is_re_export,
        target_name,
        resolved_to: None,
        confidence: 0.0,
        reason: ImportUseResolutionReason::UseCrateResolved
            .as_str()
            .to_string(),
        diagnostics: vec![],
    }])
}

/// 解析文本可见性前缀
fn parse_text_visibility(trimmed: &str) -> (&str, String, bool) {
    if let Some(rest) = trimmed.strip_prefix("pub(crate)") {
        (rest, "pub-crate".to_string(), true)
    } else if let Some(rest) = trimmed.strip_prefix("pub(super)") {
        (rest, "pub-super".to_string(), true)
    } else if let Some(rest) = trimmed.strip_prefix("pub(in ") {
        (rest, "pub-restricted".to_string(), true)
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        (rest, "public".to_string(), true)
    } else {
        (trimmed, "private".to_string(), false)
    }
}

/// 判断路径类型
fn determine_path_kind(first_segment: &str) -> ImportUseKind {
    match first_segment {
        "crate" => ImportUseKind::Crate,
        "self" => ImportUseKind::SelfPath,
        "super" => ImportUseKind::Super,
        s if !s.is_empty() && s.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) => {
            ImportUseKind::External
        }
        _ => ImportUseKind::Unknown,
    }
}

/// 展开 grouped import 为独立 ImportUse 条目
fn expand_grouped_import(
    base_path: &str,
    group_content: &str,
    source_path: &str,
    line_num: u32,
    raw_text: &str,
    visibility: &str,
    is_re_export: bool,
) -> Vec<ImportUse> {
    let members: Vec<&str> = group_content.split(',').map(|s| s.trim()).collect();
    let mut results = Vec::new();

    for (idx, member) in members.iter().enumerate() {
        if member.is_empty() {
            continue;
        }

        // 处理 "Bar as B" 形式
        let (member_name, alias) = if let Some(as_pos) = member.find(" as ") {
            let name = member[..as_pos].trim().to_string();
            let a = member[as_pos + 4..].trim().to_string();
            (name, Some(a))
        } else {
            (member.to_string(), None)
        };

        let full_path = if member_name == "self" {
            base_path.to_string()
        } else {
            format!("{}::{}", base_path, member_name)
        };

        let segments: Vec<&str> = full_path.split("::").collect();
        let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));
        let target_name = alias.as_deref().unwrap_or(&member_name).to_string();

        results.push(ImportUse {
            id: format!("{}::use::{}::{}", source_path, line_num, idx),
            source_path: source_path.to_string(),
            module_path: None,
            raw_text: raw_text.to_string(),
            line_start: line_num,
            line_end: line_num,
            visibility: visibility.to_string(),
            path_kind: path_kind.as_str().to_string(),
            original_path: full_path.clone(),
            expanded_path: Some(full_path),
            alias,
            is_re_export,
            target_name,
            resolved_to: None,
            confidence: 0.0,
            reason: ImportUseResolutionReason::UseGroupExpanded
                .as_str()
                .to_string(),
            diagnostics: vec![],
        });
    }

    results
}

// ============================================================
// 解析逻辑
// ============================================================

/// 解析 import 路径到文件目标
fn resolve_import(
    import: &mut ImportUse,
    repo_root: &Path,
    crate_root_abs: &Path,
    _module_path: &Option<String>,
) {
    let path_kind_str = import.path_kind.as_str();

    match path_kind_str {
        "crate" => resolve_crate_path(import, repo_root, crate_root_abs),
        "self" => resolve_self_path(import, repo_root, crate_root_abs),
        "super" => resolve_super_path(import, repo_root, crate_root_abs),
        "external" => {
            // external crate 只标记不解析，stop-line（D5）
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseExternalSkipped
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-external-skipped".to_string(),
                severity: "info".to_string(),
                message: format!("external crate import 跳过: {}", import.original_path),
                target_name: Some(import.target_name.clone()),
            });
        }
        _ => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetUnresolved
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!("无法识别路径类型: {}", import.original_path),
                target_name: Some(import.target_name.clone()),
            });
        }
    }
}

/// 解析 crate:: 路径，复用 root_resolution::resolve_module_chain
fn resolve_crate_path(import: &mut ImportUse, repo_root: &Path, crate_root_abs: &Path) {
    let segments = root_resolution::parse_crate_path(&import.original_path);
    let result = root_resolution::resolve_module_chain(repo_root, crate_root_abs, &segments);

    match result {
        ModuleResolveResult::Resolved {
            resolved_path,
            reason: _resolve_reason,
            confidence,
        } => {
            let resolved_kind = if segments.is_empty() {
                "crate-root"
            } else if segments.len() == 1 {
                "module"
            } else {
                "module-chain"
            };

            let target_module_path = if segments.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", segments.join("::"))
            };

            import.resolved_to = Some(ImportUseTarget {
                resolved_path: Some(resolved_path),
                resolved_kind: Some(resolved_kind.to_string()),
                target_module_path: Some(target_module_path),
                target_file_path: None,
            });
            import.confidence = confidence;
            // 保持原始 reason（group-expanded / alias-resolved）或覆盖为 use-crate-resolved
            if import.reason != ImportUseResolutionReason::UseGroupExpanded.as_str()
                && import.reason != ImportUseResolutionReason::UseAliasResolved.as_str()
                && import.reason != ImportUseResolutionReason::UseReexportResolved.as_str()
            {
                import.reason = ImportUseResolutionReason::UseCrateResolved
                    .as_str()
                    .to_string();
            }
        }
        ModuleResolveResult::NotDeclared {
            path_checked,
            module_name,
        } => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetUnresolved
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!("module {} 在 {} 中无 mod 声明", module_name, path_checked),
                target_name: Some(import.target_name.clone()),
            });
        }
        ModuleResolveResult::FileMissing {
            path_checked,
            module_name,
        } => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetUnresolved
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!("mod {} 声明存在但文件缺失于 {}", module_name, path_checked),
                target_name: Some(import.target_name.clone()),
            });
        }
        ModuleResolveResult::Ambiguous {
            path_checked,
            module_name,
            file_a,
            file_b,
        } => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetAmbiguous
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-ambiguous".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "module {} 在 {} 有多个候选: {} vs {}",
                    module_name, path_checked, file_a, file_b
                ),
                target_name: Some(import.target_name.clone()),
            });
        }
    }
}

/// 解析 self:: 路径
///
/// 策略：以当前 modulePath 为基准，计算绝对路径。
/// 已知限制（D7）：modulePath 可能是 flat（如 "crate"），影响 self:: 解析精度。
fn resolve_self_path(import: &mut ImportUse, repo_root: &Path, crate_root_abs: &Path) {
    let module_path = import.module_path.as_deref().unwrap_or("crate");

    // self::foo::Bar → crate::{modulePath}::foo::Bar
    let self_rest = import
        .original_path
        .strip_prefix("self::")
        .unwrap_or(&import.original_path);

    let expanded = if module_path == "crate" {
        format!("crate::{}", self_rest)
    } else {
        format!("{}::{}", module_path, self_rest)
    };

    import.expanded_path = Some(expanded.clone());

    let segments = root_resolution::parse_crate_path(&expanded);
    let result = root_resolution::resolve_module_chain(repo_root, crate_root_abs, &segments);

    match result {
        ModuleResolveResult::Resolved { resolved_path, .. } => {
            let target_module_path = if segments.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", segments.join("::"))
            };

            import.resolved_to = Some(ImportUseTarget {
                resolved_path: Some(resolved_path),
                resolved_kind: Some("module".to_string()),
                target_module_path: Some(target_module_path),
                target_file_path: None,
            });
            import.confidence = 0.75;
            import.reason = ImportUseResolutionReason::UseSelfResolved
                .as_str()
                .to_string();
        }
        _ => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetUnresolved
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "self:: 路径无法解析: {} → {}",
                    import.original_path, expanded
                ),
                target_name: Some(import.target_name.clone()),
            });
        }
    }
}

/// 解析 super:: 路径
///
/// 策略：从当前 modulePath 去掉末段，作为父 module。
/// 支持 super::super:: 多级。如果 modulePath == "crate"，super:: 无法解析。
fn resolve_super_path(import: &mut ImportUse, repo_root: &Path, crate_root_abs: &Path) {
    let module_path = import.module_path.as_deref().unwrap_or("crate");

    // 计算 super 级数
    let mut super_count = 0usize;
    let mut rest = import.original_path.as_str();
    while let Some(r) = rest.strip_prefix("super::") {
        super_count += 1;
        rest = r;
    }

    // 从 modulePath 去掉 super_count 段
    let mut base_parts: Vec<&str> = module_path.split("::").collect();
    for _ in 0..super_count {
        if base_parts.len() <= 1 {
            // 已经在 crate root，super:: 无法再向上
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseSuperAtCrateRoot
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-super-at-crate-root".to_string(),
                severity: "info".to_string(),
                message: format!("super:: 在 crate root 使用: {}", import.original_path),
                target_name: Some(import.target_name.clone()),
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

    import.expanded_path = Some(expanded.clone());

    let segments = root_resolution::parse_crate_path(&expanded);
    let result = root_resolution::resolve_module_chain(repo_root, crate_root_abs, &segments);

    match result {
        ModuleResolveResult::Resolved { resolved_path, .. } => {
            let target_module_path = if segments.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", segments.join("::"))
            };

            import.resolved_to = Some(ImportUseTarget {
                resolved_path: Some(resolved_path),
                resolved_kind: Some("module".to_string()),
                target_module_path: Some(target_module_path),
                target_file_path: None,
            });
            import.confidence = 0.75;
            import.reason = ImportUseResolutionReason::UseSuperResolved
                .as_str()
                .to_string();
        }
        _ => {
            import.confidence = 0.0;
            import.reason = ImportUseResolutionReason::UseTargetUnresolved
                .as_str()
                .to_string();
            import.diagnostics.push(ImportUseDiagnostic {
                code: "use-path-unresolved".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "super:: 路径无法解析: {} → {}",
                    import.original_path, expanded
                ),
                target_name: Some(import.target_name.clone()),
            });
        }
    }
}

// ============================================================
// tree-sitter 提取实现
// ============================================================

#[cfg(feature = "tree-sitter-extraction")]
fn extract_with_tree_sitter(
    source_text: &str,
    source_path: &str,
    _module_path: &Option<String>,
) -> Option<Vec<ImportUse>> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).ok()?;

    let tree = parser.parse(source_text, None)?;
    let root = tree.root_node();
    let source_bytes = source_text.as_bytes();

    let mut imports = Vec::new();
    collect_use_declarations(&root, source_bytes, source_path, &mut imports);

    Some(imports)
}

#[cfg(feature = "tree-sitter-extraction")]
fn collect_use_declarations(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    source_path: &str,
    imports: &mut Vec<ImportUse>,
) {
    if node.kind() == "use_declaration" {
        let decls = process_use_declaration(node, source_bytes, source_path);
        imports.extend(decls);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_use_declarations(&child, source_bytes, source_path, imports);
    }
}

#[cfg(feature = "tree-sitter-extraction")]
fn process_use_declaration(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    source_path: &str,
) -> Vec<ImportUse> {
    let line_start = byte_to_line(source_bytes, node.start_byte());
    let line_end = byte_to_line(source_bytes, node.end_byte());
    let raw_text = node.utf8_text(source_bytes).unwrap_or("").to_string();

    let (visibility, is_re_export) = extract_use_visibility(node, source_bytes);

    let mut cursor = node.walk();
    let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

    let argument = children.iter().find(|c| {
        let k = c.kind();
        k == "scoped_identifier"
            || k == "scoped_use_list"
            || k == "use_wildcard"
            || k == "use_as_clause"
            || k == "identifier"
    });

    match argument {
        Some(arg) => process_use_argument(
            arg,
            source_bytes,
            source_path,
            line_start,
            line_end,
            &raw_text,
            &visibility,
            is_re_export,
            0,
        ),
        None => vec![],
    }
}

#[cfg(feature = "tree-sitter-extraction")]
fn extract_use_visibility(node: &tree_sitter::Node, source_bytes: &[u8]) -> (String, bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = child.utf8_text(source_bytes).unwrap_or("");
            if text == "pub" {
                return ("public".to_string(), true);
            } else if text.starts_with("pub(crate)") {
                return ("pub-crate".to_string(), true);
            } else if text.starts_with("pub(super)") {
                return ("pub-super".to_string(), true);
            } else if text.starts_with("pub(in") {
                return ("pub-restricted".to_string(), true);
            }
        }
    }
    ("private".to_string(), false)
}

#[cfg(feature = "tree-sitter-extraction")]
fn process_use_argument(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    source_path: &str,
    line_start: u32,
    line_end: u32,
    raw_text: &str,
    visibility: &str,
    is_re_export: bool,
    start_index: u32,
) -> Vec<ImportUse> {
    let kind = node.kind();

    match kind {
        "scoped_identifier" | "identifier" => {
            let path_text = node.utf8_text(source_bytes).unwrap_or("").to_string();
            let segments: Vec<&str> = path_text.split("::").collect();
            let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));
            let target_name = segments.last().unwrap_or(&"").to_string();

            vec![ImportUse {
                id: format!("{}::use::{}::{}", source_path, line_start, start_index),
                source_path: source_path.to_string(),
                module_path: None,
                raw_text: raw_text.to_string(),
                line_start,
                line_end,
                visibility: visibility.to_string(),
                path_kind: path_kind.as_str().to_string(),
                original_path: path_text.clone(),
                expanded_path: Some(path_text),
                alias: None,
                is_re_export,
                target_name,
                resolved_to: None,
                confidence: 0.0,
                reason: ImportUseResolutionReason::UseCrateResolved
                    .as_str()
                    .to_string(),
                diagnostics: vec![],
            }]
        }

        "use_wildcard" => {
            vec![ImportUse {
                id: format!("{}::use::{}::{}", source_path, line_start, start_index),
                source_path: source_path.to_string(),
                module_path: None,
                raw_text: raw_text.to_string(),
                line_start,
                line_end,
                visibility: visibility.to_string(),
                path_kind: "unknown".to_string(),
                original_path: node.utf8_text(source_bytes).unwrap_or("*").to_string(),
                expanded_path: None,
                alias: None,
                is_re_export,
                target_name: "*".to_string(),
                resolved_to: None,
                confidence: 0.0,
                reason: ImportUseResolutionReason::UseGlobUnsupported
                    .as_str()
                    .to_string(),
                diagnostics: vec![ImportUseDiagnostic {
                    code: "use-glob-unsupported".to_string(),
                    severity: "info".to_string(),
                    message: "glob import 不展开".to_string(),
                    target_name: Some("*".to_string()),
                }],
            }]
        }

        "use_as_clause" => {
            let mut cursor = node.walk();
            let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

            let mut original_path = String::new();
            let mut alias_name = String::new();
            let mut identifiers: Vec<String> = Vec::new();

            for child in &children {
                match child.kind() {
                    "scoped_identifier" | "identifier" => {
                        let text = child.utf8_text(source_bytes).unwrap_or("").to_string();
                        identifiers.push(text);
                    }
                    _ => {}
                }
            }

            if identifiers.len() >= 2 {
                original_path = identifiers[0].clone();
                alias_name = identifiers[1].clone();
            } else if identifiers.len() == 1 {
                original_path = identifiers[0].clone();
            }

            let segments: Vec<&str> = original_path.split("::").collect();
            let path_kind = determine_path_kind(segments.first().copied().unwrap_or(""));
            let target_name = if alias_name.is_empty() {
                segments.last().unwrap_or(&"").to_string()
            } else {
                alias_name.clone()
            };

            vec![ImportUse {
                id: format!("{}::use::{}::{}", source_path, line_start, start_index),
                source_path: source_path.to_string(),
                module_path: None,
                raw_text: raw_text.to_string(),
                line_start,
                line_end,
                visibility: visibility.to_string(),
                path_kind: path_kind.as_str().to_string(),
                original_path: original_path.clone(),
                expanded_path: Some(original_path),
                alias: if alias_name.is_empty() {
                    None
                } else {
                    Some(alias_name)
                },
                is_re_export,
                target_name,
                resolved_to: None,
                confidence: 0.0,
                reason: ImportUseResolutionReason::UseAliasResolved
                    .as_str()
                    .to_string(),
                diagnostics: vec![],
            }]
        }

        "scoped_use_list" => {
            let mut results = Vec::new();
            let mut idx = start_index;

            let node_text = node.utf8_text(source_bytes).unwrap_or("");
            let base_path = if let Some(brace_pos) = node_text.find("::{") {
                node_text[..brace_pos].to_string()
            } else {
                String::new()
            };

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "use_list" {
                    let mut inner_cursor = child.walk();
                    for item in child.children(&mut inner_cursor) {
                        match item.kind() {
                            "identifier" | "scoped_identifier" => {
                                let member_name =
                                    item.utf8_text(source_bytes).unwrap_or("").to_string();
                                let full_path = if base_path.is_empty() {
                                    member_name.clone()
                                } else {
                                    format!("{}::{}", base_path, member_name)
                                };
                                let segments: Vec<&str> = full_path.split("::").collect();
                                let path_kind =
                                    determine_path_kind(segments.first().copied().unwrap_or(""));

                                results.push(ImportUse {
                                    id: format!("{}::use::{}::{}", source_path, line_start, idx),
                                    source_path: source_path.to_string(),
                                    module_path: None,
                                    raw_text: raw_text.to_string(),
                                    line_start,
                                    line_end,
                                    visibility: visibility.to_string(),
                                    path_kind: path_kind.as_str().to_string(),
                                    original_path: full_path.clone(),
                                    expanded_path: Some(full_path),
                                    alias: None,
                                    is_re_export,
                                    target_name: member_name,
                                    resolved_to: None,
                                    confidence: 0.0,
                                    reason: ImportUseResolutionReason::UseGroupExpanded
                                        .as_str()
                                        .to_string(),
                                    diagnostics: vec![],
                                });
                                idx += 1;
                            }
                            "use_as_clause" => {
                                let sub_results = process_use_argument(
                                    &item,
                                    source_bytes,
                                    source_path,
                                    line_start,
                                    line_end,
                                    raw_text,
                                    visibility,
                                    is_re_export,
                                    idx,
                                );
                                idx += sub_results.len() as u32;
                                results.extend(sub_results);
                            }
                            "self" => {
                                // self in group → 代表 base_path module 本身
                                let target_name =
                                    base_path.split("::").last().unwrap_or("").to_string();
                                let segments: Vec<&str> = base_path.split("::").collect();
                                let path_kind =
                                    determine_path_kind(segments.first().copied().unwrap_or(""));

                                results.push(ImportUse {
                                    id: format!("{}::use::{}::{}", source_path, line_start, idx),
                                    source_path: source_path.to_string(),
                                    module_path: None,
                                    raw_text: raw_text.to_string(),
                                    line_start,
                                    line_end,
                                    visibility: visibility.to_string(),
                                    path_kind: path_kind.as_str().to_string(),
                                    original_path: base_path.clone(),
                                    expanded_path: Some(base_path.clone()),
                                    alias: None,
                                    is_re_export,
                                    target_name,
                                    resolved_to: None,
                                    confidence: 0.0,
                                    reason: ImportUseResolutionReason::UseGroupExpanded
                                        .as_str()
                                        .to_string(),
                                    diagnostics: vec![],
                                });
                                idx += 1;
                            }
                            "use_wildcard" => {
                                results.push(ImportUse {
                                    id: format!("{}::use::{}::{}", source_path, line_start, idx),
                                    source_path: source_path.to_string(),
                                    module_path: None,
                                    raw_text: raw_text.to_string(),
                                    line_start,
                                    line_end,
                                    visibility: visibility.to_string(),
                                    path_kind: "unknown".to_string(),
                                    original_path: format!("{}::*", base_path),
                                    expanded_path: None,
                                    alias: None,
                                    is_re_export,
                                    target_name: "*".to_string(),
                                    resolved_to: None,
                                    confidence: 0.0,
                                    reason: ImportUseResolutionReason::UseGlobUnsupported
                                        .as_str()
                                        .to_string(),
                                    diagnostics: vec![ImportUseDiagnostic {
                                        code: "use-glob-unsupported".to_string(),
                                        severity: "info".to_string(),
                                        message: "glob import 不展开".to_string(),
                                        target_name: Some("*".to_string()),
                                    }],
                                });
                                idx += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }

            results
        }

        _ => vec![],
    }
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
