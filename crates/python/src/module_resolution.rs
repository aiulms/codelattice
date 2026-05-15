//! Python module resolution: maps files to module names and resolves imports.
//!
//! Supports src-layout (`src/pkg/...`) and flat-layout (`pkg/...`) projects.
//! Handles absolute imports, relative imports (level 1+), and re-exports
//! found in `__init__.py` files.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Index mapping Python module names to file paths and back.
#[derive(Debug, Clone)]
pub struct PythonModuleIndex {
    /// Absolute project root path.
    pub project_root: PathBuf,
    /// Package root directories (e.g., `src/` for src-layout, project root for flat-layout).
    pub package_roots: Vec<PathBuf>,
    /// module_name -> absolute file path (e.g., "shop.api" -> /path/src/shop/api.py)
    pub module_to_file: BTreeMap<String, PathBuf>,
    /// absolute file path -> module_name (reverse map)
    pub file_to_module: BTreeMap<PathBuf, String>,
    /// Re-exports found in __init__.py files.
    /// module_name -> list of ReExportInfo
    pub re_exports: HashMap<String, Vec<ReExportInfo>>,
}

/// A re-export found in an `__init__.py` file.
#[derive(Debug, Clone)]
pub struct ReExportInfo {
    /// Name being re-exported (e.g., "create_order")
    pub name: String,
    /// Source module where the name comes from (e.g., "shop.api")
    pub source_module: String,
    /// Source symbol name (same as name unless aliased)
    pub source_symbol: String,
}

/// Result of resolving an import.
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Module name that was resolved to.
    pub target_module: String,
    /// File path of the target module (if found).
    pub target_file: Option<PathBuf>,
    /// Specific symbol being imported (for from-imports).
    pub target_symbol: Option<String>,
    /// Alias (for as-imports).
    pub alias: Option<String>,
    /// Confidence score (0.0-1.0).
    pub confidence: f64,
    /// Human-readable reason string.
    pub reason: String,
}

/// Diagnostic for unresolvable imports.
#[derive(Debug, Clone)]
pub struct ImportDiagnostic {
    pub module_path: String,
    pub imported_name: Option<String>,
    pub line: usize,
    pub reason_code: String,
    pub message: String,
}

// Confidence constants
const CONFIDENCE_EXPLICIT_ABSOLUTE: f64 = 0.90;
const CONFIDENCE_RELATIVE: f64 = 0.85;
const CONFIDENCE_REEXPORT: f64 = 0.75;
const CONFIDENCE_PACKAGE_INIT: f64 = 0.80;

impl PythonModuleIndex {
    /// Build a module index from the project root and source files.
    pub fn build(project_root: &Path, source_files: &[PathBuf]) -> Self {
        // 1. Detect package roots
        let package_roots = detect_package_roots(project_root, source_files);

        // 2. Build module<->file mappings
        let mut module_to_file = BTreeMap::new();
        let mut file_to_module = BTreeMap::new();

        for file in source_files {
            if file.extension().and_then(|e| e.to_str()) != Some("py") {
                continue;
            }
            for pkg_root in &package_roots {
                if let Ok(rel) = file.strip_prefix(pkg_root) {
                    let module_name = path_to_module_name(rel);
                    module_to_file.insert(module_name.clone(), file.clone());
                    file_to_module.insert(file.clone(), module_name);
                    break; // first matching package root wins
                }
            }
        }

        // 3. Parse __init__.py files for re-exports
        let re_exports = extract_re_exports(source_files, &file_to_module);

        Self {
            project_root: project_root.to_path_buf(),
            package_roots,
            module_to_file,
            file_to_module,
            re_exports,
        }
    }

    /// Resolve an import statement to a target module/file.
    ///
    /// Parameters:
    /// - module_path: the module part of the import (e.g., "shop.models" or "" for `from . import X`)
    /// - imported_name: the symbol being imported (for from-imports), None for bare imports
    /// - alias: the alias (for as-imports)
    /// - level: number of dots in relative import (0 = absolute)
    /// - source_file: the file containing this import (for relative resolution)
    pub fn resolve_import(
        &self,
        module_path: &str,
        imported_name: Option<&str>,
        alias: Option<&str>,
        level: usize,
        source_file: &Path,
    ) -> Result<ResolvedImport, ImportDiagnostic> {
        // 1. Resolve relative level to absolute module name
        let absolute_module = if level > 0 {
            self.resolve_relative(module_path, level, source_file)?
        } else {
            module_path.to_string()
        };

        // 2. Try direct module match as file (e.g., "shop.api" -> shop/api.py)
        if let Some(target_file) = self.module_to_file.get(&absolute_module) {
            // Found the module as a file
            let is_init = target_file.file_name().and_then(|n| n.to_str()) == Some("__init__.py");

            // If this is an __init__.py and we have an imported_name, check re-exports
            if is_init && imported_name.is_some() {
                if let Some(resolved) = self.try_resolve_via_reexport(
                    &absolute_module,
                    imported_name,
                    alias,
                    source_file,
                ) {
                    return Ok(resolved);
                }
            }

            let confidence = if level > 0 {
                CONFIDENCE_RELATIVE
            } else {
                CONFIDENCE_EXPLICIT_ABSOLUTE
            };

            let reason = if level > 0 {
                format!(
                    "relative-import-level-{}: resolved to {}",
                    level, absolute_module
                )
            } else {
                format!("explicit-import: resolved to {}", absolute_module)
            };

            return Ok(ResolvedImport {
                target_module: absolute_module,
                target_file: Some(target_file.clone()),
                target_symbol: imported_name.map(|s| s.to_string()),
                alias: alias.map(|s| s.to_string()),
                confidence,
                reason,
            });
        }

        // 3. Try as a package (directory with __init__.py)
        //    e.g., "shop" might not be a file but a package directory
        //    Check if absolute_module has children in module_to_file
        let package_prefix = format!("{}.", absolute_module);
        let is_package = self
            .module_to_file
            .keys()
            .any(|k| k.starts_with(&package_prefix));

        if is_package {
            // It's a known package. Check re-exports if we have an imported_name
            if let Some(resolved) =
                self.try_resolve_via_reexport(&absolute_module, imported_name, alias, source_file)
            {
                return Ok(resolved);
            }

            // Package exists but no re-export for the symbol
            // Find the __init__.py for this package
            let init_module = format!("{}.__init__", absolute_module);
            let target_file = self
                .module_to_file
                .get(&absolute_module)
                .or_else(|| self.module_to_file.get(&init_module))
                .cloned();

            let confidence = if level > 0 {
                CONFIDENCE_RELATIVE
            } else {
                CONFIDENCE_PACKAGE_INIT
            };

            let reason = format!("package-import: resolved to package {}", absolute_module);

            return Ok(ResolvedImport {
                target_module: absolute_module,
                target_file,
                target_symbol: imported_name.map(|s| s.to_string()),
                alias: alias.map(|s| s.to_string()),
                confidence,
                reason,
            });
        }

        // 4. If imported_name is Some, try to resolve the symbol through re-exports
        //    of the parent module (e.g., "from shop import create_order" -> shop.__init__.py re-export)
        if let Some(name) = imported_name {
            if let Some(resolved) =
                self.try_resolve_via_reexport(&absolute_module, Some(name), alias, source_file)
            {
                return Ok(resolved);
            }
        }

        // 5. Unresolved
        Err(ImportDiagnostic {
            module_path: module_path.to_string(),
            imported_name: imported_name.map(|s| s.to_string()),
            line: 0,
            reason_code: "python-module-not-found".to_string(),
            message: format!(
                "module '{}' not found in project (resolved to '{}')",
                module_path, absolute_module
            ),
        })
    }

    /// Try to resolve an import through __init__.py re-exports.
    fn try_resolve_via_reexport(
        &self,
        module: &str,
        imported_name: Option<&str>,
        alias: Option<&str>,
        _source_file: &Path,
    ) -> Option<ResolvedImport> {
        let name = imported_name?;
        let exports = self.re_exports.get(module)?;

        for reexport in exports {
            if reexport.name == name {
                // Found re-export. Resolve source module to file.
                let target_file = self.module_to_file.get(&reexport.source_module).cloned();

                return Some(ResolvedImport {
                    target_module: reexport.source_module.clone(),
                    target_file,
                    target_symbol: Some(reexport.source_symbol.clone()),
                    alias: alias.map(|s| s.to_string()),
                    confidence: CONFIDENCE_REEXPORT,
                    reason: format!(
                        "re-export: {} re-exports {} from {}",
                        module, reexport.name, reexport.source_module
                    ),
                });
            }
        }

        None
    }

    /// Resolve a relative import to an absolute module name.
    fn resolve_relative(
        &self,
        module_path: &str,
        level: usize,
        source_file: &Path,
    ) -> Result<String, ImportDiagnostic> {
        // Get current file's module name
        let current_module =
            self.file_to_module
                .get(source_file)
                .ok_or_else(|| ImportDiagnostic {
                    module_path: module_path.to_string(),
                    imported_name: None,
                    line: 0,
                    reason_code: "python-relative-import-outside-package".to_string(),
                    message: format!("source file {:?} is not in a known package", source_file),
                })?;

        // Go up `level` segments from current module
        let segments: Vec<&str> = current_module.split('.').collect();
        // For __init__.py files, the module already represents the package,
        // so we need level-1 removals. For regular files, the module
        // represents the file within the package, so level removals.
        // Python semantics: from .X import Y (level=1) in shop/api.py
        //   means: go up 1 level from shop.api -> shop, then append X -> shop.X
        // from ..X import Y (level=2) in shop/utils/formatters.py
        //   means: go up 2 levels from shop.utils.formatters -> shop, then append X -> shop.X
        if level > segments.len() {
            return Err(ImportDiagnostic {
                module_path: module_path.to_string(),
                imported_name: None,
                line: 0,
                reason_code: "python-relative-import-outside-package".to_string(),
                message: format!(
                    "relative import level {} exceeds package depth {}",
                    level,
                    segments.len()
                ),
            });
        }

        let base_count = segments.len().saturating_sub(level);
        let base_segments = &segments[..base_count];
        let mut result = base_segments.join(".");
        if !module_path.is_empty() {
            if !result.is_empty() {
                result.push('.');
            }
            result.push_str(module_path);
        }

        if result.is_empty() {
            return Err(ImportDiagnostic {
                module_path: module_path.to_string(),
                imported_name: None,
                line: 0,
                reason_code: "python-relative-import-empty".to_string(),
                message: format!(
                    "relative import level {} with module_path '{}' resolved to empty",
                    level, module_path
                ),
            });
        }

        Ok(result)
    }
}

/// Detect package roots from project structure.
fn detect_package_roots(project_root: &Path, source_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // Check for src-layout: project_root/src/ contains dirs with __init__.py
    let src_dir = project_root.join("src");
    if src_dir.is_dir() {
        let has_package_in_src = source_files.iter().any(|f| {
            f.starts_with(&src_dir) && f.file_name().and_then(|n| n.to_str()) == Some("__init__.py")
        });
        if has_package_in_src {
            roots.push(src_dir);
            return roots; // src-layout takes priority
        }
    }

    // Flat-layout: project root is the package root
    roots.push(project_root.to_path_buf());
    roots
}

/// Convert a relative file path to a Python module name.
/// e.g., "shop/api.py" -> "shop.api"
/// e.g., "shop/__init__.py" -> "shop"
/// e.g., "shop/utils/formatters.py" -> "shop.utils.formatters"
fn path_to_module_name(rel_path: &Path) -> String {
    let path_str = rel_path.with_extension("");
    let path_str = path_str.to_string_lossy();

    // Handle both / and \ separators
    let path_str = path_str.replace('\\', ".");
    let path_str = path_str.replace('/', ".");

    // Remove trailing ".__init__" if present (won't exist after with_extension(""))
    // but check for ".__init__" segment in case the extension handling differs
    if let Some(stripped) = path_str.strip_suffix(".__init__") {
        return stripped.to_string();
    }
    if let Some(stripped) = path_str.strip_suffix(".__init") {
        return stripped.to_string();
    }
    path_str
}

/// Extract re-exports from __init__.py files.
///
/// Only parses simple forms: `from .module import name [as alias]`
/// Does NOT handle try/except, star, or conditional imports.
fn extract_re_exports(
    source_files: &[PathBuf],
    file_to_module: &BTreeMap<PathBuf, String>,
) -> HashMap<String, Vec<ReExportInfo>> {
    let mut result = HashMap::new();

    for file in source_files {
        if file.file_name().and_then(|n| n.to_str()) != Some("__init__.py") {
            continue;
        }
        let module_name = match file_to_module.get(file) {
            Some(m) => m.clone(),
            None => continue,
        };

        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut exports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            // Match: from .module import name
            // Match: from .module import name as alias
            // NOT: from . import * or try:/except: blocks
            if !trimmed.starts_with("from .") || trimmed.contains('*') {
                continue;
            }
            // Simple regex-free parsing
            if let Some(rest) = trimmed.strip_prefix("from ") {
                if let Some(import_idx) = rest.find(" import ") {
                    let module_part = &rest[..import_idx];
                    let names_part = &rest[import_idx + 8..]; // skip " import "

                    // Resolve module_part (e.g., ".api" -> "shop.api")
                    let dot_count = module_part.chars().take_while(|c| *c == '.').count();
                    let sub_module = &module_part[dot_count..];

                    // Build absolute source module
                    let mut segments: Vec<&str> = module_name.split('.').collect();
                    // First dot means current package, additional dots go up
                    if dot_count > 1 {
                        let remove = dot_count - 1;
                        if remove < segments.len() {
                            segments.truncate(segments.len() - remove);
                        }
                    }
                    if !sub_module.is_empty() {
                        for part in sub_module.split('.') {
                            segments.push(part);
                        }
                    }
                    let source_module = segments.join(".");

                    // Parse names: "name" or "name as alias"
                    for name_part in names_part.split(',') {
                        let name_part = name_part.trim();
                        if name_part.is_empty() {
                            continue;
                        }
                        // Handle parentheses that might appear in multi-line imports
                        let name_part = name_part.trim_start_matches('(').trim_end_matches(')');
                        if name_part.is_empty() {
                            continue;
                        }

                        if let Some(as_idx) = name_part.find(" as ") {
                            let name = name_part[..as_idx].trim();
                            if !name.is_empty() {
                                exports.push(ReExportInfo {
                                    name: name.to_string(),
                                    source_module: source_module.clone(),
                                    source_symbol: name.to_string(),
                                });
                            }
                        } else {
                            exports.push(ReExportInfo {
                                name: name_part.to_string(),
                                source_module: source_module.clone(),
                                source_symbol: name_part.to_string(),
                            });
                        }
                    }
                }
            }
        }

        if !exports.is_empty() {
            result.insert(module_name, exports);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_module_name_regular_file() {
        let path = Path::new("shop/api.py");
        assert_eq!(path_to_module_name(path), "shop.api");
    }

    #[test]
    fn path_to_module_name_init_file() {
        let path = Path::new("shop/__init__.py");
        assert_eq!(path_to_module_name(path), "shop");
    }

    #[test]
    fn path_to_module_name_nested_file() {
        let path = Path::new("shop/utils/formatters.py");
        assert_eq!(path_to_module_name(path), "shop.utils.formatters");
    }

    #[test]
    fn path_to_module_name_nested_init() {
        let path = Path::new("shop/utils/__init__.py");
        assert_eq!(path_to_module_name(path), "shop.utils");
    }

    #[test]
    fn path_to_module_name_top_level_file() {
        let path = Path::new("main.py");
        assert_eq!(path_to_module_name(path), "main");
    }

    #[test]
    fn relative_import_level_1_sibling() {
        // shop.api -> level=1, module_path="services" -> "shop.services"
        let mut idx = PythonModuleIndex {
            project_root: PathBuf::from("/project"),
            package_roots: vec![PathBuf::from("/project/src")],
            module_to_file: BTreeMap::new(),
            file_to_module: {
                let mut m = BTreeMap::new();
                m.insert(
                    PathBuf::from("/project/src/shop/api.py"),
                    "shop.api".to_string(),
                );
                m
            },
            re_exports: HashMap::new(),
        };
        idx.module_to_file.insert(
            "shop.api".to_string(),
            PathBuf::from("/project/src/shop/api.py"),
        );
        idx.module_to_file.insert(
            "shop.services".to_string(),
            PathBuf::from("/project/src/shop/services.py"),
        );

        let source_file = PathBuf::from("/project/src/shop/api.py");
        let result = idx.resolve_relative("services", 1, &source_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "shop.services");
    }

    #[test]
    fn relative_import_level_2_parent() {
        // shop.utils.formatters -> level=2, module_path="config" -> "shop.config"
        let mut idx = PythonModuleIndex {
            project_root: PathBuf::from("/project"),
            package_roots: vec![PathBuf::from("/project/src")],
            module_to_file: BTreeMap::new(),
            file_to_module: {
                let mut m = BTreeMap::new();
                m.insert(
                    PathBuf::from("/project/src/shop/utils/formatters.py"),
                    "shop.utils.formatters".to_string(),
                );
                m
            },
            re_exports: HashMap::new(),
        };
        idx.module_to_file.insert(
            "shop.utils.formatters".to_string(),
            PathBuf::from("/project/src/shop/utils/formatters.py"),
        );
        idx.module_to_file.insert(
            "shop.config".to_string(),
            PathBuf::from("/project/src/shop/config.py"),
        );

        let source_file = PathBuf::from("/project/src/shop/utils/formatters.py");
        let result = idx.resolve_relative("config", 2, &source_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "shop.config");
    }

    #[test]
    fn relative_import_level_1_empty_module_path() {
        // shop.api -> level=1, module_path="" -> "shop"
        let mut idx = PythonModuleIndex {
            project_root: PathBuf::from("/project"),
            package_roots: vec![PathBuf::from("/project/src")],
            module_to_file: BTreeMap::new(),
            file_to_module: {
                let mut m = BTreeMap::new();
                m.insert(
                    PathBuf::from("/project/src/shop/api.py"),
                    "shop.api".to_string(),
                );
                m
            },
            re_exports: HashMap::new(),
        };
        idx.module_to_file.insert(
            "shop".to_string(),
            PathBuf::from("/project/src/shop/__init__.py"),
        );

        let source_file = PathBuf::from("/project/src/shop/api.py");
        let result = idx.resolve_relative("", 1, &source_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "shop");
    }

    #[test]
    fn relative_import_exceeds_depth() {
        // shop.api -> level=5 is too many
        let idx = PythonModuleIndex {
            project_root: PathBuf::from("/project"),
            package_roots: vec![PathBuf::from("/project/src")],
            module_to_file: BTreeMap::new(),
            file_to_module: {
                let mut m = BTreeMap::new();
                m.insert(
                    PathBuf::from("/project/src/shop/api.py"),
                    "shop.api".to_string(),
                );
                m
            },
            re_exports: HashMap::new(),
        };

        let source_file = PathBuf::from("/project/src/shop/api.py");
        let result = idx.resolve_relative("foo", 5, &source_file);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().reason_code,
            "python-relative-import-outside-package"
        );
    }
}
