//! TypeScript module resolution: resolves import specifiers to files.
//!
//! Supports:
//! - Relative imports (`./foo`, `../bar`) with extension/index resolution
//! - tsconfig `paths` exact and wildcard (`*`) matches
//! - Monorepo workspace package imports (via root `package.json` `workspaces`)
//! - External package detection (no edge, diagnostic only)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::manifest::strip_json5_comments;
use crate::tsconfig::{discover_tsconfigs, TsConfigInfo};

/// How an import specifier was resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum TsResolutionKind {
    /// Relative import resolved to a file (e.g. `./foo` → `foo.ts`).
    RelativeFile,
    /// Relative import resolved to an index file (e.g. `./dir` → `dir/index.ts`).
    RelativeIndex,
    /// tsconfig paths exact match (e.g. `@shared` → `packages/shared/src/index.ts`).
    TsConfigPathExact,
    /// tsconfig paths wildcard match (e.g. `@core/logger` → `packages/app/src/core/logger.ts`).
    TsConfigPathWildcard,
    /// Workspace package root import (e.g. `@pkg/shared` → `packages/shared/src/index.ts`).
    WorkspacePackage,
    /// Workspace package with subpath (e.g. `@pkg/shared/format` → `packages/shared/src/format.ts`).
    WorkspacePackageSubpath,
    /// External npm package (not indexed, no edge).
    External,
    /// Import could not be resolved to any file.
    Unresolved,
}

impl std::fmt::Display for TsResolutionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelativeFile => write!(f, "relative-file"),
            Self::RelativeIndex => write!(f, "relative-index"),
            Self::TsConfigPathExact => write!(f, "tsconfig-path-exact"),
            Self::TsConfigPathWildcard => write!(f, "tsconfig-path-wildcard"),
            Self::WorkspacePackage => write!(f, "workspace-package"),
            Self::WorkspacePackageSubpath => write!(f, "workspace-package-subpath"),
            Self::External => write!(f, "external"),
            Self::Unresolved => write!(f, "unresolved"),
        }
    }
}

/// Result of resolving a TypeScript import specifier.
#[derive(Debug, Clone)]
pub struct ResolvedTsImport {
    /// Original specifier string.
    pub specifier: String,
    /// Resolved absolute file path (if found).
    pub target_file: Option<PathBuf>,
    /// Module name (for workspace/tsconfig alias imports).
    pub target_module: Option<String>,
    /// How the import was resolved.
    pub resolution_kind: TsResolutionKind,
    /// Confidence score (0.0–1.0), None for external/unresolved.
    pub confidence: Option<f64>,
    /// Human-readable reason.
    pub reason: String,
}

// Confidence constants
const CONFIDENCE_RELATIVE: f64 = 0.90;
const CONFIDENCE_EXACT: f64 = 0.90;
const CONFIDENCE_WILDCARD: f64 = 0.85;
const CONFIDENCE_WORKSPACE: f64 = 0.80;
const CONFIDENCE_WORKSPACE_SUBPATH: f64 = 0.75;

/// TypeScript module resolver.
///
/// Pre-builds indexes from tsconfig paths and workspace packages,
/// then resolves import specifiers to files.
#[derive(Debug, Clone)]
pub struct TsModuleResolver {
    /// Absolute project root.
    pub project_root: PathBuf,
    /// Discovered tsconfig files with merged paths.
    pub tsconfigs: Vec<TsConfigInfo>,
    /// Workspace packages: package name → package root directory.
    pub workspace_packages: BTreeMap<String, PathBuf>,
    /// All known source files: normalized absolute path.
    pub known_files: BTreeMap<PathBuf, ()>,
}

impl TsModuleResolver {
    /// Build a resolver from a project root and its source files.
    pub fn build(project_root: &Path, source_files: &[PathBuf]) -> Self {
        let tsconfigs = discover_tsconfigs(project_root);

        // Discover workspace packages from root package.json
        let mut workspace_packages = BTreeMap::new();
        if let Ok(content) = std::fs::read_to_string(project_root.join("package.json")) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                let patterns = extract_workspaces(&parsed);
                for pattern in patterns {
                    discover_workspace_packages(project_root, &pattern, &mut workspace_packages);
                }
            }
        }

        // Build known files set
        let known_files: BTreeMap<PathBuf, ()> =
            source_files.iter().map(|f| (f.clone(), ())).collect();

        Self {
            project_root: project_root.to_path_buf(),
            tsconfigs,
            workspace_packages,
            known_files,
        }
    }

    /// Resolve an import specifier from a given importer file.
    pub fn resolve_import(&self, importer: &Path, specifier: &str) -> ResolvedTsImport {
        // 1. Relative imports
        if specifier.starts_with('.') || specifier.starts_with('/') {
            return self.resolve_relative(importer, specifier);
        }

        // 2. tsconfig paths (exact then wildcard)
        if let Some(resolved) = self.resolve_tsconfig_paths(specifier) {
            return resolved;
        }

        // Check if specifier matched a tsconfig pattern but resolution failed
        if self.matches_tsconfig_pattern(specifier) {
            return ResolvedTsImport {
                specifier: specifier.to_string(),
                target_file: None,
                target_module: None,
                resolution_kind: TsResolutionKind::Unresolved,
                confidence: None,
                reason: "typescript-import-unresolved".to_string(),
            };
        }

        // 3. Workspace package imports
        if let Some(resolved) = self.resolve_workspace(specifier) {
            return resolved;
        }

        // 4. External or unresolved
        if is_likely_external(specifier) {
            ResolvedTsImport {
                specifier: specifier.to_string(),
                target_file: None,
                target_module: None,
                resolution_kind: TsResolutionKind::External,
                confidence: None,
                reason: "typescript-external-package-not-indexed".to_string(),
            }
        } else {
            ResolvedTsImport {
                specifier: specifier.to_string(),
                target_file: None,
                target_module: None,
                resolution_kind: TsResolutionKind::Unresolved,
                confidence: None,
                reason: "typescript-import-unresolved".to_string(),
            }
        }
    }

    /// Resolve a relative import specifier.
    fn resolve_relative(&self, importer: &Path, specifier: &str) -> ResolvedTsImport {
        let base = importer.parent().unwrap_or(Path::new("."));
        let target_dir = base.join(specifier);

        // Try extensions
        let extensions = [".ts", ".tsx", ".d.ts"];
        for ext in &extensions {
            let candidate = target_dir.with_extension(&ext[1..]); // strip leading dot
            if candidate.extension().is_none() {
                // specifier already has extension — try as-is too
                let with_ext = if specifier.ends_with(".ts") || specifier.ends_with(".tsx") {
                    base.join(specifier)
                } else {
                    let mut c = target_dir.as_path().to_path_buf();
                    c.set_extension(&ext[1..]);
                    c
                };
                if self.known_files.contains_key(&with_ext) || with_ext.is_file() {
                    return ResolvedTsImport {
                        specifier: specifier.to_string(),
                        target_file: Some(with_ext),
                        target_module: None,
                        resolution_kind: TsResolutionKind::RelativeFile,
                        confidence: Some(CONFIDENCE_RELATIVE),
                        reason: "typescript-relative-import-resolved".to_string(),
                    };
                }
            }
            let mut candidate_path = target_dir.clone();
            candidate_path.set_extension(&ext[1..]);
            if self.known_files.contains_key(&candidate_path) || candidate_path.is_file() {
                return ResolvedTsImport {
                    specifier: specifier.to_string(),
                    target_file: Some(candidate_path),
                    target_module: None,
                    resolution_kind: TsResolutionKind::RelativeFile,
                    confidence: Some(CONFIDENCE_RELATIVE),
                    reason: "typescript-relative-import-resolved".to_string(),
                };
            }
        }

        // Try as directory with index
        let index_files = ["index.ts", "index.tsx", "index.d.ts"];
        for idx in &index_files {
            let candidate = target_dir.join(idx);
            if self.known_files.contains_key(&candidate) || candidate.is_file() {
                return ResolvedTsImport {
                    specifier: specifier.to_string(),
                    target_file: Some(candidate),
                    target_module: None,
                    resolution_kind: TsResolutionKind::RelativeIndex,
                    confidence: Some(CONFIDENCE_RELATIVE),
                    reason: "typescript-relative-import-resolved".to_string(),
                };
            }
        }

        ResolvedTsImport {
            specifier: specifier.to_string(),
            target_file: None,
            target_module: None,
            resolution_kind: TsResolutionKind::Unresolved,
            confidence: None,
            reason: "typescript-import-unresolved".to_string(),
        }
    }

    /// Check if a specifier matches any tsconfig path pattern (exact or wildcard prefix).
    fn matches_tsconfig_pattern(&self, specifier: &str) -> bool {
        for tsconfig in &self.tsconfigs {
            if tsconfig.paths.contains_key(specifier) {
                return true;
            }
            for pattern in tsconfig.paths.keys() {
                if let Some(star_idx) = pattern.find('*') {
                    let prefix = &pattern[..star_idx];
                    if specifier.starts_with(prefix) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Resolve against tsconfig paths (exact + wildcard).
    fn resolve_tsconfig_paths(&self, specifier: &str) -> Option<ResolvedTsImport> {
        for tsconfig in &self.tsconfigs {
            let base = tsconfig.base_url.as_ref().unwrap_or(&tsconfig.path);

            // Exact match
            if let Some(targets) = tsconfig.paths.get(specifier) {
                if let Some(file) = self.try_resolve_targets(base, targets) {
                    return Some(ResolvedTsImport {
                        specifier: specifier.to_string(),
                        target_file: Some(file),
                        target_module: Some(specifier.to_string()),
                        resolution_kind: TsResolutionKind::TsConfigPathExact,
                        confidence: Some(CONFIDENCE_EXACT),
                        reason: "typescript-tsconfig-path-exact".to_string(),
                    });
                }
            }

            // Wildcard match: find the best matching pattern
            let mut best_match: Option<(&String, &Vec<String>, &str)> = None;
            let mut best_prefix_len = 0;
            for (pattern, targets) in &tsconfig.paths {
                if let Some(star_idx) = pattern.find('*') {
                    let prefix = &pattern[..star_idx];
                    let suffix = &pattern[star_idx + 1..];
                    if specifier.starts_with(prefix) && specifier.ends_with(suffix) {
                        let suffix_len = if suffix.is_empty() { 0 } else { suffix.len() };
                        let remaining = &specifier[prefix.len()..specifier.len() - suffix_len];
                        if !remaining.is_empty() || suffix.is_empty() {
                            if prefix.len() > best_prefix_len {
                                best_prefix_len = prefix.len();
                                best_match = Some((pattern, targets, remaining));
                            }
                        }
                    }
                }
            }

            if let Some((_pattern, targets, remaining)) = best_match {
                let expanded: Vec<String> =
                    targets.iter().map(|t| t.replace('*', remaining)).collect();
                if let Some(file) = self.try_resolve_targets(base, &expanded) {
                    return Some(ResolvedTsImport {
                        specifier: specifier.to_string(),
                        target_file: Some(file),
                        target_module: Some(specifier.to_string()),
                        resolution_kind: TsResolutionKind::TsConfigPathWildcard,
                        confidence: Some(CONFIDENCE_WILDCARD),
                        reason: "typescript-tsconfig-path-wildcard".to_string(),
                    });
                }
            }
        }
        None
    }

    /// Try to resolve a list of target patterns against the filesystem.
    fn try_resolve_targets(&self, base: &Path, targets: &[String]) -> Option<PathBuf> {
        for target in targets {
            let candidate = base.join(target);

            // Direct file match
            if self.known_files.contains_key(&candidate) || candidate.is_file() {
                return Some(candidate);
            }

            // Try extensions
            for ext in &[".ts", ".tsx", ".d.ts"] {
                let mut with_ext = candidate.clone();
                with_ext.set_extension(&ext[1..]);
                if self.known_files.contains_key(&with_ext) || with_ext.is_file() {
                    return Some(with_ext);
                }
            }

            // Try as directory with index
            for idx in &["index.ts", "index.tsx", "index.d.ts"] {
                let index = candidate.join(idx);
                if self.known_files.contains_key(&index) || index.is_file() {
                    return Some(index);
                }
            }
        }
        None
    }

    /// Resolve a workspace package import.
    fn resolve_workspace(&self, specifier: &str) -> Option<ResolvedTsImport> {
        // Exact package name
        if let Some(pkg_root) = self.workspace_packages.get(specifier) {
            let entry = self.find_package_entry(pkg_root);
            if let Some(file) = entry {
                return Some(ResolvedTsImport {
                    specifier: specifier.to_string(),
                    target_file: Some(file),
                    target_module: Some(specifier.to_string()),
                    resolution_kind: TsResolutionKind::WorkspacePackage,
                    confidence: Some(CONFIDENCE_WORKSPACE),
                    reason: "typescript-workspace-package-import".to_string(),
                });
            }
        }

        // Subpath: check if specifier starts with a known package name + "/"
        let mut best_match: Option<(&String, &PathBuf, &str)> = None;
        let mut best_len = 0;
        for (name, root) in &self.workspace_packages {
            if specifier.starts_with(&format!("{}/", name)) && name.len() > best_len {
                let subpath = &specifier[name.len() + 1..];
                best_match = Some((name, root, subpath));
                best_len = name.len();
            }
        }

        if let Some((name, pkg_root, subpath)) = best_match {
            // Try src/{subpath}.ts, src/{subpath}/index.ts, etc.
            let src_dir = pkg_root.join("src");
            let subpath_dir = src_dir.join(subpath);

            // Direct file match with extension probing
            for ext in &[".ts", ".tsx"] {
                let mut candidate = subpath_dir.clone();
                candidate.set_extension(&ext[1..]);
                if self.known_files.contains_key(&candidate) || candidate.is_file() {
                    return Some(ResolvedTsImport {
                        specifier: specifier.to_string(),
                        target_file: Some(candidate),
                        target_module: Some(name.clone()),
                        resolution_kind: TsResolutionKind::WorkspacePackageSubpath,
                        confidence: Some(CONFIDENCE_WORKSPACE_SUBPATH),
                        reason: "typescript-workspace-subpath-import".to_string(),
                    });
                }
            }

            // Try as directory with index
            for idx in &["index.ts", "index.tsx"] {
                let candidate = subpath_dir.join(idx);
                if self.known_files.contains_key(&candidate) || candidate.is_file() {
                    return Some(ResolvedTsImport {
                        specifier: specifier.to_string(),
                        target_file: Some(candidate),
                        target_module: Some(name.clone()),
                        resolution_kind: TsResolutionKind::WorkspacePackageSubpath,
                        confidence: Some(CONFIDENCE_WORKSPACE_SUBPATH),
                        reason: "typescript-workspace-subpath-import".to_string(),
                    });
                }
            }
        }

        None
    }

    /// Find the entry file for a workspace package.
    fn find_package_entry(&self, pkg_root: &Path) -> Option<PathBuf> {
        let candidates = [
            pkg_root.join("src/index.ts"),
            pkg_root.join("src/index.tsx"),
            pkg_root.join("index.ts"),
            pkg_root.join("index.tsx"),
        ];
        for candidate in &candidates {
            if self.known_files.contains_key(candidate) || candidate.is_file() {
                return Some(candidate.clone());
            }
        }
        None
    }
}

/// Extract workspace patterns from a root package.json value.
fn extract_workspaces(parsed: &serde_json::Value) -> Vec<String> {
    let mut patterns = Vec::new();

    // workspaces: ["packages/*"]
    if let Some(arr) = parsed.get("workspaces").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                patterns.push(s.to_string());
            } else if let Some(obj) = item.as_object() {
                // Bolt-style: { packages: ["packages/*"] }
                if let Some(packages) = obj.get("packages").and_then(|v| v.as_array()) {
                    for p in packages {
                        if let Some(s) = p.as_str() {
                            patterns.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    patterns
}

/// Discover workspace packages from a glob-like pattern (e.g. "packages/*").
fn discover_workspace_packages(
    project_root: &Path,
    pattern: &str,
    packages: &mut BTreeMap<String, PathBuf>,
) {
    // Only handle simple patterns: "dir/*"
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() != 2 || !parts[1].is_empty() {
        return;
    }
    let base_dir = project_root.join(parts[0]);
    if let Ok(entries) = std::fs::read_dir(&base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let pkg_json = path.join("package.json");
            if let Ok(content) = std::fs::read_to_string(&pkg_json) {
                let cleaned = strip_json5_comments(&content);
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cleaned) {
                    if let Some(name) = parsed.get("name").and_then(|v| v.as_str()) {
                        packages.insert(name.to_string(), path);
                    }
                }
            }
        }
    }
}

/// Check if a specifier looks like an external npm package.
fn is_likely_external(specifier: &str) -> bool {
    // Scoped packages: @scope/name, @scope/name/subpath
    if specifier.starts_with('@') {
        return specifier.contains('/') || specifier.len() > 1;
    }
    // Plain package names: react, lodash, etc.
    !specifier.contains('/') && !specifier.is_empty()
}
