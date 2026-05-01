//! 生成 stub ProjectModelOutput
//!
//! 当前不执行 Cargo manifest 扫描，所有 facts 为空。
//! diagnostics 显式声明 scan-not-implemented，避免消费者误以为输出是完整分析结果。

use crate::diagnostic::Diagnostic;
use crate::model::*;

/// 生成 stub 输出。
/// 当前 inspect 是 stub，不执行 Cargo scan，因为 ProjectModel parser 尚未实现。
/// diagnostics 必须显式输出 scan-not-implemented，让消费者知道输出不是真实分析结果。
pub fn generate_stub_output(repo_root: &str) -> ProjectModelOutput {
    let scan_not_implemented = Diagnostic {
        code: "project-model-scan-not-implemented".to_string(),
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
        generated_at: stub_generated_at_iso8601(),
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
        // partial 为 false：当前没有执行任何分析，不是"部分分析"而是"未分析"
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

/// 返回合法 ISO 8601 占位值。
/// 当前工程骨架不引入时间库，generatedAt 又是 runtime-only 字段，
/// 因此先使用稳定占位值，避免输出看似 ISO 但实际不可解析的时间字符串。
fn stub_generated_at_iso8601() -> String {
    "1970-01-01T00:00:00Z".to_string()
}
