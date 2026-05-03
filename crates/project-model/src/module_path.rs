//! sourcePath → modulePath 映射
//!
//! 从 ProjectModel 数据构建 ModulePathMap，将每个 .rs 文件映射到其 crate 内 module 路径。
//! 两趟扫描算法：
//!   第一趟：从每个 target 的 crate root 出发，递归扫描 mod 声明构建 module tree
//!   第二趟：对 sourceOwnership 中所有有 target 归属的 .rs 文件，填充未在 module tree 中的文件
//!
//! 消费方：Symbol.modulePath / ImportUse.modulePath / Graph Module node

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{ModulePathDiagnostic, ModulePathEntry, SourceOwnership, TargetModel};
use crate::root_resolution::scan_mod_declarations;

/// sourcePath → modulePath 映射表
pub struct ModulePathMap {
    entries: Vec<ModulePathEntry>,
    diagnostics: Vec<ModulePathDiagnostic>,
    /// sourcePath → modulePath 快速查找
    lookup: HashMap<String, String>,
}

impl ModulePathMap {
    /// 查询 sourcePath 的 modulePath，默认 "crate"
    pub fn get(&self, source_path: &str) -> &str {
        self.lookup
            .get(source_path)
            .map(|s| s.as_str())
            .unwrap_or("crate")
    }

    /// 获取所有条目
    pub fn entries(&self) -> &[ModulePathEntry] {
        &self.entries
    }

    /// 获取所有 diagnostics
    pub fn diagnostics(&self) -> &[ModulePathDiagnostic] {
        &self.diagnostics
    }
}

/// 最大递归深度，防止循环或过深嵌套
const MAX_MODULE_DEPTH: usize = 16;

/// 从 ProjectModel 数据构建 ModulePathMap
pub fn build_module_path_map(
    repo_root: &Path,
    source_ownership: &[SourceOwnership],
    targets: &[TargetModel],
) -> ModulePathMap {
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    let mut lookup = HashMap::new();

    // 收集所有有归属的 .rs 文件，用于第二趟 fallback
    let mut owned_rs_files: HashSet<String> = HashSet::new();
    for so in source_ownership {
        if so.package.is_some() && so.source_path.ends_with(".rs") {
            owned_rs_files.insert(so.source_path.clone());
        }
    }

    // 第一趟：对每个 target，从 crate root 递归构建 module tree
    for target in targets {
        let crate_root_rel = &target.crate_root_file;
        let crate_root_abs = repo_root.join(crate_root_rel);

        if !crate_root_abs.exists() {
            continue;
        }

        // crate root 自身 → "crate"
        let root_path = normalize_path(crate_root_rel);
        entries.push(ModulePathEntry {
            source_path: root_path.clone(),
            module_path: "crate".to_string(),
            confidence: 1.0,
            reason: "module-path-crate-root".to_string(),
        });
        lookup.insert(root_path.clone(), "crate".to_string());

        // 从 crate root 开始递归扫描 mod 声明
        let mut visited: HashSet<PathBuf> = HashSet::new();
        visited.insert(crate_root_abs.clone());
        let mut ctx = ScanContext {
            repo_root,
            entries: &mut entries,
            diagnostics: &mut diagnostics,
            lookup: &mut lookup,
            visited: &mut visited,
        };
        scan_module_tree(&mut ctx, &crate_root_abs, "crate", 0);
    }

    // 第二趟：对未在 module tree 中的文件，降级为 "crate"
    for source_path in &owned_rs_files {
        if !lookup.contains_key(source_path.as_str()) {
            entries.push(ModulePathEntry {
                source_path: source_path.clone(),
                module_path: "crate".to_string(),
                confidence: 0.50,
                reason: "module-path-no-declaration".to_string(),
            });
            diagnostics.push(ModulePathDiagnostic {
                code: "module-path-no-declaration".to_string(),
                severity: "info".to_string(),
                message: format!("文件 {} 无 mod 声明，modulePath 降级为 crate", source_path),
                source_path: source_path.clone(),
            });
            lookup.insert(source_path.clone(), "crate".to_string());
        }
    }

    // 排序确保输出稳定
    entries.sort_by(|a, b| a.source_path.cmp(&b.source_path));

    ModulePathMap {
        entries,
        diagnostics,
        lookup,
    }
}

/// 递归扫描的共享上下文
struct ScanContext<'a> {
    repo_root: &'a Path,
    entries: &'a mut Vec<ModulePathEntry>,
    diagnostics: &'a mut Vec<ModulePathDiagnostic>,
    lookup: &'a mut HashMap<String, String>,
    visited: &'a mut HashSet<PathBuf>,
}

/// 递归扫描 module tree
///
/// 从 parent_file 开始，扫描 mod 声明，查找对应的 .rs 文件或 mod.rs 目录，
/// 将找到的文件映射为 parent_module_path::name
fn scan_module_tree(
    ctx: &mut ScanContext,
    parent_file: &Path,
    parent_module_path: &str,
    depth: usize,
) {
    if depth >= MAX_MODULE_DEPTH {
        return;
    }

    let declarations = scan_mod_declarations(parent_file);
    let parent_dir = parent_file.parent().unwrap_or(parent_file);

    for decl in &declarations {
        let candidate_file = parent_dir.join(format!("{}.rs", decl.name));
        let candidate_dir = parent_dir.join(&decl.name).join("mod.rs");

        let file_exists = candidate_file.exists();
        let dir_exists = candidate_dir.exists();

        if file_exists && dir_exists {
            let rel_file = normalize_path(&path_relative_to(&candidate_file, ctx.repo_root));
            let rel_dir = normalize_path(&path_relative_to(&candidate_dir, ctx.repo_root));
            ctx.diagnostics.push(ModulePathDiagnostic {
                code: "module-path-ambiguous-file-dir".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "module {} 同时存在 {} 和 {}，优先使用 .rs 文件",
                    decl.name, rel_file, rel_dir
                ),
                source_path: rel_file.clone(),
            });
        }

        let module_file = if file_exists {
            candidate_file
        } else if dir_exists {
            candidate_dir
        } else {
            let parent_rel = normalize_path(&path_relative_to(parent_file, ctx.repo_root));
            ctx.diagnostics.push(ModulePathDiagnostic {
                code: "module-path-file-missing".to_string(),
                severity: "warning".to_string(),
                message: format!("mod {} 声明存在但文件缺失于 {}", decl.name, parent_rel),
                source_path: parent_rel,
            });
            continue;
        };

        if ctx.visited.contains(&module_file) {
            continue;
        }
        ctx.visited.insert(module_file.clone());

        let module_path = if parent_module_path == "crate" {
            format!("crate::{}", decl.name)
        } else {
            format!("{}::{}", parent_module_path, decl.name)
        };

        let source_path = normalize_path(&path_relative_to(&module_file, ctx.repo_root));

        let confidence = if depth == 0 { 0.90 } else { 0.85 };
        let reason = if depth == 0 {
            "module-path-mod-resolved".to_string()
        } else {
            "module-path-chain-resolved".to_string()
        };

        ctx.entries.push(ModulePathEntry {
            source_path: source_path.clone(),
            module_path: module_path.clone(),
            confidence,
            reason,
        });
        ctx.lookup.insert(source_path.clone(), module_path.clone());

        scan_module_tree(ctx, &module_file, &module_path, depth + 1);
    }
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

/// 将路径分隔符统一为 /
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}
