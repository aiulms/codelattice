//! 生成 ProjectModelOutput
//!
//! 第一刀：manifest scanner 扫描 Cargo.toml，填充 packages/workspaces/targets。
//! 第二刀：source ownership scanner 扫描 .rs 文件，填充 sourceOwnership + stats。
//! 第三刀：root resolution scanner 解析 crate:: 路径，填充 rootResolution + stats。
//! stdout 只输出 JSON，human-readable logs 输出到 stderr。

use crate::diagnostic::{codes, Diagnostic};
use crate::item::{create_best_extractor, ItemExtractionInput};
use crate::manifest;
use crate::model::*;
use crate::root_resolution;
use crate::source;

pub fn inspect_project_model(root: &std::path::Path) -> ProjectModelOutput {
    inspect_project_model_with_symbols(root, false)
}

/// 带 symbol 提取选项的 inspect
pub fn inspect_project_model_with_symbols(
    root: &std::path::Path,
    include_symbols: bool,
) -> ProjectModelOutput {
    let root_display = root.display().to_string();
    let scan = manifest::scan_manifests(root);

    // 第二刀：基于 manifest scanner 结果扫描 source ownership
    let source_result = source::scan_source_ownership(root, &scan.packages, &scan.targets);

    // 第三刀：基于 source ownership + targets 执行 root resolution
    let queries = root_resolution::load_root_queries(root);
    let rr_result = root_resolution::scan_root_resolution(
        root,
        &source_result.source_ownership,
        &scan.targets,
        &queries,
    );

    // 合并 diagnostics
    let mut all_diagnostics = scan.diagnostics;
    all_diagnostics.extend(source_result.diagnostics);
    all_diagnostics.extend(rr_result.diagnostics);
    let diagnostics_count = all_diagnostics.len() as u32;

    // item/symbol 提取：第三刀使用 best extractor（tree-sitter 优先，fallback 到 text）
    let (symbols, symbol_diagnostics, symbol_count) = if include_symbols {
        let extractor = create_best_extractor();
        let inputs = build_extraction_inputs(root, &source_result.source_ownership, &scan.packages);
        let result = crate::item::extract_symbols_from_files(&*extractor, &inputs);
        let count = result.symbols.len() as u32;
        (result.symbols, result.diagnostics, count)
    } else {
        (vec![], vec![], 0u32)
    };

    ProjectModelOutput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: "project-model inspect".to_string(),
        repo_root: root_display,
        generated_at: generate_timestamp(),
        project_model: ProjectModelSummary {
            manifest_count: scan.manifest_count,
            package_count: scan.packages.len() as u32,
            workspace_count: scan.workspaces.len() as u32,
            diagnostics_count,
        },
        packages: scan.packages,
        workspaces: scan.workspaces,
        targets: scan.targets,
        source_ownership: source_result.source_ownership,
        root_resolution: rr_result.root_resolution,
        diagnostics: all_diagnostics,
        partial: scan.partial,
        warnings: vec![],
        stats: Stats {
            source_file_count: source_result.source_file_count,
            owned_file_count: source_result.owned_file_count,
            unowned_file_count: source_result.unowned_file_count,
            resolution_success_count: rr_result.resolution_success_count,
            resolution_fail_count: rr_result.resolution_fail_count,
            symbol_count,
        },
        symbols,
        symbol_diagnostics,
    }
}

/// 返回合法 ISO 8601 占位值。
/// 当前不引入时间库，generatedAt 是 runtime-only 字段，使用稳定占位值。
fn generate_timestamp() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

/// 生成 stub 输出（保留向后兼容，供无 Cargo.toml 场景 fallback）
pub fn generate_stub_output(repo_root: &str) -> ProjectModelOutput {
    let scan_not_implemented = Diagnostic {
        code: codes::SCAN_NOT_IMPLEMENTED.to_string(),
        severity: "info".to_string(),
        message: "当前输出为工程骨架 stub，尚未执行 Cargo manifest 扫描".to_string(),
        path: repo_root.to_string(),
        confidence: None,
        reason: None,
        related_paths: vec![],
        suggested_action: Some("等待 ProjectModel parser 实现后重新运行".to_string()),
    };

    ProjectModelOutput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: "project-model inspect".to_string(),
        repo_root: repo_root.to_string(),
        generated_at: generate_timestamp(),
        project_model: ProjectModelSummary {
            manifest_count: 0,
            package_count: 0,
            workspace_count: 0,
            diagnostics_count: 1,
        },
        packages: vec![],
        workspaces: vec![],
        targets: vec![],
        source_ownership: vec![],
        root_resolution: vec![],
        diagnostics: vec![scan_not_implemented],
        partial: false,
        warnings: vec![],
        stats: Stats {
            source_file_count: 0,
            owned_file_count: 0,
            unowned_file_count: 0,
            resolution_success_count: 0,
            resolution_fail_count: 0,
            symbol_count: 0,
        },
        symbols: vec![],
        symbol_diagnostics: vec![],
    }
}

/// 从 sourceOwnership 构建提取输入
///
/// 只处理有 package owner 的 .rs 文件，跳过 outside-package 文件。
/// 读取文件内容用于 text-level 扫描。
fn build_extraction_inputs(
    root: &std::path::Path,
    source_ownership: &[SourceOwnership],
    packages: &[PackageModel],
) -> Vec<ItemExtractionInput> {
    let mut inputs = Vec::new();

    for so in source_ownership {
        // 跳过无 package owner 的文件
        let pkg_name = match &so.package {
            Some(p) => p.clone(),
            None => continue,
        };

        let module_path = Some("crate".to_string());

        // 使用绝对路径读取文件内容
        let abs_path = root.join(&so.source_path);
        let source_text = match std::fs::read_to_string(&abs_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        inputs.push(ItemExtractionInput {
            source_path: so.source_path.clone(),
            source_text,
            package_name: pkg_name,
            target_name: so.target.clone(),
            module_path,
        });
    }

    let _ = packages;
    inputs
}
