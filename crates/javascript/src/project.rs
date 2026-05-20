//! JavaScript project detection and source file discovery.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::manifest::JsManifest;

/// Kind of JavaScript project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsProjectKind {
    /// Standard JavaScript project (package.json with .js/.mjs/.cjs files)
    JavaScript,
    /// JSX project (React, inferred from .jsx files)
    Jsx,
}

/// Minimal JavaScript project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsProject {
    pub root: PathBuf,
    pub kind: JsProjectKind,
    pub manifest: Option<JsManifest>,
    pub source_files: Vec<PathBuf>,
}

/// JavaScript source file extensions.
const JS_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs"];

/// Walk up from `start` until a JavaScript project marker is found.
pub fn find_javascript_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if current.join("package.json").is_file() {
            if has_js_source_files(&current) {
                return Some(current);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Check if directory has JavaScript source files.
fn has_js_source_files(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !matches!(
                    name,
                    "node_modules" | "dist" | "build" | ".git" | ".cache" | "target"
                ) && has_js_source_files(&path)
                {
                    return true;
                }
            } else if is_js_source_file(&path) {
                return true;
            }
        }
    }
    false
}

/// Recursively list all JavaScript source files under `dir`.
pub fn list_source_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    list_source_files_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn list_source_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.')
                || matches!(
                    name,
                    "node_modules"
                        | "dist"
                        | "build"
                        | "out"
                        | ".output"
                        | "coverage"
                        | ".cache"
                        | "target"
                )
            {
                continue;
            }
            list_source_files_recursive(&path, files)?;
        } else if is_js_source_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

/// Check if a file path has a JavaScript-like extension.
pub fn is_js_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| JS_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Determine the project kind from source files.
pub fn detect_project_kind(root: &Path) -> JsProjectKind {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "jsx" {
                        return JsProjectKind::Jsx;
                    }
                }
            }
        }
    }
    JsProjectKind::JavaScript
}
