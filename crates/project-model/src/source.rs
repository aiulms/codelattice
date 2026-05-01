//! Source 文件归属扫描器
//!
//! 扫描 repo root 下所有 .rs 文件，判断每个文件属于哪个 package / target。
//! 本轮只做 path-based ownership，不解析 mod 声明，不建 module reachability graph。
//! rootResolution 留空（第三刀实现 module resolution）。
//!
//! ownership 策略：
//! - .rs 文件必须在某个 package root 下才可能有 package owner
//! - nearest package root wins（nested package 场景）
//! - 不在任何 package root 下 → source-outside-package diagnostic
//! - target 归属：src/lib.rs → lib, src/main.rs → bin, src/bin/foo.rs → named bin
//! - 共享模块：单 target package 归入唯一 target，多 target package 标 ambiguous
//! - outside package 用 info diagnostic 而不是 fatal error，因为不阻止 inspect 输出

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::{codes, Diagnostic};
use crate::model::*;

/// source ownership 扫描结果
pub struct SourceScanResult {
    pub source_ownership: Vec<SourceOwnership>,
    pub diagnostics: Vec<Diagnostic>,
    pub source_file_count: u32,
    pub owned_file_count: u32,
    pub unowned_file_count: u32,
}

/// 扫描 repo root 下所有 .rs 文件，判定归属
pub fn scan_source_ownership(
    root: &Path,
    packages: &[PackageModel],
    targets: &[TargetModel],
) -> SourceScanResult {
    // 收集所有 .rs 文件
    let mut rs_files: Vec<PathBuf> = Vec::new();
    let skip_dirs: HashSet<&str> =
        HashSet::from(["target", ".git", "node_modules", "vendor", "fixtures"]);
    collect_rs_files(root, root, &skip_dirs, &mut rs_files);

    // 构建 package root → package name 映射（按路径长度降序，nearest first）
    let mut package_roots: Vec<(PathBuf, String)> = packages
        .iter()
        .map(|p| {
            let pkg_root = if p.package_root == "." {
                root.to_path_buf()
            } else {
                root.join(&p.package_root)
            };
            (pkg_root, p.name.clone())
        })
        .collect();
    package_roots.sort_by(|a, b| b.0.cmp(&a.0));

    let mut source_ownership = Vec::new();
    let mut diagnostics = Vec::new();
    let mut source_file_count: u32 = 0;
    let mut owned_file_count: u32 = 0;
    let mut unowned_file_count: u32 = 0;

    for rs_file in &rs_files {
        source_file_count += 1;

        // 查找 nearest package root
        let pkg_match = find_nearest_package(rs_file, &package_roots);

        match pkg_match {
            Some((pkg_name, pkg_root_path, is_nearest)) => {
                // 验证文件在 package root/src/ 下（Rust 约定：只有 src/ 下才是 source）
                let pkg_src = pkg_root_path.join("src");
                if !rs_file.starts_with(&pkg_src) {
                    // 文件在 package root 下但不在 src/ 下，视为 outside-package
                    unowned_file_count += 1;
                    let rel = path_relative_to(rs_file, root);
                    diagnostics.push(Diagnostic {
                        code: codes::SOURCE_OUTSIDE_PACKAGE.to_string(),
                        severity: "info".to_string(),
                        message: format!("source file 不在 package src/ 目录下: {rel}"),
                        path: rel.clone(),
                        confidence: Some(0.30),
                        reason: Some("source-outside-package".to_string()),
                        related_paths: vec![],
                        suggested_action: Some("检查文件是否应移入 package src/ 目录".to_string()),
                    });
                    source_ownership.push(SourceOwnership {
                        source_path: rel,
                        package: None,
                        target: None,
                        ownership_reason: "source-outside-package".to_string(),
                        confidence: 0.30,
                    });
                    continue;
                }

                owned_file_count += 1;

                // 查找 target 归属
                let target_match = find_target_for_file(
                    rs_file,
                    root,
                    pkg_root_path,
                    &pkg_name,
                    targets,
                    packages,
                );

                let (target_name, reason, confidence) = match target_match {
                    TargetMatchResult::ExactTarget {
                        name,
                        reason,
                        confidence,
                    } => (Some(name), reason, confidence),
                    TargetMatchResult::SingleTarget {
                        name,
                        reason,
                        confidence,
                    } => (Some(name), reason, confidence),
                    TargetMatchResult::AmbiguousTarget {
                        name,
                        reason,
                        confidence,
                    } => {
                        diagnostics.push(Diagnostic {
                            code: codes::SOURCE_TARGET_AMBIGUOUS.to_string(),
                            severity: "warning".to_string(),
                            message: format!(
                                "package {pkg_name} 有多个 target，无法确定 {rs_file_rel} 的 target 归属",
                                rs_file_rel = path_relative_to(rs_file, root)
                            ),
                            path: path_relative_to(rs_file, root),
                            confidence: Some(confidence),
                            reason: Some(reason.to_string()),
                            related_paths: vec![],
                            suggested_action: Some(
                                "实现 module reachability 后可精确判定 target 归属".to_string(),
                            ),
                        });
                        (name, reason.to_string(), confidence)
                    }
                    TargetMatchResult::NoTarget { reason, confidence } => {
                        diagnostics.push(Diagnostic {
                            code: codes::SOURCE_TARGET_MISSING.to_string(),
                            severity: "warning".to_string(),
                            message: format!(
                                "package {pkg_name} 无 target，{rs_file_rel} 无法归属 target",
                                rs_file_rel = path_relative_to(rs_file, root)
                            ),
                            path: path_relative_to(rs_file, root),
                            confidence: Some(confidence),
                            reason: Some(reason.to_string()),
                            related_paths: vec![],
                            suggested_action: Some(
                                "确认 package 有有效的 target（src/lib.rs 或 src/main.rs）"
                                    .to_string(),
                            ),
                        });
                        (None, reason.to_string(), confidence)
                    }
                };

                let pkg_reason = if is_nearest {
                    "source-owned-by-nearest-package-root"
                } else {
                    "source-owned-by-package-root"
                };

                // 如果 reason 已包含 target 信息，使用 target reason；否则使用 package reason
                let final_reason = if reason.starts_with("source-owned-by-")
                    || reason == "source-target-ambiguous"
                    || reason == "source-target-missing"
                {
                    reason
                } else {
                    pkg_reason.to_string()
                };

                source_ownership.push(SourceOwnership {
                    source_path: path_relative_to(rs_file, root),
                    package: Some(pkg_name.clone()),
                    target: target_name,
                    ownership_reason: final_reason,
                    confidence,
                });
            }
            None => {
                // 文件不在任何 package root 下
                unowned_file_count += 1;
                let rel = path_relative_to(rs_file, root);
                diagnostics.push(Diagnostic {
                    code: codes::SOURCE_OUTSIDE_PACKAGE.to_string(),
                    severity: "info".to_string(),
                    message: format!("source file 不在任何 package root 下: {rel}"),
                    path: rel.clone(),
                    confidence: Some(0.30),
                    reason: Some("source-outside-package".to_string()),
                    related_paths: vec![],
                    suggested_action: Some("检查文件是否应移入 package src/ 目录".to_string()),
                });
                source_ownership.push(SourceOwnership {
                    source_path: rel,
                    package: None,
                    target: None,
                    ownership_reason: "source-outside-package".to_string(),
                    confidence: 0.30,
                });
            }
        }
    }

    // 按 source_path 排序确保输出稳定
    source_ownership.sort_by(|a, b| a.source_path.cmp(&b.source_path));

    SourceScanResult {
        source_ownership,
        diagnostics,
        source_file_count,
        owned_file_count,
        unowned_file_count,
    }
}

/// target 匹配结果
enum TargetMatchResult {
    ExactTarget {
        name: String,
        reason: String,
        confidence: f32,
    },
    SingleTarget {
        name: String,
        reason: String,
        confidence: f32,
    },
    AmbiguousTarget {
        name: Option<String>,
        reason: String,
        confidence: f32,
    },
    NoTarget {
        reason: String,
        confidence: f32,
    },
}

/// 递归收集 .rs 文件
fn collect_rs_files(dir: &Path, root: &Path, skip_dirs: &HashSet<&str>, result: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if skip_dirs.contains(name.to_string_lossy().as_ref()) {
                        continue;
                    }
                }
                // 不跟随 symlink
                if path.is_symlink() {
                    continue;
                }
                collect_rs_files(&path, root, skip_dirs, result);
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                result.push(path);
            }
        }
    }
}

/// 查找 nearest package root
fn find_nearest_package<'a>(
    file: &Path,
    package_roots: &'a [(PathBuf, String)],
) -> Option<(String, &'a PathBuf, bool)> {
    let mut found: Option<(String, &PathBuf, bool)> = None;
    let mut match_count = 0;

    for (pkg_root, pkg_name) in package_roots {
        if file.starts_with(pkg_root) {
            match_count += 1;
            // package_roots 已按路径长度降序排列，第一个匹配即为 nearest
            if found.is_none() {
                found = Some((pkg_name.clone(), pkg_root, match_count > 1));
            }
        }
    }

    // 如果有多个匹配，标记为 nearest
    if let Some((name, root, _)) = found {
        found = Some((name, root, match_count > 1));
    }

    found
}

/// 查找文件所属 target
fn find_target_for_file(
    file: &Path,
    _repo_root: &Path,
    pkg_root: &PathBuf,
    pkg_name: &str,
    all_targets: &[TargetModel],
    _all_packages: &[PackageModel],
) -> TargetMatchResult {
    // 文件相对于 package root 的路径
    let rel_to_pkg = file.strip_prefix(pkg_root).unwrap_or(file);
    let rel_str = rel_to_pkg.to_string_lossy().replace('\\', "/");

    // 检查是否为 target root 文件
    // src/lib.rs → lib target
    if rel_str == "src/lib.rs" {
        return TargetMatchResult::ExactTarget {
            name: pkg_name.replace('-', "_"),
            reason: "source-owned-by-lib-target-root".to_string(),
            confidence: 0.90,
        };
    }

    // src/main.rs → default bin target
    if rel_str == "src/main.rs" {
        return TargetMatchResult::ExactTarget {
            name: pkg_name.replace('-', "_"),
            reason: "source-owned-by-bin-target-root".to_string(),
            confidence: 0.90,
        };
    }

    // src/bin/foo.rs → named bin target
    if let Some(rest) = rel_str.strip_prefix("src/bin/") {
        if rest.ends_with(".rs") && !rest.contains('/') {
            let bin_name = rest.trim_end_matches(".rs");
            return TargetMatchResult::ExactTarget {
                name: bin_name.to_string(),
                reason: "source-owned-by-named-bin-target-root".to_string(),
                confidence: 0.90,
            };
        }
        // src/bin/foo/main.rs → named bin target (optional, 极低成本顺手包含)
        if let Some(main_rest) = rest.strip_suffix("/main.rs") {
            if !main_rest.contains('/') {
                return TargetMatchResult::ExactTarget {
                    name: main_rest.to_string(),
                    reason: "source-owned-by-named-bin-target-root".to_string(),
                    confidence: 0.90,
                };
            }
        }
    }

    // 非 target root 文件：检查 package 有多少个 target
    let pkg_targets: Vec<&TargetModel> = all_targets
        .iter()
        .filter(|t| t.package_name == pkg_name)
        .collect();

    if pkg_targets.is_empty() {
        // package 无 target
        return TargetMatchResult::NoTarget {
            reason: "source-target-missing".to_string(),
            confidence: 0.50,
        };
    }

    if pkg_targets.len() == 1 {
        // 单 target package，归入唯一 target
        let t = pkg_targets[0];
        let reason = format!("source-owned-by-{}-target-root", t.kind);
        return TargetMatchResult::SingleTarget {
            name: t.name.clone(),
            reason,
            confidence: 0.80,
        };
    }

    // 多 target package，不猜 target（不做 mod graph traversal）
    TargetMatchResult::AmbiguousTarget {
        name: None,
        reason: "source-target-ambiguous".to_string(),
        confidence: 0.50,
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
