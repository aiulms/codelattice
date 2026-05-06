//! Cangjie import statement extraction and package resolution.
//!
//! Ports the TS adapter's import resolution strategy:
//! - Parse import statements from tree-sitter-cangjie AST
//! - Split raw import paths into targets (grouped, wildcard, alias forms)
//! - Resolve package names to local directories using project metadata
//!   (workspace members, path-based dependencies, cjpm.lock entries)
//!
//! Does NOT spawn cjpm tree (deferred to future slice).
//!
//! Available only when the `tree-sitter-cangjie` feature is enabled.

use std::path::{Path, PathBuf};

use crate::project::CangjieProject;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Visibility modifier on an import statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportVisibility {
    Public,
    Protected,
    Internal,
    Private,
}

/// Package alias from `import demo.math as math`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAlias {
    pub package_name: String,
    pub alias: String,
}

/// A parsed Cangjie import statement.
#[derive(Debug, Clone, PartialEq)]
pub struct CangjieImport {
    /// Raw import path text (e.g. "demo.math.{add, sub}").
    pub raw_path: String,
    /// Visibility modifier.
    pub visibility: ImportVisibility,
    /// Whether this is a wildcard import (ends with `.*`).
    pub is_wildcard: bool,
    /// Package alias binding, if present.
    pub package_alias: Option<PackageAlias>,
    /// File path where this import was found.
    pub file_path: String,
}

/// A named import candidate: `import demo.math.add` → package=demo.math, exported=add, local=add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCandidate {
    pub package_name: String,
    pub exported_name: String,
    pub local_name: String,
}

/// How an import was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionKind {
    /// Resolved to a workspace member package.
    WorkspaceMember,
    /// Resolved via a path-based dependency in cjpm.toml.
    PathDependency,
    /// Resolved via a cjpm.lock [[requires]] entry with a local source path.
    LockEntry,
    /// External package (std / core prefix) — skipped.
    External,
}

/// Result of resolving an import to a local package directory.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedImport {
    /// Package name that owns the target symbol.
    pub target_package_name: String,
    /// Resolved package source directory on disk, if found.
    pub target_dir: Option<PathBuf>,
    /// How this was resolved.
    pub resolution: ResolutionKind,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Package prefixes that are external (stdlib / core) and not locally resolvable.
const EXTERNAL_PACKAGE_PREFIXES: &[&str] = &["core", "std"];

/// Cangjie identifier pattern.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s.chars().next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_external_package(name: &str) -> bool {
    EXTERNAL_PACKAGE_PREFIXES
        .iter()
        .any(|prefix| name == *prefix || name.starts_with(&format!("{}.", prefix)))
}

// ---------------------------------------------------------------------------
// String parsers (port of TS `parseCangjieImportTargets()` etc.)
// ---------------------------------------------------------------------------

/// Split a raw import path into individual target strings.
///
/// Handles:
/// - `"demo.math.add"` → `["demo.math.add"]`
/// - `"{add, sub}"` → `["add", "sub"]`
/// - `"demo.math.{add, sub}"` → `["demo.math.{add, sub}"]`
/// - `"demo.math.*"` → `["demo.math.*"]`
///
/// Port of TS `parseCangjieImportTargets()`.
pub fn parse_import_targets(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    // Grouped top-level: {a, b, c}
    if raw.starts_with('{') && raw.ends_with('}') {
        return split_top_level_comma(&raw[1..raw.len() - 1])
            .into_iter()
            .map(|s| strip_alias(&s).to_string())
            .collect();
    }
    vec![strip_alias(raw).to_string()]
}

/// Split a string by top-level commas (respecting brace nesting).
/// Port of TS `splitTopLevelComma()`.
fn split_top_level_comma(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;

    for (i, ch) in raw.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(raw[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(raw[start..].trim().to_string());
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}

/// Strip ` as AliasName` suffix from a raw target.
fn strip_alias(raw: &str) -> &str {
    // Find last " as " and check what follows is a valid identifier
    if let Some(pos) = raw.rfind(" as ") {
        let alias_part = &raw[pos + 4..].trim();
        if is_valid_identifier(alias_part) {
            return raw[..pos].trim();
        }
    }
    raw.trim()
}

/// Check whether a raw target has an alias suffix.
fn has_alias(raw: &str) -> bool {
    if let Some(pos) = raw.rfind(" as ") {
        let alias_part = &raw[pos + 4..].trim();
        return is_valid_identifier(alias_part);
    }
    false
}

/// Parse named import candidates from a raw import path.
///
/// Handles:
/// - `"demo.math.add"` → 1 candidate (package=demo.math, exported=add, local=add)
/// - `"demo.math.add as plus"` → 1 candidate (package=demo.math, exported=add, local=plus)
/// - `"demo.math.{add, sub}"` → 2 candidates
/// - `"demo.math.*"` → empty (wildcard — no specific exports)
/// - `"public import ..."` prefix → empty (visibility handled separately)
///
/// Port of TS `parseCangjieNamedImportCandidates()`.
pub fn parse_named_import_candidates(raw: &str) -> Vec<ImportCandidate> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("public ") {
        return vec![];
    }

    let targets = if raw.starts_with('{') && raw.ends_with('}') {
        split_top_level_comma(&raw[1..raw.len() - 1])
    } else {
        vec![raw.to_string()]
    };

    targets
        .into_iter()
        .flat_map(|t| parse_named_candidates_from_target(&t))
        .collect()
}

/// Parse candidates from a single target (non-grouped).
fn parse_named_candidates_from_target(raw: &str) -> Vec<ImportCandidate> {
    let target = raw.trim();
    if target.is_empty() || target.contains('*') {
        return vec![];
    }

    // Alias form: `demo.math.add as plus`
    if has_alias(target) {
        let stripped = strip_alias(target);
        let alias_name = target
            .rfind(" as ")
            .map(|pos| target[pos + 4..].trim().to_string())
            .unwrap_or_default();

        if stripped.contains('{') || stripped.contains('}') {
            return vec![];
        }

        let segments: Vec<&str> = stripped.split('.').filter(|s| !s.is_empty()).collect();
        if segments.len() < 3 {
            return vec![];
        }

        let package_name = segments[..segments.len() - 1].join(".");
        let exported_name = segments[segments.len() - 1].to_string();

        if !is_valid_identifier(&exported_name) || !is_valid_identifier(&alias_name) {
            return vec![];
        }

        return vec![ImportCandidate {
            package_name,
            exported_name,
            local_name: alias_name,
        }];
    }

    // Grouped form: `demo.math.{add, sub}`
    if let Some(grouped) = parse_grouped_import(target) {
        let package_name = grouped.0;
        if package_name.is_empty() {
            return vec![];
        }

        return grouped
            .1
            .into_iter()
            .filter(|s| is_valid_identifier(s))
            .map(|sym| ImportCandidate {
                package_name: package_name.clone(),
                exported_name: sym.clone(),
                local_name: sym,
            })
            .collect();
    }

    // Simple form: `demo.math.add` → last segment = exported, rest = package
    let last_dot = target.rfind('.');
    if last_dot.map_or(true, |pos| pos == 0 || pos == target.len() - 1) {
        return vec![];
    }

    let package_name = target[..last_dot.unwrap()].trim().to_string();
    let exported_name = target[last_dot.unwrap() + 1..].trim().to_string();

    if !is_valid_identifier(&exported_name) {
        return vec![];
    }

    vec![ImportCandidate {
        package_name,
        exported_name: exported_name.clone(),
        local_name: exported_name,
    }]
}

/// Parse grouped import form: `"demo.math.{add, sub}"` → Some(("demo.math", vec!["add", "sub"]))
fn parse_grouped_import(target: &str) -> Option<(String, Vec<String>)> {
    let brace_start = target.find(".{")?;
    let brace_end = target.rfind('}')?;
    if brace_end != target.len() - 1 {
        return None;
    }

    let package_name = target[..brace_start].trim().to_string();
    let inner = &target[brace_start + 2..brace_end];
    let symbols = split_top_level_comma(inner);

    Some((package_name, symbols))
}

/// Extract the package name from a raw import target.
fn package_name_from_target(raw: &str) -> Option<String> {
    let target = strip_alias(raw);
    if target.is_empty() {
        return None;
    }

    // Grouped form: `demo.math.{add, sub}` → "demo.math"
    if let Some((pkg_name, _)) = parse_grouped_import(target) {
        return Some(pkg_name);
    }

    // Wildcard: `demo.math.*` → "demo.math"
    if target.ends_with(".*") {
        return Some(target[..target.len() - 2].trim().to_string());
    }

    Some(target.to_string())
}

// ---------------------------------------------------------------------------
// Package resolution
// ---------------------------------------------------------------------------

/// Candidate directory suffixes for a given package name, ordered by priority.
///
/// Port of TS `candidatePackageDirs()` (simplified — no `cangjieConfig` context;
/// uses `CangjieProject` directly).
fn candidate_package_dirs(package_name: &str, project: &CangjieProject) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    let segments: Vec<&str> = package_name.split('.').filter(|s| !s.is_empty()).collect();

    let mut add = |dir: String| {
        let normalized = dir
            .replace('\\', "/")
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string();
        if !normalized.is_empty() && !dirs.contains(&normalized) {
            dirs.push(normalized);
        }
    };

    for pkg in &project.packages {
        let base_dir = if pkg.module_dir.is_empty() {
            pkg.src_dir.clone()
        } else {
            format!("{}/{}", pkg.module_dir, pkg.src_dir)
        };

        if package_name == pkg.name {
            add(base_dir.clone());
        } else if let Some(rest) = package_name.strip_prefix(&format!("{}.", pkg.name)) {
            let relative = rest.replace('.', "/");
            add(format!("{}/{}", base_dir, relative));
        }

        // Fallback: append package name as path under each package's src dir
        add(format!("{}/{}", base_dir, package_name.replace('.', "/")));
        if segments.len() > 1 {
            add(format!("{}/{}", base_dir, segments[1..].join("/")));
        }
    }

    // Simple fallback: package name as path
    add(package_name.replace('.', "/"));

    // Path-based dependency fallback
    for dep in &project.manifest.dependencies {
        if dep.name == package_name || package_name.starts_with(&format!("{}.", dep.name)) {
            if let Some(ref dep_path) = dep.path {
                let abs = if Path::new(dep_path).is_absolute() {
                    PathBuf::from(dep_path)
                } else {
                    project.root.join(dep_path)
                };
                let normalized = abs.to_string_lossy().replace('\\', "/");
                add(normalized.clone());

                // If package_name has extra segments beyond dep name
                if let Some(rest) = package_name.strip_prefix(&format!("{}.", dep.name)) {
                    let relative = rest.replace('.', "/");
                    add(format!("{}/{}", normalized, relative));
                }
            }
        }
    }

    dirs
}

/// Check whether any `.cj` files exist under a candidate directory.
///
/// Uses the project's `source_files` list as a file index (avoids re-scanning disk).
/// `dir` is relative to the project root.
fn has_files_in_dir(dir: &str, project: &CangjieProject) -> bool {
    // Resolve the candidate dir to an absolute path
    let abs_dir = project.root.join(dir);
    let normalized_dir = abs_dir
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    project.source_files.iter().any(|f| {
        let f_str = f.to_string_lossy().replace('\\', "/");
        // Check if file path starts with the candidate directory path
        f_str.starts_with(&normalized_dir)
    })
}

/// Resolve a package name to a local directory using project metadata.
fn resolve_package_by_name(package_name: &str, project: &CangjieProject) -> Option<ResolvedImport> {
    if is_external_package(package_name) {
        return Some(ResolvedImport {
            target_package_name: package_name.to_string(),
            target_dir: None,
            resolution: ResolutionKind::External,
        });
    }

    for dir in candidate_package_dirs(package_name, project) {
        if has_files_in_dir(&dir, project) {
            // Determine resolution kind
            let kind = if project.packages.iter().any(|p| p.name == package_name) {
                ResolutionKind::WorkspaceMember
            } else if project
                .manifest
                .dependencies
                .iter()
                .any(|d| d.name == package_name && d.path.is_some())
            {
                ResolutionKind::PathDependency
            } else {
                ResolutionKind::LockEntry
            };

            return Some(ResolvedImport {
                target_package_name: package_name.to_string(),
                target_dir: Some(project.root.join(&dir)),
                resolution: kind,
            });
        }
    }

    // Last resort: strip last segment and try parent package
    if let Some(last_dot) = package_name.rfind('.') {
        let parent = &package_name[..last_dot];
        if !parent.is_empty() {
            return resolve_package_by_name(parent, project);
        }
    }

    None
}

/// Resolve an import candidate to a target package directory.
pub fn resolve_import_target(
    candidate: &ImportCandidate,
    project: &CangjieProject,
) -> Option<ResolvedImport> {
    resolve_package_by_name(&candidate.package_name, project)
}

// ---------------------------------------------------------------------------
// AST import extraction
// ---------------------------------------------------------------------------

/// Extract all import statements from a tree-sitter Cangjie parse tree.
///
/// Walks the AST looking for `importList` nodes and extracts:
/// - Raw import path text (from `packageFull`, `subGroupOfPackage`, `packageAlias`,
///   `packageGroup`, `scoped_identifier`, or `identifier` child)
/// - Visibility modifiers (from `modifiers` child)
/// - Package alias bindings (from `packageAlias` child fields)
/// - Wildcard detection
///
/// Available only when the `tree-sitter-cangjie` feature is enabled.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn extract_cangjie_imports(
    source: &str,
    file_path: &Path,
    tree: &tree_sitter::Tree,
) -> Vec<CangjieImport> {
    let source_bytes = source.as_bytes();
    let mut imports = Vec::new();

    walk_for_imports(tree.root_node(), source_bytes, file_path, &mut imports);

    imports
}

#[cfg(feature = "tree-sitter-cangjie")]
fn idx(i: usize) -> u32 {
    i.try_into().unwrap()
}

#[cfg(feature = "tree-sitter-cangjie")]
fn walk_for_imports(
    node: tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
    out: &mut Vec<CangjieImport>,
) {
    if node.kind() == "importList" {
        if let Some(import) = parse_import_list(node, source, file_path) {
            out.push(import);
        }
        return; // Don't recurse into importList children
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(idx(i)) {
            walk_for_imports(child, source, file_path, out);
        }
    }
}

/// Known import source node kinds inside `importList`.
#[cfg(feature = "tree-sitter-cangjie")]
const IMPORT_SOURCE_KINDS: &[&str] = &[
    "packageFull",
    "subGroupOfPackage",
    "packageAlias",
    "packageGroup",
    "scoped_identifier",
    "identifier",
];

#[cfg(feature = "tree-sitter-cangjie")]
fn parse_import_list(
    node: tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
) -> Option<CangjieImport> {
    let mut raw_path = String::new();
    let mut visibility = ImportVisibility::Private;
    let mut is_wildcard = false;
    let mut package_alias = None;

    for i in 0..node.named_child_count() {
        let child = node.named_child(idx(i));
        let kind = child.map(|c| c.kind().to_string());

        match kind.as_deref() {
            Some("modifiers") => {
                visibility = parse_modifiers(child.unwrap(), source);
            }
            Some("packageAlias") => {
                let alias = parse_package_alias_node(child.unwrap(), source);
                if alias.is_some() {
                    raw_path = child.unwrap().utf8_text(source).unwrap_or("").to_string();
                    package_alias = alias;
                }
            }
            Some(k) if IMPORT_SOURCE_KINDS.contains(&k) => {
                let text = child.unwrap().utf8_text(source).unwrap_or("").to_string();
                raw_path = text.clone();
                // Detect wildcard from text patterns
                if text.ends_with(".*") {
                    is_wildcard = true;
                }
                // Check if this is a packageGroup (e.g. `demo.math.*`)
                if k == "packageGroup" {
                    is_wildcard = true;
                }
            }
            _ => {}
        }
    }

    if raw_path.is_empty() {
        return None;
    }

    // Also check for wildcard via subGroupOfPackage text
    if raw_path.contains(".*") {
        is_wildcard = true;
    }

    Some(CangjieImport {
        raw_path,
        visibility,
        is_wildcard,
        package_alias,
        file_path: file_path.to_string_lossy().to_string(),
    })
}

#[cfg(feature = "tree-sitter-cangjie")]
fn parse_modifiers(node: tree_sitter::Node, source: &[u8]) -> ImportVisibility {
    let text = node.utf8_text(source).unwrap_or("");
    if text.contains("public") {
        ImportVisibility::Public
    } else if text.contains("protected") {
        ImportVisibility::Protected
    } else if text.contains("internal") {
        ImportVisibility::Internal
    } else {
        ImportVisibility::Private
    }
}

#[cfg(feature = "tree-sitter-cangjie")]
fn parse_package_alias_node(node: tree_sitter::Node, source: &[u8]) -> Option<PackageAlias> {
    let mut package_name = String::new();
    let mut alias = String::new();

    // packageAlias has named children: packageName and alias (field names)
    for i in 0..node.named_child_count() {
        let child = node.named_child(idx(i))?;
        let kind = child.kind();
        let text = child.utf8_text(source).unwrap_or("").to_string();
        match kind {
            "packageName" | "scoped_identifier" | "identifier" if package_name.is_empty() => {
                package_name = text;
            }
            "alias" => {
                alias = text;
            }
            _ if package_name.is_empty() => {
                // First unnamed identifier-like child is likely the package name
                package_name = text;
            }
            _ if alias.is_empty() => {
                alias = text;
            }
            _ => {}
        }
    }

    if package_name.is_empty() || alias.is_empty() {
        return None;
    }

    Some(PackageAlias {
        package_name,
        alias,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_import_targets --

    #[test]
    fn parse_targets_single() {
        let result = parse_import_targets("demo.math.add");
        assert_eq!(result, vec!["demo.math.add"]);
    }

    #[test]
    fn parse_targets_grouped() {
        let result = parse_import_targets("{add, sub}");
        assert_eq!(result, vec!["add", "sub"]);
    }

    #[test]
    fn parse_targets_wildcard() {
        let result = parse_import_targets("demo.math.*");
        assert_eq!(result, vec!["demo.math.*"]);
    }

    #[test]
    fn parse_targets_with_alias() {
        let result = parse_import_targets("demo.math.add as plus");
        assert_eq!(result, vec!["demo.math.add"]);
    }

    #[test]
    fn parse_targets_empty() {
        let result = parse_import_targets("");
        assert!(result.is_empty() || result == vec![""]);
    }

    // -- parse_named_import_candidates --

    #[test]
    fn named_candidates_simple() {
        let result = parse_named_import_candidates("demo.math.add");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].package_name, "demo.math");
        assert_eq!(result[0].exported_name, "add");
        assert_eq!(result[0].local_name, "add");
    }

    #[test]
    fn named_candidates_grouped() {
        let result = parse_named_import_candidates("demo.math.{add, sub}");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].package_name, "demo.math");
        assert_eq!(result[0].exported_name, "add");
        assert_eq!(result[1].package_name, "demo.math");
        assert_eq!(result[1].exported_name, "sub");
    }

    #[test]
    fn named_candidates_alias() {
        let result = parse_named_import_candidates("demo.math.add as plus");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].package_name, "demo.math");
        assert_eq!(result[0].exported_name, "add");
        assert_eq!(result[0].local_name, "plus");
    }

    #[test]
    fn named_candidates_wildcard() {
        let result = parse_named_import_candidates("demo.math.*");
        assert!(result.is_empty());
    }

    #[test]
    fn named_candidates_public_prefix() {
        let result = parse_named_import_candidates("public demo.api.add");
        assert!(result.is_empty());
    }

    #[test]
    fn named_candidates_empty() {
        let result = parse_named_import_candidates("");
        assert!(result.is_empty());
    }

    // -- is_external_package --

    #[test]
    fn external_package_std() {
        assert!(is_external_package("std.collection"));
        assert!(is_external_package("std"));
    }

    #[test]
    fn external_package_core() {
        assert!(is_external_package("core.lang"));
        assert!(is_external_package("core"));
    }

    #[test]
    fn external_package_normal() {
        assert!(!is_external_package("demo.math"));
        assert!(!is_external_package("myapp"));
    }

    // -- is_valid_identifier --

    #[test]
    fn identifier_valid() {
        assert!(is_valid_identifier("add"));
        assert!(is_valid_identifier("_private"));
        assert!(is_valid_identifier("MyType"));
    }

    #[test]
    fn identifier_invalid() {
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123abc"));
        assert!(!is_valid_identifier("has space"));
    }

    // -- split_top_level_comma --

    #[test]
    fn split_simple() {
        let result = split_top_level_comma("add, sub, mul");
        assert_eq!(result, vec!["add", "sub", "mul"]);
    }

    #[test]
    fn split_nested_braces() {
        let result = split_top_level_comma("a, {b, c}, d");
        assert_eq!(result, vec!["a", "{b, c}", "d"]);
    }

    #[test]
    fn split_single() {
        let result = split_top_level_comma("add");
        assert_eq!(result, vec!["add"]);
    }

    // -- strip_alias --

    #[test]
    fn strip_alias_simple() {
        assert_eq!(strip_alias("demo.math.add as plus"), "demo.math.add");
    }

    #[test]
    fn strip_alias_none() {
        assert_eq!(strip_alias("demo.math.add"), "demo.math.add");
    }

    // -- package_name_from_target --

    #[test]
    fn package_name_simple() {
        assert_eq!(
            package_name_from_target("demo.math.add"),
            Some("demo.math.add".to_string())
        );
    }

    #[test]
    fn package_name_wildcard() {
        assert_eq!(
            package_name_from_target("demo.math.*"),
            Some("demo.math".to_string())
        );
    }

    #[test]
    fn package_name_grouped() {
        assert_eq!(
            package_name_from_target("demo.math.{add, sub}"),
            Some("demo.math".to_string())
        );
    }
}
