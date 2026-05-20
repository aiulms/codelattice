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

/// JavaScript manifest (parsed package.json).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsManifest {
    pub name: String,
    pub version: String,
    pub main: Option<String>,
    pub module: Option<String>,
    pub exports: Option<HashMap<String, serde_json::Value>>,
    pub bin: Option<HashMap<String, String>>,
    pub scripts: Option<HashMap<String, String>>,
    pub dependencies: Option<HashMap<String, String>>,
    pub dev_dependencies: Option<HashMap<String, String>>,
}

/// Parse package.json at the given path.
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

    let exports = json
        .get("exports")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let bin = json
        .get("bin")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let scripts = json
        .get("scripts")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let dependencies = json
        .get("dependencies")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let dev_dependencies = json
        .get("devDependencies")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    Ok(JsManifest {
        name,
        version,
        main,
        module,
        exports,
        bin,
        scripts,
        dependencies,
        dev_dependencies,
    })
}
