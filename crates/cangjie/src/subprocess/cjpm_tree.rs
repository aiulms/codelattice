//! cjpm tree subprocess runner — spawns `cjpm tree --skip-script` and parses
//! the text output into a structured dependency tree.
//!
//! Also provides recursive workspace subtree search for resolving tree dependency
//! package names to their source directories on disk.
//!
//! Port of TS `cjpm-metadata.ts`:
//! - `parseCjpmTreeOutput()` → `parse_cjpm_tree_output()`
//! - `runCjpmTree()` → `run_cjpm_tree()`
//! - `findPackageDirByName()` → `find_package_dir_by_name()`
//! - `resolveTreeDependencyDir()` → `resolve_tree_dependency_dir()`
//!
//! All functions gracefully degrade when cjpm is not available.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::diagnostics::runner::{build_cangjie_spawn_env, resolve_cangjie_tool};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A node in the cjpm tree dependency graph (compiler-resolved transitive deps).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CjpmTreeNode {
    /// Package name.
    pub name: String,
    /// Direct dependencies of this package.
    pub children: Vec<CjpmTreeNode>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum recursion depth for find_package_dir_by_name.
/// Aligned with TS `MAX_TREE_DEP_SEARCH_DEPTH`.
const MAX_TREE_DEP_SEARCH_DEPTH: u32 = 3;

/// Timeout for cjpm tree subprocess.
/// Aligned with TS `TREE_TIMEOUT_MS`.
const TREE_TIMEOUT_SECS: u64 = 30;

/// Directory names to skip during recursive search.
/// Aligned with TS `SKIP_DIRS`.
const SKIP_DIRS: &[&str] = &[
    ".",
    "..",
    ".git",
    ".cache",
    ".generated",
    "node_modules",
    "target",
];

// ---------------------------------------------------------------------------
// cjpm tree output parser
// ---------------------------------------------------------------------------

/// Parse `cjpm tree --skip-script` text output into structured dependency tree.
///
/// Input format:
/// ```text
/// |-- root_pkg
///     └── dep1
///         └── subdep
///     └── dep2
/// |-- root_pkg2
///     └── dep3
/// ```
///
/// - `|--` marks root nodes (ASCII hyphen)
/// - `└──` marks child nodes (Unicode box-drawing)
/// - Each indentation level = 4 spaces
///
/// This is a pure text parser — zero SDK dependency.
pub fn parse_cjpm_tree_output(output: &str) -> Vec<CjpmTreeNode> {
    /// Flat entry collected during first pass.
    struct FlatEntry {
        depth: usize,
        name: String,
    }

    let mut entries: Vec<FlatEntry> = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        // Determine indentation level: count leading spaces before the tree marker
        let indent = trimmed.len() - trimmed.trim_start().len();
        let depth = indent / 4;

        // Extract package name from the trimmed content (after leading spaces)
        let content = trimmed.trim_start();
        let name = if let Some(rest) = content
            .strip_prefix("|-- ")
            .or_else(|| content.strip_prefix("└── "))
        {
            rest.trim().to_string()
        } else {
            continue;
        };

        if name.is_empty() {
            continue;
        }

        entries.push(FlatEntry { depth, name });
    }

    // Phase 2: assemble entries into tree
    let mut roots: Vec<CjpmTreeNode> = Vec::new();
    // Stack of (depth, node) — tracks the most recent node at each depth.
    // After pushing a child, we update the stack entry for that depth.
    // We use indices into `roots` and children Vecs to avoid borrow issues.
    // stack entries are Vec<usize> — a path of child indices from a root.
    struct StackFrame {
        depth: usize,
        /// How to reach this node: root_idx, child_idx_at_level1, child_idx_at_level2, ...
        path: Vec<usize>,
    }
    let mut stack: Vec<StackFrame> = Vec::new();

    for entry in &entries {
        let node = CjpmTreeNode {
            name: entry.name.clone(),
            children: Vec::new(),
        };

        if entry.depth == 0 {
            roots.push(node);
            stack.truncate(0);
            stack.push(StackFrame {
                depth: 0,
                path: vec![roots.len() - 1],
            });
        } else {
            // Find parent: most recent frame with depth == entry.depth - 1
            // Pop any frames at >= entry.depth (they're sibling subtrees)
            while stack.last().map_or(false, |f| f.depth >= entry.depth) {
                stack.pop();
            }

            if let Some(parent_frame) = stack.last() {
                if parent_frame.depth == entry.depth - 1 {
                    // Walk the path to insert child
                    let mut target_children: &mut Vec<CjpmTreeNode> = &mut roots;
                    // Navigate to the parent node first
                    for &idx in &parent_frame.path {
                        let tmp = target_children; // rebind to help borrow checker
                        target_children = &mut tmp[idx].children;
                    }

                    target_children.push(node);
                    let child_idx = target_children.len() - 1;

                    // Build path for the child
                    let mut new_path = parent_frame.path.clone();
                    new_path.push(child_idx);

                    stack.push(StackFrame {
                        depth: entry.depth,
                        path: new_path,
                    });
                }
                // else: depth jump (malformed), skip
            }
            // else: no parent found (malformed), skip
        }
    }

    roots
}

// ---------------------------------------------------------------------------
// cjpm tree runner
// ---------------------------------------------------------------------------

/// Check whether cjpm is available via CANGJIE_HOME / CANGJIE_SDK_HOME / PATH.
pub fn is_cjpm_available() -> bool {
    resolve_cangjie_tool("cjpm", "bin").is_some()
}

/// Run `cjpm tree --skip-script` in the given repo root directory.
///
/// Spawns cjpm as a subprocess, captures stdout, and parses the tree output.
/// Returns an empty Vec on SDK absence, timeout, non-zero exit, or parse error.
pub fn run_cjpm_tree(repo_root: &Path) -> Vec<CjpmTreeNode> {
    let cjpm_path = match resolve_cangjie_tool("cjpm", "bin") {
        Some(p) => p,
        None => return Vec::new(),
    };

    let env = build_cangjie_spawn_env();

    let mut child = match Command::new(&cjpm_path)
        .args(["tree", "--skip-script"])
        .current_dir(repo_root)
        .env_clear()
        .envs(&env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut stdout = String::new();
    if let Some(ref mut out) = child.stdout {
        let _ = out.read_to_string(&mut stdout);
    }

    // Wait with timeout
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(TREE_TIMEOUT_SECS);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Vec::new();
                }
                break;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Vec::new();
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return Vec::new(),
        }
    }

    parse_cjpm_tree_output(&stdout)
}

// ---------------------------------------------------------------------------
// Tree dependency directory resolution
// ---------------------------------------------------------------------------

/// Recursively find a package directory by [package].name in a workspace subtree.
///
/// Starting from `start_dir`, checks each subdirectory for a `cjpm.toml` with
/// matching `[package].name`. Returns the package's `src-dir` path on success.
///
/// Max recursion depth: 3 (aligned with TS `MAX_TREE_DEP_SEARCH_DEPTH`).
/// Skips hidden directories and known artifact directories (target, .git, etc.).
pub fn find_package_dir_by_name(
    target_name: &str,
    start_dir: &Path,
    depth: u32,
) -> Option<PathBuf> {
    if depth > MAX_TREE_DEP_SEARCH_DEPTH {
        return None;
    }

    let entries = match fs::read_dir(start_dir) {
        Ok(e) => e,
        Err(_) => return None, // permission errors etc.
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if SKIP_DIRS.contains(&name_str.as_ref()) {
            continue;
        }
        if name_str.starts_with('.') {
            continue;
        }

        let subdir = entry.path();
        let toml_path = subdir.join("cjpm.toml");

        if toml_path.exists() {
            if let Ok(content) = fs::read_to_string(&toml_path) {
                if let Ok(manifest) = crate::manifest::parse_cjpm_toml(&content) {
                    if manifest.package.as_ref().and_then(|p| p.name.as_deref())
                        == Some(target_name)
                    {
                        let src_dir = manifest
                            .package
                            .as_ref()
                            .map(|p| p.src_dir.as_str())
                            .unwrap_or("src");
                        return Some(subdir.join(src_dir));
                    }
                }
            }
        }

        // Recurse into subdirectories
        if let Some(found) = find_package_dir_by_name(target_name, &subdir, depth + 1) {
            return Some(found);
        }
    }

    None
}

/// Module-level cache for tree dependency directory resolution.
/// Key: `package_name::root1:root2:...`
use std::cell::RefCell;
thread_local! {
    static TREE_DEP_DIR_CACHE: RefCell<HashMap<String, Option<PathBuf>>> = RefCell::new(HashMap::new());
}

/// Resolve a tree dependency package name to its src directory on disk.
///
/// Searches workspace member subtrees for a matching `cjpm.toml` `[package].name`.
/// Results are cached per `(package_name, workspace_roots)` combination.
///
/// Port of TS `resolveTreeDependencyDir()`.
pub fn resolve_tree_dependency_dir(
    package_name: &str,
    workspace_roots: &[PathBuf],
) -> Option<PathBuf> {
    let cache_key = format!(
        "{}::{}",
        package_name,
        workspace_roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(":")
    );

    // Check cache
    let cached = TREE_DEP_DIR_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned());
    if let Some(result) = cached {
        return result;
    }

    // Search workspace roots
    let result = workspace_roots
        .iter()
        .find_map(|root| find_package_dir_by_name(package_name, root, 0));

    // Store in cache
    TREE_DEP_DIR_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, result.clone());
    });

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_cjpm_tree_output ----

    #[test]
    fn parse_single_root_no_children() {
        let output = "|-- root_pkg\n";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "root_pkg");
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn parse_root_with_one_child() {
        let output = "|-- root_pkg\n    └── dep1\n";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "root_pkg");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "dep1");
    }

    #[test]
    fn parse_root_with_multiple_children() {
        let output = "|-- root_pkg\n    └── dep1\n    └── dep2\n";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].name, "dep1");
        assert_eq!(tree[0].children[1].name, "dep2");
    }

    #[test]
    fn parse_nested_children_depth_2() {
        let output = "|-- root_pkg\n    └── dep1\n        └── subdep\n";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children[0].name, "subdep");
    }

    #[test]
    fn parse_multiple_roots() {
        let output = "|-- root_pkg1\n    └── dep1\n|-- root_pkg2\n    └── dep2\n";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "root_pkg1");
        assert_eq!(tree[1].name, "root_pkg2");
    }

    #[test]
    fn parse_empty_output() {
        let tree = parse_cjpm_tree_output("");
        assert!(tree.is_empty());
    }

    #[test]
    fn parse_lines_without_tree_markers() {
        let output = "some random text\nanother line\n";
        let tree = parse_cjpm_tree_output(output);
        assert!(tree.is_empty());
    }

    #[test]
    fn parse_realistic_cjpm_tree_output() {
        let output = "\
|-- imports-basic
    └── dep1
        └── subdep1
    └── dep2
";
        let tree = parse_cjpm_tree_output(output);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "imports-basic");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].name, "dep1");
        assert_eq!(tree[0].children[0].children[0].name, "subdep1");
        assert_eq!(tree[0].children[1].name, "dep2");
    }

    // ---- is_cjpm_available ----

    #[test]
    fn is_cjpm_available_does_not_panic() {
        // Should not panic regardless of SDK state
        let _ = is_cjpm_available();
    }

    // ---- run_cjpm_tree ----

    #[test]
    fn run_cjpm_tree_does_not_panic_on_nonexistent_dir() {
        let result = run_cjpm_tree(Path::new("/nonexistent/path"));
        // Should return empty (either cjpm not found or cjpm fails)
        // The key assertion: does not panic
        let _ = result;
    }

    #[test]
    fn run_cjpm_tree_without_sdk() {
        // When SDK is not available, returns empty Vec
        if !is_cjpm_available() {
            let result = run_cjpm_tree(Path::new("."));
            assert!(result.is_empty());
        }
    }
}
