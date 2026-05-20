//! package.json manifest parsing for JavaScript projects.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JsManifestError {
    #[error("failed to read package.json: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("failed to parse package.json: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsManifest {
    pub name: String,
    pub version: String,
    pub main: Option<String>,
    pub module: Option<String>,
    pub browser: Option<String>,
    pub types: Option<String>,
    pub typings: Option<String>,
    pub exports: Option<HashMap<String, serde_json::Value>>,
    pub bin: Option<serde_json::Value>,
    pub scripts: Option<HashMap<String, String>>,
    pub dependencies: Option<HashMap<String, String>>,
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub entry_points: Vec<JsEntryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsEntryPoint {
    pub field: String,
    pub path: String,
    pub kind: JsEntryPointKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsEntryPointKind {
    Main,
    Module,
    Browser,
    Bin,
    Exports,
    Types,
}

pub fn parse_package_json(path: &Path) -> Result<JsManifest, JsManifestError> {
    let content = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    let main = json.get("main").and_then(|v| v.as_str()).map(String::from);
    let module = json
        .get("module")
        .and_then(|v| v.as_str())
        .map(String::from);
    let browser = json
        .get("browser")
        .and_then(|v| v.as_str())
        .map(String::from);
    let types = json.get("types").and_then(|v| v.as_str()).map(String::from);
    let typings = json
        .get("typings")
        .and_then(|v| v.as_str())
        .map(String::from);

    let exports = json
        .get("exports")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let bin = json.get("bin").cloned();

    let scripts = json
        .get("scripts")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let dependencies = json
        .get("dependencies")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let dev_dependencies = json
        .get("devDependencies")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mut entry_points = Vec::new();
    if let Some(ref p) = main {
        entry_points.push(JsEntryPoint {
            field: "main".to_string(),
            path: p.clone(),
            kind: JsEntryPointKind::Main,
        });
    }
    if let Some(ref p) = module {
        entry_points.push(JsEntryPoint {
            field: "module".to_string(),
            path: p.clone(),
            kind: JsEntryPointKind::Module,
        });
    }
    if let Some(ref p) = browser {
        entry_points.push(JsEntryPoint {
            field: "browser".to_string(),
            path: p.clone(),
            kind: JsEntryPointKind::Browser,
        });
    }
    if let Some(ref p) = types {
        entry_points.push(JsEntryPoint {
            field: "types".to_string(),
            path: p.clone(),
            kind: JsEntryPointKind::Types,
        });
    }
    if let Some(ref p) = typings {
        entry_points.push(JsEntryPoint {
            field: "typings".to_string(),
            path: p.clone(),
            kind: JsEntryPointKind::Types,
        });
    }
    if let Some(ref bin_val) = bin {
        match bin_val {
            serde_json::Value::String(s) => {
                entry_points.push(JsEntryPoint {
                    field: "bin".to_string(),
                    path: s.clone(),
                    kind: JsEntryPointKind::Bin,
                });
            }
            serde_json::Value::Object(map) => {
                for (_name, val) in map {
                    if let Some(s) = val.as_str() {
                        entry_points.push(JsEntryPoint {
                            field: "bin".to_string(),
                            path: s.to_string(),
                            kind: JsEntryPointKind::Bin,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(ref exports_map) = exports {
        for (key, val) in exports_map {
            let path_str = resolve_export_value(val);
            if let Some(p) = path_str {
                entry_points.push(JsEntryPoint {
                    field: format!("exports.{}", key),
                    path: p,
                    kind: JsEntryPointKind::Exports,
                });
            }
        }
    }

    Ok(JsManifest {
        name,
        version,
        main,
        module,
        browser,
        types,
        typings,
        exports,
        bin,
        scripts,
        dependencies,
        dev_dependencies,
        entry_points,
    })
}

fn resolve_export_value(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => {
            for key in &["default", "import", "require", "node", "browser"] {
                if let Some(v) = map.get(*key) {
                    if let Some(s) = resolve_export_value(v) {
                        return Some(s);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// 检测 JS 项目是否使用特定框架（heuristic）。
pub fn detect_framework_hints(manifest: &JsManifest) -> Vec<JsFrameworkHint> {
    let mut hints = Vec::new();
    let deps = manifest
        .dependencies
        .as_ref()
        .into_iter()
        .chain(manifest.dev_dependencies.as_ref().into_iter())
        .flat_map(|m| m.keys());

    let dep_names: Vec<&String> = deps.collect();

    for name in &dep_names {
        match name.as_str() {
            "react" | "react-dom" => {
                hints.push(JsFrameworkHint {
                    framework: "react".to_string(),
                    confidence: 0.85,
                    reason: "dependency-detected".to_string(),
                });
            }
            "next" => {
                hints.push(JsFrameworkHint {
                    framework: "nextjs".to_string(),
                    confidence: 0.90,
                    reason: "dependency-detected".to_string(),
                });
            }
            "express" => {
                hints.push(JsFrameworkHint {
                    framework: "express".to_string(),
                    confidence: 0.90,
                    reason: "dependency-detected".to_string(),
                });
            }
            "koa" => {
                hints.push(JsFrameworkHint {
                    framework: "koa".to_string(),
                    confidence: 0.90,
                    reason: "dependency-detected".to_string(),
                });
            }
            "fastify" => {
                hints.push(JsFrameworkHint {
                    framework: "fastify".to_string(),
                    confidence: 0.90,
                    reason: "dependency-detected".to_string(),
                });
            }
            "vite" => {
                hints.push(JsFrameworkHint {
                    framework: "vite".to_string(),
                    confidence: 0.80,
                    reason: "devDependency-detected".to_string(),
                });
            }
            "webpack" | "webpack-cli" => {
                hints.push(JsFrameworkHint {
                    framework: "webpack".to_string(),
                    confidence: 0.80,
                    reason: "devDependency-detected".to_string(),
                });
            }
            _ => {}
        }
    }

    if let Some(ref scripts) = manifest.scripts {
        for (_key, val) in scripts {
            if val.contains("next ") || val.contains("next build") || val.contains("next dev") {
                if !hints.iter().any(|h| h.framework == "nextjs") {
                    hints.push(JsFrameworkHint {
                        framework: "nextjs".to_string(),
                        confidence: 0.70,
                        reason: "script-hint".to_string(),
                    });
                }
            }
            if val.contains("vite") {
                if !hints.iter().any(|h| h.framework == "vite") {
                    hints.push(JsFrameworkHint {
                        framework: "vite".to_string(),
                        confidence: 0.70,
                        reason: "script-hint".to_string(),
                    });
                }
            }
        }
    }

    hints.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hints.dedup_by(|a, b| a.framework == b.framework);
    hints
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsFrameworkHint {
    pub framework: String,
    pub confidence: f64,
    pub reason: String,
}
