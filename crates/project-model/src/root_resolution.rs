//! crate:: root 解析器
//!
//! 基于 sourceOwnership.target 找到 crate root，扫描 mod 声明，
//! 解析 crate::module / crate::module::submodule 到 module source file。
//!
//! 这是 rootResolution 第一刀：
//! - 使用显式 root-queries.txt 作为 query 输入（不是 Rust parser 提取）
//! - 只处理 crate:: 形式，不处理 self:: / super:: / external crate
//! - 只解析到 module/file 层，不解析 item-level symbol（fn/struct/enum/trait）
//! - 使用 text-level mod 声明扫描，不引入 tree-sitter
//! - ambiguous 时 no-edge（不猜测）
//! - 必须有 mod 声明才解析 out-of-line module（file 存在但无声明 → no-edge）

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::{codes, Diagnostic};
use crate::model::*;

/// root resolution 扫描结果
pub struct RootResolutionResult {
    pub root_resolution: Vec<RootResolution>,
    pub diagnostics: Vec<Diagnostic>,
    pub resolution_success_count: u32,
    pub resolution_fail_count: u32,
}

/// 从 root-queries.txt 加载 query 列表
///
/// 文件格式：每行 sourcePath<TAB>queryPath
/// 这是 ProjectModel rootResolution 第一刀的显式 test input，不是最终 parser。
pub fn load_root_queries(root: &Path) -> Vec<(String, String)> {
    let query_file = root.join("root-queries.txt");
    let content = match std::fs::read_to_string(&query_file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut queries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((source_path, query_path)) = line.split_once('\t') {
            let source_path = source_path.trim().to_string();
            let query_path = query_path.trim().to_string();
            if !source_path.is_empty() && !query_path.is_empty() {
                queries.push((source_path, query_path));
            }
        }
    }
    queries
}

/// 执行 root resolution
pub fn scan_root_resolution(
    root: &Path,
    source_ownership: &[SourceOwnership],
    targets: &[TargetModel],
    queries: &[(String, String)],
) -> RootResolutionResult {
    let mut root_resolution = Vec::new();
    let mut diagnostics = Vec::new();
    let mut resolution_success_count: u32 = 0;
    let mut resolution_fail_count: u32 = 0;

    for (source_path, query_path) in queries {
        // 只处理 crate:: 形式
        if !query_path.starts_with("crate::") {
            continue;
        }

        // 查找 source file 的 ownership
        let ownership = source_ownership
            .iter()
            .find(|s| s.source_path == *source_path);

        let target_name = match ownership {
            Some(o) => o.target.as_ref(),
            None => None,
        };

        // target=null 时跳过 rootResolution
        let target_name = match target_name {
            Some(t) => t,
            None => {
                resolution_fail_count += 1;
                diagnostics.push(Diagnostic {
                    code: codes::ROOT_RESOLUTION_SKIPPED.to_string(),
                    severity: "info".to_string(),
                    message: format!(
                        "source {source_path} 无 target，跳过 rootResolution: {query_path}"
                    ),
                    path: source_path.clone(),
                    confidence: None,
                    reason: Some("root-resolution-skipped".to_string()),
                    related_paths: vec![],
                    suggested_action: None,
                });
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: None,
                    target_kind: None,
                    root_reason: "root-resolution-skipped".to_string(),
                    confidence: 0.0,
                    resolved_kind: None,
                    crate_root_file: None,
                });
                continue;
            }
        };

        // 查找 target 的 crate root file
        let target = targets.iter().find(|t| t.name == *target_name);
        let crate_root_rel = match target {
            Some(t) => t.crate_root_file.clone(),
            None => {
                resolution_fail_count += 1;
                diagnostics.push(Diagnostic {
                    code: codes::CRATE_ROOT_MISSING.to_string(),
                    severity: "warning".to_string(),
                    message: format!("source {source_path} 的 target {target_name} 无 crate root"),
                    path: source_path.clone(),
                    confidence: None,
                    reason: Some("crate-root-missing".to_string()),
                    related_paths: vec![],
                    suggested_action: None,
                });
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: None,
                    target_kind: None,
                    root_reason: "crate-root-missing".to_string(),
                    confidence: 0.0,
                    resolved_kind: None,
                    crate_root_file: None,
                });
                continue;
            }
        };

        let target_kind = target.map(|t| t.kind.clone());
        let crate_root_abs = root.join(&crate_root_rel);

        // 解析 crate:: 路径
        let module_segments = parse_crate_path(query_path);
        let resolved = resolve_module_chain(root, &crate_root_abs, &module_segments);

        match resolved {
            ModuleResolveResult::Resolved {
                resolved_path,
                reason,
                confidence,
            } => {
                resolution_success_count += 1;
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: Some(resolved_path),
                    target_kind: target_kind,
                    root_reason: reason,
                    confidence,
                    resolved_kind: Some("module".to_string()),
                    crate_root_file: Some(crate_root_rel),
                });
            }
            ModuleResolveResult::NotDeclared {
                path_checked,
                module_name,
            } => {
                resolution_fail_count += 1;
                diagnostics.push(Diagnostic {
                    code: codes::MODULE_NOT_DECLARED.to_string(),
                    severity: "info".to_string(),
                    message: format!(
                        "module {module_name} 在 {path_checked} 中无 mod 声明: {query_path}"
                    ),
                    path: source_path.clone(),
                    confidence: Some(0.50),
                    reason: Some("module-not-declared".to_string()),
                    related_paths: vec![],
                    suggested_action: Some("添加 mod 声明或确认 module 名称正确".to_string()),
                });
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: None,
                    target_kind: target_kind,
                    root_reason: "module-not-declared".to_string(),
                    confidence: 0.50,
                    resolved_kind: None,
                    crate_root_file: Some(crate_root_rel),
                });
            }
            ModuleResolveResult::FileMissing {
                path_checked,
                module_name,
            } => {
                resolution_fail_count += 1;
                diagnostics.push(Diagnostic {
                    code: codes::MODULE_FILE_MISSING.to_string(),
                    severity: "warning".to_string(),
                    message: format!(
                        "mod {module_name} 声明存在但文件缺失于 {path_checked}: {query_path}"
                    ),
                    path: source_path.clone(),
                    confidence: Some(0.50),
                    reason: Some("module-file-missing".to_string()),
                    related_paths: vec![],
                    suggested_action: Some("创建对应 module 文件或移除 mod 声明".to_string()),
                });
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: None,
                    target_kind: target_kind,
                    root_reason: "module-file-missing".to_string(),
                    confidence: 0.50,
                    resolved_kind: None,
                    crate_root_file: Some(crate_root_rel),
                });
            }
            ModuleResolveResult::Ambiguous {
                path_checked,
                module_name,
                file_a,
                file_b,
            } => {
                resolution_fail_count += 1;
                diagnostics.push(Diagnostic {
                    code: codes::CRATE_PATH_AMBIGUOUS.to_string(),
                    severity: "warning".to_string(),
                    message: format!(
                        "module {module_name} 在 {path_checked} 有多个候选: {file_a} vs {file_b}"
                    ),
                    path: source_path.clone(),
                    confidence: None,
                    reason: Some("crate-path-ambiguous".to_string()),
                    related_paths: vec![file_a, file_b],
                    suggested_action: Some("移除其中一个 module 文件以消除歧义".to_string()),
                });
                root_resolution.push(RootResolution {
                    source_path: source_path.clone(),
                    query_path: query_path.clone(),
                    resolved_path: None,
                    target_kind: target_kind,
                    root_reason: "crate-path-ambiguous".to_string(),
                    confidence: 0.0,
                    resolved_kind: None,
                    crate_root_file: Some(crate_root_rel),
                });
            }
        }
    }

    // 按 source_path + query_path 排序确保输出稳定
    root_resolution.sort_by(|a, b| {
        a.source_path
            .cmp(&b.source_path)
            .then(a.query_path.cmp(&b.query_path))
    });

    RootResolutionResult {
        root_resolution,
        diagnostics,
        resolution_success_count,
        resolution_fail_count,
    }
}

/// module 解析结果
enum ModuleResolveResult {
    Resolved {
        resolved_path: String,
        reason: String,
        confidence: f32,
    },
    NotDeclared {
        path_checked: String,
        module_name: String,
    },
    FileMissing {
        path_checked: String,
        module_name: String,
    },
    Ambiguous {
        path_checked: String,
        module_name: String,
        file_a: String,
        file_b: String,
    },
}

/// 解析 crate:: 路径为 module segments
fn parse_crate_path(query: &str) -> Vec<&str> {
    // crate::a::b::c → ["a", "b", "c"]
    query
        .strip_prefix("crate::")
        .map(|rest| rest.split("::").collect())
        .unwrap_or_default()
}

/// 逐级解析 module chain
///
/// 从 crate root 开始，逐个 segment 查找 mod 声明和对应文件。
/// 任一级失败则返回失败原因。
fn resolve_module_chain(
    repo_root: &Path,
    crate_root_abs: &Path,
    segments: &[&str],
) -> ModuleResolveResult {
    if segments.is_empty() {
        // crate:: 自身 → crate root
        return ModuleResolveResult::Resolved {
            resolved_path: path_relative_to(crate_root_abs, repo_root),
            reason: "crate-root-resolved".to_string(),
            confidence: 0.90,
        };
    }

    // 当前搜索的目录（从 crate root 所在目录开始）
    let mut current_dir = crate_root_abs
        .parent()
        .unwrap_or(crate_root_abs)
        .to_path_buf();
    // 当前文件（用于扫描 mod 声明）
    let mut current_file = crate_root_abs.to_path_buf();
    // 已使用的 visited set 防止循环
    let mut visited: HashSet<PathBuf> = HashSet::new();
    visited.insert(current_file.clone());

    let mut resolved_path = String::new();
    let mut depth = 0;

    for &segment in segments {
        depth += 1;

        // 在 current_file 中扫描 mod 声明
        let declared_modules = scan_mod_declarations(&current_file);

        // 检查是否有 cfg/path-attribute 等需跳过的声明
        let has_declaration = declared_modules.iter().any(|m| m.name == segment);

        if !has_declaration {
            return ModuleResolveResult::NotDeclared {
                path_checked: path_relative_to(&current_file, repo_root),
                module_name: segment.to_string(),
            };
        }

        // 查找 module 对应文件
        let candidate_a = current_dir.join(format!("{segment}.rs"));
        let candidate_b = current_dir.join(segment).join("mod.rs");

        let a_exists = candidate_a.exists();
        let b_exists = candidate_b.exists();

        if a_exists && b_exists {
            // 两个都存在 → ambiguous
            return ModuleResolveResult::Ambiguous {
                path_checked: path_relative_to(&current_file, repo_root),
                module_name: segment.to_string(),
                file_a: path_relative_to(&candidate_a, repo_root),
                file_b: path_relative_to(&candidate_b, repo_root),
            };
        }

        let module_file = if a_exists {
            candidate_a
        } else if b_exists {
            candidate_b
        } else {
            // 声明存在但文件不存在
            return ModuleResolveResult::FileMissing {
                path_checked: path_relative_to(&current_file, repo_root),
                module_name: segment.to_string(),
            };
        };

        // 防循环
        if visited.contains(&module_file) {
            return ModuleResolveResult::NotDeclared {
                path_checked: path_relative_to(&current_file, repo_root),
                module_name: segment.to_string(),
            };
        }
        visited.insert(module_file.clone());

        resolved_path = path_relative_to(&module_file, repo_root);
        current_dir = module_file.parent().unwrap_or(&module_file).to_path_buf();
        current_file = module_file;
    }

    let reason = if depth == 1 {
        "module-declaration-resolved".to_string()
    } else {
        "module-chain-resolved".to_string()
    };
    let confidence = if depth == 1 { 0.85 } else { 0.80 };

    ModuleResolveResult::Resolved {
        resolved_path,
        reason,
        confidence,
    }
}

/// mod 声明信息
struct ModDeclaration {
    name: String,
}

/// text-level mod 声明扫描
///
/// 这是 module reachability 的最小 evidence，不是完整 Rust parser。
/// 支持：mod name; / pub mod name;
/// 跳过：inline mod name { / block comment 中的 mod / cfg-gated / path attribute / macro
fn scan_mod_declarations(file: &Path) -> Vec<ModDeclaration> {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut declarations = Vec::new();
    let mut in_block_comment = false;

    let mut pos = 0usize;
    let content_bytes = content.as_bytes();

    while pos < content_bytes.len() {
        if in_block_comment {
            // 查找 */
            if pos + 1 < content_bytes.len()
                && content_bytes[pos] == b'*'
                && content_bytes[pos + 1] == b'/'
            {
                in_block_comment = false;
                pos += 2;
                continue;
            }
            pos += 1;
            continue;
        }

        // 检查 // 行注释
        if pos + 1 < content_bytes.len()
            && content_bytes[pos] == b'/'
            && content_bytes[pos + 1] == b'/'
        {
            // 跳到行尾
            while pos < content_bytes.len() && content_bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // 检查 /* 块注释开始
        if pos + 1 < content_bytes.len()
            && content_bytes[pos] == b'/'
            && content_bytes[pos + 1] == b'*'
        {
            in_block_comment = true;
            pos += 2;
            continue;
        }

        // 检查 mod 声明
        // 模式：[pub] mod <name> ;
        if pos + 3 <= content_bytes.len() && &content[pos..pos + 3] == "mod" {
            // 确认 mod 前面是行首或空白或 pub
            let before_ok = if pos == 0 {
                true
            } else {
                let b = content_bytes[pos - 1];
                b == b' ' || b == b'\t' || b == b'\n' || b == b'\r'
            };

            if before_ok {
                // 确认 mod 后面是空白
                if pos + 3 < content_bytes.len() {
                    let after = content_bytes[pos + 3];
                    if after == b' ' || after == b'\t' {
                        // 尝试解析 mod name;
                        if let Some(decl) = try_parse_mod_decl(&content[pos..]) {
                            // 检查是否是 inline mod name { (跳过)
                            declarations.push(decl);
                        }
                    }
                }
            }
        }

        pos += 1;
    }

    declarations
}

/// 尝试从当前位置解析 mod 声明
fn try_parse_mod_decl(s: &str) -> Option<ModDeclaration> {
    // 模式：mod <name> ;
    // 跳过 inline：mod <name> {
    let s = s.strip_prefix("mod")?;
    let s = s.trim_start();

    // 读取 name（identifier 字符：字母/数字/_）
    let name_end = s
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    if name_end == 0 {
        return None;
    }
    let name = &s[..name_end];
    let rest = s[name_end..].trim_start();

    // 必须以 ; 结尾（out-of-line module）
    if rest.starts_with(';') {
        return Some(ModDeclaration {
            name: name.to_string(),
        });
    }

    // inline mod name { ... } → 不处理
    if rest.starts_with('{') {
        return None;
    }

    None
}

/// 计算 path 相对于 base 的 POSIX 相对路径
fn path_relative_to(path: &Path, base: &Path) -> String {
    match path.strip_prefix(base) {
        Ok(rel) => {
            let s = rel.to_string_lossy().to_string();
            if s.is_empty() {
                ".".to_string()
            } else {
                s.replace('\\', "/")
            }
        }
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}
