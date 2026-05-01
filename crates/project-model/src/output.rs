//! 生成 ProjectModelOutput
//!
//! 第一刀实现：调用 manifest scanner 扫描 Cargo.toml，填充 packages/workspaces/targets。
//! sourceOwnership / rootResolution 暂时为空数组（第二刀实现）。
//! stdout 只输出 JSON，human-readable logs 输出到 stderr。

use crate::diagnostic::{codes, Diagnostic};
use crate::manifest;
use crate::model::*;

/// 从 repo root 执行 manifest scan 并生成完整 ProjectModelOutput
pub fn inspect_project_model(root: &std::path::Path) -> ProjectModelOutput {
    let root_display = root.display().to_string();
    let scan = manifest::scan_manifests(root);

    let diagnostics_count = scan.diagnostics.len() as u32;

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
        source_ownership: vec![],
        root_resolution: vec![],
        diagnostics: scan.diagnostics,
        partial: scan.partial,
        warnings: vec![],
        stats: Stats {
            source_file_count: 0,
            owned_file_count: 0,
            unowned_file_count: 0,
            resolution_success_count: 0,
            resolution_fail_count: 0,
        },
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
        },
    }
}
