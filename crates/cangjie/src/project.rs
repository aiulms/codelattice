use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

use crate::manifest::{CangjieManifest, CangjieManifestError, WorkspaceManifest};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Minimal Cangjie project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjieProject {
    /// Absolute path to the project root (directory containing cjpm.toml).
    pub root: PathBuf,
    /// Parsed root cjpm.toml manifest.
    pub manifest: CangjieManifest,
    /// All packages in the project (workspace members, or single root package).
    pub packages: Vec<CangjiePackageInfo>,
    /// All Cangjie source files (.cj) discovered under src-dirs.
    pub source_files: Vec<PathBuf>,
}

/// Metadata for a single package within a Cangjie project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CangjiePackageInfo {
    /// Package name from [package].name.
    pub name: String,
    /// Module directory relative to project root (empty string for root package).
    pub module_dir: String,
    /// Source directory relative to module_dir.
    pub src_dir: String,
    /// Package version.
    pub version: Option<String>,
    /// cjc-version constraint.
    pub cjc_version: Option<String>,
    /// output-type.
    pub output_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Project root detection
// ---------------------------------------------------------------------------

/// Walk up from `start` until a `cjpm.toml` is found.
///
/// Returns the directory containing cjpm.toml, or `None` if not found
/// before reaching the filesystem root.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if current.join("cjpm.toml").is_file() {
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

/// Cangjie source file extension.
const CJ_EXTENSION: &str = "cj";

/// Recursively list all `.cj` files under `dir`.
///
/// Returns absolute paths. Non-recursable directories are silently skipped.
pub fn list_source_files(dir: &Path) -> Result<Vec<PathBuf>, io::Error> {
    let mut files = Vec::new();
    collect_cj_files(dir, &mut files)?;
    Ok(files)
}

fn collect_cj_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), io::Error> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // skip unreadable dirs
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // Skip hidden dirs and common non-source dirs
        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            // Skip common build/cache dirs
            if file_name == "target" || file_name == ".cache" || file_name == ".generated" {
                continue;
            }
            collect_cj_files(&path, out)?;
        } else if path.extension().map(|e| e == CJ_EXTENSION).unwrap_or(false) {
            out.push(path);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Project model builder
// ---------------------------------------------------------------------------

/// Build a minimal `CangjieProject` from a workspace manifest.
///
/// `root` is the directory containing the workspace-level cjpm.toml.
/// Discovers all source files under each package's src-dir.
pub fn build_project_model(root: &Path) -> Result<CangjieProject, CangjieManifestError> {
    let ws: WorkspaceManifest = crate::manifest::resolve_workspace_manifest(root)?;

    let mut packages = Vec::new();
    let mut source_files = Vec::new();

    // If root has a [package], add it
    if let Some(ref pkg) = ws.root.package {
        if let Some(ref name) = pkg.name {
            if !name.is_empty() {
                let src_dir_path = root.join(&pkg.src_dir);
                if let Ok(sf) = list_source_files(&src_dir_path) {
                    source_files.extend(sf);
                }
                packages.push(CangjiePackageInfo {
                    name: name.clone(),
                    module_dir: String::new(),
                    src_dir: pkg.src_dir.clone(),
                    version: pkg.version.clone(),
                    cjc_version: pkg.cjc_version.clone(),
                    output_type: pkg.output_type.clone(),
                });
            }
        }
    }

    // Add workspace members
    for (member_dir, member_manifest) in &ws.members {
        if let Some(ref pkg) = member_manifest.package {
            if let Some(ref name) = pkg.name {
                if !name.is_empty() {
                    let src_dir_path = root.join(member_dir).join(&pkg.src_dir);
                    if let Ok(sf) = list_source_files(&src_dir_path) {
                        source_files.extend(sf);
                    }
                    packages.push(CangjiePackageInfo {
                        name: name.clone(),
                        module_dir: member_dir.clone(),
                        src_dir: pkg.src_dir.clone(),
                        version: pkg.version.clone(),
                        cjc_version: pkg.cjc_version.clone(),
                        output_type: pkg.output_type.clone(),
                    });
                }
            }
        }
    }

    Ok(CangjieProject {
        root: root.to_path_buf(),
        manifest: ws.root,
        packages,
        source_files,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_project_root_finds_workspace_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("cjpm-workspace");
        let root = find_project_root(&fixture).unwrap();
        assert!(root.join("cjpm.toml").exists());
    }

    #[test]
    fn find_project_root_from_subdirectory_finds_nearest() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("cjpm-workspace")
            .join("pkg1");
        // pkg1 has its own cjpm.toml, so find_project_root returns pkg1
        let root = find_project_root(&fixture).unwrap();
        assert!(root.join("cjpm.toml").exists());
        assert!(root.ends_with("pkg1"));
    }

    #[test]
    fn find_project_root_returns_none_at_filesystem_root() {
        let root = find_project_root(Path::new("/"));
        assert!(root.is_none());
    }

    #[test]
    fn list_source_files_finds_cj_files() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("cjpm-basic")
            .join("src");
        let files = list_source_files(&fixture).unwrap();
        assert!(!files.is_empty());
        for f in &files {
            assert!(f.extension().unwrap() == "cj");
        }
    }

    #[test]
    fn list_source_files_empty_for_nonexistent_dir() {
        let files = list_source_files(Path::new("/nonexistent/path")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn build_project_model_single_package() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("cjpm-basic");
        let project = build_project_model(&root).unwrap();
        assert_eq!(project.packages.len(), 1);
        assert_eq!(project.packages[0].name, "basic");
        assert_eq!(project.packages[0].src_dir, "src");
        assert!(!project.source_files.is_empty());
        // main.cj should be found
        let has_main = project
            .source_files
            .iter()
            .any(|f| f.to_string_lossy().contains("main.cj"));
        assert!(has_main);
    }

    #[test]
    fn build_project_model_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("cjpm-workspace");
        let project = build_project_model(&root).unwrap();
        assert_eq!(project.packages.len(), 2);
        let names: Vec<&str> = project.packages.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"pkg1"));
        assert!(names.contains(&"pkg2"));
    }
}
