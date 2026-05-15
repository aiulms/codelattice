//! Manifest parsing for TypeScript projects.
//!
//! Supports:
//! - `oh-package.json5` (HarmonyOS ArkTS)
//! - `tsconfig.json` (TypeScript)
//! - `package.json` (Node.js / npm)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parsed TypeScript project manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsManifest {
    /// Project name (from package.json name field, or directory name).
    pub name: String,
    /// Project version (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Detected manifest kind.
    pub kind: TsManifestKind,
    /// Source directories (from tsconfig include, or default ["src"]).
    pub source_dirs: Vec<String>,
    /// Dependencies (from package.json or oh-package.json5).
    #[serde(default)]
    pub dependencies: Vec<TsDependency>,
}

/// Kind of manifest file detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TsManifestKind {
    /// oh-package.json5 (HarmonyOS ArkTS).
    OhPackageJson5,
    /// package.json (Node.js / npm).
    PackageJson,
    /// tsconfig.json (TypeScript compiler config).
    TsconfigJson,
}

/// A single dependency entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsDependency {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum TsManifestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No manifest file found in {0}")]
    NotFound(PathBuf),
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load the appropriate manifest from a project root directory.
///
/// Priority: oh-package.json5 > package.json > tsconfig.json
pub fn load_ts_manifest(root: &Path) -> Result<TsManifest, TsManifestError> {
    // Try oh-package.json5 first (ArkTS)
    if root.join("oh-package.json5").is_file() {
        return parse_oh_package_json5(&root.join("oh-package.json5"));
    }

    // Try package.json
    if root.join("package.json").is_file() {
        return parse_package_json(&root.join("package.json"));
    }

    // Try tsconfig.json
    if root.join("tsconfig.json").is_file() {
        return parse_tsconfig_json(&root.join("tsconfig.json"));
    }

    Err(TsManifestError::NotFound(root.to_path_buf()))
}

/// Parse an `oh-package.json5` file.
///
/// These files use JSON5 syntax (comments, trailing commas). We do a simple
/// pre-processing pass to strip comments before JSON parsing.
pub fn parse_oh_package_json5(path: &Path) -> Result<TsManifest, TsManifestError> {
    let content = std::fs::read_to_string(path)?;
    let cleaned = strip_json5_comments(&content);
    let parsed: serde_json::Value = serde_json::from_str(&cleaned)?;

    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
        )
        .to_string();

    let version = parsed
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);

    let dependencies = extract_dependencies(&parsed);

    Ok(TsManifest {
        name,
        version,
        kind: TsManifestKind::OhPackageJson5,
        source_dirs: vec!["entry/src/main/ets".to_string()],
        dependencies,
    })
}

/// Parse a `package.json` file.
pub fn parse_package_json(path: &Path) -> Result<TsManifest, TsManifestError> {
    let content = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&content)?;

    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
        )
        .to_string();

    let version = parsed
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut deps = Vec::new();

    // Extract from dependencies and devDependencies
    for key in &["dependencies", "devDependencies"] {
        if let Some(obj) = parsed.get(key).and_then(|v| v.as_object()) {
            for (k, v) in obj {
                deps.push(TsDependency {
                    name: k.clone(),
                    version: v.as_str().map(String::from),
                    path: None,
                });
            }
        }
    }

    Ok(TsManifest {
        name,
        version,
        kind: TsManifestKind::PackageJson,
        source_dirs: vec!["src".to_string()],
        dependencies: deps,
    })
}

/// Parse a `tsconfig.json` file (minimal — extract include paths only).
pub fn parse_tsconfig_json(path: &Path) -> Result<TsManifest, TsManifestError> {
    let content = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&content)?;

    let source_dirs = parsed
        .get("include")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["src".to_string()]);

    let name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(TsManifest {
        name,
        version: None,
        kind: TsManifestKind::TsconfigJson,
        source_dirs,
        dependencies: vec![],
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract dependencies from a JSON value (looks for "dependencies" and "devDependencies").
fn extract_dependencies(parsed: &serde_json::Value) -> Vec<TsDependency> {
    let mut deps = Vec::new();
    for key in &["dependencies", "devDependencies", "overridesDependencies"] {
        if let Some(obj) = parsed.get(key).and_then(|v| v.as_object()) {
            for (k, v) in obj {
                deps.push(TsDependency {
                    name: k.clone(),
                    version: v.as_str().map(String::from),
                    path: None,
                });
            }
        }
    }
    deps
}

/// Strip single-line (// ...) and multi-line (/* ... */) comments from JSON5.
///
/// This is a simple heuristic that handles most common JSON5 patterns.
/// It does not handle comments inside string literals perfectly, but works
/// well for typical oh-package.json5 files.
pub fn strip_json5_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if in_string {
            result.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                result.push(chars[i]);
            } else if c == string_char {
                in_string = false;
            }
        } else {
            match c {
                '"' | '\'' => {
                    in_string = true;
                    string_char = c;
                    result.push(c);
                }
                '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                    // Single-line comment — skip to end of line
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
                '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                    // Multi-line comment — skip to */
                    i += 2;
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        i += 1;
                    }
                    i += 2; // skip */
                    continue;
                }
                _ => {
                    result.push(c);
                }
            }
        }
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_json5_single_line_comments() {
        let input = r#"{ "name": "foo", // comment
        "version": "1.0" }"#;
        let cleaned = strip_json5_comments(input);
        assert!(!cleaned.contains("comment"));
        assert!(cleaned.contains(r#""name": "foo""#));
    }

    #[test]
    fn strip_json5_multi_line_comments() {
        let input = r#"{ "name": /* inline */ "foo" }"#;
        let cleaned = strip_json5_comments(input);
        assert!(!cleaned.contains("inline"));
        assert!(cleaned.contains(r#""name":  "foo""#));
    }
}
