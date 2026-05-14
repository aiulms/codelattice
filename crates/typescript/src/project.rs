//! TypeScript/TSX source file discovery and project root detection.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::manifest::TsManifest;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Kind of TypeScript-like project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TsProjectKind {
    /// Standard TypeScript project (has tsconfig.json or package.json).
    TypeScript,
    /// TSX project (React JSX, inferred from tsx files or jsx config).
    Tsx,
    /// HarmonyOS ArkTS project (has oh-package.json5).
    ArkTS,
}

/// Minimal TypeScript project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsProject {
    /// Absolute path to the project root.
    pub root: PathBuf,
    /// Kind of project detected.
    pub kind: TsProjectKind,
    /// Parsed manifest (if available).
    pub manifest: Option<TsManifest>,
    /// All TypeScript/TSX/ArkTS source files discovered.
    pub source_files: Vec<PathBuf>,
}

/// Metadata for a single package/module within a TypeScript project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsPackageInfo {
    /// Package name from package.json or inferred from directory.
    pub name: String,
    /// Root directory of this package relative to project root.
    pub relative_dir: String,
    /// Source files belonging to this package.
    pub source_files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Project root detection
// ---------------------------------------------------------------------------

/// Walk up from `start` until a recognizable project marker is found.
///
/// Markers (in priority order):
/// 1. `oh-package.json5` → ArkTS project
/// 2. `tsconfig.json` → TypeScript project
/// 3. `package.json` (with .ts/.tsx files) → TypeScript project
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        // ArkTS project marker (highest priority for HarmonyOS)
        if current.join("oh-package.json5").is_file() {
            return Some(current);
        }
        // TypeScript project markers
        if current.join("tsconfig.json").is_file() {
            return Some(current);
        }
        // Generic Node.js project with TypeScript files
        if current.join("package.json").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Walk up from `start` until a TypeScript-specific project marker is found.
///
/// Unlike `find_project_root`, this ignores `oh-package.json5` and only
/// recognizes `tsconfig.json` and `package.json` as markers.
/// Use this when `--language typescript` is explicitly specified.
pub fn find_typescript_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if current.join("tsconfig.json").is_file() {
            return Some(current);
        }
        if current.join("package.json").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Source file discovery
// ---------------------------------------------------------------------------

/// TypeScript/TSX/ArkTS source file extensions.
const TS_EXTENSIONS: &[&str] = &["ts", "tsx", "ets"];

/// Recursively list all TypeScript-like source files under `dir`.
///
/// Returns absolute paths. Skips `node_modules/`, `.git/`, `dist/`, `build/`,
/// and other common non-source directories.
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

        // Skip hidden directories and common non-source dirs
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
        } else if is_ts_source_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

/// Check if a file path has a TypeScript-like extension.
pub fn is_ts_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| TS_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Determine the project kind from the project root markers.
pub fn detect_project_kind(root: &Path) -> TsProjectKind {
    if root.join("oh-package.json5").is_file() {
        TsProjectKind::ArkTS
    } else {
        // Default to TypeScript; TSX is a runtime distinction based on file content
        TsProjectKind::TypeScript
    }
}
