//! 生成 ProjectModelOutput
//!
//! 第一刀：manifest scanner 扫描 Cargo.toml，填充 packages/workspaces/targets。
//! 第二刀：source ownership scanner 扫描 .rs 文件，填充 sourceOwnership + stats。
//! 第三刀：root resolution scanner 解析 crate:: 路径，填充 rootResolution + stats。
//! stdout 只输出 JSON，human-readable logs 输出到 stderr。

use std::time::Instant;

use crate::calls;
use crate::diagnostic::{codes, Diagnostic};
use crate::graph;
use crate::imports;
use crate::item::{create_best_extractor, ItemExtractionInput};
use crate::manifest;
use crate::model::*;
use crate::root_resolution;
use crate::source;

pub fn inspect_project_model(root: &std::path::Path) -> ProjectModelOutput {
    inspect_project_model_with_options(root, false, false, false, false)
}

/// 带 symbol 提取选项的 inspect
pub fn inspect_project_model_with_symbols(
    root: &std::path::Path,
    include_symbols: bool,
) -> ProjectModelOutput {
    inspect_project_model_with_options(root, include_symbols, false, false, false)
}

/// 带全部选项的 inspect（symbol + graph + imports + calls）
///
/// v0.2 contract: 当 include_graph && include_calls 时，自动将 include_symbols 视为 true，
/// 以确保 CALLS edge 的 source/target symbol node 存在（graph edge endpoint integrity）。
pub fn inspect_project_model_with_options(
    root: &std::path::Path,
    mut include_symbols: bool,
    include_graph: bool,
    include_imports: bool,
    include_calls: bool,
) -> ProjectModelOutput {
    if include_graph {
        include_symbols = true;
    }
    let total_start = Instant::now();
    let root_display = root.display().to_string();

    let t0 = Instant::now();
    let scan = manifest::scan_manifests(root);
    let manifest_scan_ms = t0.elapsed().as_millis() as u64;

    let t0 = Instant::now();
    let source_result = source::scan_source_ownership(root, &scan.packages, &scan.targets);
    let source_ownership_ms = t0.elapsed().as_millis() as u64;

    let t0 = Instant::now();
    let queries = root_resolution::load_root_queries(root);
    let rr_result = root_resolution::scan_root_resolution(
        root,
        &source_result.source_ownership,
        &scan.targets,
        &queries,
    );
    let root_resolution_ms = t0.elapsed().as_millis() as u64;

    let mut all_diagnostics = scan.diagnostics;
    all_diagnostics.extend(source_result.diagnostics);
    all_diagnostics.extend(rr_result.diagnostics);
    let diagnostics_count = all_diagnostics.len() as u32;

    let t0 = Instant::now();
    let module_path_map = crate::module_path::build_module_path_map(
        root,
        &source_result.source_ownership,
        &scan.targets,
    );
    let module_path_map_ms = t0.elapsed().as_millis() as u64;

    let need_symbols = include_symbols || include_imports || include_calls;
    let t0 = Instant::now();
    let (symbols, symbol_diagnostics, symbol_count) = if need_symbols {
        let inputs = build_extraction_inputs(
            root,
            &source_result.source_ownership,
            &scan.packages,
            &module_path_map,
        );
        let result = if inputs.len() >= 8 {
            crate::item::extract_symbols_from_files_parallel(
                || crate::item::create_best_extractor(),
                &inputs,
            )
        } else {
            let extractor = create_best_extractor();
            crate::item::extract_symbols_from_files(&*extractor, &inputs)
        };
        let count = result.symbols.len() as u32;
        (result.symbols, result.diagnostics, count)
    } else {
        (vec![], vec![], 0u32)
    };
    let symbol_extraction_ms = t0.elapsed().as_millis() as u64;

    let need_imports = include_imports || include_calls;
    let t0 = Instant::now();
    let (import_list, import_diagnostics, import_count) = if need_imports {
        let result = imports::extract_and_resolve_imports(
            root,
            &source_result.source_ownership,
            &scan.targets,
            &module_path_map,
            &symbols,
        );
        let count = result.import_count;
        (result.imports, result.diagnostics, count)
    } else {
        (vec![], vec![], 0u32)
    };
    let import_resolution_ms = t0.elapsed().as_millis() as u64;

    let t0 = Instant::now();
    let (call_list, call_diags, call_count) = if include_calls {
        let result = calls::extract_and_resolve_calls(
            root,
            &source_result.source_ownership,
            &scan.targets,
            &scan.packages,
            &module_path_map,
            &symbols,
            &import_list,
        );
        let count = result.calls.len() as u32;
        (result.calls, result.diagnostics, count)
    } else {
        (vec![], vec![], 0u32)
    };
    let call_resolution_ms = t0.elapsed().as_millis() as u64;

    let t0 = Instant::now();
    let call_external_crate_total = call_list
        .iter()
        .filter(|c| c.call_kind == "external-crate")
        .count() as u32;
    let call_external_crate_classified =
        call_list.iter().filter(|c| c.known_crate.is_some()).count() as u32;
    let graph_assembly_ms = t0.elapsed().as_millis() as u64;

    let total_ms = total_start.elapsed().as_millis() as u64;

    let analysis_trace = crate::model::AnalysisTrace {
        manifest_scan_ms,
        source_ownership_ms,
        root_resolution_ms,
        module_path_map_ms,
        symbol_extraction_ms,
        import_resolution_ms,
        call_resolution_ms,
        graph_assembly_ms,
        serialization_ms: 0,
        total_ms,
        source_file_count: source_result.source_file_count,
        symbol_count,
        import_count,
        call_count,
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
            symbol_count: if include_symbols { symbol_count } else { 0 },
            import_count,
            call_count,
            call_external_crate_total,
            call_external_crate_classified,
        },
        symbols: if include_symbols { symbols } else { vec![] },
        symbol_diagnostics: if include_symbols {
            symbol_diagnostics
        } else {
            vec![]
        },
        imports: if include_imports { import_list } else { vec![] },
        import_diagnostics: if include_imports {
            import_diagnostics
        } else {
            vec![]
        },
        calls: call_list,
        call_diagnostics: call_diags,
        analysis_trace: Some(analysis_trace),
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
            import_count: 0,
            call_count: 0,
            call_external_crate_total: 0,
            call_external_crate_classified: 0,
        },
        symbols: vec![],
        symbol_diagnostics: vec![],
        imports: vec![],
        import_diagnostics: vec![],
        calls: vec![],
        call_diagnostics: vec![],
        analysis_trace: None,
    }
}

/// 从 sourceOwnership 构建提取输入
///
/// 只处理有 package owner 的 .rs 文件，跳过 outside-package 文件。
/// 读取文件内容用于 text-level 扫描。
/// 使用 ModulePathMap 为每个文件提供精确 modulePath。
fn build_extraction_inputs(
    root: &std::path::Path,
    source_ownership: &[SourceOwnership],
    packages: &[PackageModel],
    module_path_map: &crate::module_path::ModulePathMap,
) -> Vec<ItemExtractionInput> {
    let mut inputs = Vec::new();

    for so in source_ownership {
        let pkg_name = match &so.package {
            Some(p) => p.clone(),
            None => continue,
        };

        // 使用 ModulePathMap 查找精确 modulePath，fallback 到 "crate"
        let module_path = Some(module_path_map.get(&so.source_path).to_string());

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

/// 从 ProjectModelOutput 生成 GraphOutput
pub fn emit_graph_output(pm: &ProjectModelOutput) -> graph::GraphOutput {
    graph::emit_graph(pm)
}
