// workspace_inspect —— `codelattice inspect` 信封组装（多语言卡 2）。
//
// 分层：workspace-model 的 inspect_workspace_inventory 只产语言与证据；
// analyzable 表达"这台二进制现在能不能跑"，在本层按 feature 判定（冻结映射表）。
// 桶归属（projects / sourceOnlyAreas / unsupportedAreas）由 model 层按静态
// 语言支持表划分；feature 关闭不挪桶，只降 analyzable 并附 reason。

use gitnexus_workspace_model::{InspectionArea, InspectionEvidence};
use serde_json::{json, Value};
use std::path::Path;

/// analyzability 冻结映射（执行卡）：`.js` 走扫描器 typescript 口径跟 TS feature，
/// 不另开 javascript 行；表外语言恒 language-not-supported。
pub fn language_analyzable(language: &str) -> (bool, &'static str) {
    let enabled = match language {
        "rust" => cfg!(feature = "tree-sitter-extraction"),
        "shell" => true, // gitnexus-shell 非 optional
        "typescript" => cfg!(feature = "tree-sitter-typescript"),
        "python" => cfg!(feature = "tree-sitter-python"),
        "c" => cfg!(feature = "tree-sitter-c"),
        "cpp" => cfg!(feature = "tree-sitter-cpp"),
        "cangjie" => cfg!(feature = "tree-sitter-cangjie"),
        "arkts" => cfg!(feature = "tree-sitter-arkts"),
        _ => return (false, "language-not-supported"),
    };
    if enabled {
        (true, "")
    } else {
        (false, "language-support-disabled-in-this-binary")
    }
}

/// 体检行 → JSON。缺席语义：name/language/reason/recognition 可选字段
/// 不存在即未知，禁止写 ""/null。
fn area_json(area: &InspectionArea) -> Value {
    let evidence = match &area.evidence {
        InspectionEvidence::Manifest { file } => json!({"kind": "manifest", "file": file}),
        InspectionEvidence::ExtensionHistogram { extension, count } => {
            json!({"kind": "extension-histogram", "extension": extension, "count": count})
        }
    };
    let mut row = json!({
        "relativePath": area.relative_path,
        "confidence": area.confidence,
        "evidence": evidence,
        "sourceFileCount": area.source_file_count,
    });
    if let Some(name) = &area.name {
        row["name"] = json!(name);
    }
    if let Some(lang) = &area.language {
        row["language"] = json!(lang);
    }
    match area.recognition {
        None => {
            let (ok, reason) = language_analyzable(area.language.as_deref().unwrap_or(""));
            row["analyzable"] = json!(ok);
            if !ok {
                row["reason"] = json!(reason);
            }
        }
        Some("known-unsupported") => {
            row["analyzable"] = json!(false);
            row["reason"] = json!("language-not-supported");
            row["recognition"] = json!("known-unsupported");
        }
        Some("unrecognized") => {
            row["analyzable"] = json!(false);
            row["reason"] = json!("language-not-supported");
            row["recognition"] = json!("unrecognized");
        }
        Some(other) => {
            // model 层新增分档时的防守：未知的 recognition 不静默丢弃
            row["analyzable"] = json!(false);
            row["reason"] = json!("language-not-supported");
            row["recognition"] = json!(other);
        }
    }
    row
}

/// 组装 workspaceInspection.v1 信封。
pub fn build_inspection(root: &Path) -> Result<Value, String> {
    let insp = gitnexus_workspace_model::inspect_workspace_inventory(root)?;
    Ok(json!({
        "schemaVersion": "codelattice.workspaceInspection.v1",
        "root": root.display().to_string(),
        "generatedAt": crate::now_iso8601(),
        "generatedFrom": {
            "staticAnalysis": true,
            "projectContentRead": false,
            "scriptsExecuted": false,
        },
        "projects": insp.projects.iter().map(area_json).collect::<Vec<_>>(),
        "sourceOnlyAreas": insp.source_only_areas.iter().map(area_json).collect::<Vec<_>>(),
        "unsupportedAreas": insp.unsupported_areas.iter().map(area_json).collect::<Vec<_>>(),
        "cautions": [
            "Directory/manifest-level static scan only; no source content was read.",
            "Extension-histogram confidence is heuristic; language identity is not proof of analyzability.",
            "Nested suppression only applies to the manifest project's own language; other languages are reported separately.",
        ],
        "recommendedNextActions": [
            "Run `codelattice analyze --root <project> --language <lang> --format webui-snapshot` on an analyzable project row.",
            "Unsupported areas are visible for awareness only; this binary will not analyze them.",
        ],
    }))
}

/// inspect 子命令入口：MVP 仅 json。
pub fn run_inspect_command(root: &str, format: &str) {
    if format != "json" {
        eprintln!("错误：当前仅支持 --format json");
        std::process::exit(1);
    }
    let root_path = match crate::check_root(root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    match build_inspection(root_path) {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
