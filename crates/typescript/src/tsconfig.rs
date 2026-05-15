//! tsconfig.json parsing with `extends` chain, `baseUrl`, and `paths` support.
//!
//! Handles JSONC via `strip_json5_comments` from manifest.rs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::manifest::strip_json5_comments;

/// Parsed tsconfig information relevant to module resolution.
#[derive(Debug, Clone)]
pub struct TsConfigInfo {
    /// Absolute path to this tsconfig file.
    pub path: PathBuf,
    /// `compilerOptions.rootDir` resolved absolute.
    pub root_dir: Option<PathBuf>,
    /// `compilerOptions.baseUrl` resolved absolute.
    pub base_url: Option<PathBuf>,
    /// `compilerOptions.paths` — alias pattern → list of relative targets.
    pub paths: BTreeMap<String, Vec<String>>,
    /// `extends` relative path (if present), already resolved to absolute.
    pub extends_path: Option<PathBuf>,
}

/// Error type for tsconfig operations.
#[derive(Debug, thiserror::Error)]
pub enum TsConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error in {path}: {err}")]
    Json {
        path: PathBuf,
        #[source]
        err: serde_json::Error,
    },
}

/// Load and parse a tsconfig.json file, resolving its `extends` chain.
pub fn load_tsconfig(path: &Path) -> Result<TsConfigInfo, TsConfigError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };

    let raw = std::fs::read_to_string(&path)?;
    let cleaned = strip_json5_comments(&raw);
    let parsed: serde_json::Value =
        serde_json::from_str(&cleaned).map_err(|err| TsConfigError::Json {
            path: path.clone(),
            err,
        })?;

    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();

    // Resolve extends chain: load parent first, then overlay child
    let extends_path = parsed.get("extends").and_then(|v| v.as_str()).map(|rel| {
        let mut candidate = dir.join(rel);
        if !candidate.extension().map(|e| e == "json").unwrap_or(false) {
            candidate.set_extension("json");
        }
        candidate
    });

    let (parent_base_url, parent_paths, parent_root_dir) =
        if let Some(ref parent_path) = extends_path {
            if let Ok(parent) = load_tsconfig(parent_path) {
                (parent.base_url, parent.paths, parent.root_dir)
            } else {
                (None, BTreeMap::new(), None)
            }
        } else {
            (None, BTreeMap::new(), None)
        };

    let compiler = parsed.get("compilerOptions").and_then(|v| v.as_object());

    // rootDir
    let child_root_dir = compiler
        .and_then(|c| c.get("rootDir"))
        .and_then(|v| v.as_str())
        .map(|r| dir.join(r));
    let root_dir = child_root_dir.or(parent_root_dir);

    // baseUrl — relative to the defining tsconfig's directory
    let child_base_url = compiler
        .and_then(|c| c.get("baseUrl"))
        .and_then(|v| v.as_str())
        .map(|b| dir.join(b));
    let base_url = child_base_url.or(parent_base_url);

    // paths — merge child over parent (same key → child wins)
    let mut paths = parent_paths;
    if let Some(paths_obj) = compiler
        .and_then(|c| c.get("paths"))
        .and_then(|v| v.as_object())
    {
        for (key, val) in paths_obj {
            let targets = val
                .as_array()
                .map(|arr| {
                    let mut items: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    items.sort();
                    items
                })
                .unwrap_or_default();
            paths.insert(key.clone(), targets);
        }
    }

    Ok(TsConfigInfo {
        path,
        root_dir,
        base_url,
        paths,
        extends_path,
    })
}

/// Discover all tsconfig files under a project root.
///
/// Finds `tsconfig.json` and `tsconfig.*.json` files at the root and
/// one level down (for monorepo packages).
pub fn discover_tsconfigs(project_root: &Path) -> Vec<TsConfigInfo> {
    let mut results = Vec::new();
    let mut candidates = Vec::new();

    // Root level
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "tsconfig.json" || name_str.starts_with("tsconfig.") {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    candidates.push(path);
                }
            }
        }
    }

    // One level down (packages/*)
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "dist" {
                continue;
            }
            if let Ok(sub_entries) = std::fs::read_dir(&path) {
                for sub in sub_entries.flatten() {
                    let name = sub.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str == "tsconfig.json" || name_str.starts_with("tsconfig.") {
                        let sub_path = sub.path();
                        if sub_path.extension().map(|e| e == "json").unwrap_or(false) {
                            candidates.push(sub_path);
                        }
                    }
                }
            }
        }
    }

    candidates.sort();
    for path in candidates {
        if let Ok(info) = load_tsconfig(&path) {
            results.push(info);
        }
    }

    results
}
