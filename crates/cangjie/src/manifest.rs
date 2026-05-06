use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parsed cjpm.toml manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjieManifest {
    #[serde(default)]
    pub package: Option<CangjiePackage>,
    #[serde(default)]
    pub workspace: Option<CangjieWorkspace>,
    #[serde(default)]
    pub dependencies: Vec<CangjieDependency>,
}

/// [package] section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjiePackage {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(rename = "cjc-version", default)]
    pub cjc_version: Option<String>,
    #[serde(rename = "src-dir", default = "default_src_dir")]
    pub src_dir: String,
    #[serde(rename = "output-type", default)]
    pub output_type: Option<String>,
}

fn default_src_dir() -> String {
    "src".to_string()
}

/// [workspace] section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjieWorkspace {
    pub members: Vec<String>,
    #[serde(rename = "build-members", default)]
    pub build_members: Option<Vec<String>>,
}

/// A single dependency entry from [dependencies] etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjieDependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum CangjieManifestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Missing package name")]
    MissingPackageName,
}

// ---------------------------------------------------------------------------
// Dependency section parser
// ---------------------------------------------------------------------------

/// A dependency value can be a simple version string or an inline table.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DepValue {
    Simple(String),
    Table {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        git: Option<String>,
    },
}

fn parse_deps(table: Option<&toml::Value>) -> Vec<CangjieDependency> {
    let Some(toml::Value::Table(table)) = table else {
        return vec![];
    };

    table
        .iter()
        .filter_map(|(name, value)| {
            let dep: DepValue = toml::Value::try_into(value.clone()).ok()?;
            let dep = match dep {
                DepValue::Simple(version) => CangjieDependency {
                    name: name.clone(),
                    path: None,
                    version: Some(version),
                    git: None,
                },
                DepValue::Table { path, version, git } => CangjieDependency {
                    name: name.clone(),
                    path,
                    version,
                    git,
                },
            };
            Some(dep)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Main parser
// ---------------------------------------------------------------------------

/// Intermediate struct for deserializing the parts we can do with serde.
#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default)]
    package: Option<CangjiePackage>,
    #[serde(default)]
    workspace: Option<CangjieWorkspace>,
}

/// Parse a cjpm.toml string into structured metadata.
///
/// Handles:
/// - `[package]` with name, version, cjc-version, src-dir, output-type
/// - `[workspace]` with members, build-members
/// - `[dependencies]` with simple string or inline table values
///
/// # Examples
///
/// ```
/// use gitnexus_cangjie::manifest::parse_cjpm_toml;
///
/// let toml = r#"
/// [package]
/// name = "myapp"
/// version = "0.1.0"
/// "#;
/// let manifest = parse_cjpm_toml(toml).unwrap();
/// assert_eq!(manifest.package.unwrap().name.as_deref(), Some("myapp"));
/// ```
pub fn parse_cjpm_toml(source: &str) -> Result<CangjieManifest, CangjieManifestError> {
    let root: toml::Value = toml::from_str(source)?;

    // Deserialize [package] and [workspace] from the root table
    let raw: RawManifest = toml::Value::try_into(root.clone())?;

    let mut package = raw.package;
    let mut workspace = raw.workspace;

    // Validate [package]: name must be non-empty; if absent or empty, treat package as absent
    if let Some(ref pkg) = package {
        if pkg.name.as_deref().unwrap_or("").is_empty() {
            package = None;
        }
    }

    // Validate [workspace]: members must be non-empty
    if let Some(ref ws) = workspace {
        if ws.members.is_empty() {
            workspace = None;
        }
    }

    let dependencies = parse_deps(root.get("dependencies"));

    Ok(CangjieManifest {
        package,
        workspace,
        dependencies,
    })
}

/// Load and parse cjpm.toml from a file path.
pub fn load_cjpm_manifest(path: &Path) -> Result<CangjieManifest, CangjieManifestError> {
    let content = std::fs::read_to_string(path)?;
    parse_cjpm_toml(&content)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_package() {
        let toml = r#"
[package]
name = "myapp"
version = "0.1.0"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        let pkg = m.package.unwrap();
        assert_eq!(pkg.name.as_deref(), Some("myapp"));
        assert_eq!(pkg.version.as_deref(), Some("0.1.0"));
        assert_eq!(pkg.src_dir, "src"); // default
        assert!(m.workspace.is_none());
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn parse_basic_package_with_src_dir() {
        let toml = r#"
[package]
name = "mylib"
src-dir = "source"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        let pkg = m.package.unwrap();
        assert_eq!(pkg.name.as_deref(), Some("mylib"));
        assert_eq!(pkg.src_dir, "source");
        assert_eq!(pkg.version, None);
    }

    #[test]
    fn parse_full_package_like_cjgui() {
        let toml = r#"
[package]
cjc-version = "1.1.0"
name = "cjgui"
version = "0.0.0"
output-type = "static"
src-dir = "src"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        let pkg = m.package.unwrap();
        assert_eq!(pkg.name.as_deref(), Some("cjgui"));
        assert_eq!(pkg.cjc_version.as_deref(), Some("1.1.0"));
        assert_eq!(pkg.output_type.as_deref(), Some("static"));
    }

    #[test]
    fn parse_package_with_cjc_version() {
        let toml = r#"
[package]
name = "mylib"
cjc-version = "1.0.5"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        let pkg = m.package.unwrap();
        assert_eq!(pkg.cjc_version.as_deref(), Some("1.0.5"));
    }

    #[test]
    fn parse_simple_version_dependency() {
        let toml = r#"
[package]
name = "myapp"

[dependencies]
serde = "1.0"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "serde");
        assert_eq!(m.dependencies[0].version.as_deref(), Some("1.0"));
        assert!(m.dependencies[0].path.is_none());
    }

    #[test]
    fn parse_path_dependency() {
        let toml = r#"
[package]
name = "myapp"

[dependencies]
mylib = { path = "../mylib", version = "0.1.0" }
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "mylib");
        assert_eq!(m.dependencies[0].path.as_deref(), Some("../mylib"));
        assert_eq!(m.dependencies[0].version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn parse_workspace() {
        let toml = r#"
[workspace]
members = ["pkg1", "pkg2"]
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert!(m.package.is_none());
        let ws = m.workspace.unwrap();
        assert_eq!(ws.members, vec!["pkg1", "pkg2"]);
    }

    #[test]
    fn parse_workspace_with_build_members() {
        let toml = r#"
[workspace]
members = ["pkg1", "pkg2", "tests"]
build-members = ["pkg1", "pkg2"]
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        let ws = m.workspace.unwrap();
        assert_eq!(ws.members, vec!["pkg1", "pkg2", "tests"]);
        assert_eq!(
            ws.build_members.as_deref(),
            Some(&["pkg1".to_string(), "pkg2".to_string()][..])
        );
    }

    #[test]
    fn parse_empty_workspace_returns_none() {
        let toml = r#"
[workspace]
members = []
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert!(m.workspace.is_none());
    }

    #[test]
    fn parse_missing_package_name_yields_none() {
        let toml = r#"
[package]
version = "0.1.0"
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert!(m.package.is_none(), "package without name should be treated as absent");
    }

    #[test]
    fn parse_malformed_toml_returns_error() {
        let err = parse_cjpm_toml("this is not valid toml {{{").unwrap_err();
        assert!(matches!(err, CangjieManifestError::Toml(_)));
    }

    #[test]
    fn parse_empty_dependencies() {
        let toml = r#"
[package]
name = "myapp"

[dependencies]
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn parse_git_dependency() {
        let toml = r#"
[package]
name = "myapp"

[dependencies]
mylib = { git = "https://example.com/repo", version = "1.0" }
"#;
        let m = parse_cjpm_toml(toml).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(
            m.dependencies[0].git.as_deref(),
            Some("https://example.com/repo")
        );
    }
}
