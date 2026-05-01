//! Cargo.toml manifest scanner
//!
//! 从 repo root 开始扫描 Cargo.toml，发现 package / workspace / target。
//! 只做 manifest-derived model，不执行 cargo metadata，不引入 rust-analyzer / tree-sitter。
//! sourceOwnership / rootResolution 暂不实现（第二刀）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::diagnostic::{codes, Diagnostic};
use crate::model::*;

/// TOML 反序列化结构：Cargo.toml 顶层
#[derive(Debug, serde::Deserialize, Default)]
struct CargoToml {
    package: Option<CargoPackage>,
    workspace: Option<CargoWorkspace>,
    features: Option<toml::value::Table>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct CargoPackage {
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct CargoWorkspace {
    members: Option<Vec<String>>,
}

/// scanner 结果
pub struct ScanResult {
    pub packages: Vec<PackageModel>,
    pub workspaces: Vec<WorkspaceModel>,
    pub targets: Vec<TargetModel>,
    pub diagnostics: Vec<Diagnostic>,
    pub manifest_count: u32,
    pub partial: bool,
}

/// 从 repo root 扫描 Cargo manifest
pub fn scan_manifests(root: &Path) -> ScanResult {
    let mut packages = Vec::new();
    let mut workspaces = Vec::new();
    let mut targets = Vec::new();
    let mut diagnostics = Vec::new();
    let mut manifest_count: u32 = 0;
    let mut partial = false;
    // 已发现的 package 根目录，防止重复
    let mut seen_package_dirs: HashSet<PathBuf> = HashSet::new();

    let root_cargo = root.join("Cargo.toml");

    if !root_cargo.exists() {
        // root Cargo.toml 不存在：输出 diagnostic 但仍扫描子目录
        diagnostics.push(Diagnostic {
            code: codes::CARGO_TOML_MISSING.to_string(),
            severity: "error".to_string(),
            message: "repo root 下未找到 Cargo.toml".to_string(),
            path: ".".to_string(),
            confidence: None,
            reason: None,
            related_paths: vec![],
            suggested_action: Some("确认 --root 指向包含 Cargo.toml 的目录".to_string()),
        });
        // 继续扫描子目录，可能发现 subdirectory packages
        scan_subdirectory_packages(
            root,
            root,
            &mut packages,
            &mut targets,
            &mut diagnostics,
            &mut manifest_count,
            &mut seen_package_dirs,
        );
        return ScanResult {
            packages,
            workspaces,
            targets,
            diagnostics,
            manifest_count,
            partial,
        };
    }

    // 解析 root Cargo.toml
    let cargo_toml = match parse_cargo_toml(&root_cargo) {
        Ok(t) => t,
        Err(d) => {
            diagnostics.push(d);
            return ScanResult {
                packages,
                workspaces,
                targets,
                diagnostics,
                manifest_count,
                partial: true,
            };
        }
    };

    manifest_count += 1;

    let has_package = cargo_toml.package.is_some();
    let has_workspace = cargo_toml.workspace.is_some();

    if has_workspace {
        // 处理 workspace
        let ws = cargo_toml.workspace.as_ref().unwrap();
        let raw_members: Vec<String> = ws.members.clone().unwrap_or_default();

        let mut expanded_members: Vec<String> = Vec::new();

        for member_pattern in &raw_members {
            if is_complex_glob(member_pattern) {
                // complex glob 不展开，输出 warning
                diagnostics.push(Diagnostic {
                    code: codes::COMPLEX_GLOB_UNSUPPORTED.to_string(),
                    severity: "warning".to_string(),
                    message: format!("complex glob 模式不支持: {member_pattern}"),
                    path: ".".to_string(),
                    confidence: None,
                    reason: None,
                    related_paths: vec![],
                    suggested_action: Some(
                        "使用简单 glob（如 crates/*）或显式列出成员".to_string(),
                    ),
                });
                partial = true;
                continue;
            }

            // 尝试 glob 展开
            let matched = expand_glob_member(root, member_pattern);
            if matched.is_empty() {
                // member 路径不存在，输出 warning（不是 fatal，其他 member 可能仍有效）
                diagnostics.push(Diagnostic {
                    code: codes::WORKSPACE_MEMBER_PATH_MISSING.to_string(),
                    severity: "warning".to_string(),
                    message: format!("workspace member 路径不存在: {member_pattern}"),
                    path: member_pattern.clone(),
                    confidence: None,
                    reason: None,
                    related_paths: vec![],
                    suggested_action: Some("检查 workspace.members 中的路径是否正确".to_string()),
                });
            }

            for member_dir in &matched {
                let member_cargo = member_dir.join("Cargo.toml");
                if !member_cargo.exists() {
                    // 目录存在但 Cargo.toml 不存在
                    let rel = path_relative_to(member_dir, root);
                    diagnostics.push(Diagnostic {
                        code: codes::WORKSPACE_MEMBER_PATH_MISSING.to_string(),
                        severity: "warning".to_string(),
                        message: format!("workspace member 缺少 Cargo.toml: {rel}"),
                        path: rel.clone(),
                        confidence: None,
                        reason: None,
                        related_paths: vec![],
                        suggested_action: Some("检查该目录是否为有效 Cargo package".to_string()),
                    });
                    continue;
                }

                let rel = path_relative_to(member_dir, root);
                expanded_members.push(rel.clone());

                let member_toml = match parse_cargo_toml(&member_cargo) {
                    Ok(t) => {
                        manifest_count += 1;
                        t
                    }
                    Err(d) => {
                        diagnostics.push(d);
                        continue;
                    }
                };

                let discovery = if member_pattern.contains('*') {
                    DiscoveryReason::WorkspaceGlob
                } else {
                    DiscoveryReason::WorkspaceExplicit
                };

                add_package_from_manifest(
                    &member_toml,
                    member_dir,
                    root,
                    true,
                    discovery,
                    &mut packages,
                    &mut targets,
                    &mut diagnostics,
                    &mut seen_package_dirs,
                );
            }
        }

        workspaces.push(WorkspaceModel {
            manifest_path: "Cargo.toml".to_string(),
            workspace_root: ".".to_string(),
            raw_members: raw_members.clone(),
            expanded_members,
        });
    }

    if has_package {
        // root 含 [package]，可能是 standalone 或 workspace root + own package
        add_package_from_manifest(
            &cargo_toml,
            root,
            root,
            has_workspace,
            DiscoveryReason::RootManifest,
            &mut packages,
            &mut targets,
            &mut diagnostics,
            &mut seen_package_dirs,
        );
    } else if !has_workspace {
        // root Cargo.toml 既无 [package] 也无 [workspace]：异常，但已解析
        diagnostics.push(Diagnostic {
            code: codes::PACKAGE_NAME_MISSING.to_string(),
            severity: "warning".to_string(),
            message: "root Cargo.toml 不含 [package] 或 [workspace]".to_string(),
            path: "Cargo.toml".to_string(),
            confidence: None,
            reason: None,
            related_paths: vec![],
            suggested_action: Some("确认 Cargo.toml 格式正确".to_string()),
        });
    }

    // 扫描 immediate subdirectories 发现非 workspace member 的 package
    // 只在 root 不是 virtual workspace 时扫描（virtual workspace 的 package 全由 members 声明）
    if !has_workspace || has_package {
        scan_subdirectory_packages(
            root,
            root,
            &mut packages,
            &mut targets,
            &mut diagnostics,
            &mut manifest_count,
            &mut seen_package_dirs,
        );
    }

    ScanResult {
        packages,
        workspaces,
        targets,
        diagnostics,
        manifest_count,
        partial,
    }
}

/// 解析单个 Cargo.toml 文件
fn parse_cargo_toml(path: &Path) -> Result<CargoToml, Diagnostic> {
    let content = std::fs::read_to_string(path).map_err(|e| Diagnostic {
        code: codes::CARGO_TOML_PARSE_ERROR.to_string(),
        severity: "error".to_string(),
        message: format!("无法读取 Cargo.toml: {e}"),
        path: path.to_string_lossy().to_string(),
        confidence: None,
        reason: None,
        related_paths: vec![],
        suggested_action: Some("检查文件权限和编码".to_string()),
    })?;

    toml::from_str(&content).map_err(|e| Diagnostic {
        code: codes::CARGO_TOML_PARSE_ERROR.to_string(),
        severity: "error".to_string(),
        message: format!("Cargo.toml 解析失败: {e}"),
        path: path.to_string_lossy().to_string(),
        confidence: None,
        reason: None,
        related_paths: vec![],
        suggested_action: Some("修正 Cargo.toml 语法错误".to_string()),
    })
}

/// 从已解析的 manifest 添加 package 和其 default targets
fn add_package_from_manifest(
    cargo_toml: &CargoToml,
    package_dir: &Path,
    root: &Path,
    is_workspace_member: bool,
    discovery_reason: DiscoveryReason,
    packages: &mut Vec<PackageModel>,
    targets: &mut Vec<TargetModel>,
    diagnostics: &mut Vec<Diagnostic>,
    seen: &mut HashSet<PathBuf>,
) {
    if seen.contains(package_dir) {
        return;
    }
    seen.insert(package_dir.to_path_buf());

    let pkg = match &cargo_toml.package {
        Some(p) => p,
        None => return,
    };

    let name = match &pkg.name {
        Some(n) => n.clone(),
        None => {
            diagnostics.push(Diagnostic {
                code: codes::PACKAGE_NAME_MISSING.to_string(),
                severity: "warning".to_string(),
                message: "package 缺少 name 字段".to_string(),
                path: path_relative_to(package_dir, root),
                confidence: None,
                reason: None,
                related_paths: vec![],
                suggested_action: Some("在 [package] 中添加 name 字段".to_string()),
            });
            "unknown".to_string()
        }
    };

    let manifest_rel = path_relative_to(&package_dir.join("Cargo.toml"), root);
    let package_root_rel = path_relative_to(package_dir, root);

    // 发现 default targets
    let src_dir = package_dir.join("src");
    let mut target_count: u32 = 0;

    // lib target: src/lib.rs
    let lib_rs = src_dir.join("lib.rs");
    if lib_rs.exists() {
        target_count += 1;
        targets.push(TargetModel {
            package_name: name.clone(),
            name: name.clone().replace('-', "_"),
            kind: TargetKind::Lib.as_str().to_string(),
            crate_root_file: format!("{package_root_rel}/src/lib.rs"),
            source_root_dir: format!("{package_root_rel}/src"),
        });
    } else {
        // target 文件不存在不阻止输出，只发 info diagnostic
        // 只有当没有其他 target 时才报告
        let main_rs = src_dir.join("main.rs");
        if !main_rs.exists() {
            diagnostics.push(Diagnostic {
                code: codes::TARGET_ROOT_MISSING.to_string(),
                severity: "info".to_string(),
                message: format!(
                    "package {name} 未找到 default target（src/lib.rs 或 src/main.rs）"
                ),
                path: format!("{package_root_rel}/src"),
                confidence: None,
                reason: None,
                related_paths: vec![],
                suggested_action: Some("创建 src/lib.rs 或 src/main.rs".to_string()),
            });
        }
    }

    // bin target: src/main.rs
    let main_rs = src_dir.join("main.rs");
    if main_rs.exists() {
        target_count += 1;
        targets.push(TargetModel {
            package_name: name.clone(),
            name: name.clone().replace('-', "_"),
            kind: TargetKind::Bin.as_str().to_string(),
            crate_root_file: format!("{package_root_rel}/src/main.rs"),
            source_root_dir: format!("{package_root_rel}/src"),
        });
    }

    // named bin targets: src/bin/*.rs
    if let Ok(entries) = std::fs::read_dir(src_dir.join("bin")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "rs") {
                if let Some(file_stem) = path.file_stem() {
                    let bin_name = file_stem.to_string_lossy().to_string();
                    target_count += 1;
                    targets.push(TargetModel {
                        package_name: name.clone(),
                        name: bin_name.clone(),
                        kind: TargetKind::Bin.as_str().to_string(),
                        crate_root_file: format!("{package_root_rel}/src/bin/{bin_name}.rs"),
                        source_root_dir: format!("{package_root_rel}/src"),
                    });
                }
            }
        }
    }

    // feature names
    let feature_names: Vec<String> = cargo_toml
        .features
        .as_ref()
        .map(|f| f.keys().cloned().collect())
        .unwrap_or_default();

    packages.push(PackageModel {
        name,
        manifest_path: manifest_rel,
        package_root: package_root_rel,
        target_count,
        feature_names,
        is_workspace_member,
        discovery_reason: discovery_reason.as_str().to_string(),
    });
}

/// 递归扫描子目录发现 package（支持嵌套 package 如 backend/tools/）
fn scan_subdirectory_packages(
    dir: &Path,
    base: &Path,
    packages: &mut Vec<PackageModel>,
    targets: &mut Vec<TargetModel>,
    diagnostics: &mut Vec<Diagnostic>,
    manifest_count: &mut u32,
    seen: &mut HashSet<PathBuf>,
) {
    let skip_dirs: HashSet<&str> = HashSet::from(["node_modules", ".git", "target", "fixtures"]);

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name() {
                if skip_dirs.contains(name.to_string_lossy().as_ref()) {
                    continue;
                }
            }
            if seen.contains(&path) {
                continue;
            }

            let cargo_toml = path.join("Cargo.toml");
            if cargo_toml.exists() {
                let toml_content = match parse_cargo_toml(&cargo_toml) {
                    Ok(t) => {
                        *manifest_count += 1;
                        t
                    }
                    Err(d) => {
                        diagnostics.push(d);
                        continue;
                    }
                };

                if toml_content.package.is_some() {
                    add_package_from_manifest(
                        &toml_content,
                        &path,
                        base,
                        false,
                        DiscoveryReason::SubdirectoryScan,
                        packages,
                        targets,
                        diagnostics,
                        seen,
                    );
                    // 递归扫描：package 子目录可能还有嵌套 package
                    scan_subdirectory_packages(
                        &path,
                        base,
                        packages,
                        targets,
                        diagnostics,
                        manifest_count,
                        seen,
                    );
                } else {
                    // 非 package 子目录也递归扫描
                    scan_subdirectory_packages(
                        &path,
                        base,
                        packages,
                        targets,
                        diagnostics,
                        manifest_count,
                        seen,
                    );
                }
            } else {
                // 无 Cargo.toml 的子目录也递归扫描
                scan_subdirectory_packages(
                    &path,
                    base,
                    packages,
                    targets,
                    diagnostics,
                    manifest_count,
                    seen,
                );
            }
        }
    }
}

/// 判断是否为 complex glob（含 **、{、} 等）
fn is_complex_glob(pattern: &str) -> bool {
    pattern.contains("**") || pattern.contains('{') || pattern.contains('}')
}

/// 展开简单 glob（只支持 `prefix/*` 形式）
fn expand_glob_member(root: &Path, pattern: &str) -> Vec<PathBuf> {
    if !pattern.contains('*') {
        // 非 glob，直接路径
        let dir = root.join(pattern);
        return if dir.is_dir() { vec![dir] } else { vec![] };
    }

    // 只处理 `prefix/*` 形式的 simple glob
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() != 2 {
        // 非 simple glob
        return vec![];
    }

    let prefix = parts[0];
    // 去掉尾部 /
    let prefix = prefix.trim_end_matches('/');
    let suffix = parts[1];
    // suffix 应为空（如 crates/*）或 /
    let suffix = suffix.trim_start_matches('/');

    let glob_dir = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };

    if !glob_dir.is_dir() {
        return vec![];
    }

    let mut result = Vec::new();
    let skip_dirs: HashSet<&str> = HashSet::from(["node_modules", ".git", "target", "fixtures"]);

    if let Ok(entries) = std::fs::read_dir(&glob_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name() {
                if skip_dirs.contains(name.to_string_lossy().as_ref()) {
                    continue;
                }
            }

            if suffix.is_empty() {
                // crates/*：每个子目录都是 candidate
                if path.join("Cargo.toml").exists() {
                    result.push(path);
                }
            } else {
                // suffix 非空，暂不处理
            }
        }
    }

    result
}

/// 计算 dir 相对于 base 的 POSIX 相对路径
fn path_relative_to(dir: &Path, base: &Path) -> String {
    match dir.strip_prefix(base) {
        Ok(rel) => {
            let s = rel.to_string_lossy().to_string();
            if s.is_empty() {
                ".".to_string()
            } else {
                // 统一为 POSIX 分隔符
                s.replace('\\', "/")
            }
        }
        Err(_) => dir.to_string_lossy().replace('\\', "/"),
    }
}
