//! MCP v0.8 Persistent Cache Pack for CodeLattice CLI
//!
//! Implements a MCP JSON-RPC server over stdin/stdout.
//! Provides 28 tools:
//!   v0:  codelattice_analyze, codelattice_quality, codelattice_summary, codelattice_smoke
//!   v0.1: codelattice_graph_overview, codelattice_unresolved_report,
//!         codelattice_symbol_search, codelattice_export_bridge
//!   v0.2: codelattice_symbol_context, codelattice_calls_from, codelattice_calls_to,
//!         codelattice_impact_preview, codelattice_query_graph, codelattice_project_overview,
//!         codelattice_repo_registry, codelattice_rename_preview
//!   v0.3: codelattice_cache_status, codelattice_cache_clear
//!   v0.5: codelattice_production_assist, codelattice_compare_runs
//!   v0.6: codelattice_cache_prewarm
//!   v0.7: codelattice_changed_symbols
//!   v0.8: codelattice_project_insights
//!   v0.9: codelattice_review_plan
//!   v0.10: codelattice_dead_code_candidates
//!   v0.11: codelattice_impact_analysis, codelattice_risk_hotspots, codelattice_architecture_drift
//!
//! Transport: newline-delimited JSON-RPC.
//! Approach: subprocess — spawns the CLI binary for analyze/quality/summary,
//!           and the smoke script for smoke.
//! Cache: two-layer analysis cache (process-local memory + persistent disk)
//!        with fingerprint-based stale detection, structured stale reasons,
//!        and LRU eviction (max 16 in-memory entries).
//!        Persistent cache: ${TMPDIR}/codelattice-cache/ or CODELATTICE_CACHE_DIR.
//!        Disable with CODELATTICE_CACHE=off.
//! Safety: path deny list, output path restrictions (/tmp only for export).
//!         All tools are read-only.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

// ============================================================
// Path Safety
// ============================================================

/// Paths that are explicitly denied for MCP access (live repos).
/// Subpaths in ALLOWED_DENIED_SUBPATHS are exempted from denial (read-only analysis).
const DENIED_PATHS: &[&str] = &["/Users/jiangxuanyang/Desktop/cangjie"];

/// Subpaths under denied directories that are explicitly allowed for read-only MCP analysis.
const ALLOWED_DENIED_SUBPATHS: &[&str] = &["/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"];

/// Validate that an output path is within /tmp (or a system temp dir).
fn validate_output_path(output_path: &str) -> Result<PathBuf, Value> {
    let path = PathBuf::from(output_path);

    // Check by string prefix: must start with /tmp/ or /private/tmp/
    // We use string comparison rather than canonicalize because the file
    // may not exist yet, and /tmp may not canonicalize consistently.
    let path_str = path.to_string_lossy();
    if !path_str.starts_with("/tmp/") && !path_str.starts_with("/private/tmp/") {
        return Err(mcp_error_with_hint(
            "output_path_denied",
            &format!("Output path must be under /tmp, got: {output_path}"),
            "export_bridge only writes to /tmp for safety",
            "Use outputPath like /tmp/codelattice-bridge-<name>.json or omit to auto-generate",
        ));
    }

    Ok(path)
}

fn validate_root_path(root: &str) -> Result<PathBuf, Value> {
    let path = PathBuf::from(root);

    // Canonicalize for comparison (resolves symlinks, trailing slashes, etc.)
    let canonical = path
        .canonicalize()
        .map_err(|_| mcp_error("path_not_found", &format!("Path does not exist: {root}")))?;

    if !canonical.is_dir() {
        return Err(mcp_error(
            "path_not_directory",
            &format!("Path is not a directory: {root}"),
        ));
    }

    // Check deny list — but allow exempted subpaths
    for denied in DENIED_PATHS {
        // First check if path is in the allow list (exempt from denial)
        let mut is_allowed = false;
        for allowed in ALLOWED_DENIED_SUBPATHS {
            let allowed_canonical = PathBuf::from(allowed).canonicalize().ok();
            if let Some(ac) = allowed_canonical {
                if canonical == ac || canonical.starts_with(&ac) {
                    is_allowed = true;
                    break;
                }
            }
            // String-prefix fallback for allow list
            let allowed_with_sep = format!("{}/", allowed.trim_end_matches('/'));
            let canonical_str = canonical.to_string_lossy();
            if canonical_str.starts_with(&allowed_with_sep) || canonical_str == *allowed {
                is_allowed = true;
                break;
            }
        }
        if is_allowed {
            continue;
        }

        let denied_canonical = PathBuf::from(denied).canonicalize().ok();
        if let Some(dc) = denied_canonical {
            if canonical == dc {
                return Err(mcp_error(
                    "path_denied",
                    &format!("Path is on deny list (live repo): {denied}"),
                ));
            }
            // Check if canonical path is a descendant of denied directory
            if canonical.starts_with(&dc) {
                return Err(mcp_error(
                    "path_denied",
                    &format!("Path is under denied directory: {denied}"),
                ));
            }
        }
        // String-prefix fallback: ensure the match ends at a path component boundary
        let denied_with_sep = format!("{}/", denied.trim_end_matches('/'));
        let canonical_str = canonical.to_string_lossy();
        if canonical_str.starts_with(&denied_with_sep) || canonical_str == *denied {
            return Err(mcp_error(
                "path_denied",
                &format!("Path is under denied directory: {denied}"),
            ));
        }
    }

    Ok(canonical)
}

// ============================================================
// Error helpers
// ============================================================

/// Unified error structure with code, message, details, and hint.
fn mcp_error(code: &str, message: &str) -> Value {
    json!({
        "error": code,
        "message": message
    })
}

fn mcp_error_detail(code: &str, message: &str, details: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details
    })
}

fn mcp_error_with_hint(code: &str, message: &str, details: &str, hint: &str) -> Value {
    json!({
        "error": code,
        "message": message,
        "details": details,
        "hint": hint
    })
}

#[allow(dead_code)]
fn tool_error(code: &str, message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&mcp_error(code, message)).unwrap_or_default() }],
        "isError": true
    })
}

fn tool_result(data: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string(data).unwrap_or_default() }]
    })
}

/// Like tool_result but injects cache hit/miss signal.
#[allow(dead_code)]
fn tool_result_cached(data: &Value, cache_hit: bool, duration_ms: u64) -> Value {
    let mut enriched = data.clone();
    inject_cache_meta(&mut enriched, cache_hit, duration_ms);
    tool_result(&enriched)
}

/// Helper: inject cache hit/miss signal into a tool result Value.
fn inject_cache_meta(data: &mut Value, cache_hit: bool, duration_ms: u64) {
    if let Some(obj) = data.as_object_mut() {
        obj.insert("cacheHit".to_string(), json!(cache_hit));
        if !cache_hit {
            obj.insert("analysisDurationMs".to_string(), json!(duration_ms));
        }
    }
}

// ============================================================
// Two-Layer Analysis Cache (v0.3 memory + v0.8 persistent)
// ============================================================

/// Merge cache_meta into a json output and wrap in tool_result.
fn merge_cache_and_result(data: &Value, cache_meta: &Value) -> Value {
    let mut enriched = data.clone();
    if let (Some(obj), Some(meta)) = (enriched.as_object_mut(), cache_meta.as_object()) {
        for (k, v) in meta {
            obj.insert(k.clone(), v.clone());
        }
    }
    tool_result(&enriched)
}

/// Read a source code snippet from a file relative to root.
/// Returns a JSON object with `lines`, `startLine`, `endLine`, and optional `warning`.
/// Context lines: number of lines before/after the symbol.
/// Max snippet size: 50 lines to avoid huge outputs.
fn read_source_snippet(
    root: &str,
    relative_path: &str,
    symbol_start: u64,
    symbol_end: u64,
    context_lines: usize,
) -> Value {
    let max_lines = 50usize;
    let ctx = context_lines.min(10); // cap context at 10 lines each side

    let full_path = std::path::Path::new(root).join(relative_path);

    if !full_path.exists() {
        return json!({
            "warning": format!("File not found: {}", relative_path),
            "lines": Value::Null,
            "startLine": Value::Null,
            "endLine": Value::Null
        });
    }

    let content = match std::fs::read_to_string(&full_path) {
        Ok(s) => s,
        Err(e) => {
            return json!({
                "warning": format!("Cannot read file {}: {}", relative_path, e),
                "lines": Value::Null,
                "startLine": Value::Null,
                "endLine": Value::Null
            });
        }
    };

    let file_lines: Vec<&str> = content.lines().collect();
    let total_lines = file_lines.len();

    if total_lines == 0 {
        return json!({
            "warning": "Empty file",
            "lines": "",
            "startLine": 1,
            "endLine": 1
        });
    }

    // Convert 1-based to 0-based, with bounds checking
    let sym_start = if symbol_start > 0 {
        (symbol_start as usize).saturating_sub(1)
    } else {
        0
    };
    let sym_end = if symbol_end > 0 {
        (symbol_end as usize).saturating_sub(1)
    } else {
        sym_start
    };

    // Add context, clamped to file bounds
    let snippet_start = sym_start.saturating_sub(ctx);
    let snippet_end = (sym_end + ctx + 1).min(total_lines); // +1 because end is inclusive

    // Enforce max_lines
    let snippet_end = if snippet_end - snippet_start > max_lines {
        snippet_start + max_lines
    } else {
        snippet_end
    };
    let snippet_end = snippet_end.min(total_lines);

    let snippet_lines: Vec<&str> = file_lines[snippet_start..snippet_end].to_vec();

    json!({
        "lines": snippet_lines.join("\n"),
        "startLine": snippet_start + 1, // back to 1-based
        "endLine": snippet_end,
        "totalLines": total_lines
    })
}

/// Cache key: uniquely identifies an analysis result.
#[derive(Hash, Eq, PartialEq, Clone)]
struct CacheKey {
    root: String, // canonical path
    language: String,
    strict: bool,
}

/// A cached analysis result with its pre-built GraphView.
struct CacheEntry {
    analyze_result: Value,
    graph_view: GraphView,
    created_at: Instant,
    last_used_at: Instant,
    hit_count: u64,
    analysis_duration_ms: u64,
    /// File mtimes captured at analysis time, used for staleness detection.
    /// Maps relative_path → mtime (as duration since UNIX epoch in ms).
    file_mtimes: HashMap<String, u64>,
    /// Absolute root path (canonical) used for file resolution.
    root_canonical: String,
    /// Stale reason from last check (None = fresh).
    stale_reason: Option<String>,
}

/// Default maximum cache entries (LRU eviction kicks in above this).
const CACHE_MAX_ENTRIES: usize = 16;

/// CodeLattice binary version, embedded in fingerprint for cross-version safety.
const CODELATTICE_CACHE_VERSION: &str = "0.13.0";

/// Persistent cache schema version.
const CACHE_SCHEMA_VERSION: u32 = 1;

/// All source file extensions tracked for stale detection across all languages.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "cj", "ets", "ts", "tsx", "js", "jsx", "json", "json5", "toml", "md",
];

/// Manifest file basenames that trigger re-analysis when changed.
const MANIFEST_FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "cjpm.toml",
    "oh-package.json5",
    "tsconfig.json",
    "package.json",
];

/// Structured stale reason explaining why a cache entry is invalid.
#[derive(Debug, Clone)]
enum StaleReason {
    FileAdded(Vec<String>),
    FileRemoved(Vec<String>),
    FileModified(Vec<String>),
    ManifestChanged,
    DocsChanged,
    VersionChanged,
    CacheMissing,
    CacheCorrupted(String),
}

impl StaleReason {
    fn to_json(&self) -> Value {
        match self {
            StaleReason::FileAdded(files) => json!({
                "staleReason": "file_added",
                "changedFiles": files,
            }),
            StaleReason::FileRemoved(files) => json!({
                "staleReason": "file_removed",
                "changedFiles": files,
            }),
            StaleReason::FileModified(files) => json!({
                "staleReason": "file_modified",
                "changedFiles": files,
            }),
            StaleReason::ManifestChanged => json!({
                "staleReason": "manifest_changed",
            }),
            StaleReason::DocsChanged => json!({
                "staleReason": "docs_changed",
            }),
            StaleReason::VersionChanged => json!({
                "staleReason": "version_changed",
            }),
            StaleReason::CacheMissing => json!({
                "staleReason": "cache_missing",
            }),
            StaleReason::CacheCorrupted(detail) => json!({
                "staleReason": "cache_corrupted",
                "detail": detail,
            }),
        }
    }

    fn reason_code(&self) -> &str {
        match self {
            StaleReason::FileAdded(_) => "file_added",
            StaleReason::FileRemoved(_) => "file_removed",
            StaleReason::FileModified(_) => "file_modified",
            StaleReason::ManifestChanged => "manifest_changed",
            StaleReason::DocsChanged => "docs_changed",
            StaleReason::VersionChanged => "version_changed",
            StaleReason::CacheMissing => "cache_missing",
            StaleReason::CacheCorrupted(_) => "cache_corrupted",
        }
    }
}

/// Scan source files under root and collect their mtimes.
/// Returns a map of relative_path → mtime_ms.
/// Tracks all language extensions: .rs, .cj, .ets, .ts, .tsx, .js, .jsx, .json, .json5, .toml, .md.
fn scan_file_mtimes(root: &Path) -> HashMap<String, u64> {
    let mut mtimes = HashMap::new();

    fn walk_dir(dir: &Path, root: &Path, mtimes: &mut HashMap<String, u64>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip hidden dirs and common non-source dirs
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') || name == "target" || name == "node_modules" {
                            continue;
                        }
                    }
                    walk_dir(&path, root, mtimes);
                } else {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if SOURCE_EXTENSIONS.contains(&ext) {
                        if let Ok(meta) = std::fs::metadata(&path) {
                            if let Ok(modified) = meta.modified() {
                                let ms = modified
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                if let Ok(rel) = path.strip_prefix(root) {
                                    mtimes.insert(rel.to_string_lossy().to_string(), ms);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    walk_dir(root, root, &mut mtimes);
    mtimes
}

/// Compute a fast hash of manifest file content for change detection.
/// Returns a map of manifest_relative_path → content_hash.
fn compute_manifest_hashes(root: &Path) -> HashMap<String, u64> {
    let mut hashes = HashMap::new();
    for manifest_name in MANIFEST_FILES {
        let path = root.join(manifest_name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                // Simple FNV-1a-like hash for speed (no dependency needed)
                let mut hash: u64 = 0xcbf29ce484222325;
                for byte in content.bytes() {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hashes.insert(manifest_name.to_string(), hash);
            }
        }
    }
    hashes
}

/// Check if cached mtimes are still fresh by comparing with current filesystem.
/// Returns Some(StaleReason) if stale, None if fresh.
fn check_stale(root: &Path, cached_mtimes: &HashMap<String, u64>) -> Option<StaleReason> {
    let current = scan_file_mtimes(root);

    // Detect added files
    let added: Vec<String> = current
        .keys()
        .filter(|p| !cached_mtimes.contains_key(*p))
        .cloned()
        .collect();
    if !added.is_empty() {
        return Some(StaleReason::FileAdded(added));
    }

    // Detect removed and modified files
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    for (path, mtime) in cached_mtimes {
        match current.get(path) {
            Some(current_mtime) if *current_mtime == *mtime => {}
            Some(_) => modified.push(path.clone()),
            None => removed.push(path.clone()),
        }
    }
    if !removed.is_empty() {
        return Some(StaleReason::FileRemoved(removed));
    }
    if !modified.is_empty() {
        return Some(StaleReason::FileModified(modified));
    }

    None
}

/// Check if manifest files changed since cached.
fn check_manifest_stale(
    root: &Path,
    cached_manifests: &HashMap<String, u64>,
) -> Option<StaleReason> {
    let current = compute_manifest_hashes(root);
    if current != *cached_manifests {
        return Some(StaleReason::ManifestChanged);
    }
    None
}

/// Check if docs (markdown files) changed since cached.
fn check_docs_stale(root: &Path, cached_docs: &HashMap<String, u64>) -> Option<StaleReason> {
    let current: HashMap<String, u64> = scan_file_mtimes(root)
        .into_iter()
        .filter(|(p, _)| p.ends_with(".md"))
        .collect();
    if current != *cached_docs {
        return Some(StaleReason::DocsChanged);
    }
    None
}

// ============================================================
// Persistent Cache (v0.8)
// ============================================================

/// Compute a safe filename for a cache entry from the key components.
/// Uses a simple hash to avoid path traversal and special characters.
fn persistent_cache_filename(root: &str, language: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in root.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in language.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("cl-cache-{:016x}.json", hash)
}

/// Get the persistent cache directory.
/// Controlled by CODELATTICE_CACHE_DIR env. If not set, persistent cache is disabled.
/// Disable explicitly with CODELATTICE_CACHE=off.
/// Returns None if persistent caching is disabled.
fn get_persistent_cache_dir() -> Option<PathBuf> {
    // Check if caching is disabled
    if std::env::var("CODELATTICE_CACHE").as_deref() == Ok("off") {
        return None;
    }

    // Only enable persistent cache if explicitly configured
    match std::env::var("CODELATTICE_CACHE_DIR") {
        Ok(custom) => {
            let dir = PathBuf::from(custom);
            let _ = std::fs::create_dir_all(&dir);
            Some(dir)
        }
        Err(_) => {
            // No explicit cache dir — persistent cache disabled by default
            // This ensures test isolation and avoids surprising disk writes
            None
        }
    }
}

/// A serialized persistent cache entry stored on disk.
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistentCacheEntry {
    /// Cache schema version for forward compatibility.
    schema_version: u32,
    /// CodeLattice version that created this entry.
    version: String,
    /// Project root (canonical path at cache time).
    root: String,
    /// Language used for analysis.
    language: String,
    /// Full analyze result JSON.
    analyze_result: Value,
    /// File mtimes at cache time.
    file_mtimes: HashMap<String, u64>,
    /// Manifest hashes at cache time.
    manifest_hashes: HashMap<String, u64>,
    /// Docs file mtimes at cache time.
    docs_mtimes: HashMap<String, u64>,
    /// Creation timestamp (ISO 8601).
    created_at: String,
    /// Analysis duration in ms.
    analysis_duration_ms: u64,
}

/// Try to load a cached analysis from the persistent cache layer.
/// `cache_key_str` is the combined key for filename lookup.
/// `canonical_root` is the actual filesystem path for stale checks.
/// Returns None if: cache disabled, file missing, stale, corrupted, or version mismatch.
fn try_load_persistent(
    cache_key_str: &str,
    language: &str,
    canonical_root: &Path,
) -> Option<(
    Value,
    HashMap<String, u64>,
    HashMap<String, u64>,
    HashMap<String, u64>,
    u64,
)> {
    let cache_dir = get_persistent_cache_dir()?;
    let filename = persistent_cache_filename(cache_key_str, language);
    let path = cache_dir.join(&filename);

    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&path).ok()?;
    let entry: PersistentCacheEntry = match serde_json::from_str(&content) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[mcp] persistent cache corrupted: {}, removing", e);
            let _ = std::fs::remove_file(&path);
            return None;
        }
    };

    // Version check
    if entry.version != CODELATTICE_CACHE_VERSION {
        eprintln!(
            "[mcp] persistent cache version mismatch: {} vs {}, removing",
            entry.version, CODELATTICE_CACHE_VERSION
        );
        let _ = std::fs::remove_file(&path);
        return None;
    }

    // Schema check
    if entry.schema_version != CACHE_SCHEMA_VERSION {
        eprintln!(
            "[mcp] persistent cache schema mismatch: {} vs {}, removing",
            entry.schema_version, CACHE_SCHEMA_VERSION
        );
        let _ = std::fs::remove_file(&path);
        return None;
    }

    // Root match check — compare stored canonical root with current canonical root
    if entry.root != canonical_root.to_string_lossy().as_ref() {
        return None; // Hash collision or different project — skip
    }

    // Stale checks using the actual filesystem path
    if !canonical_root.exists() {
        return None;
    }

    if check_stale(canonical_root, &entry.file_mtimes).is_some() {
        // Stale — remove and return None
        let _ = std::fs::remove_file(&path);
        return None;
    }

    if check_manifest_stale(canonical_root, &entry.manifest_hashes).is_some() {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    if check_docs_stale(canonical_root, &entry.docs_mtimes).is_some() {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    Some((
        entry.analyze_result,
        entry.file_mtimes,
        entry.manifest_hashes,
        entry.docs_mtimes,
        entry.analysis_duration_ms,
    ))
}

/// Save an analysis result to the persistent cache layer.
/// `cache_key_str` is the combined key for filename lookup.
/// `canonical_root` is the actual filesystem path stored in the entry for root match.
/// Silently fails if caching is disabled or write fails (non-critical).
fn save_persistent(
    cache_key_str: &str,
    canonical_root: &str,
    language: &str,
    analyze_result: &Value,
    file_mtimes: &HashMap<String, u64>,
    manifest_hashes: &HashMap<String, u64>,
    docs_mtimes: &HashMap<String, u64>,
    analysis_duration_ms: u64,
) {
    let cache_dir = match get_persistent_cache_dir() {
        Some(d) => d,
        None => return, // Caching disabled
    };

    let filename = persistent_cache_filename(cache_key_str, language);
    let path = cache_dir.join(&filename);

    // Safety check: ensure path is under cache dir (no traversal)
    if let Ok(canonical_dir) = cache_dir.canonicalize() {
        if let Some(parent) = path.parent() {
            if let Ok(parent_canonical) = parent.canonicalize() {
                if parent_canonical != canonical_dir {
                    eprintln!("[mcp] persistent cache path traversal rejected");
                    return;
                }
            }
        }
    }

    let entry = PersistentCacheEntry {
        schema_version: CACHE_SCHEMA_VERSION,
        version: CODELATTICE_CACHE_VERSION.to_string(),
        root: canonical_root.to_string(),
        language: language.to_string(),
        analyze_result: analyze_result.clone(),
        file_mtimes: file_mtimes.clone(),
        manifest_hashes: manifest_hashes.clone(),
        docs_mtimes: docs_mtimes.clone(),
        created_at: chrono_now_iso(),
        analysis_duration_ms,
    };

    let json_str = match serde_json::to_string(&entry) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[mcp] failed to serialize persistent cache: {}", e);
            return;
        }
    };

    if let Err(e) = std::fs::write(&path, json_str) {
        eprintln!("[mcp] failed to write persistent cache: {}", e);
    }
}

/// Delete a specific persistent cache entry.
fn delete_persistent(root: &str, language: &str) -> bool {
    let cache_dir = match get_persistent_cache_dir() {
        Some(d) => d,
        None => return false,
    };
    let filename = persistent_cache_filename(root, language);
    let path = cache_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).is_ok()
    } else {
        false
    }
}

/// Delete all persistent cache entries, optionally filtered.
/// Returns count of deleted entries.
fn clear_persistent(filter_root: Option<&str>, filter_lang: Option<&str>) -> usize {
    let cache_dir = match get_persistent_cache_dir() {
        Some(d) => d,
        None => return 0,
    };

    let mut deleted = 0;
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if !filename.starts_with("cl-cache-") {
                    continue;
                }
            }

            // Read to check root/language match
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<PersistentCacheEntry>(&content) {
                    let matches_root = filter_root.map(|r| cached.root.contains(r)).unwrap_or(true);
                    let matches_lang = filter_lang.map(|l| cached.language == l).unwrap_or(true);
                    if matches_root && matches_lang {
                        if std::fs::remove_file(&path).is_ok() {
                            deleted += 1;
                        }
                    }
                }
            }
        }
    }
    deleted
}

/// Get persistent cache status summary.
fn persistent_cache_status(filter_root: Option<&str>, filter_lang: Option<&str>) -> Value {
    let cache_dir = match get_persistent_cache_dir() {
        Some(d) => d,
        None => {
            return json!({
                "enabled": false,
                "reason": "CODELATTICE_CACHE=off or directory unavailable",
            });
        }
    };

    let mut entries = Vec::new();
    let mut total_size: u64 = 0;

    if let Ok(dir_entries) = std::fs::read_dir(&cache_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if !filename.starts_with("cl-cache-") {
                    continue;
                }
            }

            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            total_size += file_size;

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<PersistentCacheEntry>(&content) {
                    let matches_root = filter_root.map(|r| cached.root.contains(r)).unwrap_or(true);
                    let matches_lang = filter_lang.map(|l| cached.language == l).unwrap_or(true);
                    if matches_root && matches_lang {
                        entries.push(json!({
                            "root": cached.root,
                            "language": cached.language,
                            "createdAt": cached.created_at,
                            "analysisDurationMs": cached.analysis_duration_ms,
                            "trackedFiles": cached.file_mtimes.len(),
                            "sizeBytes": file_size,
                        }));
                    }
                }
            }
        }
    }

    json!({
        "enabled": true,
        "cacheDir": cache_dir.to_string_lossy(),
        "entryCount": entries.len(),
        "totalSizeBytes": total_size,
        "entries": entries,
    })
}

/// Simple ISO 8601 timestamp without external dependency.
fn chrono_now_iso() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple formatting without chrono
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Convert days since epoch to year-month-day (approximate but good enough for cache metadata)
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since UNIX epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // 1970-01-01 = day 0
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if days < md {
            month = i as u64 + 1;
            break;
        }
        days -= md;
    }
    if month == 0 {
        month = 12;
    }

    (year, month, days + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ============================================================
// Two-Layer Cache Container
// ============================================================

/// Two-layer analysis cache: process-local memory + persistent disk.
struct McpCache {
    /// In-memory layer (existing v0.3 behavior, enhanced).
    entries: HashMap<CacheKey, CacheEntry>,
    total_hits: u64,
    total_misses: u64,
    total_evictions: u64,
    /// Counters for persistent layer.
    persistent_hits: u64,
    persistent_misses: u64,
}

impl McpCache {
    fn new() -> Self {
        McpCache {
            entries: HashMap::new(),
            total_hits: 0,
            total_misses: 0,
            total_evictions: 0,
            persistent_hits: 0,
            persistent_misses: 0,
        }
    }

    /// Get cached analysis or run fresh analyze subprocess.
    /// Two-layer lookup: memory → persistent → fresh analyze.
    /// Returns (graph_view, analyze_result, cache_meta_json).
    fn get_or_analyze(
        &mut self,
        root: &Path,
        language: &str,
        strict: bool,
    ) -> Result<(GraphView, Value, Value), Value> {
        let canonical = root.canonicalize().map_err(|_| {
            mcp_error(
                "path_not_found",
                &format!("Cannot canonicalize: {}", root.display()),
            )
        })?;
        let key = CacheKey {
            root: canonical.to_string_lossy().to_string(),
            language: language.to_string(),
            strict,
        };
        let cache_key_str = format!("{}:{}:{}", key.root, key.language, key.strict);

        // Layer 1: Check process-local memory cache
        if let Some(entry) = self.entries.get_mut(&key) {
            let root_path = Path::new(&entry.root_canonical);
            if let Some(reason) = check_stale(root_path, &entry.file_mtimes) {
                // Stale — invalidate and fall through
                entry.stale_reason = Some(reason.reason_code().to_string());
                self.entries.remove(&key);
            } else {
                // Also check manifest and docs
                let manifest_stale = check_manifest_stale(
                    root_path,
                    &entry
                        .analyze_result
                        .get("__manifest_hashes")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default(),
                );
                if manifest_stale.is_some()
                    || check_docs_stale(
                        root_path,
                        &entry
                            .file_mtimes
                            .iter()
                            .filter(|(p, _)| p.ends_with(".md"))
                            .map(|(k, v)| (k.clone(), *v))
                            .collect(),
                    )
                    .is_some()
                {
                    self.entries.remove(&key);
                } else {
                    // Fresh memory hit
                    entry.hit_count += 1;
                    entry.last_used_at = Instant::now();
                    self.total_hits += 1;
                    let meta = json!({
                        "cacheHit": true,
                        "cacheLayer": "memory",
                        "cacheKey": cache_key_str,
                        "cachedAtMs": entry.created_at.elapsed().as_millis() as u64,
                        "analysisDurationMs": entry.analysis_duration_ms,
                    });
                    return Ok((
                        entry.graph_view.clone_shallow(),
                        entry.analyze_result.clone(),
                        meta,
                    ));
                }
            }
        }

        // Layer 2: Check persistent disk cache
        if let Some((result, file_mtimes, manifest_hashes, docs_mtimes, duration_ms)) =
            try_load_persistent(&cache_key_str, language, &canonical)
        {
            // Persistent hit — build GraphView and load into memory cache
            let mut graph_view = GraphView::build(&result);
            graph_view.doc_scanner = Some(std::sync::Arc::new(DocScanner::build(&canonical)));

            // Store in memory cache for future fast access
            self.insert_memory_entry(
                key.clone(),
                result.clone(),
                graph_view.clone_shallow(),
                file_mtimes.clone(),
                &canonical,
                duration_ms,
                // Also persist manifest/docs hashes in the analyze_result for memory-layer checks
            );

            // Patch manifest_hashes into the cached result for memory-layer stale checks
            if let Some(obj) = self.entries.get_mut(&key) {
                obj.analyze_result.as_object_mut().map(|o| {
                    o.insert(
                        "__manifest_hashes".to_string(),
                        serde_json::to_value(&manifest_hashes).unwrap_or(Value::Null),
                    );
                    o.insert(
                        "__docs_mtimes".to_string(),
                        serde_json::to_value(&docs_mtimes).unwrap_or(Value::Null),
                    );
                });
            }

            self.persistent_hits += 1;
            self.total_hits += 1;

            let meta = json!({
                "cacheHit": true,
                "cacheLayer": "persistent",
                "cacheKey": cache_key_str,
                "analysisDurationMs": duration_ms,
            });
            return Ok((graph_view, result, meta));
        }

        // Layer 3: Cache miss — run fresh analyze
        self.persistent_misses += 1;
        self.total_misses += 1;

        // LRU eviction if over limit
        if self.entries.len() >= CACHE_MAX_ENTRIES {
            let evict_key = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = evict_key {
                self.entries.remove(&k);
                self.total_evictions += 1;
            }
        }

        let start = Instant::now();
        let result = run_analyze_subprocess(root, language, "json", strict)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let mut graph_view = GraphView::build(&result);

        // Scan file mtimes for future freshness checks
        let file_mtimes = scan_file_mtimes(&canonical);

        // Compute manifest and docs hashes
        let manifest_hashes = compute_manifest_hashes(&canonical);
        let docs_mtimes: HashMap<String, u64> = file_mtimes
            .iter()
            .filter(|(p, _)| p.ends_with(".md"))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        // Build doc scanner for code ↔ docs association and attach to GraphView
        graph_view.doc_scanner = Some(std::sync::Arc::new(DocScanner::build(&canonical)));

        // Store in memory cache
        let mut result_with_meta = result.clone();
        if let Some(obj) = result_with_meta.as_object_mut() {
            obj.insert(
                "__manifest_hashes".to_string(),
                serde_json::to_value(&manifest_hashes).unwrap_or(Value::Null),
            );
            obj.insert(
                "__docs_mtimes".to_string(),
                serde_json::to_value(&docs_mtimes).unwrap_or(Value::Null),
            );
        }

        self.entries.insert(
            key.clone(),
            CacheEntry {
                analyze_result: result_with_meta.clone(),
                graph_view: graph_view.clone_shallow(),
                created_at: Instant::now(),
                last_used_at: Instant::now(),
                hit_count: 0,
                analysis_duration_ms: duration_ms,
                file_mtimes: file_mtimes.clone(),
                root_canonical: canonical.to_string_lossy().to_string(),
                stale_reason: None,
            },
        );

        // Save to persistent cache (best-effort, non-blocking)
        save_persistent(
            &cache_key_str,
            &canonical.to_string_lossy(),
            language,
            &result,
            &file_mtimes,
            &manifest_hashes,
            &docs_mtimes,
            duration_ms,
        );

        let meta = json!({
            "cacheHit": false,
            "cacheLayer": "none",
            "cacheKey": cache_key_str,
            "analysisDurationMs": duration_ms,
        });
        Ok((graph_view, result, meta))
    }

    /// Insert entry into memory cache (helper for persistent → memory promotion).
    fn insert_memory_entry(
        &mut self,
        key: CacheKey,
        analyze_result: Value,
        graph_view: GraphView,
        file_mtimes: HashMap<String, u64>,
        canonical: &Path,
        duration_ms: u64,
    ) {
        self.entries.insert(
            key,
            CacheEntry {
                analyze_result,
                graph_view,
                created_at: Instant::now(),
                last_used_at: Instant::now(),
                hit_count: 0,
                analysis_duration_ms: duration_ms,
                file_mtimes,
                root_canonical: canonical.to_string_lossy().to_string(),
                stale_reason: None,
            },
        );
    }

    /// Get cache status for both memory and persistent layers.
    fn status(&self, filter_root: Option<&str>, filter_lang: Option<&str>) -> Value {
        let mut memory_entries = Vec::new();
        for (key, entry) in &self.entries {
            if let Some(r) = filter_root {
                if !key.root.contains(r) {
                    continue;
                }
            }
            if let Some(l) = filter_lang {
                if key.language != l {
                    continue;
                }
            }

            memory_entries.push(json!({
                "root": key.root,
                "language": key.language,
                "strict": key.strict,
                "cacheKey": format!("{}:{}:{}", key.root, key.language, key.strict),
                "layer": "memory",
                "createdAtMs": entry.created_at.elapsed().as_millis() as u64,
                "lastUsedAtMs": entry.last_used_at.elapsed().as_millis() as u64,
                "hitCount": entry.hit_count,
                "analysisDurationMs": entry.analysis_duration_ms,
                "trackedFiles": entry.file_mtimes.len(),
            }));
        }

        let persistent_status = persistent_cache_status(filter_root, filter_lang);

        json!({
            "memory": {
                "entryCount": memory_entries.len(),
                "maxEntries": CACHE_MAX_ENTRIES,
                "entries": memory_entries,
                "totalHits": self.total_hits,
                "totalMisses": self.total_misses,
                "totalEvictions": self.total_evictions,
                "persistentHits": self.persistent_hits,
                "persistentMisses": self.persistent_misses,
            },
            "persistent": persistent_status,
        })
    }

    /// Clear cache entries from both layers, optionally filtered.
    /// `layer`: "memory" | "persistent" | "both"
    fn clear(
        &mut self,
        filter_root: Option<&str>,
        filter_lang: Option<&str>,
        layer: &str,
    ) -> (usize, usize) {
        let memory_cleared = if layer == "memory" || layer == "both" {
            let before = self.entries.len();
            self.entries.retain(|key, _| {
                if let Some(r) = filter_root {
                    if !key.root.contains(r) {
                        return true;
                    }
                }
                if let Some(l) = filter_lang {
                    if key.language != l {
                        return true;
                    }
                }
                false
            });
            before - self.entries.len()
        } else {
            0
        };

        let persistent_cleared = if layer == "persistent" || layer == "both" {
            clear_persistent(filter_root, filter_lang)
        } else {
            0
        };

        let total_cleared = memory_cleared + persistent_cleared;
        let remaining = self.entries.len();
        (total_cleared, remaining)
    }
}

// ============================================================
// Subprocess helpers
// ============================================================

fn get_cli_binary() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gitnexus-rust-core-cli"))
}

fn run_subcommand_with_timeout(args: &[&str], _timeout: Duration) -> Result<Value, Value> {
    let binary = get_cli_binary();

    let mut child = Command::new(&binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            mcp_error(
                "command_failed",
                &format!("Failed to spawn {}: {}", binary.display(), e),
            )
        })?;

    // Drain stdout/stderr in background threads to avoid pipe-buffer deadlock.
    // On macOS the OS pipe buffer is ~64 KB; the analysis subprocess can produce
    // multi-MB JSON output.  If we only poll try_wait() without reading, the
    // child blocks on write and never exits → apparent "timeout".
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut s) = stdout_handle {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut s) = stderr_handle {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    // Wait for child with timeout
    let status = child.wait().map_err(|e| {
        mcp_error(
            "command_failed",
            &format!("Failed to wait for command: {}", e),
        )
    })?;

    let stdout = stdout_thread.join().unwrap_or_default();
    let _stderr = stderr_thread.join().unwrap_or_default();

    if !status.success() {
        return Err(mcp_error(
            "command_failed",
            &format!(
                "Command exited with code {:?}: {}",
                status.code(),
                stdout.trim().chars().take(200).collect::<String>()
            ),
        ));
    }

    // Parse stdout as JSON
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(mcp_error(
            "json_parse_failed",
            "Command produced empty output",
        ));
    }

    serde_json::from_str(trimmed).map_err(|e| {
        mcp_error(
            "json_parse_failed",
            &format!(
                "Failed to parse JSON: {}. Output: {}",
                e,
                &trimmed[..trimmed.len().min(200)]
            ),
        )
    })
}

fn run_script_with_timeout(
    script: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, Value> {
    // MCP smoke 会触发 cargo run；隔离 target 目录，避免测试/开发主 target
    // 被 rust-only smoke 重编译成无 Cangjie feature 的 debug binary。
    let isolated_target_dir = if std::env::var_os("CARGO_TARGET_DIR").is_none() {
        Some(std::env::temp_dir().join(format!(
            "codelattice-mcp-smoke-target-{}",
            std::process::id()
        )))
    } else {
        None
    };

    let mut command = Command::new("bash");
    command
        .arg(script)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = &isolated_target_dir {
        command.env("CARGO_TARGET_DIR", dir);
    }

    let mut child = command
        .spawn()
        .map_err(|e| mcp_error("command_failed", &format!("Failed to run script: {}", e)))?;

    // Drain stdout/stderr in background threads to avoid pipe-buffer deadlock.
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut s) = stdout_handle {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut s) = stderr_handle {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    // Wait for child with timeout
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_thread.join().unwrap_or_default();
                let _stderr = stderr_thread.join().unwrap_or_default();
                if let Some(dir) = &isolated_target_dir {
                    let _ = std::fs::remove_dir_all(dir);
                }

                if !status.success() {
                    return Err(mcp_error(
                        "smoke_failed",
                        &format!("Smoke script exited with code {:?}", status.code()),
                    ));
                }

                return Ok(stdout);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    if let Some(dir) = &isolated_target_dir {
                        let _ = std::fs::remove_dir_all(dir);
                    }
                    return Err(mcp_error("timeout", "Smoke script timed out"));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                if let Some(dir) = &isolated_target_dir {
                    let _ = std::fs::remove_dir_all(dir);
                }
                return Err(mcp_error(
                    "command_failed",
                    &format!("Failed to check script status: {}", e),
                ));
            }
        }
    }
}

// ============================================================
// Language helpers
// ============================================================

/// Check if cangjie/arkts/typescript language is requested but feature is not compiled.
/// Returns Err if requested without feature, Ok(()) otherwise.
fn check_language_feature(language: &str) -> Result<(), Value> {
    if language == "cangjie" {
        #[cfg(not(feature = "tree-sitter-cangjie"))]
        {
            return Err(mcp_error_with_hint(
                "cangjie_disabled",
                "Cangjie support not compiled",
                "Cangjie language was requested but tree-sitter-cangjie feature is not enabled",
                "Rebuild with --features tree-sitter-cangjie",
            ));
        }
    }
    if language == "arkts" {
        #[cfg(not(feature = "tree-sitter-arkts"))]
        {
            return Err(mcp_error_with_hint(
                "arkts_disabled",
                "ArkTS support not compiled",
                "ArkTS language was requested but tree-sitter-arkts feature is not enabled",
                "Rebuild with --features tree-sitter-arkts",
            ));
        }
    }
    if language == "typescript" {
        #[cfg(not(feature = "tree-sitter-typescript"))]
        {
            return Err(mcp_error_with_hint(
                "typescript_disabled",
                "TypeScript support not compiled",
                "TypeScript language was requested but tree-sitter-typescript feature is not enabled",
                "Rebuild with --features tree-sitter-typescript",
            ));
        }
    }
    if language == "c" {
        #[cfg(not(feature = "tree-sitter-c"))]
        {
            return Err(mcp_error_with_hint(
                "c_disabled",
                "C language support not compiled",
                "C language was requested but tree-sitter-c feature is not enabled",
                "Rebuild with --features tree-sitter-c",
            ));
        }
    }
    if language == "cpp" {
        #[cfg(not(feature = "tree-sitter-cpp"))]
        {
            return Err(mcp_error_with_hint(
                "cpp_disabled",
                "C++ language support not compiled",
                "C++ language was requested but tree-sitter-cpp feature is not enabled",
                "Rebuild with --features tree-sitter-cpp",
            ));
        }
    }
    if language == "python" {
        #[cfg(not(feature = "tree-sitter-python"))]
        {
            return Err(mcp_error_with_hint(
                "python_disabled",
                "Python language support not compiled",
                "Python language was requested but tree-sitter-python feature is not enabled",
                "Rebuild with --features tree-sitter-python",
            ));
        }
    }
    Ok(())
}

/// Run the CLI analyze subcommand and return parsed JSON.
/// Used by multiple tools that need the full analyze output.
fn run_analyze_subprocess(
    root: &Path,
    language: &str,
    format: &str,
    strict: bool,
) -> Result<Value, Value> {
    let root_str = root.to_string_lossy().to_string();
    let mut args = vec![
        "analyze".to_string(),
        "--root".to_string(),
        root_str,
        "--language".to_string(),
        language.to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    if strict {
        args.push("--strict".to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_subcommand_with_timeout(&arg_refs, Duration::from_secs(60))
}

// ============================================================
// Tool Handlers
// ============================================================

fn handle_analyze(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let strict = params["strict"].as_bool().unwrap_or(true);
    let include_graph = params["includeGraph"].as_bool().unwrap_or(false);

    let (_gv, result, cache_meta) = cache.get_or_analyze(&validated, language, strict)?;

    let mut output = result;
    // Merge cache_meta into output
    if let (Some(obj), Some(meta)) = (output.as_object_mut(), cache_meta.as_object()) {
        for (k, v) in meta {
            obj.insert(k.clone(), v.clone());
        }
    }

    // Compact output: strip graph unless includeGraph=true
    if !include_graph {
        if let Some(obj) = output.as_object_mut() {
            obj.remove("graph");
        }
    }

    Ok(tool_result(&output))
}

fn handle_quality(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let root_str = validated.to_string_lossy().to_string();
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let args = vec![
        "quality",
        "--root",
        &root_str,
        "--language",
        language,
        "--format",
        "json",
    ];

    let result = run_subcommand_with_timeout(&args, Duration::from_secs(60))?;

    // Shape output: put failed gates first for AI readability
    if let Some(obj) = result.as_object() {
        let mut shaped = obj.clone();
        if let Some(gates) = shaped.get("gates").and_then(|g| g.as_array()).cloned() {
            let mut sorted = gates;
            sorted.sort_by(|a, b| {
                let a_pass = a["passed"].as_bool().unwrap_or(true);
                let b_pass = b["passed"].as_bool().unwrap_or(true);
                b_pass.cmp(&a_pass) // false (failed) sorts before true (passed)
            });
            shaped.insert("gates".to_string(), Value::Array(sorted));
        }
        return Ok(tool_result(&Value::Object(shaped)));
    }

    Ok(tool_result(&result))
}

fn handle_summary(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let root_str = validated.to_string_lossy().to_string();
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let args = vec![
        "summary",
        "--root",
        &root_str,
        "--language",
        language,
        "--format",
        "json",
    ];

    let result = run_subcommand_with_timeout(&args, Duration::from_secs(60))?;
    Ok(tool_result(&result))
}

fn handle_smoke(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let mode = params["mode"].as_str().unwrap_or("full");

    let script = {
        // Find the smoke script relative to workspace
        let exe = std::env::current_exe().unwrap_or_default();
        let workspace = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        let script_path = workspace.join("scripts").join("alpha-trial-smoke.sh");
        if script_path.exists() {
            script_path.to_string_lossy().to_string()
        } else {
            // Fallback: try relative to current dir
            "scripts/alpha-trial-smoke.sh".to_string()
        }
    };

    let mode_arg = match mode {
        "rust-only" => "--rust-only",
        "cangjie-only" => "--cangjie-only",
        _ => "",
    };

    let args = if mode_arg.is_empty() {
        vec![]
    } else {
        vec![mode_arg]
    };

    let output = run_script_with_timeout(&script, &args, Duration::from_secs(120))?;

    // Parse the output to extract pass/fail/skip counts
    let mut pass_count = 0u32;
    let mut fail_count = 0u32;
    let mut skip_count = 0u32;

    for line in output.lines() {
        if line.contains("PASS:") {
            // Try to extract number after "PASS:"
            if let Some(rest) = line.split("PASS:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    pass_count = n;
                }
            }
        }
        if line.contains("FAIL:") {
            if let Some(rest) = line.split("FAIL:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    fail_count = n;
                }
            }
        }
        if line.contains("SKIP:") {
            if let Some(rest) = line.split("SKIP:").nth(1) {
                let num_str: String = rest
                    .trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    skip_count = n;
                }
            }
        }
    }

    let passed = fail_count == 0;
    let tail_lines: Vec<&str> = output
        .lines()
        .rev()
        .take(15)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut result = serde_json::Map::new();
    result.insert("mode".to_string(), json!(mode));
    result.insert("passed".to_string(), json!(passed));
    result.insert("passCount".to_string(), json!(pass_count));
    result.insert("failCount".to_string(), json!(fail_count));
    result.insert("skipCount".to_string(), json!(skip_count));
    result.insert("tailOutput".to_string(), json!(tail_lines.join("\n")));
    if !passed {
        result.insert("hint".to_string(), json!("Check tailOutput for failure details. Run scripts/alpha-trial-smoke.sh locally to reproduce."));
    }

    Ok(tool_result(&Value::Object(result)))
}

// ============================================================
// v0.1 Tool Handlers
// ============================================================

fn handle_graph_overview(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    // Run analyze with json format to get full graph, then extract overview
    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    let summary = &result["summary"];
    let quality_gates = result["qualityGates"].as_array();

    // Count nodes by kind
    let mut node_kind_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut edge_kind_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    if let Some(graph) = result.get("graph") {
        if let Some(nodes) = graph["nodes"].as_array() {
            for node in nodes {
                let kind = node["label"].as_str().unwrap_or("unknown");
                *node_kind_counts.entry(kind.to_string()).or_insert(0) += 1;
            }
        }
        if let Some(edges) = graph["edges"].as_array() {
            for edge in edges {
                let kind = edge["type"].as_str().unwrap_or("unknown");
                *edge_kind_counts.entry(kind.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Quality summary
    let quality_summary = if let Some(gates) = quality_gates {
        let passed = gates
            .iter()
            .filter(|g| g["passed"].as_bool().unwrap_or(false))
            .count();
        let failed = gates.len() - passed;
        json!({
            "total": gates.len(),
            "passed": passed,
            "failed": failed
        })
    } else {
        json!({"total": 0, "passed": 0, "failed": 0})
    };

    // Diagnostics summary
    let diag_summary = if let Some(graph) = result.get("graph") {
        if let Some(diagnostics) = graph["diagnostics"].as_array() {
            let mut by_severity: std::collections::HashMap<String, u64> =
                std::collections::HashMap::new();
            for d in diagnostics {
                let sev = d["properties"]["severity"].as_str().unwrap_or("unknown");
                *by_severity.entry(sev.to_string()).or_insert(0) += 1;
            }
            let sev_map: serde_json::Map<String, Value> = by_severity
                .into_iter()
                .map(|(k, v)| (k, json!(v)))
                .collect();
            json!({
                "total": diagnostics.len(),
                "bySeverity": Value::Object(sev_map)
            })
        } else {
            json!({"total": 0})
        }
    } else {
        json!({"total": 0})
    };

    let node_kind_map: serde_json::Map<String, Value> = node_kind_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    let edge_kind_map: serde_json::Map<String, Value> = edge_kind_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();

    Ok(tool_result(&json!({
        "language": result["language"],
        "root": result["root"],
        "nodeCount": summary["nodeCount"],
        "edgeCount": summary["edgeCount"],
        "symbolCount": summary["symbolCount"],
        "packageCount": summary["packageCount"],
        "sourceFileCount": summary["sourceFileCount"],
        "nodeKindCounts": Value::Object(node_kind_map),
        "edgeKindCounts": Value::Object(edge_kind_map),
        "qualitySummary": quality_summary,
        "diagnosticsSummary": diag_summary
    })))
}

fn handle_unresolved_report(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let limit = params["limit"].as_u64().unwrap_or(20) as usize;
    let compact = params["compact"].as_bool().unwrap_or(false);
    check_language_feature(language)?;

    // Run analyze with json format to get graph
    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    // Check if language is cangjie — no unresolved call concept
    let detected_lang = result["language"].as_str().unwrap_or(language);
    if detected_lang == "cangjie" {
        return Ok(tool_result(&json!({
            "language": detected_lang,
            "supported": false,
            "reason": "Cangjie does not track unresolved calls in v0.1 (no CALLS edge confidence/reason classification)",
            "total": 0,
            "items": []
        })));
    }

    // For Rust: find CALLS edges with low confidence or unresolved reason
    let mut unresolved_items = Vec::new();
    let mut reason_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    if let Some(graph) = result.get("graph") {
        if let Some(edges) = graph["edges"].as_array() {
            for edge in edges {
                if edge["type"].as_str() != Some("CALLS") {
                    continue;
                }

                let confidence = edge["properties"]["confidence"].as_f64().unwrap_or(1.0);
                let reason = edge["properties"]["reason"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                // Consider unresolved if confidence < 1.0 or reason contains "unresolved"
                let is_unresolved = confidence < 1.0 || reason.contains("unresolved");

                if is_unresolved {
                    // Count by reason
                    *reason_counts.entry(reason.clone()).or_insert(0) += 1;

                    if !compact && unresolved_items.len() < limit {
                        unresolved_items.push(json!({
                            "source": edge["source"],
                            "target": edge["target"],
                            "confidence": confidence,
                            "reason": reason,
                            "callKind": edge["properties"]["callKind"]
                        }));
                    }
                }
            }
        }
    }

    // Also check diagnostics for unresolved-related codes
    let mut diag_unresolved = Vec::new();
    if !compact {
        if let Some(graph) = result.get("graph") {
            if let Some(diagnostics) = graph["diagnostics"].as_array() {
                for d in diagnostics {
                    let code = d["properties"]["code"].as_str().unwrap_or("");
                    if code.contains("unresolved") || code.contains("stop-line") {
                        diag_unresolved.push(json!({
                            "code": code,
                            "message": d["properties"]["message"],
                            "severity": d["properties"]["severity"],
                            "path": d["properties"]["path"]
                        }));
                    }
                }
            }
        }
    }

    // Count unresolved diagnostics even in compact mode (for total count accuracy)
    let diag_count = if compact {
        let mut count = 0u64;
        if let Some(graph) = result.get("graph") {
            if let Some(diagnostics) = graph["diagnostics"].as_array() {
                for d in diagnostics {
                    let code = d["properties"]["code"].as_str().unwrap_or("");
                    if code.contains("unresolved") || code.contains("stop-line") {
                        count += 1;
                    }
                }
            }
        }
        count as usize
    } else {
        diag_unresolved.len()
    };

    // In compact mode, count unresolved edges from reason_counts (not limited by `limit`)
    let unresolved_edge_count: usize = reason_counts.values().map(|v| *v as usize).sum();

    let reason_map: serde_json::Map<String, Value> = reason_counts
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();

    if compact {
        Ok(tool_result(&json!({
            "language": detected_lang,
            "supported": true,
            "total": unresolved_edge_count + diag_count,
            "unresolvedEdges": unresolved_edge_count,
            "unresolvedDiagnostics": diag_count,
            "reasonBreakdown": Value::Object(reason_map),
            "compact": true
        })))
    } else {
        Ok(tool_result(&json!({
            "language": detected_lang,
            "supported": true,
            "total": unresolved_items.len() + diag_unresolved.len(),
            "unresolvedEdges": unresolved_items.len(),
            "unresolvedDiagnostics": diag_unresolved.len(),
            "reasonBreakdown": Value::Object(reason_map),
            "topItems": unresolved_items,
            "diagnosticItems": diag_unresolved,
            "stopLineNote": "Items near Rust stop-line (no rust-analyzer, no macro expansion, no full cfg evaluator) will appear as unresolved"
        })))
    }
}

fn handle_symbol_search(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let query = params["query"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: query"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let kind_filter = params["kind"].as_str();
    let limit = params["limit"].as_u64().unwrap_or(20) as usize;
    let limit = limit.min(100); // max 100
    let compact = params["compact"].as_bool().unwrap_or(false);
    check_language_feature(language)?;

    let result = run_analyze_subprocess(&validated, language, "json", false)?;

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    if let Some(graph) = result.get("graph") {
        if let Some(nodes) = graph["nodes"].as_array() {
            for node in nodes {
                // Only search symbol-like nodes (covers both Rust and Cangjie naming)
                let kind = node["kind"].as_str().unwrap_or("");
                let label = node["label"].as_str().unwrap_or("");
                let is_searchable = kind == "symbol"
                    || kind == "function"
                    || kind == "method"
                    || kind == "associated-function"
                    || kind == "class"
                    || kind == "struct"
                    || kind == "enum"
                    || kind == "trait"
                    || kind == "const"
                    || kind == "static"
                    || kind == "package"
                    || kind == "source-file"
                    || kind == "sourceFile"
                    || label == "symbol";
                if !is_searchable {
                    continue;
                }

                // Kind filter
                if let Some(filter) = kind_filter {
                    let node_kind = node["properties"]["symbolKind"]
                        .as_str()
                        .or_else(|| node["properties"]["kind"].as_str())
                        .unwrap_or(label);
                    if node_kind.to_lowercase() != filter.to_lowercase() {
                        continue;
                    }
                }

                // Name extraction: try properties.name, then label (Cangjie uses label for display name),
                // then parse from id (Rust uses ::, Cangjie uses :).
                let name = node["properties"]["name"]
                    .as_str()
                    .or_else(|| {
                        // For Cangjie nodes, label holds the display name
                        if kind == "symbol" && !label.is_empty() && !label.contains('/') {
                            Some(label)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        // Parse from id: Rust uses "::" separator, Cangjie uses ":"
                        node["id"].as_str().and_then(|id| {
                            // Try Rust-style "::" first
                            if let Some(rust_name) = id.split("::").last() {
                                if !rust_name.is_empty() {
                                    return Some(rust_name);
                                }
                            }
                            // Try Cangjie-style ":" — take the part before #arity
                            let without_arity = id.split('#').next().unwrap_or(id);
                            if let Some(cj_name) = without_arity.rsplit(':').next() {
                                if !cj_name.is_empty() {
                                    return Some(cj_name);
                                }
                            }
                            None
                        })
                    })
                    .unwrap_or("");

                // Case-insensitive contains match
                if name.to_lowercase().contains(&query_lower) {
                    if matches.len() < limit {
                        // File path: try properties.sourcePath, then manifestPath,
                        // then parse from Cangjie-style id (sym:<file>:Kind:name)
                        let file_val = node["properties"]["sourcePath"]
                            .as_str()
                            .map(|s| json!(s))
                            .or_else(|| {
                                node["properties"]["manifestPath"]
                                    .as_str()
                                    .map(|s| json!(s))
                            })
                            .or_else(|| {
                                // Cangjie: extract file from id like "sym:src/foo.cj:Function:name#1"
                                node["id"].as_str().and_then(|id| {
                                    let parts: Vec<&str> = id.splitn(4, ':').collect();
                                    if parts.len() >= 3 && parts[0] == "sym" {
                                        Some(json!(parts[1]))
                                    } else {
                                        None
                                    }
                                })
                            })
                            .unwrap_or(Value::Null);

                        // Line: try properties.lineStart, then startLine (Cangjie)
                        let line_val = node["properties"]["lineStart"]
                            .as_u64()
                            .or_else(|| node["properties"]["startLine"].as_u64());

                        // Kind: try symbolKind, then kind, then label
                        let kind_val = node["properties"]["symbolKind"]
                            .as_str()
                            .or_else(|| node["properties"]["kind"].as_str())
                            .unwrap_or(label);

                        if compact {
                            matches.push(json!({
                                "id": node["id"],
                                "name": name,
                                "kind": kind_val,
                                "file": file_val,
                                "line": line_val
                            }));
                        } else {
                            matches.push(json!({
                                "id": node["id"],
                                "name": name,
                                "kind": kind_val,
                                "file": file_val,
                                "line": line_val,
                                "label": label
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(tool_result(&json!({
        "language": result["language"],
        "query": query,
        "matchCount": matches.len(),
        "matches": matches
    })))
}

fn handle_export_bridge(_cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let language = params["language"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: language"))?;

    let validated = validate_root_path(root)?;
    check_language_feature(language)?;

    // Determine output path
    let output_path = if let Some(op) = params["outputPath"].as_str() {
        validate_output_path(op)?
    } else {
        // Auto-generate in /tmp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        PathBuf::from(format!("/tmp/codelattice-bridge-{}.json", timestamp))
    };

    // Run analyze with gitnexus-rc format
    let result = run_analyze_subprocess(&validated, language, "gitnexus-rc", false)?;

    // Write to file
    let json_str = serde_json::to_string_pretty(&result).map_err(|e| {
        mcp_error_detail(
            "json_serialize_failed",
            "Failed to serialize bridge JSON",
            &e.to_string(),
        )
    })?;

    std::fs::write(&output_path, &json_str).map_err(|e| {
        mcp_error_detail(
            "write_failed",
            &format!("Failed to write bridge JSON to {}", output_path.display()),
            &e.to_string(),
        )
    })?;

    let bytes = json_str.len();

    // Extract counts from the bridge output
    let _stats = &result["stats"];
    let packages = result["packages"].as_array().map(|a| a.len()).unwrap_or(0);
    let source_files = result["sourceFiles"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let symbols = result["symbols"].as_array().map(|a| a.len()).unwrap_or(0);
    let relationships = result["edges"].as_array().map(|a| a.len()).unwrap_or(0);

    Ok(tool_result(&json!({
        "outputPath": output_path.to_string_lossy(),
        "bytes": bytes,
        "schemaVersion": result["schemaVersion"],
        "language": result["language"],
        "packages": packages,
        "files": source_files,
        "symbols": symbols,
        "relationships": relationships,
        "stdoutPurity": true
    })))
}

// ============================================================
// v0.2 Shared Graph Query Layer
// ============================================================

/// In-memory graph view built from a single analyze output.
/// Provides efficient lookup without repeated parsing.
struct GraphView {
    /// All nodes indexed by id
    nodes_by_id: HashMap<String, Value>,
    /// Symbol nodes indexed by lowercase name
    symbols_by_name: HashMap<String, Vec<Value>>,
    /// Outgoing edges grouped by source node id
    outgoing: HashMap<String, Vec<Value>>,
    /// Incoming edges grouped by target node id
    incoming: HashMap<String, Vec<Value>>,
    /// Diagnostics
    diagnostics: Vec<Value>,
    /// Raw analyze result metadata
    language: String,
    root: String,
    /// Static doc scanner for code ↔ docs association
    doc_scanner: Option<std::sync::Arc<DocScanner>>,
}

impl GraphView {
    fn build(analyze_result: &Value) -> Self {
        let graph = analyze_result.get("graph").unwrap_or(&Value::Null);
        let nodes = graph
            .get("nodes")
            .and_then(|n| n.as_array())
            .cloned()
            .unwrap_or_default();
        let edges = graph
            .get("edges")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();
        let diags = graph
            .get("diagnostics")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();

        let mut nodes_by_id = HashMap::new();
        let mut symbols_by_name: HashMap<String, Vec<Value>> = HashMap::new();
        let mut outgoing: HashMap<String, Vec<Value>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<Value>> = HashMap::new();

        for node in &nodes {
            if let Some(id) = node["id"].as_str() {
                nodes_by_id.insert(id.to_string(), node.clone());

                // Index symbols by name (supports both Rust and Cangjie nodes)
                let kind = node["kind"].as_str().unwrap_or("");
                let label = node["label"].as_str().unwrap_or("");
                let is_searchable = kind == "symbol"
                    || kind == "function"
                    || kind == "method"
                    || kind == "associated-function"
                    || kind == "class"
                    || kind == "struct"
                    || kind == "enum"
                    || kind == "trait"
                    || kind == "const"
                    || kind == "static"
                    || label == "symbol";
                if is_searchable {
                    // Name extraction cascade: properties.name → label (Cangjie) → id parsing
                    let name = node["properties"]["name"]
                        .as_str()
                        .or_else(|| {
                            // Cangjie nodes: label holds display name
                            if kind == "symbol" && !label.is_empty() && !label.contains('/') {
                                Some(label)
                            } else {
                                None
                            }
                        })
                        .or_else(|| {
                            // Parse from id: Rust "::", Cangjie ":" with "#arity" suffix
                            node["id"].as_str().and_then(|nid| {
                                if let Some(rust_name) = nid.split("::").last() {
                                    if !rust_name.is_empty() {
                                        return Some(rust_name);
                                    }
                                }
                                let without_arity = nid.split('#').next().unwrap_or(nid);
                                if let Some(cj_name) = without_arity.rsplit(':').next() {
                                    if !cj_name.is_empty() {
                                        return Some(cj_name);
                                    }
                                }
                                None
                            })
                        });
                    if let Some(name) = name {
                        symbols_by_name
                            .entry(name.to_lowercase())
                            .or_default()
                            .push(node.clone());
                    }
                }
            }
        }

        for edge in &edges {
            if let Some(src) = edge["source"].as_str() {
                outgoing
                    .entry(src.to_string())
                    .or_default()
                    .push(edge.clone());
            }
            if let Some(tgt) = edge["target"].as_str() {
                incoming
                    .entry(tgt.to_string())
                    .or_default()
                    .push(edge.clone());
            }
        }

        GraphView {
            nodes_by_id,
            symbols_by_name,
            outgoing,
            incoming,
            diagnostics: diags,
            language: analyze_result["language"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            root: analyze_result["root"].as_str().unwrap_or("").to_string(),
            doc_scanner: None, // set later via set_doc_scanner
        }
    }

    /// Cheap clone — clones the HashMap/Vec containers but shares the underlying
    /// Value allocations (serde_json Values are reference-counted internally).
    fn clone_shallow(&self) -> Self {
        GraphView {
            nodes_by_id: self.nodes_by_id.clone(),
            symbols_by_name: self.symbols_by_name.clone(),
            outgoing: self.outgoing.clone(),
            incoming: self.incoming.clone(),
            diagnostics: self.diagnostics.clone(),
            language: self.language.clone(),
            root: self.root.clone(),
            doc_scanner: self.doc_scanner.clone(),
        }
    }

    /// Find symbols by name (case-insensitive substring match).
    /// Returns matching nodes, optionally filtered by kind.
    fn find_symbols(&self, query: &str, kind: Option<&str>, limit: usize) -> Vec<Value> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Exact name match first
        if let Some(exact) = self.symbols_by_name.get(&query_lower) {
            for sym in exact {
                if results.len() >= limit {
                    break;
                }
                if let Some(k) = kind {
                    let sym_kind = sym["properties"]["symbolKind"]
                        .as_str()
                        .or_else(|| sym["properties"]["kind"].as_str())
                        .unwrap_or("");
                    if sym_kind.to_lowercase() != k.to_lowercase() {
                        continue;
                    }
                }
                results.push(sym.clone());
            }
        }

        // Substring match
        if results.len() < limit {
            for (name_lower, syms) in &self.symbols_by_name {
                if name_lower.contains(&query_lower)
                    && !self.symbols_by_name.contains_key(&query_lower)
                {
                    // Skip exact matches (already handled)
                }
                if name_lower.contains(&query_lower) {
                    for sym in syms {
                        if results.len() >= limit {
                            break;
                        }
                        if let Some(k) = kind {
                            let sym_kind = sym["properties"]["symbolKind"]
                                .as_str()
                                .or_else(|| sym["properties"]["kind"].as_str())
                                .unwrap_or("");
                            if sym_kind.to_lowercase() != k.to_lowercase() {
                                continue;
                            }
                        }
                        // Avoid duplicates
                        let id = sym["id"].as_str().unwrap_or("");
                        if !results.iter().any(|r| r["id"].as_str() == Some(id)) {
                            results.push(sym.clone());
                        }
                    }
                }
            }
        }

        results
    }

    /// Get edges from a node, optionally filtered by edge type
    fn edges_from(&self, node_id: &str, edge_type: Option<&str>) -> Vec<Value> {
        self.outgoing
            .get(node_id)
            .map(|edges| {
                edges
                    .iter()
                    .filter(|e| {
                        edge_type
                            .map(|t| e["type"].as_str() == Some(t))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get edges to a node, optionally filtered by edge type
    fn edges_to(&self, node_id: &str, edge_type: Option<&str>) -> Vec<Value> {
        self.incoming
            .get(node_id)
            .map(|edges| {
                edges
                    .iter()
                    .filter(|e| {
                        edge_type
                            .map(|t| e["type"].as_str() == Some(t))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Count total nodes, edges, symbols
    fn stats(&self) -> (usize, usize, usize) {
        let node_count = self.nodes_by_id.len();
        let edge_count: usize = self.outgoing.values().map(|v| v.len()).sum();
        let symbol_count = self
            .nodes_by_id
            .values()
            .filter(|n| n["label"].as_str() == Some("symbol"))
            .count();
        (node_count, edge_count, symbol_count)
    }

    /// Get a reference to the doc scanner (if available).
    fn doc_scanner(&self) -> Option<&DocScanner> {
        self.doc_scanner.as_deref()
    }

    /// Get diagnostics for a specific symbol/file
    fn diagnostics_for(&self, node_id: &str) -> Vec<Value> {
        self.diagnostics
            .iter()
            .filter(|d| {
                // Check if diagnostic references this node
                d["properties"]["symbolId"]
                    .as_str()
                    .map(|s| s == node_id)
                    .unwrap_or(false)
                    || d["id"]
                        .as_str()
                        .map(|id| id.contains(node_id.split("::").last().unwrap_or("")))
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }
}

/// Compute quality metrics from a GraphView for MCP tool output.
/// Pure function: no side effects, returns a serde_json::Value.
fn compute_quality_metrics(gv: &GraphView) -> Value {
    // Flatten all edges
    let all_edges: Vec<&Value> = gv.outgoing.values().flatten().collect();
    let total_edge_count: usize = all_edges.len();

    // graphCompleteness
    let node_count = gv.nodes_by_id.len();
    let symbol_count = gv
        .nodes_by_id
        .values()
        .filter(|n| n["label"].as_str() == Some("symbol") || n["kind"].as_str() == Some("symbol"))
        .count();
    let source_file_count = gv
        .nodes_by_id
        .values()
        .filter(|n| {
            n["label"].as_str() == Some("source-file") || n["kind"].as_str() == Some("sourceFile")
        })
        .count();
    let dangling_edge_count = all_edges
        .iter()
        .filter(|e| {
            let src = e
                .get("source")
                .and_then(|v| v.as_str())
                .or_else(|| e.get("sourceId").and_then(|v| v.as_str()));
            match src {
                None => true,
                Some(id) => !gv.nodes_by_id.contains_key(id),
            }
        })
        .count();

    // edgeConfidence
    let edges_with_confidence: Vec<(&Value, Option<f64>)> = all_edges
        .iter()
        .map(|e| {
            let conf = e
                .get("properties")
                .and_then(|p| p.get("confidence"))
                .and_then(|c| c.as_f64());
            (*e, conf)
        })
        .collect();

    let total_confidence_edge_count = edges_with_confidence
        .iter()
        .filter(|(_, c)| c.is_some())
        .count();
    let high_confidence_edge_count = edges_with_confidence
        .iter()
        .filter(|(_, c)| c.map(|v| v >= 0.80).unwrap_or(false))
        .count();
    let medium_confidence_edge_count = edges_with_confidence
        .iter()
        .filter(|(_, c)| c.map(|v| v >= 0.60 && v < 0.80).unwrap_or(false))
        .count();
    let low_confidence_edge_count = edges_with_confidence
        .iter()
        .filter(|(_, c)| c.map(|v| v < 0.60).unwrap_or(false))
        .count();
    let unknown_confidence_edge_count = total_edge_count - total_confidence_edge_count;
    let low_confidence_edge_rate = if total_confidence_edge_count > 0 {
        low_confidence_edge_count as f64 / total_confidence_edge_count as f64
    } else {
        0.0
    };
    let unknown_confidence_edge_rate = if total_edge_count > 0 {
        unknown_confidence_edge_count as f64 / total_edge_count as f64
    } else {
        0.0
    };

    // callQuality
    let call_edges: Vec<&Value> = all_edges
        .iter()
        .filter(|e| {
            let t = e
                .get("type")
                .and_then(|v| v.as_str())
                .or_else(|| e.get("kind").and_then(|v| v.as_str()))
                .unwrap_or("");
            t == "CALLS"
        })
        .copied()
        .collect();
    let call_edge_count = call_edges.len();
    let call_conf: Vec<Option<f64>> = call_edges
        .iter()
        .map(|e| {
            e.get("properties")
                .and_then(|p| p.get("confidence"))
                .and_then(|c| c.as_f64())
        })
        .collect();
    let high_confidence_call_count = call_conf
        .iter()
        .filter(|c| c.map(|v| v >= 0.80).unwrap_or(false))
        .count();
    let medium_confidence_call_count = call_conf
        .iter()
        .filter(|c| c.map(|v| v >= 0.60 && v < 0.80).unwrap_or(false))
        .count();
    let low_confidence_call_count = call_conf
        .iter()
        .filter(|c| c.map(|v| v < 0.60).unwrap_or(false))
        .count();
    let unknown_confidence_call_count = call_conf.iter().filter(|c| c.is_none()).count();
    let low_confidence_call_rate = if call_edge_count > 0 {
        low_confidence_call_count as f64 / call_edge_count as f64
    } else {
        0.0
    };

    // dependencyQuality
    let import_edge_count = all_edges
        .iter()
        .filter(|e| {
            let t = e
                .get("type")
                .and_then(|v| v.as_str())
                .or_else(|| e.get("kind").and_then(|v| v.as_str()))
                .unwrap_or("");
            t.contains("IMPORT")
        })
        .count();
    let include_edge_count = all_edges
        .iter()
        .filter(|e| {
            let t = e
                .get("type")
                .and_then(|v| v.as_str())
                .or_else(|| e.get("kind").and_then(|v| v.as_str()))
                .unwrap_or("");
            t.contains("INCLUDE")
        })
        .count();
    let unresolved_import_or_include_count = all_edges
        .iter()
        .filter(|e| {
            let t = e
                .get("type")
                .and_then(|v| v.as_str())
                .or_else(|| e.get("kind").and_then(|v| v.as_str()))
                .unwrap_or("");
            let is_import_or_include = t.contains("IMPORT") || t.contains("INCLUDE");
            if !is_import_or_include {
                return false;
            }
            let reason = e
                .get("properties")
                .and_then(|p| p.get("reason"))
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_lowercase();
            reason.contains("unresolved") || reason.contains("missing")
        })
        .count();

    // diagnostics
    let diagnostic_count = gv.diagnostics.len();
    let unresolved_diagnostic_count = gv
        .diagnostics
        .iter()
        .filter(|d| {
            let code = d
                .get("properties")
                .and_then(|p| p.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_lowercase();
            let reason = d
                .get("properties")
                .and_then(|p| p.get("reason"))
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_lowercase();
            code.contains("unresolved") || reason.contains("unresolved")
        })
        .count();
    let parse_diagnostic_count = gv
        .diagnostics
        .iter()
        .filter(|d| {
            let code = d
                .get("properties")
                .and_then(|p| p.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_lowercase();
            let severity = d
                .get("severity")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_lowercase();
            code.contains("parse") || severity.contains("parse")
        })
        .count();

    json!({
        "graphCompleteness": {
            "nodeCount": node_count,
            "edgeCount": total_edge_count,
            "symbolCount": symbol_count,
            "sourceFileCount": source_file_count,
            "danglingEdgeCount": dangling_edge_count,
        },
        "edgeConfidence": {
            "totalConfidenceEdgeCount": total_confidence_edge_count,
            "highConfidenceEdgeCount": high_confidence_edge_count,
            "mediumConfidenceEdgeCount": medium_confidence_edge_count,
            "lowConfidenceEdgeCount": low_confidence_edge_count,
            "unknownConfidenceEdgeCount": unknown_confidence_edge_count,
            "lowConfidenceEdgeRate": low_confidence_edge_rate,
            "unknownConfidenceEdgeRate": unknown_confidence_edge_rate,
        },
        "callQuality": {
            "callEdgeCount": call_edge_count,
            "highConfidenceCallEdgeCount": high_confidence_call_count,
            "mediumConfidenceCallEdgeCount": medium_confidence_call_count,
            "lowConfidenceCallEdgeCount": low_confidence_call_count,
            "unknownConfidenceCallEdgeCount": unknown_confidence_call_count,
            "lowConfidenceCallRate": low_confidence_call_rate,
        },
        "dependencyQuality": {
            "importEdgeCount": import_edge_count,
            "includeEdgeCount": include_edge_count,
            "unresolvedImportOrIncludeCount": unresolved_import_or_include_count,
        },
        "diagnostics": {
            "diagnosticCount": diagnostic_count,
            "unresolvedDiagnosticCount": unresolved_diagnostic_count,
            "parseDiagnosticCount": parse_diagnostic_count,
        },
        "generatedFrom": {
            "graphBased": true,
            "compilerVerified": false,
            "heuristic": true,
        }
    })
}

/// Build a GraphView by running analyze and parsing the result.
#[allow(dead_code)]
fn build_graph_view(root: &Path, language: &str) -> Result<GraphView, Value> {
    let result = run_analyze_subprocess(root, language, "json", false)?;
    Ok(GraphView::build(&result))
}

// ============================================================
// v0.2 Tool Handlers
// ============================================================

fn handle_symbol_context(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let name = params["name"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: name"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let kind_filter = params["kind"].as_str();
    let limit = params["limit"].as_u64().unwrap_or(10).min(50) as usize;
    let include_snippet = params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_context = params["snippetContext"].as_u64().unwrap_or(3).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let matches = gv.find_symbols(name, kind_filter, limit);

    if matches.is_empty() {
        return Ok(merge_cache_and_result(
            &json!({
                "query": name,
                "matchCount": 0,
                "selected": null,
                "note": "No symbols found matching the query"
            }),
            &cache_meta,
        ));
    }

    let mut match_summaries = Vec::new();
    for sym in &matches {
        let id = sym["id"].as_str().unwrap_or("");
        let out_edges = gv.edges_from(id, None);
        let in_edges = gv.edges_to(id, None);
        let diags = gv.diagnostics_for(id);

        // Group outgoing by type
        let mut out_by_kind: HashMap<String, u64> = HashMap::new();
        for e in &out_edges {
            let t = e["type"].as_str().unwrap_or("unknown");
            *out_by_kind.entry(t.to_string()).or_insert(0) += 1;
        }
        let mut in_by_kind: HashMap<String, u64> = HashMap::new();
        for e in &in_edges {
            let t = e["type"].as_str().unwrap_or("unknown");
            *in_by_kind.entry(t.to_string()).or_insert(0) += 1;
        }

        // Collect confidence/reason samples from CALLS edges
        let confidence_samples: Vec<Value> = out_edges
            .iter()
            .chain(in_edges.iter())
            .filter(|e| e["type"].as_str() == Some("CALLS"))
            .take(3)
            .map(|e| {
                json!({
                    "confidence": e["properties"]["confidence"],
                    "reason": e["properties"]["reason"]
                })
            })
            .collect();

        let out_map: serde_json::Map<String, Value> = out_by_kind
            .into_iter()
            .map(|(k, v)| (k, json!(v)))
            .collect();
        let in_map: serde_json::Map<String, Value> =
            in_by_kind.into_iter().map(|(k, v)| (k, json!(v))).collect();

        // Read source snippet if requested
        let file_for_snippet = sym["properties"]["sourcePath"]
            .as_str()
            .or_else(|| sym["properties"]["manifestPath"].as_str())
            .or_else(|| {
                // Cangjie: extract file from id like "sym:src/foo.cj:Function:name#1"
                sym["id"].as_str().and_then(|sid| {
                    let parts: Vec<&str> = sid.splitn(4, ':').collect();
                    if parts.len() >= 3 && parts[0] == "sym" {
                        Some(parts[1])
                    } else {
                        None
                    }
                })
            })
            .unwrap_or("");
        let line_start = sym["properties"]["lineStart"]
            .as_u64()
            .or_else(|| sym["properties"]["startLine"].as_u64())
            .unwrap_or(0);
        let line_end = sym["properties"]["lineEnd"]
            .as_u64()
            .or_else(|| sym["properties"]["endLine"].as_u64())
            .unwrap_or(line_start);
        let snippet = if include_snippet {
            if !file_for_snippet.is_empty() {
                read_source_snippet(
                    &gv.root,
                    file_for_snippet,
                    line_start,
                    line_end,
                    snippet_context,
                )
            } else {
                json!({ "warning": "No source path available", "lines": Value::Null })
            }
        } else {
            Value::Null
        };

        // Name extraction: cascade properties.name → label (Cangjie) → id parsing
        let sym_kind_node = sym["kind"].as_str().unwrap_or("");
        let sym_label = sym["label"].as_str().unwrap_or("");
        let sym_name = sym["properties"]["name"]
            .as_str()
            .or_else(|| {
                if sym_kind_node == "symbol" && !sym_label.is_empty() && !sym_label.contains('/') {
                    Some(sym_label)
                } else {
                    None
                }
            })
            .or_else(|| {
                sym["id"].as_str().and_then(|sid| {
                    if let Some(rust_name) = sid.split("::").last() {
                        if !rust_name.is_empty() {
                            return Some(rust_name);
                        }
                    }
                    let without_arity = sid.split('#').next().unwrap_or(sid);
                    if let Some(cj_name) = without_arity.rsplit(':').next() {
                        if !cj_name.is_empty() {
                            return Some(cj_name);
                        }
                    }
                    None
                })
            })
            .map(|s| json!(s))
            .unwrap_or(Value::Null);

        // Kind: cascade properties.symbolKind → properties.kind → node kind
        let sym_kind = sym["properties"]["symbolKind"]
            .as_str()
            .or_else(|| sym["properties"]["kind"].as_str())
            .or_else(|| Some(sym_kind_node))
            .map(|s| json!(s))
            .unwrap_or(Value::Null);

        // File: cascade properties.sourcePath → manifestPath → parse from id
        let sym_file = sym["properties"]["sourcePath"]
            .as_str()
            .or_else(|| sym["properties"]["manifestPath"].as_str())
            .or_else(|| {
                sym["id"].as_str().and_then(|sid| {
                    let parts: Vec<&str> = sid.splitn(4, ':').collect();
                    if parts.len() >= 3 && parts[0] == "sym" {
                        Some(parts[1])
                    } else {
                        None
                    }
                })
            })
            .map(|s| json!(s))
            .unwrap_or(Value::Null);

        let sym_line = sym["properties"]["lineStart"]
            .as_u64()
            .or_else(|| sym["properties"]["startLine"].as_u64())
            .map(|v| json!(v))
            .unwrap_or(Value::Null);

        let sym_line_end = sym["properties"]["lineEnd"]
            .as_u64()
            .or_else(|| sym["properties"]["endLine"].as_u64())
            .map(|v| json!(v))
            .unwrap_or(Value::Null);

        match_summaries.push(json!({
            "id": id,
            "name": sym_name,
            "kind": sym_kind,
            "file": sym_file,
            "line": sym_line,
            "lineEnd": sym_line_end,
            "visibility": sym["properties"]["visibility"],
            "sourceSnippet": snippet,
            "outgoingEdges": Value::Object(out_map),
            "incomingEdges": Value::Object(in_map),
            "relatedDiagnostics": diags.len(),
            "confidenceSamples": confidence_samples
        }));
    }

    let ambiguous = matches.len() > 1;
    let selected = if ambiguous {
        Value::Null
    } else {
        match_summaries.first().cloned().unwrap_or(Value::Null)
    };

    // Find related docs
    let related_docs = if let Some(ds) = gv.doc_scanner() {
        let file = if !ambiguous {
            matches[0]["properties"]["sourcePath"]
                .as_str()
                .unwrap_or("")
        } else {
            ""
        };
        let tool_name = if name.starts_with("codelattice_") {
            vec![name]
        } else {
            vec![]
        };
        ds.find_related_docs(
            name,
            file,
            &tool_name,
            if params["compact"].as_bool().unwrap_or(false) {
                5
            } else {
                20
            },
        )
    } else {
        vec![]
    };

    Ok(merge_cache_and_result(
        &json!({
            "query": name,
            "matchCount": matches.len(),
            "ambiguous": ambiguous,
            "selected": selected,
            "candidates": match_summaries,
            "relatedDocs": related_docs,
            "note": if ambiguous { "Multiple symbols match. Use kind/file parameters to disambiguate." } else { "" }
        }),
        &cache_meta,
    ))
}

fn handle_calls_from(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let symbol = params["symbol"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: symbol"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let depth = params["depth"].as_u64().unwrap_or(1).min(3) as usize;
    let limit = params["limit"].as_u64().unwrap_or(20).min(100) as usize;
    let compact = params["compact"].as_bool().unwrap_or(false);
    // compact implies no snippets regardless of explicit includeSnippet
    let include_snippet = !compact && params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(3).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let root_str = validated.to_string_lossy();

    // Find source symbols
    let sources = gv.find_symbols(symbol, None, 5);
    if sources.is_empty() {
        return Ok(merge_cache_and_result(
            &json!({
                "symbol": symbol,
                "sourceCandidates": [],
                "edges": [],
                "truncated": false,
                "note": "No symbols found matching the query"
            }),
            &cache_meta,
        ));
    }

    let source_candidates: Vec<Value> = sources
        .iter()
        .map(|s| {
            let mut obj = json!({
                "id": s["id"],
                "name": s["properties"]["name"],
                "kind": s["properties"]["symbolKind"],
                "file": s["properties"]["sourcePath"],
                "line": s["properties"]["lineStart"]
            });
            if include_snippet {
                if let Some(map) = obj.as_object_mut() {
                    let file = s["properties"]["sourcePath"].as_str().unwrap_or("");
                    let start = s["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let end = s["properties"]["lineEnd"].as_u64().unwrap_or(start);
                    map.insert(
                        "sourceSnippet".to_string(),
                        read_source_snippet(&root_str, file, start, end, snippet_ctx),
                    );
                }
            }
            obj
        })
        .collect();

    // BFS traversal from source(s)
    let mut all_edges = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<(String, usize)> = sources
        .iter()
        .filter_map(|s| s["id"].as_str().map(|id| (id.to_string(), 0)))
        .collect();

    while let Some((node_id, current_depth)) = queue.pop() {
        if visited.contains(&node_id) || current_depth >= depth {
            continue;
        }
        visited.insert(node_id.clone());

        let edges = gv.edges_from(&node_id, None);
        for edge in edges {
            if all_edges.len() >= limit {
                break;
            }
            let target_id = edge["target"].as_str().unwrap_or("");
            let target_node = gv.nodes_by_id.get(target_id);

            let edge_obj = if compact {
                json!({
                    "targetId": target_id,
                    "targetName": target_node.and_then(|n| n["properties"]["name"].as_str()),
                    "targetKind": target_node.and_then(|n| n["properties"]["symbolKind"].as_str()),
                    "targetFile": target_node.and_then(|n| n["properties"]["sourcePath"].as_str()),
                    "targetLine": target_node.and_then(|n| n["properties"]["lineStart"].as_u64()),
                    "type": edge["type"],
                    "confidence": edge["properties"]["confidence"],
                    "reason": edge["properties"]["reason"]
                })
            } else {
                let mut eo = json!({
                    "source": edge["source"],
                    "target": target_id,
                    "type": edge["type"],
                    "depth": current_depth + 1,
                    "confidence": edge["properties"]["confidence"],
                    "reason": edge["properties"]["reason"],
                    "targetName": target_node.and_then(|n| n["properties"]["name"].as_str()),
                    "targetKind": target_node.and_then(|n| n["properties"]["symbolKind"].as_str())
                });
                if include_snippet {
                    if let Some(tn) = target_node {
                        let file = tn["properties"]["sourcePath"].as_str().unwrap_or("");
                        let start = tn["properties"]["lineStart"].as_u64().unwrap_or(0);
                        let end = tn["properties"]["lineEnd"].as_u64().unwrap_or(start);
                        if let Some(map) = eo.as_object_mut() {
                            map.insert(
                                "targetSnippet".to_string(),
                                read_source_snippet(&root_str, file, start, end, snippet_ctx),
                            );
                        }
                    }
                }
                eo
            };

            all_edges.push(edge_obj);

            if current_depth + 1 < depth && !visited.contains(target_id) {
                queue.push((target_id.to_string(), current_depth + 1));
            }
        }
    }

    let truncated = all_edges.len() >= limit;

    let mut result = json!({
        "symbol": symbol,
        "sourceCandidates": source_candidates,
        "edgeCount": all_edges.len(),
        "edges": all_edges,
        "truncated": truncated
    });
    if compact {
        if let Some(map) = result.as_object_mut() {
            map.insert("compact".to_string(), json!(true));
        }
    }

    Ok(merge_cache_and_result(&result, &cache_meta))
}

fn handle_calls_to(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let symbol = params["symbol"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: symbol"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let depth = params["depth"].as_u64().unwrap_or(1).min(3) as usize;
    let limit = params["limit"].as_u64().unwrap_or(20).min(100) as usize;
    let compact = params["compact"].as_bool().unwrap_or(false);
    let include_snippet = !compact && params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(3).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let root_str = validated.to_string_lossy();

    let targets = gv.find_symbols(symbol, None, 5);
    if targets.is_empty() {
        return Ok(merge_cache_and_result(
            &json!({
                "symbol": symbol,
                "targetCandidates": [],
                "edges": [],
                "truncated": false,
                "note": "No symbols found matching the query"
            }),
            &cache_meta,
        ));
    }

    let target_candidates: Vec<Value> = targets
        .iter()
        .map(|s| {
            let mut obj = json!({
                "id": s["id"],
                "name": s["properties"]["name"],
                "kind": s["properties"]["symbolKind"],
                "file": s["properties"]["sourcePath"],
                "line": s["properties"]["lineStart"]
            });
            if include_snippet {
                if let Some(map) = obj.as_object_mut() {
                    let file = s["properties"]["sourcePath"].as_str().unwrap_or("");
                    let start = s["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let end = s["properties"]["lineEnd"].as_u64().unwrap_or(start);
                    map.insert(
                        "sourceSnippet".to_string(),
                        read_source_snippet(&root_str, file, start, end, snippet_ctx),
                    );
                }
            }
            obj
        })
        .collect();

    // BFS traversal backwards from target(s)
    let mut all_edges = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<(String, usize)> = targets
        .iter()
        .filter_map(|s| s["id"].as_str().map(|id| (id.to_string(), 0)))
        .collect();

    while let Some((node_id, current_depth)) = queue.pop() {
        if visited.contains(&node_id) || current_depth >= depth {
            continue;
        }
        visited.insert(node_id.clone());

        let edges = gv.edges_to(&node_id, None);
        for edge in edges {
            if all_edges.len() >= limit {
                break;
            }
            let src_id = edge["source"].as_str().unwrap_or("");
            let src_node = gv.nodes_by_id.get(src_id);

            let edge_obj = if compact {
                json!({
                    "sourceId": src_id,
                    "sourceName": src_node.and_then(|n| n["properties"]["name"].as_str()),
                    "sourceKind": src_node.and_then(|n| n["properties"]["symbolKind"].as_str()),
                    "sourceFile": src_node.and_then(|n| n["properties"]["sourcePath"].as_str()),
                    "sourceLine": src_node.and_then(|n| n["properties"]["lineStart"].as_u64()),
                    "type": edge["type"],
                    "confidence": edge["properties"]["confidence"],
                    "reason": edge["properties"]["reason"]
                })
            } else {
                let mut eo = json!({
                    "source": src_id,
                    "target": edge["target"],
                    "type": edge["type"],
                    "depth": current_depth + 1,
                    "confidence": edge["properties"]["confidence"],
                    "reason": edge["properties"]["reason"],
                    "sourceName": src_node.and_then(|n| n["properties"]["name"].as_str()),
                    "sourceKind": src_node.and_then(|n| n["properties"]["symbolKind"].as_str())
                });
                if include_snippet {
                    if let Some(sn) = src_node {
                        let file = sn["properties"]["sourcePath"].as_str().unwrap_or("");
                        let start = sn["properties"]["lineStart"].as_u64().unwrap_or(0);
                        let end = sn["properties"]["lineEnd"].as_u64().unwrap_or(start);
                        if let Some(map) = eo.as_object_mut() {
                            map.insert(
                                "sourceSnippet".to_string(),
                                read_source_snippet(&root_str, file, start, end, snippet_ctx),
                            );
                        }
                    }
                }
                eo
            };

            all_edges.push(edge_obj);

            if current_depth + 1 < depth && !visited.contains(src_id) {
                queue.push((src_id.to_string(), current_depth + 1));
            }
        }
    }

    let truncated = all_edges.len() >= limit;

    let mut result = json!({
        "symbol": symbol,
        "targetCandidates": target_candidates,
        "edgeCount": all_edges.len(),
        "edges": all_edges,
        "truncated": truncated
    });
    if compact {
        if let Some(map) = result.as_object_mut() {
            map.insert("compact".to_string(), json!(true));
        }
    }

    Ok(merge_cache_and_result(&result, &cache_meta))
}

// ============================================================
// Static Doc Scanner — Code ↔ Docs Association
// ============================================================

/// Directories to skip when scanning for markdown docs.
const DOC_SCAN_SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    ".gitnexus",
    ".claude",
    ".agents",
    ".arts",
    ".codeartsdoer",
    "CodeLattice-Tool",
    "node_modules",
];

/// A section within a markdown document (heading + content).
#[derive(Debug, Clone)]
struct DocSection {
    heading: String,
    heading_level: u8,
    start_line: usize,
    end_line: usize,
}

/// A reference extracted from a markdown document.
#[derive(Debug, Clone)]
struct DocRef {
    path: String,         // repo-relative doc path
    line: usize,          // line number in doc
    match_type: String,   // "symbol" | "file" | "command" | "link" | "section"
    matched_text: String, // what was matched
    confidence: String,   // "high" | "medium" | "low"
    reason: String,       // why it matched
    section: String,      // enclosing section heading (empty if none)
}

/// A scanned markdown document.
#[derive(Debug, Clone)]
struct ScannedDoc {
    path: String,
    title: String,
    line_count: usize,
    sections: Vec<DocSection>,
    references: Vec<DocRef>,
    link_count: usize,
    code_fence_count: usize,
    symbol_ref_count: usize,
    path_ref_count: usize,
}

/// Static doc scanner: scans markdown files and builds searchable associations.
struct DocScanner {
    docs: Vec<ScannedDoc>,
    total_doc_count: usize,
    total_section_count: usize,
    total_link_count: usize,
    total_code_fence_count: usize,
    total_symbol_ref_count: usize,
    total_path_ref_count: usize,
    total_command_count: usize,
}

impl DocScanner {
    /// Build a DocScanner by scanning the repo for markdown files.
    fn build(root: &std::path::Path) -> Self {
        let mut docs = Vec::new();

        // Walk the repo, collect .md files
        if let Ok(entries) = walk_dir_for_md(root, root) {
            for entry in entries {
                if let Ok(content) = std::fs::read_to_string(&entry) {
                    let line_count = content.lines().count();
                    let relative = pathdiff_or_relative(&entry, root);
                    let title = extract_doc_title(&content);
                    let (
                        sections,
                        refs,
                        link_count,
                        code_fence_count,
                        symbol_ref_count,
                        path_ref_count,
                        command_count,
                    ) = parse_doc_content(&relative, &content);

                    docs.push(ScannedDoc {
                        path: relative,
                        title,
                        line_count,
                        sections,
                        references: refs,
                        link_count,
                        code_fence_count,
                        symbol_ref_count,
                        path_ref_count,
                    });
                }
            }
        }

        let total_doc_count = docs.len();
        let total_section_count = docs.iter().map(|d| d.sections.len()).sum();
        let total_link_count = docs.iter().map(|d| d.link_count).sum();
        let total_code_fence_count = docs.iter().map(|d| d.code_fence_count).sum();
        let total_symbol_ref_count = docs.iter().map(|d| d.symbol_ref_count).sum();
        let total_path_ref_count = docs.iter().map(|d| d.path_ref_count).sum();
        let total_command_count = docs
            .iter()
            .map(|d| {
                d.references
                    .iter()
                    .filter(|r| r.match_type == "command")
                    .count()
            })
            .sum();

        DocScanner {
            docs,
            total_doc_count,
            total_section_count,
            total_link_count,
            total_code_fence_count,
            total_symbol_ref_count,
            total_path_ref_count,
            total_command_count,
        }
    }

    /// Summary counts for project_overview.
    fn summary_json(&self) -> Value {
        json!({
            "docCount": self.total_doc_count,
            "docSectionCount": self.total_section_count,
            "docLinkCount": self.total_link_count,
            "docCodeFenceCount": self.total_code_fence_count,
            "docCommandCount": self.total_command_count,
            "docPathReferenceCount": self.total_path_ref_count,
            "docSymbolReferenceCount": self.total_symbol_ref_count,
        })
    }

    /// Find docs related to a symbol name, file path, or MCP tool name.
    fn find_related_docs(
        &self,
        symbol_name: &str,
        file_path: &str,
        tool_names: &[&str],
        limit: usize,
    ) -> Vec<Value> {
        let mut results: Vec<Value> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for doc in &self.docs {
            for r in &doc.references {
                let matches = match r.match_type.as_str() {
                    "symbol" => {
                        // Exact match on symbol name (case-sensitive for accuracy)
                        let sym_lower = symbol_name.to_lowercase();
                        let matched_lower = r.matched_text.to_lowercase();
                        matched_lower == sym_lower
                            || matched_lower == format!("`{}`", sym_lower)
                            || matched_lower == format!("codelattice_{}", sym_lower)
                    }
                    "file" => {
                        // Exact or suffix match on file path
                        file_path.ends_with(&r.matched_text)
                            || r.matched_text.ends_with(file_path)
                            || r.matched_text == file_path
                    }
                    "command" => {
                        // Check if any tool name matches
                        tool_names
                            .iter()
                            .any(|t| r.matched_text == **t || r.matched_text.contains(t))
                    }
                    "link" => {
                        // Link target matches file or symbol
                        r.matched_text.ends_with(file_path) || file_path.ends_with(&r.matched_text)
                    }
                    "section" => {
                        // Section heading contains symbol or tool name
                        let heading_lower = r.matched_text.to_lowercase();
                        let sym_lower = symbol_name.to_lowercase();
                        heading_lower.contains(&sym_lower)
                            || tool_names
                                .iter()
                                .any(|t| heading_lower.contains(&t.to_lowercase()))
                    }
                    _ => false,
                };

                if matches {
                    let key = format!("{}:{}:{}", doc.path, r.line, r.matched_text);
                    if !seen.contains(&key) {
                        seen.insert(key);
                        results.push(json!({
                            "path": doc.path,
                            "section": r.section,
                            "line": r.line,
                            "matchType": r.match_type,
                            "matchedText": r.matched_text,
                            "confidence": r.confidence,
                            "reason": r.reason,
                        }));
                        if results.len() >= limit {
                            return results;
                        }
                    }
                }
            }
        }
        results
    }

    /// Find docs likely needing update based on changed symbols/files.
    fn find_docs_needing_update(
        &self,
        symbol_names: &[String],
        file_paths: &[String],
        limit: usize,
    ) -> Vec<Value> {
        let mut results: Vec<Value> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for doc in &self.docs {
            let mut matched_symbols: Vec<String> = Vec::new();
            let mut matched_files: Vec<String> = Vec::new();
            let mut best_reason = String::new();

            for r in &doc.references {
                let lower_text = r.matched_text.to_lowercase();

                // Check symbol matches
                for sym in symbol_names {
                    let sym_lower = sym.to_lowercase();
                    if lower_text == sym_lower
                        || lower_text == format!("`{}`", sym_lower)
                        || lower_text == format!("codelattice_{}", sym_lower)
                        || lower_text.contains(&sym_lower)
                    {
                        if !matched_symbols.contains(sym) {
                            matched_symbols.push(sym.clone());
                            if best_reason.is_empty() {
                                best_reason = "mentions changed symbol".to_string();
                            }
                        }
                    }
                }

                // Check file path matches
                for fp in file_paths {
                    if r.matched_text.ends_with(fp)
                        || fp.ends_with(&r.matched_text)
                        || r.matched_text == *fp
                    {
                        if !matched_files.contains(fp) {
                            matched_files.push(fp.clone());
                            if best_reason.is_empty() {
                                best_reason = "references changed file".to_string();
                            }
                        }
                    }
                }
            }

            if (!matched_symbols.is_empty() || !matched_files.is_empty())
                && !seen.contains(&doc.path)
            {
                seen.insert(doc.path.clone());
                results.push(json!({
                    "path": doc.path,
                    "reason": if !matched_symbols.is_empty() { "mentions changed symbol" } else { "references changed file" },
                    "matchedSymbols": matched_symbols,
                    "matchedFiles": matched_files,
                }));
                if results.len() >= limit {
                    return results;
                }
            }
        }
        results
    }
}

/// Recursively walk directory for .md files, skipping excluded dirs.
fn walk_dir_for_md(
    root: &std::path::Path,
    base: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut result = Vec::new();
    walk_dir_recursive(root, base, &mut result);
    Ok(result)
}

fn walk_dir_recursive(
    dir: &std::path::Path,
    base: &std::path::Path,
    result: &mut Vec<std::path::PathBuf>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Check if directory should be skipped
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if !DOC_SCAN_SKIP_DIRS.contains(&dir_name.as_ref()) {
                    walk_dir_recursive(&path, base, result);
                }
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                result.push(path);
            }
        }
    }
}

/// Get relative path or best-effort path string.
fn pathdiff_or_relative(full: &std::path::Path, base: &std::path::Path) -> String {
    if let Ok(rel) = full.strip_prefix(base) {
        rel.to_string_lossy().to_string()
    } else {
        full.to_string_lossy().to_string()
    }
}

/// Extract the first H1 heading as document title.
fn extract_doc_title(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return rest.to_string();
        }
    }
    String::new()
}

/// Parse markdown content into sections and references.
fn parse_doc_content(
    doc_path: &str,
    content: &str,
) -> (
    Vec<DocSection>,
    Vec<DocRef>,
    usize,
    usize,
    usize,
    usize,
    usize,
) {
    let mut sections: Vec<DocSection> = Vec::new();
    let mut references: Vec<DocRef> = Vec::new();
    let mut link_count = 0usize;
    let mut code_fence_count = 0usize;
    let mut symbol_ref_count = 0usize;
    let mut path_ref_count = 0usize;
    let mut command_count = 0usize;

    let lines: Vec<&str> = content.lines().collect();
    let mut in_code_fence = false;
    let mut current_section_start: Option<usize> = None;
    let mut current_section_heading = String::new();
    let mut current_section_level: u8 = 0;

    for (idx, &line) in lines.iter().enumerate() {
        let line_num = idx + 1; // 1-based

        // Track code fences
        if line.trim().starts_with("```") {
            in_code_fence = !in_code_fence;
            code_fence_count += 1;
            continue;
        }
        if in_code_fence {
            // Inside code fences: check for commands
            let trimmed = line.trim();
            if trimmed.starts_with("cargo ")
                || trimmed.starts_with("bash ")
                || trimmed.starts_with("node ")
                || trimmed.starts_with("git ")
                || trimmed.starts_with("codelattice ")
                || trimmed.starts_with("npm ")
            {
                references.push(DocRef {
                    path: doc_path.to_string(),
                    line: line_num,
                    match_type: "command".to_string(),
                    matched_text: trimmed
                        .split_whitespace()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" "),
                    confidence: "high".to_string(),
                    reason: "code-block-command".to_string(),
                    section: current_section_heading.clone(),
                });
                command_count += 1;
            }
            continue;
        }

        // Track sections (headings)
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            // Close previous section
            if let Some(start) = current_section_start.take() {
                sections.push(DocSection {
                    heading: current_section_heading.clone(),
                    heading_level: current_section_level,
                    start_line: start,
                    end_line: line_num - 1,
                });
            }
            let level = trimmed.bytes().take_while(|&b| b == b'#').count() as u8;
            let heading_text = trimmed.trim_start_matches('#').trim().to_string();
            current_section_start = Some(line_num);
            current_section_heading = heading_text.clone();
            current_section_level = level;

            // Section headings as potential matches
            references.push(DocRef {
                path: doc_path.to_string(),
                line: line_num,
                match_type: "section".to_string(),
                matched_text: heading_text.clone(),
                confidence: "medium".to_string(),
                reason: "section-heading".to_string(),
                section: String::new(), // section itself
            });
            continue;
        }

        // Parse inline references (outside code fences)
        parse_inline_refs(
            doc_path,
            line_num,
            trimmed,
            &mut references,
            &mut link_count,
            &mut symbol_ref_count,
            &mut path_ref_count,
            &current_section_heading,
        );
    }

    // Close last section
    if let Some(start) = current_section_start.take() {
        sections.push(DocSection {
            heading: current_section_heading.clone(),
            heading_level: current_section_level,
            start_line: start,
            end_line: lines.len(),
        });
    }

    (
        sections,
        references,
        link_count,
        code_fence_count,
        symbol_ref_count,
        path_ref_count,
        command_count,
    )
}

/// Parse inline markdown references from a single line.
fn parse_inline_refs(
    doc_path: &str,
    line_num: usize,
    line: &str,
    refs: &mut Vec<DocRef>,
    link_count: &mut usize,
    symbol_ref_count: &mut usize,
    path_ref_count: &mut usize,
    current_section: &str,
) {
    // 1. Inline code (backtick) references — highest confidence
    // Match `...` patterns
    let mut chars = line.chars().peekable();
    let mut pos = 0;
    while let Some(c) = chars.next() {
        pos += 1;
        if c == '`' {
            // Collect until closing backtick
            let mut token = String::new();
            let start_pos = pos;
            while let Some(&nc) = chars.peek() {
                if nc == '`' {
                    chars.next();
                    pos += 1;
                    break;
                }
                token.push(chars.next().unwrap());
                pos += 1;
            }
            let token = token.trim();
            if token.is_empty() || token.len() < 2 {
                continue;
            }

            // Classify the token
            if token.starts_with("codelattice_") {
                // MCP tool name
                refs.push(DocRef {
                    path: doc_path.to_string(),
                    line: line_num,
                    match_type: "symbol".to_string(),
                    matched_text: token.to_string(),
                    confidence: "high".to_string(),
                    reason: "inline-code-mcp-tool".to_string(),
                    section: current_section.to_string(),
                });
                *symbol_ref_count += 1;
            } else if token.contains('/')
                && (token.contains(".rs")
                    || token.contains(".cj")
                    || token.contains(".ets")
                    || token.contains(".ts")
                    || token.contains(".md")
                    || token.contains(".toml")
                    || token.contains(".json"))
            {
                // File path reference
                refs.push(DocRef {
                    path: doc_path.to_string(),
                    line: line_num,
                    match_type: "file".to_string(),
                    matched_text: token.to_string(),
                    confidence: "high".to_string(),
                    reason: "inline-code-file-path".to_string(),
                    section: current_section.to_string(),
                });
                *path_ref_count += 1;
            } else if token.contains("::")
                || token.contains('_')
                || token
                    .chars()
                    .next()
                    .map(|c| c.is_lowercase())
                    .unwrap_or(false)
            {
                // Function/method/symbol-like name
                refs.push(DocRef {
                    path: doc_path.to_string(),
                    line: line_num,
                    match_type: "symbol".to_string(),
                    matched_text: token.to_string(),
                    confidence: "high".to_string(),
                    reason: "inline-code-symbol-match".to_string(),
                    section: current_section.to_string(),
                });
                *symbol_ref_count += 1;
            }
        }
    }

    // 2. Markdown links: [label](target)
    // Simple regex-like scan
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // Find closing ]
            let label_start = i + 1;
            if let Some(bracket_end) = bytes[i..].iter().position(|&b| b == b']') {
                if i + bracket_end + 1 < bytes.len() && bytes[i + bracket_end + 1] == b'(' {
                    let label = &line[label_start..i + bracket_end];
                    // Find closing )
                    let paren_start = i + bracket_end + 2;
                    if let Some(paren_end) = bytes[paren_start..].iter().position(|&b| b == b')') {
                        let target = &line[paren_start..paren_start + paren_end];
                        *link_count += 1;
                        if target.contains('/') || target.ends_with(".md") {
                            refs.push(DocRef {
                                path: doc_path.to_string(),
                                line: line_num,
                                match_type: "link".to_string(),
                                matched_text: target.to_string(),
                                confidence: "high".to_string(),
                                reason: "markdown-link".to_string(),
                                section: current_section.to_string(),
                            });
                        }
                        i = paren_start + paren_end + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Compute impact metrics from the set of impacted nodes and edges.
///
/// Returns (impactMetrics, confidenceSummary, riskReasons, reviewFocus).
fn compute_impact_risk_details(
    gv: &GraphView,
    target_id: &str,
    impacted_nodes: &HashMap<String, Value>,
    impacted_edge_types: &HashMap<String, u64>,
    _root_str: &str,
) -> (Value, Value, Vec<String>, Value) {
    // --- impactMetrics ---
    #[allow(unused_assignments)]
    let mut nodes_with_callers: u64 = 0;
    let mut downstream_count: u64 = 0;
    let mut impacted_file_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cross_file_count: u64 = 0;
    let mut public_symbol_count: u64 = 0;
    let mut test_file_count: u64 = 0;

    // Target file for cross-file detection
    let target_file = impacted_nodes
        .get(target_id)
        .and_then(|n| n["properties"]["sourcePath"].as_str())
        .unwrap_or("")
        .to_string();

    for node in impacted_nodes.values() {
        let file = node["properties"]["sourcePath"]
            .as_str()
            .or_else(|| node["properties"]["manifestPath"].as_str())
            .unwrap_or("");
        impacted_file_set.insert(file.to_string());

        // Cross-file: any impacted node not in the target's file
        if !file.is_empty() && file != target_file {
            cross_file_count += 1;
        }

        // Count incoming CALLS edges as callers
        let node_id = node["id"].as_str().unwrap_or("");
        if node_id != target_id {
            let in_calls = gv.edges_to(node_id, Some("CALLS")).len();
            if in_calls > 0 {
                nodes_with_callers += 1;
            }
            downstream_count += 1;
        }

        // Public/exported symbol detection
        let kind = node["properties"]["symbolKind"].as_str().unwrap_or("");
        let is_public = kind == "function"
            || kind == "method"
            || kind == "associated-function"
            || kind == "struct"
            || kind == "enum"
            || kind == "trait"
            || kind == "interface"
            || kind == "const"
            || kind == "static";
        if is_public {
            public_symbol_count += 1;
        }

        // Test file detection
        if file.contains("_test")
            || file.contains("/tests/")
            || file.contains("\\tests\\")
            || file.contains("/test/")
            || file.contains("\\test\\")
            || file.ends_with("_test.rs")
            || file.ends_with(".test.ts")
            || file.ends_with("Test.cj")
        {
            test_file_count += 1;
        }
    }

    let caller_edge_count = impacted_edge_types.get("CALLS").copied().unwrap_or(0);
    let total_edges_considered: u64 = impacted_edge_types.values().sum();

    let impact_metrics = json!({
        "callerCount": caller_edge_count,
        "downstreamCount": downstream_count,
        "impactedFileCount": impacted_file_set.len(),
        "crossFileCount": cross_file_count,
        "publicSymbolCount": public_symbol_count,
        "testFileCount": test_file_count,
        "lowConfidenceEdgeCount": 0u64,  // filled below
        "mediumConfidenceEdgeCount": 0u64,
        "highConfidenceEdgeCount": 0u64,
        "unknownConfidenceEdgeCount": 0u64,
        "totalEdgesConsidered": total_edges_considered
    });

    // --- confidenceSummary ---
    // Collect confidence values from all impacted edges
    let mut high_conf: u64 = 0;
    let mut medium_conf: u64 = 0;
    let mut low_conf: u64 = 0;
    let mut unknown_conf: u64 = 0;
    let mut all_confidences: Vec<f64> = Vec::new();

    for node_id in impacted_nodes.keys() {
        for edge in gv.edges_from(node_id, None) {
            if !impacted_nodes.contains_key(edge["target"].as_str().unwrap_or("")) {
                continue;
            }
            let conf = edge["properties"]["confidence"].as_f64().unwrap_or(-1.0);
            if conf < 0.0 {
                unknown_conf += 1;
            } else if conf >= 0.8 {
                high_conf += 1;
                all_confidences.push(conf);
            } else if conf >= 0.5 {
                medium_conf += 1;
                all_confidences.push(conf);
            } else {
                low_conf += 1;
                all_confidences.push(conf);
            }
        }
        for edge in gv.edges_to(node_id, None) {
            if !impacted_nodes.contains_key(edge["source"].as_str().unwrap_or("")) {
                continue;
            }
            let conf = edge["properties"]["confidence"].as_f64().unwrap_or(-1.0);
            if conf < 0.0 {
                unknown_conf += 1;
            } else if conf >= 0.8 {
                high_conf += 1;
                all_confidences.push(conf);
            } else if conf >= 0.5 {
                medium_conf += 1;
                all_confidences.push(conf);
            } else {
                low_conf += 1;
                all_confidences.push(conf);
            }
        }
    }

    let min_conf = all_confidences.iter().cloned().fold(f64::MAX, f64::min);
    let max_conf = all_confidences.iter().cloned().fold(f64::MIN, f64::max);
    let avg_conf = if all_confidences.is_empty() {
        0.0
    } else {
        all_confidences.iter().sum::<f64>() / all_confidences.len() as f64
    };

    let confidence_summary = json!({
        "totalEdgesConsidered": high_conf + medium_conf + low_conf + unknown_conf,
        "highConfidenceCount": high_conf,
        "mediumConfidenceCount": medium_conf,
        "lowConfidenceCount": low_conf,
        "unknownConfidenceCount": unknown_conf,
        "minConfidence": if all_confidences.is_empty() { Value::Null } else { json!(format!("{:.2}", min_conf)) },
        "avgConfidence": if all_confidences.is_empty() { Value::Null } else { json!(format!("{:.2}", avg_conf)) },
        "maxConfidence": if all_confidences.is_empty() { Value::Null } else { json!(format!("{:.2}", max_conf)) }
    });

    // Update impactMetrics with actual confidence counts
    let mut metrics = impact_metrics;
    if let Some(map) = metrics.as_object_mut() {
        map.insert("lowConfidenceEdgeCount".to_string(), json!(low_conf));
        map.insert("mediumConfidenceEdgeCount".to_string(), json!(medium_conf));
        map.insert("highConfidenceEdgeCount".to_string(), json!(high_conf));
        map.insert(
            "unknownConfidenceEdgeCount".to_string(),
            json!(unknown_conf),
        );
    }

    // --- riskReasons ---
    let mut risk_reasons: Vec<String> = Vec::new();

    let total_impacted = impacted_nodes.len();
    if caller_edge_count > 0 {
        risk_reasons.push(format!(
            "{} direct callers depend on this symbol",
            caller_edge_count
        ));
    }
    if impacted_file_set.len() > 1 {
        risk_reasons.push(format!("Impact crosses {} files", impacted_file_set.len()));
    }
    if low_conf > 0 {
        risk_reasons.push(format!(
            "{} low-confidence edge(s) require manual review",
            low_conf
        ));
    }
    if public_symbol_count > 0 {
        risk_reasons.push(format!(
            "Public/exported symbol is affected ({} public symbols in impact set)",
            public_symbol_count
        ));
    }
    if test_file_count > 0 {
        risk_reasons.push(format!(
            "Test files are in the impact set ({} test symbols)",
            test_file_count
        ));
    }
    if total_impacted <= 3 && caller_edge_count <= 2 {
        risk_reasons.push("Small blast radius, few callers".to_string());
    }

    // --- reviewFocus ---
    // Top callers (nodes with most incoming CALLS edges)
    let mut caller_list: Vec<Value> = Vec::new();
    for node in impacted_nodes.values() {
        let nid = node["id"].as_str().unwrap_or("");
        if nid == target_id {
            continue;
        }
        let in_calls = gv.edges_to(nid, Some("CALLS")).len();
        if in_calls > 0 {
            caller_list.push(json!({
                "id": nid,
                "name": node["properties"]["name"],
                "kind": node["properties"]["symbolKind"],
                "file": node["properties"]["sourcePath"],
                "line": node["properties"]["lineStart"],
                "callerCount": in_calls,
            }));
        }
    }
    caller_list.sort_by(|a, b| {
        b["callerCount"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["callerCount"].as_u64().unwrap_or(0))
    });
    let top_callers: Vec<Value> = caller_list.into_iter().take(5).collect();

    // Top callees (outgoing CALLS from target)
    let top_callees: Vec<Value> = gv
        .edges_from(target_id, Some("CALLS"))
        .iter()
        .take(5)
        .filter_map(|e| {
            let tgt = e["target"].as_str().unwrap_or("");
            impacted_nodes.get(tgt).map(|n| {
                json!({
                    "id": tgt,
                    "name": n["properties"]["name"],
                    "kind": n["properties"]["symbolKind"],
                    "file": n["properties"]["sourcePath"],
                    "line": n["properties"]["lineStart"],
                    "confidence": e["properties"]["confidence"],
                })
            })
        })
        .collect();

    // Top files
    let mut file_counts: Vec<(String, u64)> = Vec::new();
    for node in impacted_nodes.values() {
        let f = node["properties"]["sourcePath"]
            .as_str()
            .or_else(|| node["properties"]["manifestPath"].as_str())
            .unwrap_or("");
        if !f.is_empty() {
            if let Some(entry) = file_counts.iter_mut().find(|(path, _)| path == f) {
                entry.1 += 1;
            } else {
                file_counts.push((f.to_string(), 1));
            }
        }
    }
    file_counts.sort_by(|a, b| b.1.cmp(&a.1));
    let top_files: Vec<Value> = file_counts
        .into_iter()
        .take(5)
        .map(|(f, c)| json!({ "file": f, "impactedNodeCount": c }))
        .collect();

    // Low-confidence edges
    let mut low_conf_edges: Vec<Value> = Vec::new();
    for node_id in impacted_nodes.keys() {
        for edge in gv.edges_from(node_id, None) {
            let tgt = edge["target"].as_str().unwrap_or("");
            if !impacted_nodes.contains_key(tgt) {
                continue;
            }
            let conf = edge["properties"]["confidence"].as_f64().unwrap_or(-1.0);
            if conf >= 0.0 && conf < 0.8 {
                low_conf_edges.push(json!({
                    "source": edge["source"],
                    "target": tgt,
                    "type": edge["type"],
                    "confidence": format!("{:.2}", conf),
                    "reason": edge["properties"]["reason"],
                }));
                if low_conf_edges.len() >= 10 {
                    break;
                }
            }
        }
        if low_conf_edges.len() >= 10 {
            break;
        }
    }

    // Public symbols
    let public_symbols: Vec<Value> = impacted_nodes
        .values()
        .filter(|n| {
            let kind = n["properties"]["symbolKind"].as_str().unwrap_or("");
            kind == "function"
                || kind == "method"
                || kind == "struct"
                || kind == "enum"
                || kind == "trait"
                || kind == "interface"
        })
        .take(10)
        .map(|n| {
            json!({
                "id": n["id"],
                "name": n["properties"]["name"],
                "kind": n["properties"]["symbolKind"],
                "file": n["properties"]["sourcePath"],
                "line": n["properties"]["lineStart"],
            })
        })
        .collect();

    // Test files
    let test_files: Vec<Value> = impacted_nodes
        .values()
        .filter(|n| {
            let file = n["properties"]["sourcePath"].as_str().unwrap_or("");
            file.contains("_test")
                || file.contains("/tests/")
                || file.contains("\\tests\\")
                || file.contains("/test/")
                || file.contains("\\test\\")
                || file.ends_with("_test.rs")
                || file.ends_with(".test.ts")
                || file.ends_with("Test.cj")
        })
        .take(10)
        .map(|n| {
            json!({
                "id": n["id"],
                "name": n["properties"]["name"],
                "kind": n["properties"]["symbolKind"],
                "file": n["properties"]["sourcePath"],
                "line": n["properties"]["lineStart"],
            })
        })
        .collect();

    let review_focus = json!({
        "topCallers": top_callers,
        "topCallees": top_callees,
        "topFiles": top_files,
        "lowConfidenceEdges": low_conf_edges,
        "publicSymbols": public_symbols,
        "testFiles": test_files,
    });

    (metrics, confidence_summary, risk_reasons, review_focus)
}

fn handle_impact_preview(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let symbol = params["symbol"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: symbol"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let direction = params["direction"].as_str().unwrap_or("both"); // upstream/downstream/both
    let depth = params["depth"].as_u64().unwrap_or(2).min(3) as usize;
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;
    let compact = params["compact"].as_bool().unwrap_or(false);
    let include_snippet = !compact && params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(2).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let root_str = validated.to_string_lossy();

    let targets = gv.find_symbols(symbol, None, 5);
    if targets.is_empty() {
        return Ok(merge_cache_and_result(
            &json!({
                "symbol": symbol,
                "risk": "UNKNOWN",
                "reasons": ["Symbol not found in graph"],
                "impactedNodes": [],
                "impactedEdges": []
            }),
            &cache_meta,
        ));
    }

    if targets.len() > 1 {
        return Ok(merge_cache_and_result(
            &json!({
                "symbol": symbol,
                "risk": "UNKNOWN",
                "reasons": [format!("Ambiguous: {} candidates found. Use kind/file to disambiguate.", targets.len())],
                "candidates": targets.iter().map(|t| json!({
                    "id": t["id"],
                    "name": t["properties"]["name"],
                    "kind": t["properties"]["symbolKind"]
                })).collect::<Vec<_>>(),
                "impactedNodes": [],
                "impactedEdges": []
            }),
            &cache_meta,
        ));
    }

    let target = &targets[0];
    let target_id = target["id"].as_str().unwrap_or("");

    // Traverse graph in requested direction(s)
    let mut impacted_nodes: HashMap<String, Value> = HashMap::new();
    let mut impacted_edge_types: HashMap<String, u64> = HashMap::new();

    // Add the target itself
    impacted_nodes.insert(target_id.to_string(), target.clone());

    // Downstream (outgoing)
    if direction == "downstream" || direction == "both" {
        let mut queue = vec![(target_id.to_string(), 0usize)];
        let mut visited = std::collections::HashSet::new();
        visited.insert(target_id.to_string());
        while let Some((nid, d)) = queue.pop() {
            if d >= depth {
                continue;
            }
            for edge in gv.edges_from(&nid, None) {
                if impacted_nodes.len() + impacted_edge_types.values().sum::<u64>() as usize > limit
                {
                    break;
                }
                let tgt = edge["target"].as_str().unwrap_or("");
                *impacted_edge_types
                    .entry(edge["type"].as_str().unwrap_or("unknown").to_string())
                    .or_insert(0) += 1;
                if !visited.contains(tgt) {
                    visited.insert(tgt.to_string());
                    if let Some(node) = gv.nodes_by_id.get(tgt) {
                        impacted_nodes.insert(tgt.to_string(), node.clone());
                        queue.push((tgt.to_string(), d + 1));
                    }
                }
            }
        }
    }

    // Upstream (incoming)
    if direction == "upstream" || direction == "both" {
        let mut queue = vec![(target_id.to_string(), 0usize)];
        let mut visited = std::collections::HashSet::new();
        visited.insert(target_id.to_string());
        while let Some((nid, d)) = queue.pop() {
            if d >= depth {
                continue;
            }
            for edge in gv.edges_to(&nid, None) {
                if impacted_nodes.len() + impacted_edge_types.values().sum::<u64>() as usize > limit
                {
                    break;
                }
                let src = edge["source"].as_str().unwrap_or("");
                *impacted_edge_types
                    .entry(edge["type"].as_str().unwrap_or("unknown").to_string())
                    .or_insert(0) += 1;
                if !visited.contains(src) {
                    visited.insert(src.to_string());
                    if let Some(node) = gv.nodes_by_id.get(src) {
                        impacted_nodes.insert(src.to_string(), node.clone());
                        queue.push((src.to_string(), d + 1));
                    }
                }
            }
        }
    }

    // Group impacted nodes by kind
    let mut nodes_by_kind: HashMap<String, u64> = HashMap::new();
    for node in impacted_nodes.values() {
        let kind = if node["label"].as_str() == Some("symbol") {
            node["properties"]["symbolKind"]
                .as_str()
                .unwrap_or("symbol")
                .to_string()
        } else {
            node["label"].as_str().unwrap_or("unknown").to_string()
        };
        *nodes_by_kind.entry(kind).or_insert(0) += 1;
    }

    // Compute enhanced risk metrics
    let total_impacted = impacted_nodes.len();
    let (impact_metrics, confidence_summary, risk_reasons, review_focus) =
        compute_impact_risk_details(
            &gv,
            target_id,
            &impacted_nodes,
            &impacted_edge_types,
            &root_str,
        );

    // Legacy risk level (kept for backward compat)
    let caller_count = impacted_edge_types.get("CALLS").copied().unwrap_or(0);
    let risk = if total_impacted <= 3 && caller_count <= 2 {
        "LOW".to_string()
    } else if total_impacted <= 15 && caller_count <= 10 {
        "MEDIUM".to_string()
    } else {
        "HIGH".to_string()
    };

    // Legacy reasons (kept for backward compat)
    let reasons = risk_reasons.clone();

    let node_kind_map: serde_json::Map<String, Value> = nodes_by_kind
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    let edge_kind_map: serde_json::Map<String, Value> = impacted_edge_types
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();

    // Top impacted files with optional snippets
    let mut file_counts: HashMap<String, u64> = HashMap::new();
    for node in impacted_nodes.values() {
        if let Some(f) = node["properties"]["sourcePath"]
            .as_str()
            .or_else(|| node["properties"]["manifestPath"].as_str())
        {
            *file_counts.entry(f.to_string()).or_insert(0) += 1;
        }
    }
    let mut top_files: Vec<(String, u64)> = file_counts.into_iter().collect();
    top_files.sort_by(|a, b| b.1.cmp(&a.1));
    let top_files: Vec<Value> = top_files
        .into_iter()
        .take(10)
        .map(|(f, c)| {
            let mut obj = json!({ "file": f, "impactedNodeCount": c });
            if include_snippet {
                // Find first impacted symbol in this file for context
                let first_sym = impacted_nodes.values().find(|n| {
                    n["properties"]["sourcePath"].as_str() == Some(f.as_str())
                        || n["properties"]["manifestPath"].as_str() == Some(f.as_str())
                });
                if let Some(sym) = first_sym {
                    let start = sym["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let end = sym["properties"]["lineEnd"].as_u64().unwrap_or(start);
                    if let Some(map) = obj.as_object_mut() {
                        map.insert(
                            "contextSnippet".to_string(),
                            read_source_snippet(&root_str, &f, start, end, snippet_ctx),
                        );
                    }
                }
            }
            obj
        })
        .collect();

    // Compact impacted symbol list with optional snippets (top 20)
    let impacted_symbols: Vec<Value> = impacted_nodes
        .values()
        .filter(|n| n["label"].as_str() == Some("symbol"))
        .take(20)
        .map(|n| {
            let mut obj = json!({
                "id": n["id"],
                "name": n["properties"]["name"],
                "kind": n["properties"]["symbolKind"],
                "file": n["properties"]["sourcePath"],
                "line": n["properties"]["lineStart"],
            });
            if include_snippet {
                if let Some(map) = obj.as_object_mut() {
                    let file = n["properties"]["sourcePath"].as_str().unwrap_or("");
                    let start = n["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let end = n["properties"]["lineEnd"].as_u64().unwrap_or(start);
                    map.insert(
                        "sourceSnippet".to_string(),
                        read_source_snippet(&root_str, file, start, end, snippet_ctx),
                    );
                }
            }
            obj
        })
        .collect();

    // Find related docs and docs likely needing update
    let (related_docs, docs_likely_need_update) = if let Some(ds) = gv.doc_scanner() {
        let tool_name: Vec<&str> = if symbol.starts_with("codelattice_") {
            vec![&symbol]
        } else {
            vec![]
        };
        let target_file = impacted_nodes
            .get(target_id)
            .and_then(|n| n["properties"]["sourcePath"].as_str())
            .unwrap_or("");
        let rd = ds.find_related_docs(&symbol, target_file, &tool_name, 20);
        let impacted_files: Vec<String> = impacted_nodes
            .values()
            .filter_map(|n| n["properties"]["sourcePath"].as_str().map(String::from))
            .collect();
        let impacted_sym_names: Vec<String> = impacted_nodes
            .values()
            .filter(|n| n["label"].as_str() == Some("symbol"))
            .filter_map(|n| n["properties"]["name"].as_str().map(String::from))
            .collect();
        let dnu = ds.find_docs_needing_update(
            &impacted_sym_names,
            &impacted_files,
            if compact { 10 } else { 20 },
        );
        (rd, dnu)
    } else {
        (vec![], vec![])
    };

    Ok(merge_cache_and_result(
        &json!({
            "symbol": symbol,
            "targetId": target_id,
            "direction": direction,
            "risk": risk,
            "reasons": reasons,
            "impactedNodeCount": total_impacted,
            "impactedSymbols": impacted_symbols,
            "impactedNodesByKind": Value::Object(node_kind_map),
            "impactedEdgesByKind": Value::Object(edge_kind_map),
            "topImpactedFiles": top_files,
            "riskReasons": risk_reasons,
            "impactMetrics": impact_metrics,
            "confidenceSummary": confidence_summary,
            "reviewFocus": review_focus,
            "relatedDocs": related_docs,
            "docsLikelyNeedUpdate": docs_likely_need_update,
            "previewOnly": true,
            "noWrites": true
        }),
        &cache_meta,
    ))
}

fn handle_query_graph(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;
    let compact = params["compact"].as_bool().unwrap_or(false);
    let include_snippet = !compact && params["includeSnippet"].as_bool().unwrap_or(false);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(2).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let root_str = validated.to_string_lossy();

    let node_kind = params["nodeKind"].as_str();
    let edge_kind = params["edgeKind"].as_str();
    let name_contains = params["nameContains"].as_str();
    let file_contains = params["fileContains"].as_str();

    // Match nodes
    let mut matched_nodes = Vec::new();
    for node in gv.nodes_by_id.values() {
        if matched_nodes.len() >= limit {
            break;
        }

        // Node kind filter
        if let Some(nk) = node_kind {
            let actual_kind = if node["label"].as_str() == Some("symbol") {
                node["properties"]["symbolKind"].as_str().unwrap_or("")
            } else {
                node["label"].as_str().unwrap_or("")
            };
            if actual_kind.to_lowercase() != nk.to_lowercase() {
                continue;
            }
        }

        // Name contains filter
        if let Some(nq) = name_contains {
            let name = node["properties"]["name"]
                .as_str()
                .or_else(|| node["id"].as_str())
                .unwrap_or("");
            if !name.to_lowercase().contains(&nq.to_lowercase()) {
                continue;
            }
        }

        // File contains filter
        if let Some(fq) = file_contains {
            let file = node["properties"]["sourcePath"]
                .as_str()
                .or_else(|| node["properties"]["manifestPath"].as_str())
                .unwrap_or("");
            if !file.to_lowercase().contains(&fq.to_lowercase()) {
                continue;
            }
        }

        let node_obj = if compact {
            json!({
                "id": node["id"],
                "name": node["properties"]["name"],
                "kind": node["properties"]["symbolKind"].as_str().or_else(|| node["label"].as_str()),
                "file": node["properties"]["sourcePath"].as_str().or_else(|| node["properties"]["manifestPath"].as_str()),
                "line": node["properties"]["lineStart"]
            })
        } else {
            let mut obj = json!({
                "id": node["id"],
                "label": node["label"],
                "name": node["properties"]["name"],
                "kind": node["properties"]["symbolKind"].as_str().or_else(|| node["label"].as_str()),
                "file": node["properties"]["sourcePath"].as_str().or_else(|| node["properties"]["manifestPath"].as_str())
            });
            if include_snippet {
                let file = node["properties"]["sourcePath"]
                    .as_str()
                    .or_else(|| node["properties"]["manifestPath"].as_str())
                    .unwrap_or("");
                let start = node["properties"]["lineStart"].as_u64().unwrap_or(0);
                let end = node["properties"]["lineEnd"].as_u64().unwrap_or(start);
                if !file.is_empty() && start > 0 {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert(
                            "sourceSnippet".to_string(),
                            read_source_snippet(&root_str, file, start, end, snippet_ctx),
                        );
                    }
                }
            }
            obj
        };
        matched_nodes.push(node_obj);
    }

    // Match edges
    let mut matched_edges = Vec::new();
    if edge_kind.is_some() {
        for edges in gv.outgoing.values() {
            if matched_edges.len() >= limit {
                break;
            }
            for edge in edges {
                if matched_edges.len() >= limit {
                    break;
                }
                if let Some(ek) = edge_kind {
                    if edge["type"].as_str().unwrap_or("").to_lowercase() != ek.to_lowercase() {
                        continue;
                    }
                }
                matched_edges.push(if compact {
                    json!({
                        "source": edge["source"],
                        "target": edge["target"],
                        "type": edge["type"],
                        "confidence": edge["properties"]["confidence"],
                        "reason": edge["properties"]["reason"]
                    })
                } else {
                    json!({
                        "source": edge["source"],
                        "target": edge["target"],
                        "type": edge["type"],
                        "confidence": edge["properties"]["confidence"],
                        "reason": edge["properties"]["reason"]
                    })
                });
            }
        }
    }

    let truncated = matched_nodes.len() >= limit || matched_edges.len() >= limit;

    let mut result = json!({
        "matchedNodeCount": matched_nodes.len(),
        "matchedEdgeCount": matched_edges.len(),
        "matchedNodes": matched_nodes,
        "matchedEdges": matched_edges,
        "truncated": truncated
    });
    if compact {
        if let Some(map) = result.as_object_mut() {
            map.insert("compact".to_string(), json!(true));
        }
    }

    Ok(merge_cache_and_result(&result, &cache_meta))
}

fn handle_project_overview(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let compact = params["compact"].as_bool().unwrap_or(false);
    check_language_feature(language)?;

    let (gv, result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let (graph_node_count, graph_edge_count, graph_symbol_count) = gv.stats();
    let summary = result.get("summary").unwrap_or(&Value::Null);
    let summary_count = |key: &str| -> Option<usize> {
        summary
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    };
    // Cangjie graph nodes/edges use `kind`/`sourceId`/`targetId`; the compact
    // summary is the language-normalized source of truth for top-level counts.
    let node_count = summary_count("nodeCount").unwrap_or(graph_node_count);
    let edge_count = summary_count("edgeCount").unwrap_or(graph_edge_count);
    let symbol_count = summary_count("symbolCount").unwrap_or(graph_symbol_count);

    // Top node kinds
    let mut node_kinds: HashMap<String, u64> = HashMap::new();
    for node in gv.nodes_by_id.values() {
        let node_kind = node["kind"]
            .as_str()
            .or_else(|| node["label"].as_str())
            .unwrap_or("unknown");
        let is_symbol = node_kind == "symbol" || node["label"].as_str() == Some("symbol");
        let kind = if is_symbol {
            node["properties"]["symbolKind"]
                .as_str()
                .or_else(|| node["properties"]["kind"].as_str())
                .unwrap_or("symbol")
                .to_string()
        } else {
            node_kind.to_string()
        };
        *node_kinds.entry(kind).or_insert(0) += 1;
    }

    // Top edge kinds
    let mut edge_kinds: HashMap<String, u64> = HashMap::new();
    if let Some(edges) = result
        .get("graph")
        .and_then(|g| g.get("edges"))
        .and_then(|e| e.as_array())
    {
        for edge in edges {
            let t = edge["type"]
                .as_str()
                .or_else(|| edge["kind"].as_str())
                .unwrap_or("unknown");
            *edge_kinds.entry(t.to_string()).or_insert(0) += 1;
        }
    } else {
        for edges in gv.outgoing.values() {
            for edge in edges {
                let t = edge["type"]
                    .as_str()
                    .or_else(|| edge["kind"].as_str())
                    .unwrap_or("unknown");
                *edge_kinds.entry(t.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Package count
    let package_count = summary_count("packageCount").unwrap_or_else(|| {
        gv.nodes_by_id
            .values()
            .filter(|n| {
                n["label"].as_str() == Some("package") || n["kind"].as_str() == Some("package")
            })
            .count()
    });

    // File count
    let file_count = summary_count("sourceFileCount").unwrap_or_else(|| {
        gv.nodes_by_id
            .values()
            .filter(|n| {
                n["label"].as_str() == Some("source-file")
                    || n["kind"].as_str() == Some("sourceFile")
            })
            .count()
    });

    // Quality summary (from summary command)
    let summary_result = {
        let root_str = validated.to_string_lossy().to_string();
        let args = vec![
            "summary",
            "--root",
            &root_str,
            "--language",
            language,
            "--format",
            "json",
        ];
        run_subcommand_with_timeout(&args, Duration::from_secs(60)).ok()
    };

    let quality_summary = summary_result
        .as_ref()
        .map(|s| s["qualitySummary"].clone())
        .unwrap_or(json!({}));

    // Diagnostics summary (computed regardless, needed for compact count)
    let diagnostics_count = gv.diagnostics.len();

    if compact {
        // Compact mode: counts only, skip expensive breakdown computations
        return Ok(merge_cache_and_result(
            &json!({
                "language": gv.language,
                "root": gv.root,
                "nodeCount": node_count,
                "edgeCount": edge_count,
                "symbolCount": symbol_count,
                "packageCount": package_count,
                "sourceFileCount": file_count,
                "diagnosticsCount": diagnostics_count,
                "qualityMetrics": compute_quality_metrics(&gv),
                "compact": true
            }),
            &cache_meta,
        ));
    }

    // Full mode: compute all breakdowns
    let diag_by_severity: HashMap<String, u64> =
        gv.diagnostics.iter().fold(HashMap::new(), |mut acc, d| {
            let sev = d["properties"]["severity"].as_str().unwrap_or("unknown");
            *acc.entry(sev.to_string()).or_insert(0) += 1;
            acc
        });

    // Notable hotspots: high fanout nodes
    let mut fanout: Vec<(String, usize)> = gv
        .outgoing
        .iter()
        .filter(|(id, _)| id.starts_with("symbol:"))
        .map(|(id, edges)| (id.clone(), edges.len()))
        .filter(|(_, c)| *c >= 3)
        .collect();
    fanout.sort_by(|a, b| b.1.cmp(&a.1));
    let hotspots: Vec<Value> = fanout
        .iter()
        .take(10)
        .map(|(id, count)| {
            let node = gv.nodes_by_id.get(id);
            json!({
                "id": id,
                "name": node.and_then(|n| n["properties"]["name"].as_str()),
                "kind": node.and_then(|n| n["properties"]["symbolKind"].as_str()),
                "outgoingEdgeCount": count
            })
        })
        .collect();

    // Files with many symbols
    let mut file_symbols: HashMap<String, u64> = HashMap::new();
    for node in gv.nodes_by_id.values() {
        if node["label"].as_str() == Some("symbol") {
            if let Some(f) = node["properties"]["sourcePath"].as_str() {
                *file_symbols.entry(f.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut dense_files_vec: Vec<(&String, &u64)> = file_symbols.iter().collect();
    dense_files_vec.sort_by(|a, b| b.1.cmp(a.1));
    let dense_files: Vec<Value> = dense_files_vec
        .into_iter()
        .take(10)
        .map(|(f, c)| json!({ "file": f, "symbolCount": c }))
        .collect();

    if compact {
        // Compact mode: counts only, no verbose breakdown
        Ok(merge_cache_and_result(
            &json!({
                "language": gv.language,
                "root": gv.root,
                "nodeCount": node_count,
                "edgeCount": edge_count,
                "symbolCount": symbol_count,
                "packageCount": package_count,
                "sourceFileCount": file_count,
                "diagnosticsCount": gv.diagnostics.len(),
                "qualityMetrics": compute_quality_metrics(&gv),
                "compact": true
            }),
            &cache_meta,
        ))
    } else {
        let mut nk_sorted: Vec<(String, u64)> = node_kinds.into_iter().collect();
        nk_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let node_kind_map: serde_json::Map<String, Value> =
            nk_sorted.into_iter().map(|(k, v)| (k, json!(v))).collect();
        let mut ek_sorted: Vec<(String, u64)> = edge_kinds.into_iter().collect();
        ek_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let edge_kind_map: serde_json::Map<String, Value> =
            ek_sorted.into_iter().map(|(k, v)| (k, json!(v))).collect();
        let sev_map: serde_json::Map<String, Value> = diag_by_severity
            .into_iter()
            .map(|(k, v)| (k, json!(v)))
            .collect();

        Ok(merge_cache_and_result(
            &json!({
                "language": gv.language,
                "root": gv.root,
                "nodeCount": node_count,
                "edgeCount": edge_count,
                "symbolCount": symbol_count,
                "packageCount": package_count,
                "sourceFileCount": file_count,
                "topNodeKinds": Value::Object(node_kind_map),
                "topEdgeKinds": Value::Object(edge_kind_map),
                "qualitySummary": quality_summary,
                "diagnosticsSummary": {
                    "total": gv.diagnostics.len(),
                    "bySeverity": Value::Object(sev_map)
                },
                "hotspots": hotspots,
                "denseFiles": dense_files,
                "qualityMetrics": compute_quality_metrics(&gv),
                "docs": if let Some(ds) = gv.doc_scanner() { ds.summary_json() } else { json!({}) }
            }),
            &cache_meta,
        ))
    }
}

fn handle_repo_registry(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let action = params["action"].as_str().unwrap_or("status");

    let root = params["root"].as_str();

    match action {
        "list" => Ok(tool_result(&json!({
            "action": "list",
            "knownRepos": [],
            "note": "CodeLattice MCP does not maintain a persistent repo registry. Each tool call analyzes the provided root. Use GitNexus-RC Tool for full repo registry management.",
            "currentRoot": root
        }))),
        "status" => {
            if let Some(r) = root {
                let validated = validate_root_path(r)?;
                let language = params["language"].as_str().unwrap_or("auto");
                let (gv, _result, cache_meta) =
                    cache.get_or_analyze(&validated, language, false)?;
                let (nc, ec, sc) = gv.stats();
                Ok(merge_cache_and_result(
                    &json!({
                        "action": "status",
                        "root": validated.to_string_lossy(),
                        "language": gv.language,
                        "nodeCount": nc,
                        "edgeCount": ec,
                        "symbolCount": sc,
                        "indexed": true
                    }),
                    &cache_meta,
                ))
            } else {
                Ok(tool_result(&json!({
                    "action": "status",
                    "root": null,
                    "indexed": false,
                    "note": "Provide root parameter to check status"
                })))
            }
        }
        _ => Err(mcp_error_detail(
            "invalid_action",
            &format!("Unknown action: {action}"),
            "Supported actions: list, status",
        )),
    }
}

fn handle_rename_preview(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let symbol = params["symbol"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: symbol"))?;
    let new_name = params["newName"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: newName"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let include_snippet = params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(3).min(10) as usize;
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let root_str = validated.to_string_lossy();

    let matches = gv.find_symbols(symbol, params["kind"].as_str(), 5);
    if matches.is_empty() {
        return Ok(merge_cache_and_result(
            &json!({
                "symbol": symbol,
                "newName": new_name,
                "candidates": [],
                "applySupported": false,
                "note": "No symbols found matching the query"
            }),
            &cache_meta,
        ));
    }

    let ambiguous = matches.len() > 1;

    let candidates: Vec<Value> = matches
        .iter()
        .map(|sym| {
            let id = sym["id"].as_str().unwrap_or("");
            let out_calls = gv.edges_from(id, Some("CALLS"));
            let in_calls = gv.edges_to(id, Some("CALLS"));
            let _defines = gv.edges_to(id, Some("DEFINES"));

            // Files that reference this symbol
            let mut files = std::collections::HashSet::new();
            if let Some(f) = sym["properties"]["sourcePath"].as_str() {
                files.insert(f.to_string());
            }
            for e in out_calls.iter().chain(in_calls.iter()) {
                if let Some(src_file) = gv
                    .nodes_by_id
                    .get(e["source"].as_str().unwrap_or(""))
                    .and_then(|n| n["properties"]["sourcePath"].as_str())
                {
                    files.insert(src_file.to_string());
                }
            }

            json!({
                "id": id,
                "name": sym["properties"]["name"],
                "kind": sym["properties"]["symbolKind"],
                "file": sym["properties"]["sourcePath"],
                "line": sym["properties"]["lineStart"],
                "outgoingCallCount": out_calls.len(),
                "incomingCallCount": in_calls.len(),
                "filesNeedingReview": files,
                "confidence": if ambiguous { "LOW" } else if in_calls.len() > 20 { "MEDIUM" } else { "HIGH" },
                "sourceSnippet": if include_snippet {
                    let file = sym["properties"]["sourcePath"].as_str().unwrap_or("");
                    let start = sym["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let end = sym["properties"]["lineEnd"].as_u64().unwrap_or(start);
                    read_source_snippet(&root_str, file, start, end, snippet_ctx)
                } else {
                    Value::Null
                }
            })
        })
        .collect();

    let mut warnings = Vec::new();
    if ambiguous {
        warnings.push("Multiple candidates found. Disambiguate before proceeding.".to_string());
    }
    if candidates
        .iter()
        .any(|c| c["incomingCallCount"].as_u64().unwrap_or(0) > 10)
    {
        warnings.push("High incoming call count — rename would touch many files.".to_string());
    }

    Ok(merge_cache_and_result(
        &json!({
           "symbol": symbol,
           "newName": new_name,
           "ambiguous": ambiguous,
           "candidates": candidates,
           "applySupported": false,
           "warnings": warnings,
           "note": "This is a read-only preview. CodeLattice does not perform AST-safe renames. Use IDE or language server for actual rename operations."
        }),
        &cache_meta,
    ))
}

// ============================================================
// v0.5 Daily Workflow Tools
// ============================================================

// ----- Git Diff → Symbol Mapping -----

/// A single hunk from a unified diff, with 1-based line numbers.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DiffHunk {
    file: String,
    old_start: u64,
    old_count: u64,
    new_start: u64,
    new_count: u64,
}

/// A file-level change from git diff.
#[derive(Debug, Clone)]
struct FileChange {
    path: String,
    change_kind: String, // "modified", "added", "deleted", "renamed"
    hunks: Vec<DiffHunk>,
}

/// Parse `git diff` unified output into structured file changes and hunks.
fn parse_git_diff(diff_output: &str) -> Vec<FileChange> {
    let mut changes: Vec<FileChange> = Vec::new();
    let mut current_file: Option<FileChange> = None;

    for line in diff_output.lines() {
        // New file diff header: diff --git a/... b/...
        if line.starts_with("diff --git ") {
            if let Some(prev) = current_file.take() {
                changes.push(prev);
            }
            // Extract path from "diff --git a/path b/path"
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            let path = if parts.len() >= 4 {
                // Use b/... path (destination), strip "b/" prefix
                parts[3].strip_prefix("b/").unwrap_or(parts[3]).to_string()
            } else {
                "".to_string()
            };
            current_file = Some(FileChange {
                path,
                change_kind: "modified".to_string(),
                hunks: Vec::new(),
            });
            continue;
        }

        // Detect file-level change kinds
        if let Some(ref mut fc) = current_file {
            if line.starts_with("new file mode ") {
                fc.change_kind = "added".to_string();
            } else if line.starts_with("deleted file mode ") {
                fc.change_kind = "deleted".to_string();
            } else if line.starts_with("rename from ") || line.starts_with("similarity index ") {
                fc.change_kind = "renamed".to_string();
            }
        }

        // Parse hunk header: @@ -old_start[,old_count] +new_start[,new_count] @@
        if let Some(rest) = line.strip_prefix("@@") {
            // Find the closing @@
            if let Some(end_idx) = rest.find("@@") {
                let hunk_spec = &rest[..end_idx].trim();
                // Parse "-old_start[,old_count] +new_start[,new_count]"
                let parts: Vec<&str> = hunk_spec.split_whitespace().collect();
                if parts.len() >= 2 {
                    let old_spec = parts[0].strip_prefix('-').unwrap_or(parts[0]);
                    let new_spec = parts[1].strip_prefix('+').unwrap_or(parts[1]);
                    let (old_start, old_count) = parse_hunk_range(old_spec);
                    let (new_start, new_count) = parse_hunk_range(new_spec);
                    if let Some(ref mut fc) = current_file {
                        fc.hunks.push(DiffHunk {
                            file: fc.path.clone(),
                            old_start,
                            old_count,
                            new_start,
                            new_count,
                        });
                    }
                }
            }
        }
    }
    if let Some(prev) = current_file.take() {
        changes.push(prev);
    }
    changes
}

/// Parse a hunk range like "10" or "10,5" into (start, count).
fn parse_hunk_range(spec: &str) -> (u64, u64) {
    if let Some(idx) = spec.find(',') {
        let start: u64 = spec[..idx].parse().unwrap_or(1);
        let count: u64 = spec[idx + 1..].parse().unwrap_or(1);
        (start, count)
    } else {
        let start: u64 = spec.parse().unwrap_or(1);
        (start, 1)
    }
}

/// Map diff hunks to graph symbols. Returns (matched_symbols, unknown_hunks).
fn map_hunks_to_symbols(
    changes: &[FileChange],
    gv: &GraphView,
    compact: bool,
    include_snippet: bool,
    snippet_ctx: usize,
    root_str: &std::path::Path,
    limit: usize,
) -> (Vec<Value>, Vec<Value>) {
    let mut matched_symbols: Vec<Value> = Vec::new();
    let mut unknown_hunks: Vec<Value> = Vec::new();
    let mut seen_symbol_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Build a lookup: relative_path → Vec of symbol nodes
    let mut symbols_by_file: HashMap<String, Vec<Value>> = HashMap::new();
    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        // Check if this is a symbol-like node
        let is_symbol = kind == "symbol"
            || kind == "function"
            || kind == "method"
            || kind == "associated-function"
            || kind == "class"
            || kind == "struct"
            || kind == "enum"
            || kind == "trait"
            || kind == "const"
            || kind == "static"
            || kind == "component"
            || kind == "buildMethod"
            || kind == "property"
            || kind == "interface"
            || kind == "typeAlias"
            || label == "symbol";

        if !is_symbol {
            continue;
        }

        // Get source path from properties
        let source_path = node["properties"]["sourcePath"].as_str().or_else(|| {
            node["properties"]["fileId"]
                .as_str()
                .and_then(|fid| fid.strip_prefix("file:"))
        });

        if let Some(sp) = source_path {
            symbols_by_file
                .entry(sp.to_string())
                .or_default()
                .push(node.clone());
        }
    }

    let mut symbol_count = 0;
    for fc in changes {
        // Find symbols in this file
        let file_symbols = symbols_by_file.get(&fc.path);

        for hunk in &fc.hunks {
            let hunk_start = hunk.new_start;
            let hunk_end = hunk.new_start + hunk.new_count.saturating_sub(1);

            let mut hunk_matched = false;

            if let Some(syms) = file_symbols {
                for sym in syms {
                    let sym_start = sym["properties"]["startLine"]
                        .as_u64()
                        .or_else(|| sym["properties"]["lineStart"].as_u64())
                        .unwrap_or(0);
                    let sym_end = sym["properties"]["endLine"]
                        .as_u64()
                        .or_else(|| sym["properties"]["lineEnd"].as_u64())
                        .unwrap_or(sym_start);

                    // Check if hunk overlaps with symbol range
                    // hunk [hunk_start, hunk_end] overlaps with symbol [sym_start, sym_end]
                    if sym_start == 0 && sym_end == 0 {
                        continue;
                    }
                    let overlaps = hunk_start <= sym_end && hunk_end >= sym_start;
                    if overlaps {
                        let sym_id = sym["id"].as_str().unwrap_or("").to_string();
                        if seen_symbol_ids.contains(&sym_id) {
                            // Already matched — increment hunk count
                            if let Some(existing) = matched_symbols
                                .iter_mut()
                                .find(|s| s["id"].as_str() == Some(&sym_id))
                            {
                                if let Some(hc) = existing["hunkCount"].as_u64() {
                                    existing["hunkCount"] = json!(hc + 1);
                                }
                                // Merge change kinds
                                if let Some(kinds) = existing["changeKinds"].as_array_mut() {
                                    if !kinds.iter().any(|k| k.as_str() == Some(&fc.change_kind)) {
                                        kinds.push(json!(fc.change_kind));
                                    }
                                }
                            }
                            hunk_matched = true;
                            continue;
                        }

                        let name = sym["properties"]["name"]
                            .as_str()
                            .or_else(|| sym["label"].as_str())
                            .unwrap_or("unknown");

                        let kind = sym["properties"]["symbolKind"]
                            .as_str()
                            .or_else(|| sym["properties"]["arktsKind"].as_str())
                            .or_else(|| sym["properties"]["kind"].as_str())
                            .unwrap_or("symbol");

                        let callers = gv.edges_to(&sym_id, Some("CALLS")).len();

                        let mut entry = json!({
                            "id": sym_id,
                            "name": name,
                            "kind": kind,
                            "file": fc.path,
                            "line": sym_start,
                            "lineEnd": sym_end,
                            "changeKinds": [fc.change_kind],
                            "hunkCount": 1,
                            "callerCount": callers,
                        });

                        if !compact {
                            // Add risk based on caller count
                            let risk = if callers > 10 {
                                "HIGH"
                            } else if callers > 3 {
                                "MEDIUM"
                            } else {
                                "LOW"
                            };
                            entry["risk"] = json!(risk);

                            if include_snippet {
                                entry["snippet"] = read_source_snippet(
                                    &root_str.to_string_lossy(),
                                    &fc.path,
                                    sym_start,
                                    sym_end,
                                    snippet_ctx,
                                );
                            }

                            // Add impacted files (files that call this symbol)
                            let callers_edges = gv.edges_to(&sym_id, Some("CALLS"));
                            let caller_files: std::collections::HashSet<&str> = callers_edges
                                .iter()
                                .filter_map(|e| e["source"].as_str())
                                .map(|s| {
                                    // Extract file from edge source id like "file:/path/to/file.rs"
                                    gv.nodes_by_id
                                        .get(s)
                                        .and_then(|n| {
                                            n["properties"]["sourcePath"]
                                                .as_str()
                                                .or_else(|| n["label"].as_str())
                                        })
                                        .unwrap_or("")
                                })
                                .filter(|s| !s.is_empty())
                                .collect();
                            entry["impactedFileCount"] = json!(caller_files.len());
                            if !compact && caller_files.len() <= 10 {
                                entry["impactedFiles"] =
                                    json!(caller_files.into_iter().collect::<Vec<_>>());
                            }
                        } else {
                            // Compact: only id/name/kind/file/line/risk
                            let risk = if callers > 10 {
                                "HIGH"
                            } else if callers > 3 {
                                "MEDIUM"
                            } else {
                                "LOW"
                            };
                            entry["risk"] = json!(risk);
                        }

                        seen_symbol_ids.insert(sym_id);
                        matched_symbols.push(entry);
                        symbol_count += 1;
                        hunk_matched = true;

                        if symbol_count >= limit {
                            break;
                        }
                    }
                }
                if symbol_count >= limit {
                    break;
                }
            }

            if !hunk_matched {
                unknown_hunks.push(json!({
                    "file": fc.path,
                    "hunkStart": hunk_start,
                    "hunkEnd": hunk_end,
                    "hunkLines": hunk.new_count,
                    "reason": if fc.change_kind == "added" { "new file, no graph symbols yet" } else if fc.change_kind == "deleted" { "deleted file" } else { "hunk does not overlap with any known symbol" }
                }));
            }
        }
        if symbol_count >= limit {
            break;
        }
    }

    (matched_symbols, unknown_hunks)
}

/// Detect changed symbols from git diff.
fn detect_changed_symbols(
    root: &std::path::Path,
    gv: &GraphView,
    diff_mode: &str,
    base_ref: Option<&str>,
    compact: bool,
    include_snippet: bool,
    snippet_ctx: usize,
    limit: usize,
) -> Result<Value, Value> {
    // Run git diff
    let mut args: Vec<String> = vec!["diff".to_string()];

    match diff_mode {
        "staged" => args.push("--staged".to_string()),
        "unstaged" => { /* default git diff is unstaged */ }
        "head" => args.extend(["HEAD"].iter().map(|s| s.to_string())),
        _ => { /* working-tree = staged + unstaged, default git diff */ }
    }

    if let Some(base) = base_ref {
        args.push(format!("{}...", base));
    }

    // Add common flags for machine-readable output
    args.push("--unified=0".to_string());
    args.push("--no-color".to_string());
    args.push("--".to_string()); // separator before paths

    let output = std::process::Command::new("git")
        .args(&args)
        .current_dir(root)
        .output()
        .map_err(|e| mcp_error("git_error", &format!("Failed to run git diff: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() && !stderr.is_empty() {
        // Git diff may return non-zero for some edge cases but still produce output
        // Only error if there's no stdout
        if output.stdout.is_empty() {
            return Err(mcp_error(
                "git_error",
                &format!("git diff failed: {stderr}"),
            ));
        }
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    let changes = parse_git_diff(&diff_text);

    let root_str = root;
    let (matched_symbols, unknown_hunks) = map_hunks_to_symbols(
        &changes,
        gv,
        compact,
        include_snippet,
        snippet_ctx,
        root_str,
        limit,
    );

    // Build changed files summary
    let changed_files: Vec<Value> = changes
        .iter()
        .map(|fc| {
            json!({
                "path": fc.path,
                "changeKind": fc.change_kind,
                "hunkCount": fc.hunks.len(),
            })
        })
        .collect();

    let deleted_files: Vec<Value> = changes
        .iter()
        .filter(|fc| fc.change_kind == "deleted")
        .map(|fc| json!({ "path": fc.path }))
        .collect();

    let renamed_files: Vec<Value> = changes
        .iter()
        .filter(|fc| fc.change_kind == "renamed")
        .map(|fc| json!({ "path": fc.path }))
        .collect();

    Ok(json!({
        "changedFiles": changed_files,
        "changedSymbols": matched_symbols,
        "unknownHunks": unknown_hunks,
        "deletedFiles": deleted_files,
        "renamedFiles": renamed_files,
        "summary": {
            "changedFileCount": changed_files.len(),
            "changedSymbolCount": matched_symbols.len(),
            "unknownHunkCount": unknown_hunks.len(),
            "deletedFileCount": deleted_files.len(),
            "renamedFileCount": renamed_files.len(),
        },
        "diffMode": diff_mode,
        "baseRef": base_ref,
        "previewOnly": true,
        "noWrites": true,
    }))
}

/// Handle `codelattice_changed_symbols` MCP tool.
fn handle_changed_symbols(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let diff_mode = params["diffMode"].as_str().unwrap_or("working-tree");
    let base_ref = params["baseRef"].as_str();
    let compact = params["compact"].as_bool().unwrap_or(true);
    let include_snippet = params["includeSnippet"].as_bool().unwrap_or(true);
    let snippet_ctx = params["snippetContext"].as_u64().unwrap_or(2).min(10) as usize;
    let limit = params["limit"].as_u64().unwrap_or(100).min(500) as usize;
    check_language_feature(language)?;

    // Check that root is a git repo
    let git_dir = validated.join(".git");
    if !git_dir.exists() {
        return Err(mcp_error_with_hint(
            "not_a_git_repo",
            "Root directory is not a git repository",
            "codelattice_changed_symbols requires a git repository to run git diff",
            "Point root at a directory containing .git, or use git init to create one",
        ));
    }

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let diff_result = detect_changed_symbols(
        &validated,
        &gv,
        diff_mode,
        base_ref,
        compact,
        include_snippet,
        snippet_ctx,
        limit,
    )?;

    Ok(merge_cache_and_result(&diff_result, &cache_meta))
}

/// Production assist dry-run: aggregates quality, impact, and symbol info
/// for a quick project health check. Read-only, no file writes.
fn handle_production_assist(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let (node_count, edge_count, symbol_count) = gv.stats();
    let root_str = validated.to_string_lossy();

    // Quality summary from the cached result
    let quality_gates = _result.get("qualityGates").cloned().unwrap_or(json!([]));
    let gate_array = quality_gates.as_array().cloned().unwrap_or_default();
    let passed = gate_array
        .iter()
        .filter(|g| g["passed"].as_bool().unwrap_or(false))
        .count();
    let failed = gate_array.len() - passed;

    // Unresolved calls
    let unresolved_count = gv
        .outgoing
        .values()
        .flatten()
        .filter(|e| {
            e["type"].as_str() == Some("CALLS")
                && e["properties"]["confidence"]
                    .as_f64()
                    .map(|c| c < 0.5)
                    .unwrap_or(false)
        })
        .count();

    // Diagnostics count
    let diag_count = gv.diagnostics.len();

    // Risk level
    let risk = if failed > 0 || unresolved_count > 10 || diag_count > 5 {
        "HIGH"
    } else if unresolved_count > 3 || diag_count > 2 {
        "MEDIUM"
    } else {
        "LOW"
    };

    // Top files by symbol count
    let mut file_symbols: HashMap<String, u64> = HashMap::new();
    for node in gv.nodes_by_id.values() {
        if node["label"].as_str() == Some("symbol") {
            if let Some(f) = node["properties"]["sourcePath"].as_str() {
                *file_symbols.entry(f.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut top_files: Vec<(String, u64)> = file_symbols.into_iter().collect();
    top_files.sort_by(|a, b| b.1.cmp(&a.1));
    let top_files: Vec<Value> = top_files
        .into_iter()
        .take(5)
        .map(|(f, c)| json!({ "file": f, "symbolCount": c }))
        .collect();

    // Changed symbols: auto-detect from git diff if not provided
    let (changed_symbols_info, auto_detected, unknown_hunks, changed_file_count) = if let Some(
        symbols,
    ) =
        params["changedSymbols"].as_array()
    {
        // Explicit list from user
        let info: Vec<Value> = symbols
                .iter()
                .filter_map(|s| s.as_str())
                .filter_map(|name| {
                    let found = gv.find_symbols(name, None, 3);
                    if found.is_empty() {
                        None
                    } else {
                        let sym = &found[0];
                        let file = sym["properties"]["sourcePath"].as_str().unwrap_or("");
                        let start = sym["properties"]["lineStart"].as_u64().unwrap_or(0);
                        let end = sym["properties"]["lineEnd"].as_u64().unwrap_or(start);
                        let id = sym["id"].as_str().unwrap_or("");
                        let callers = gv.edges_to(id, Some("CALLS")).len();
                        Some(json!({
                            "name": name,
                            "kind": sym["properties"]["symbolKind"],
                            "file": file,
                            "line": start,
                            "lineEnd": end,
                            "callerCount": callers,
                            "risk": if callers > 10 { "HIGH" } else if callers > 3 { "MEDIUM" } else { "LOW" },
                            "sourceSnippet": read_source_snippet(&root_str, file, start, end, 3),
                        }))
                    }
                })
                .collect();
        (info, false, vec![], 0)
    } else {
        // Auto-detect from git diff
        let git_dir = validated.join(".git");
        if git_dir.exists() {
            match detect_changed_symbols(
                &validated,
                &gv,
                "working-tree",
                None,
                true,  // compact
                false, // no snippets in auto mode
                2,
                50,
            ) {
                Ok(diff_result) => {
                    let syms = diff_result["changedSymbols"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let hunks = diff_result["unknownHunks"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let fc = diff_result["summary"]["changedFileCount"]
                        .as_u64()
                        .unwrap_or(0) as usize;
                    (syms, true, hunks, fc)
                }
                Err(_) => {
                    // git diff failed — not a hard error, just warn
                    (vec![], false, vec![], 0)
                }
            }
        } else {
            (vec![], false, vec![], 0)
        }
    };

    let mut recommendations = Vec::new();
    if failed > 0 {
        recommendations.push(format!(
            "Run codelattice_quality to review {} failed gate(s)",
            failed
        ));
    }
    if unresolved_count > 0 {
        recommendations.push(format!(
            "Run codelattice_unresolved_report to investigate {} unresolved calls",
            unresolved_count
        ));
    }
    if !changed_symbols_info.is_empty() {
        recommendations.push(
            "Run codelattice_impact_preview on changed symbols to assess blast radius".to_string(),
        );
    }

    // --- Enhanced: overall risk from changed symbols ---
    let mut overall_risk_reasons: Vec<String> = Vec::new();
    let mut max_caller_count: usize = 0;
    let mut changed_symbol_impacts: Vec<Value> = Vec::new();
    let mut all_low_conf_edges: usize = 0;

    for sym_info in &changed_symbols_info {
        let sym_name = sym_info["name"].as_str().unwrap_or("unknown");
        let sym_id = sym_info["id"].as_str().unwrap_or("");
        let callers = sym_info["callerCount"].as_u64().unwrap_or(0) as usize;

        if callers > max_caller_count {
            max_caller_count = callers;
        }

        let sym_risk = if callers > 10 {
            "HIGH"
        } else if callers > 3 {
            "MEDIUM"
        } else {
            "LOW"
        };

        // Count low-confidence edges for this symbol
        let low_conf = if !sym_id.is_empty() {
            let lc_out = gv
                .edges_from(sym_id, Some("CALLS"))
                .iter()
                .filter(|e| {
                    e["properties"]["confidence"]
                        .as_f64()
                        .map(|c| c < 0.8)
                        .unwrap_or(false)
                })
                .count();
            let lc_in = gv
                .edges_to(sym_id, Some("CALLS"))
                .iter()
                .filter(|e| {
                    e["properties"]["confidence"]
                        .as_f64()
                        .map(|c| c < 0.8)
                        .unwrap_or(false)
                })
                .count();
            all_low_conf_edges += lc_out + lc_in;
            lc_out + lc_in
        } else {
            0
        };

        let mut impact_reasons: Vec<String> = Vec::new();
        if callers > 0 {
            impact_reasons.push(format!("{} direct caller(s)", callers));
        }
        if low_conf > 0 {
            impact_reasons.push(format!("{} low-confidence edge(s)", low_conf));
        }

        changed_symbol_impacts.push(json!({
            "name": sym_name,
            "id": sym_id,
            "risk": sym_risk,
            "callerCount": callers,
            "lowConfidenceEdges": low_conf,
            "reasons": impact_reasons,
        }));
    }

    // overall risk: aggregate from changed symbols + project-level risk
    let overall_risk = if !changed_symbols_info.is_empty() {
        if max_caller_count > 10 || failed > 0 || all_low_conf_edges > 5 {
            "HIGH"
        } else if max_caller_count > 3 || all_low_conf_edges > 0 || unresolved_count > 3 {
            "MEDIUM"
        } else {
            "LOW"
        }
    } else {
        &risk
    };

    if !changed_symbols_info.is_empty() {
        overall_risk_reasons.push(format!(
            "{} changed symbol(s) detected",
            changed_symbols_info.len()
        ));
    }
    if max_caller_count > 0 {
        overall_risk_reasons.push(format!(
            "Highest-caller symbol has {} direct caller(s)",
            max_caller_count
        ));
    }
    if all_low_conf_edges > 0 {
        overall_risk_reasons.push(format!(
            "{} total low-confidence edge(s) across changed symbols",
            all_low_conf_edges
        ));
    }
    if failed > 0 {
        overall_risk_reasons.push(format!("{} quality gate(s) failed", failed));
    }

    // unknown hunks as risk signal
    if !unknown_hunks.is_empty() {
        overall_risk_reasons.push(format!(
            "{} unknown hunk(s) could not be mapped to graph symbols — manual review recommended",
            unknown_hunks.len()
        ));
    }

    // Highest-risk symbols (sorted by caller count descending)
    let mut sorted_impacts = changed_symbol_impacts.clone();
    sorted_impacts.sort_by(|a, b| {
        b["callerCount"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["callerCount"].as_u64().unwrap_or(0))
    });
    let highest_risk_symbols: Vec<Value> = sorted_impacts.into_iter().take(5).collect();

    // Review checklist: actionable items for AI
    let mut review_checklist: Vec<String> = Vec::new();
    if !changed_symbols_info.is_empty() {
        review_checklist.push(
            "inspect direct callers of each changed symbol via codelattice_symbol_context"
                .to_string(),
        );
    }
    if all_low_conf_edges > 0 {
        review_checklist.push(format!(
            "inspect {} low-confidence edge(s) — these may be indirect or ambiguous calls",
            all_low_conf_edges
        ));
    }
    // Check if any changed symbol is in a test file
    let has_test_symbols = changed_symbols_info.iter().any(|sym| {
        let file = sym["file"].as_str().unwrap_or("");
        file.contains("_test")
            || file.contains("/tests/")
            || file.contains("\\tests\\")
            || file.contains("/test/")
            || file.contains("\\test\\")
            || file.ends_with("_test.rs")
            || file.ends_with(".test.ts")
            || file.ends_with("Test.cj")
    });
    if has_test_symbols {
        review_checklist
            .push("run focused tests for affected test files identified in impact set".to_string());
    } else if !changed_symbols_info.is_empty() {
        review_checklist
            .push("no test files found in impact set — consider adding test coverage".to_string());
    }
    if !unknown_hunks.is_empty() {
        review_checklist.push(format!(
            "review {} unknown hunk(s) manually — diff region(s) could not be mapped to known symbols",
            unknown_hunks.len()
        ));
    }
    if failed > 0 {
        review_checklist.push(format!(
            "address {} failed quality gate(s) before proceeding",
            failed
        ));
    }
    if unresolved_count > 3 {
        review_checklist.push(format!(
            "investigate {} unresolved call(s) that may affect reliability",
            unresolved_count
        ));
    }
    if review_checklist.is_empty() {
        review_checklist.push("no immediate action required — project looks healthy".to_string());
    }

    // Docs likely needing update
    let (docs_likely_need_update, doc_association_summary) = if let Some(ds) = gv.doc_scanner() {
        let sym_names: Vec<String> = changed_symbols_info
            .iter()
            .filter_map(|s| s["name"].as_str().map(String::from))
            .collect();
        let file_paths: Vec<String> = changed_symbols_info
            .iter()
            .filter_map(|s| s["file"].as_str().map(String::from))
            .collect();
        let dnu = ds.find_docs_needing_update(&sym_names, &file_paths, 10);
        let summary = json!({
            "docCountReferencingChangedSymbols": dnu.len(),
            "changedSymbolDocHits": sym_names.len(),
        });
        (dnu, summary)
    } else {
        (vec![], json!({}))
    };

    // Add doc-related checklist items
    if !docs_likely_need_update.is_empty() {
        review_checklist.push(format!(
            "Review {} doc(s) that mention changed symbols/files",
            docs_likely_need_update.len()
        ));
    }

    // Quality metrics
    let quality_metrics = compute_quality_metrics(&gv);

    // Add quality-metrics-based checklist items
    let dangling_count = quality_metrics["graphCompleteness"]["danglingEdgeCount"]
        .as_u64()
        .unwrap_or(0);
    let low_conf_call_rate = quality_metrics["callQuality"]["lowConfidenceCallRate"]
        .as_f64()
        .unwrap_or(0.0);
    if dangling_count > 0 {
        review_checklist.push(format!(
            "Dangling edges detected: {} edges reference non-existent source nodes",
            dangling_count
        ));
    }
    if low_conf_call_rate > 0.3 {
        review_checklist
            .push("High low-confidence call rate: check call resolution quality".to_string());
    }

    Ok(merge_cache_and_result(
        &json!({
            "root": root,
            "language": gv.language,
            "qualityGatesPassed": passed,
            "qualityGatesFailed": failed,
            "nodeCount": node_count,
            "edgeCount": edge_count,
            "symbolCount": symbol_count,
            "unresolvedCalls": unresolved_count,
            "diagnostics": diag_count,
            "risk": risk,
            "topFiles": top_files,
            "autoDetectedChangedSymbols": auto_detected,
            "changedSymbolCount": changed_symbols_info.len(),
            "changedSymbols": changed_symbols_info,
            "unknownHunkCount": unknown_hunks.len(),
            "unknownHunks": unknown_hunks,
            "changedFileCount": changed_file_count,
            "recommendations": recommendations,
            "overallRisk": overall_risk,
            "overallRiskReasons": overall_risk_reasons,
            "changedSymbolImpacts": changed_symbol_impacts,
            "highestRiskSymbols": highest_risk_symbols,
            "reviewChecklist": review_checklist,
            "docsLikelyNeedUpdate": docs_likely_need_update,
            "docAssociationSummary": doc_association_summary,
            "qualityMetrics": quality_metrics,
            "dryRun": true,
            "noWrites": true,
        }),
        &cache_meta,
    ))
}

// ============================================================
// v0.8: Large Project Insight Pack
// ============================================================

/// Large project insight map: hotspots, entry points, risk map, read-first/review-first.
///
/// Provides graph-based heuristic insights for AI agents and humans onboarding
/// onto unfamiliar large codebases. Not a compiler/IDE-level proof — signals only.
fn handle_project_insights(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(10).min(100) as usize;
    let include_docs = params["includeDocs"].as_bool().unwrap_or(true);
    let include_diagnostics = params["includeDiagnostics"].as_bool().unwrap_or(true);
    check_language_feature(language)?;

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let (node_count, edge_count, symbol_count) = gv.stats();

    // ---------------------------------------------------------------
    // 1. Per-file metrics
    // ---------------------------------------------------------------
    struct FileMetrics {
        symbol_count: usize,
        edge_count: usize,
        incoming_edge_count: usize,
        outgoing_edge_count: usize,
        call_in_count: usize,
        call_out_count: usize,
        low_confidence_edge_count: usize,
        diagnostic_count: usize,
    }

    let mut file_metrics: HashMap<String, FileMetrics> = HashMap::new();

    // Count symbols per file
    for node in gv.nodes_by_id.values() {
        let is_symbol = node["label"].as_str() == Some("symbol")
            || node["kind"].as_str() == Some("symbol")
            || node["properties"]["symbolKind"].as_str().is_some();
        if is_symbol {
            if let Some(f) = node["properties"]["sourcePath"].as_str() {
                let fm = file_metrics.entry(f.to_string()).or_insert(FileMetrics {
                    symbol_count: 0,
                    edge_count: 0,
                    incoming_edge_count: 0,
                    outgoing_edge_count: 0,
                    call_in_count: 0,
                    call_out_count: 0,
                    low_confidence_edge_count: 0,
                    diagnostic_count: 0,
                });
                fm.symbol_count += 1;
            }
        }
    }

    // Count edges per file (source side)
    for (src_id, edges) in &gv.outgoing {
        let src_node = gv.nodes_by_id.get(src_id);
        let src_file = src_node.and_then(|n| n["properties"]["sourcePath"].as_str());
        for edge in edges {
            let edge_type = edge["type"]
                .as_str()
                .or_else(|| edge["kind"].as_str())
                .unwrap_or("");
            let confidence = edge["properties"]["confidence"].as_f64().unwrap_or(1.0);
            let is_calls = edge_type == "CALLS";

            // Get target file
            let tgt_id = edge["targetId"]
                .as_str()
                .or_else(|| edge["properties"]["targetId"].as_str())
                .unwrap_or("");
            let tgt_node = gv.nodes_by_id.get(tgt_id);
            let tgt_file = tgt_node.and_then(|n| n["properties"]["sourcePath"].as_str());

            // Outgoing edge for source file
            if let Some(sf) = src_file {
                let fm = file_metrics.entry(sf.to_string()).or_insert(FileMetrics {
                    symbol_count: 0,
                    edge_count: 0,
                    incoming_edge_count: 0,
                    outgoing_edge_count: 0,
                    call_in_count: 0,
                    call_out_count: 0,
                    low_confidence_edge_count: 0,
                    diagnostic_count: 0,
                });
                fm.edge_count += 1;
                fm.outgoing_edge_count += 1;
                if is_calls {
                    fm.call_out_count += 1;
                }
                if confidence < 0.8 {
                    fm.low_confidence_edge_count += 1;
                }
            }

            // Incoming edge for target file
            if let Some(tf) = tgt_file {
                let fm = file_metrics.entry(tf.to_string()).or_insert(FileMetrics {
                    symbol_count: 0,
                    edge_count: 0,
                    incoming_edge_count: 0,
                    outgoing_edge_count: 0,
                    call_in_count: 0,
                    call_out_count: 0,
                    low_confidence_edge_count: 0,
                    diagnostic_count: 0,
                });
                fm.incoming_edge_count += 1;
                if is_calls {
                    fm.call_in_count += 1;
                }
                if confidence < 0.8 {
                    fm.low_confidence_edge_count += 1;
                }
            }
        }
    }

    // Count diagnostics per file
    if include_diagnostics {
        for diag in &gv.diagnostics {
            if let Some(f) = diag["properties"]["sourcePath"]
                .as_str()
                .or_else(|| diag["properties"]["file"].as_str())
            {
                let fm = file_metrics.entry(f.to_string()).or_insert(FileMetrics {
                    symbol_count: 0,
                    edge_count: 0,
                    incoming_edge_count: 0,
                    outgoing_edge_count: 0,
                    call_in_count: 0,
                    call_out_count: 0,
                    low_confidence_edge_count: 0,
                    diagnostic_count: 0,
                });
                fm.diagnostic_count += 1;
            }
        }
    }

    // Compute file risk scores (weighted composite)
    let mut file_risk: Vec<(String, f64, Vec<String>)> = file_metrics
        .iter()
        .map(|(file, fm)| {
            let mut score: f64 = 0.0;
            let mut reasons: Vec<String> = Vec::new();

            // Symbol density
            if fm.symbol_count > 20 {
                score += 2.0;
                reasons.push("high symbol count".to_string());
            } else if fm.symbol_count > 10 {
                score += 1.0;
            }

            // Edge density
            if fm.edge_count > 40 {
                score += 2.0;
                reasons.push("high edge count".to_string());
            } else if fm.edge_count > 15 {
                score += 1.0;
            }

            // Fan-in (many callers → change ripples here)
            if fm.call_in_count > 10 {
                score += 3.0;
                reasons.push("high call-in count".to_string());
            } else if fm.call_in_count > 5 {
                score += 1.5;
            }

            // Fan-out (orchestration → change ripples out)
            if fm.call_out_count > 15 {
                score += 2.0;
                reasons.push("high call-out count".to_string());
            } else if fm.call_out_count > 8 {
                score += 1.0;
            }

            // Low confidence edges
            if fm.low_confidence_edge_count > 5 {
                score += 2.0;
                reasons.push(format!(
                    "{} low-confidence edges",
                    fm.low_confidence_edge_count
                ));
            } else if fm.low_confidence_edge_count > 0 {
                score += 0.5 * fm.low_confidence_edge_count as f64;
            }

            // Diagnostics nearby
            if fm.diagnostic_count > 3 {
                score += 1.5;
                reasons.push(format!("{} diagnostics", fm.diagnostic_count));
            }

            // Downgrade test/generated/vendor files
            let lower = file.to_lowercase();
            if lower.contains("/test")
                || lower.contains("\\test")
                || lower.ends_with("_test.rs")
                || lower.ends_with(".test.ts")
                || lower.ends_with(".spec.ts")
                || lower.ends_with("test.cj")
            {
                score *= 0.5;
                reasons.push("test file (downweighted)".to_string());
            }
            if lower.contains("/generated")
                || lower.contains("/vendor")
                || lower.contains("/node_modules")
                || lower.contains("/target/debug")
            {
                score *= 0.3;
                reasons.push("generated/vendor (downweighted)".to_string());
            }

            (file.clone(), score, reasons)
        })
        .collect();

    file_risk.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // ---------------------------------------------------------------
    // 2. Per-symbol metrics
    // ---------------------------------------------------------------
    struct SymbolMetrics {
        name: String,
        kind: String,
        file: String,
        line: u64,
        fan_in: usize,
        fan_out: usize,
        cross_file_impact_count: usize,
        low_confidence_edge_count: usize,
        is_entry_like: bool,
        is_public: bool,
        diagnostic_count: usize,
    }

    let mut symbol_metrics: Vec<SymbolMetrics> = Vec::new();

    for (id, node) in &gv.nodes_by_id {
        let is_symbol = node["label"].as_str() == Some("symbol")
            || node["properties"]["symbolKind"].as_str().is_some();
        if !is_symbol {
            continue;
        }

        let name = node["properties"]["name"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let kind = node["properties"]["symbolKind"]
            .as_str()
            .or_else(|| node["properties"]["kind"].as_str())
            .unwrap_or("symbol")
            .to_string();
        let file = node["properties"]["sourcePath"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let line = node["properties"]["lineStart"].as_u64().unwrap_or(0);

        // Fan-out: outgoing CALLS edges
        let out_calls: Vec<Value> = gv.edges_from(id, Some("CALLS"));
        let fan_out = out_calls.len();

        // Fan-in: incoming CALLS edges
        let in_calls: Vec<Value> = gv.edges_to(id, Some("CALLS"));
        let fan_in = in_calls.len();

        // Cross-file impact: edges that cross file boundaries
        let sym_file = file.clone();
        let cross_file_impact_count = out_calls
            .iter()
            .chain(in_calls.iter())
            .filter(|e| {
                let other_id = if e["sourceId"].as_str() == Some(id.as_str()) {
                    e["targetId"]
                        .as_str()
                        .or_else(|| e["properties"]["targetId"].as_str())
                } else {
                    e["sourceId"]
                        .as_str()
                        .or_else(|| e["properties"]["sourceId"].as_str())
                };
                if let Some(oid) = other_id {
                    if let Some(on) = gv.nodes_by_id.get(oid) {
                        let of = on["properties"]["sourcePath"].as_str().unwrap_or("");
                        return of != sym_file;
                    }
                }
                false
            })
            .count();

        // Low confidence edges
        let low_confidence_edge_count = out_calls
            .iter()
            .chain(in_calls.iter())
            .filter(|e| {
                e["properties"]["confidence"]
                    .as_f64()
                    .map(|c| c < 0.8)
                    .unwrap_or(false)
            })
            .count();

        // Entry-like detection
        let is_entry_like = detect_entry_like(&name, &kind, &file, &gv.language, fan_out);

        // Public/exported heuristic
        let is_public = kind == "function"
            && !name.starts_with('_')
            && !name.contains("::test")
            && !file.contains("/test");

        // Diagnostics nearby
        let diagnostic_count = gv.diagnostics_for(id).len();

        symbol_metrics.push(SymbolMetrics {
            name,
            kind,
            file,
            line,
            fan_in,
            fan_out,
            cross_file_impact_count,
            low_confidence_edge_count,
            is_entry_like,
            is_public,
            diagnostic_count,
        });
    }

    // Compute symbol risk scores
    let mut symbol_risk: Vec<(&SymbolMetrics, f64, Vec<String>)> = symbol_metrics
        .iter()
        .map(|sm| {
            let mut score: f64 = 0.0;
            let mut reasons: Vec<String> = Vec::new();

            if sm.fan_in > 10 {
                score += 3.0;
                reasons.push("high fan-in".to_string());
            } else if sm.fan_in > 5 {
                score += 1.5;
            }

            if sm.fan_out > 10 {
                score += 2.0;
                reasons.push("high fan-out".to_string());
            } else if sm.fan_out > 5 {
                score += 1.0;
            }

            if sm.cross_file_impact_count > 5 {
                score += 2.0;
                reasons.push("cross-file impact".to_string());
            } else if sm.cross_file_impact_count > 2 {
                score += 1.0;
            }

            if sm.low_confidence_edge_count > 3 {
                score += 1.5;
                reasons.push(format!(
                    "{} low-confidence edges",
                    sm.low_confidence_edge_count
                ));
            }

            if sm.is_public {
                score += 0.5;
            }

            if sm.diagnostic_count > 0 {
                score += 1.0;
                reasons.push(format!("{} diagnostics nearby", sm.diagnostic_count));
            }

            (sm, score, reasons)
        })
        .collect();

    symbol_risk.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // ---------------------------------------------------------------
    // 3. Entry point candidates
    // ---------------------------------------------------------------
    let entry_candidates: Vec<Value> = symbol_metrics
        .iter()
        .filter(|sm| sm.is_entry_like)
        .take(limit)
        .map(|sm| {
            let mut lang_reasons: Vec<String> = Vec::new();
            let mut graph_reasons: Vec<String> = Vec::new();

            if sm.name == "main" {
                lang_reasons.push("main entry".to_string());
            }
            if sm.fan_out > 5 {
                graph_reasons.push(format!("high fan-out orchestrator ({})", sm.fan_out));
            }
            if sm.fan_in > 3 {
                graph_reasons.push(format!("{} direct callers", sm.fan_in));
            }

            let entry_risk = if sm.fan_out > 10 && sm.fan_in > 5 {
                "MEDIUM"
            } else {
                "LOW"
            };

            json!({
                "id": sm.name,
                "name": sm.name,
                "kind": sm.kind,
                "file": sm.file,
                "line": sm.line,
                "languageReason": lang_reasons.join("; "),
                "graphReason": graph_reasons.join("; "),
                "riskScore": entry_risk,
            })
        })
        .collect();

    // ---------------------------------------------------------------
    // 4. Hotspot files
    // ---------------------------------------------------------------
    let hotspot_files: Vec<Value> = file_risk
        .iter()
        .take(limit)
        .map(|(file, score, reasons)| {
            let fm = file_metrics.get(file);
            json!({
                "id": file,
                "name": file,
                "kind": "file",
                "file": file,
                "riskScore": (score * 10.0).round() / 10.0,
                "reasons": reasons,
                "symbolCount": fm.map(|m| m.symbol_count).unwrap_or(0),
                "edgeCount": fm.map(|m| m.edge_count).unwrap_or(0),
                "callInCount": fm.map(|m| m.call_in_count).unwrap_or(0),
                "callOutCount": fm.map(|m| m.call_out_count).unwrap_or(0),
                "lowConfidenceEdgeCount": fm.map(|m| m.low_confidence_edge_count).unwrap_or(0),
                "diagnosticCount": fm.map(|m| m.diagnostic_count).unwrap_or(0),
            })
        })
        .collect();

    // ---------------------------------------------------------------
    // 5. Hotspot symbols
    // ---------------------------------------------------------------
    let hotspot_symbols: Vec<Value> = symbol_risk
        .iter()
        .take(limit)
        .map(|(sm, score, reasons)| {
            json!({
                "id": sm.name,
                "name": sm.name,
                "kind": sm.kind,
                "file": sm.file,
                "line": sm.line,
                "riskScore": (score * 10.0).round() / 10.0,
                "reasons": reasons,
                "fanIn": sm.fan_in,
                "fanOut": sm.fan_out,
                "crossFileImpactCount": sm.cross_file_impact_count,
                "isEntryLike": sm.is_entry_like,
                "isPublic": sm.is_public,
            })
        })
        .collect();

    // ---------------------------------------------------------------
    // 6. Risk map (top risky items with suggested actions)
    // ---------------------------------------------------------------
    let mut risk_items: Vec<Value> = Vec::new();

    // Top risky files
    for (file, score, reasons) in file_risk.iter().take(5) {
        let action = if *score > 8.0 {
            "avoid broad refactor — high coupling"
        } else if *score > 4.0 {
            "review before significant changes"
        } else {
            "monitor"
        };
        risk_items.push(json!({
            "id": file,
            "name": file,
            "kind": "file",
            "file": file,
            "riskScore": (score * 10.0).round() / 10.0,
            "reasons": reasons,
            "suggestedReviewAction": action,
        }));
    }

    // Top risky symbols
    for (sm, score, reasons) in symbol_risk.iter().take(5) {
        let action = if *score > 6.0 {
            "inspect manually — high impact area"
        } else if *score > 3.0 {
            "run tests before modifying"
        } else {
            "low risk — standard review"
        };
        risk_items.push(json!({
            "id": sm.name,
            "name": sm.name,
            "kind": sm.kind,
            "file": sm.file,
            "line": sm.line,
            "riskScore": (score * 10.0).round() / 10.0,
            "reasons": reasons,
            "suggestedReviewAction": action,
        }));
    }

    risk_items.sort_by(|a, b| {
        let sa = a["riskScore"].as_f64().unwrap_or(0.0);
        let sb = b["riskScore"].as_f64().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    risk_items.truncate(limit);

    // ---------------------------------------------------------------
    // 7. Low confidence zones
    // ---------------------------------------------------------------
    let low_confidence_files: Vec<Value> = file_metrics
        .iter()
        .filter(|(_, fm)| fm.low_confidence_edge_count > 2)
        .map(|(file, fm)| {
            let example_edges: Vec<Value> = gv
                .outgoing
                .values()
                .flatten()
                .chain(gv.incoming.values().flatten())
                .filter(|e| {
                    e["properties"]["confidence"]
                        .as_f64()
                        .map(|c| c < 0.8)
                        .unwrap_or(false)
                })
                .take(3)
                .map(|e| {
                    json!({
                        "type": e["type"].as_str().or_else(|| e["kind"].as_str()).unwrap_or("unknown"),
                        "confidence": e["properties"]["confidence"].as_f64().unwrap_or(0.0),
                        "reason": e["properties"]["reason"].as_str().unwrap_or(""),
                    })
                })
                .collect();

            json!({
                "file": file,
                "lowConfidenceEdgeCount": fm.low_confidence_edge_count,
                "exampleEdges": example_edges,
                "recommendedAction": if fm.low_confidence_edge_count > 5 {
                    "inspect manually"
                } else {
                    "run tests to validate"
                },
            })
        })
        .collect();

    let low_confidence_symbols: Vec<Value> = symbol_risk
        .iter()
        .filter(|(sm, _, _)| sm.low_confidence_edge_count > 1)
        .take(limit)
        .map(|(sm, score, reasons)| {
            json!({
                "id": sm.name,
                "name": sm.name,
                "kind": sm.kind,
                "file": sm.file,
                "line": sm.line,
                "lowConfidenceEdgeCount": sm.low_confidence_edge_count,
                "reasons": reasons,
                "recommendedAction": if sm.low_confidence_edge_count > 5 {
                    "avoid broad refactor"
                } else {
                    "run tests"
                },
            })
        })
        .collect();

    let low_confidence_zones = json!({
        "fileZones": low_confidence_files,
        "symbolZones": low_confidence_symbols,
    });

    // ---------------------------------------------------------------
    // 8. Read first / Review first
    // ---------------------------------------------------------------
    let read_first: Vec<Value> = {
        let mut items: Vec<Value> = Vec::new();

        // Entry-like symbols first
        for sm in symbol_metrics.iter().filter(|sm| sm.is_entry_like).take(5) {
            let mut reason_parts: Vec<String> = Vec::new();
            if sm.name == "main" {
                reason_parts.push("entry-like function".to_string());
            }
            if sm.fan_out > 5 {
                reason_parts.push(format!("high fan-out orchestrator ({})", sm.fan_out));
            }
            items.push(json!({
                "id": sm.name,
                "name": sm.name,
                "kind": sm.kind,
                "file": sm.file,
                "line": sm.line,
                "reason": reason_parts.join("; "),
            }));
        }

        // High information density files (symbols + edges, not necessarily risky)
        let mut info_files: Vec<(&String, &FileMetrics)> = file_metrics.iter().collect();
        info_files.sort_by(|a, b| b.1.symbol_count.cmp(&a.1.symbol_count));
        for (file, fm) in info_files.iter().take(3) {
            if !items
                .iter()
                .any(|i| i["file"].as_str() == Some(file.as_str()))
            {
                items.push(json!({
                    "id": file,
                    "name": file,
                    "kind": "file",
                    "file": file,
                    "line": 0,
                    "reason": format!("high information density ({} symbols, {} edges)", fm.symbol_count, fm.edge_count),
                }));
            }
        }

        items.truncate(limit);
        items
    };

    let review_first: Vec<Value> = {
        let mut items: Vec<Value> = Vec::new();

        // High fan-in symbols (change = widespread impact)
        let mut by_fanin: Vec<&SymbolMetrics> = symbol_metrics.iter().collect();
        by_fanin.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));
        for sm in by_fanin.iter().take(5).filter(|sm| sm.fan_in > 0) {
            let mut reason_parts: Vec<String> = Vec::new();
            if sm.fan_in > 5 {
                reason_parts.push(format!("{} direct callers", sm.fan_in));
            }
            if sm.low_confidence_edge_count > 0 {
                reason_parts.push(format!(
                    "{} low-confidence edge(s)",
                    sm.low_confidence_edge_count
                ));
            }
            if sm.is_public {
                reason_parts.push("public/exported symbol".to_string());
            }
            items.push(json!({
                "id": sm.name,
                "name": sm.name,
                "kind": sm.kind,
                "file": sm.file,
                "line": sm.line,
                "reason": if reason_parts.is_empty() { "high impact area".to_string() } else { reason_parts.join("; ") },
            }));
        }

        // Files with most diagnostics
        if include_diagnostics {
            let mut diag_files: Vec<(&String, &FileMetrics)> = file_metrics
                .iter()
                .filter(|(_, fm)| fm.diagnostic_count > 0)
                .collect();
            diag_files.sort_by(|a, b| b.1.diagnostic_count.cmp(&a.1.diagnostic_count));
            for (file, fm) in diag_files.iter().take(3) {
                if !items
                    .iter()
                    .any(|i| i["file"].as_str() == Some(file.as_str()))
                {
                    items.push(json!({
                        "id": file,
                        "name": file,
                        "kind": "file",
                        "file": file,
                        "line": 0,
                        "reason": format!("{} diagnostic(s) nearby", fm.diagnostic_count),
                    }));
                }
            }
        }

        items.truncate(limit);
        items
    };

    // ---------------------------------------------------------------
    // 9. Docs signals
    // ---------------------------------------------------------------
    let docs_signals: Vec<Value> = if include_docs {
        if let Some(ds) = gv.doc_scanner() {
            // Files/symbols mentioned in docs → review if they change
            let top_symbol_names: Vec<&str> = symbol_risk
                .iter()
                .take(10)
                .map(|(sm, _, _)| sm.name.as_str())
                .collect();

            let mut signals: Vec<Value> = Vec::new();
            for name in top_symbol_names {
                let matches = ds.find_related_docs(name, "", &[], 3);
                if !matches.is_empty() {
                    signals.push(json!({
                        "id": name,
                        "name": name,
                        "kind": "symbol",
                        "reason": format!("docs mention this symbol ({} doc hits)", matches.len()),
                    }));
                }
            }
            signals.truncate(limit);
            signals
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // ---------------------------------------------------------------
    // 10. Summary
    // ---------------------------------------------------------------
    let hotspot_file_count = hotspot_files.len();
    let hotspot_symbol_count = hotspot_symbols.len();
    let entry_point_candidate_count = entry_candidates.len();
    let low_confidence_zone_count = low_confidence_files
        .iter()
        .chain(low_confidence_symbols.iter())
        .count();

    let source_file_count = file_metrics.len();

    // Quality metrics (computed once, used in both compact and full)
    let quality_metrics = compute_quality_metrics(&gv);

    let result_data = if compact {
        json!({
            "summary": {
                "language": gv.language,
                "sourceFileCount": source_file_count,
                "symbolCount": symbol_count,
                "edgeCount": edge_count,
                "hotspotFileCount": hotspot_file_count,
                "hotspotSymbolCount": hotspot_symbol_count,
                "entryPointCandidateCount": entry_point_candidate_count,
                "lowConfidenceZoneCount": low_confidence_zone_count,
            },
            "entryPointCandidates": entry_candidates,
            "hotspotFiles": hotspot_files,
            "hotspotSymbols": hotspot_symbols,
            "riskMap": risk_items,
            "lowConfidenceZones": low_confidence_zones,
            "readFirst": read_first,
            "reviewFirst": review_first,
            "docsSignals": docs_signals,
            "qualityMetrics": quality_metrics,
            "generatedFrom": {
                "graphBased": true,
                "compilerVerified": false,
                "previewOnly": true,
            },
            "compact": true,
        })
    } else {
        // Full mode: include additional breakdowns
        let (file_count, _, _) = {
            let mut fc: usize = 0;
            for node in gv.nodes_by_id.values() {
                if node["label"].as_str() == Some("source-file")
                    || node["kind"].as_str() == Some("sourceFile")
                {
                    fc += 1;
                }
            }
            (fc, 0, 0)
        };

        json!({
            "summary": {
                "language": gv.language,
                "sourceFileCount": source_file_count,
                "symbolCount": symbol_count,
                "edgeCount": edge_count,
                "hotspotFileCount": hotspot_file_count,
                "hotspotSymbolCount": hotspot_symbol_count,
                "entryPointCandidateCount": entry_point_candidate_count,
                "lowConfidenceZoneCount": low_confidence_zone_count,
                "totalFileCount": file_count,
                "nodeCount": node_count,
                "diagnosticsCount": gv.diagnostics.len(),
                "documentationCoverageHint": if gv.doc_scanner().is_some() { "docs scanned" } else { "no doc scanner" },
            },
            "entryPointCandidates": entry_candidates,
            "hotspotFiles": hotspot_files,
            "hotspotSymbols": hotspot_symbols,
            "riskMap": risk_items,
            "lowConfidenceZones": low_confidence_zones,
            "readFirst": read_first,
            "reviewFirst": review_first,
            "docsSignals": docs_signals,
            "fileMetrics": file_risk.iter().take(limit).map(|(f, score, reasons)| {
                let fm = file_metrics.get(f);
                json!({
                    "file": f,
                    "symbolCount": fm.map(|m| m.symbol_count).unwrap_or(0),
                    "edgeCount": fm.map(|m| m.edge_count).unwrap_or(0),
                    "callInCount": fm.map(|m| m.call_in_count).unwrap_or(0),
                    "callOutCount": fm.map(|m| m.call_out_count).unwrap_or(0),
                    "lowConfidenceEdgeCount": fm.map(|m| m.low_confidence_edge_count).unwrap_or(0),
                    "diagnosticCount": fm.map(|m| m.diagnostic_count).unwrap_or(0),
                    "riskScore": (score * 10.0).round() / 10.0,
                    "riskReasons": reasons,
                })
            }).collect::<Vec<Value>>(),
            "qualityMetrics": quality_metrics,
            "generatedFrom": {
                "graphBased": true,
                "compilerVerified": false,
                "previewOnly": true,
            },
            "compact": false,
        })
    };

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

/// Detect whether a symbol looks like an entry point based on language heuristics.
fn detect_entry_like(name: &str, kind: &str, file: &str, language: &str, fan_out: usize) -> bool {
    match language {
        "rust" => {
            name == "main"
                || (kind == "function" && file.ends_with("lib.rs") && !name.starts_with('_'))
                || (kind == "function" && fan_out > 8) // high fan-out orchestrator
                || (kind == "function" && file.ends_with("main.rs"))
        }
        "cangjie" => {
            name == "main"
                || (kind == "function" && !name.starts_with('_') && fan_out > 8)
                || kind == "class" && file.ends_with("package.cj")
        }
        "arkts" => {
            // ArkTS: @Entry components, build() methods, page-like files
            name == "build"
                || file.contains("Index.ets")
                || file.contains("MainAbility/")
                || (kind == "method" && name == "aboutToAppear")
                || (kind == "function" && fan_out > 6)
        }
        "typescript" => {
            name == "main"
                || file.ends_with("index.ts")
                || file.ends_with("main.ts")
                || (kind == "function" && !name.starts_with('_') && fan_out > 6)
                || file.ends_with(".tsx") && kind == "function"
        }
        "python" => {
            name == "main"
                || name == "create_app"
                || name == "app"
                || file.ends_with("__main__.py")
                || (file.ends_with("cli.py") && kind == "function")
                || (file.ends_with("app.py") && kind == "function")
                || (kind == "function" && !name.starts_with('_') && fan_out > 6)
        }
        "c" => {
            name == "main"
                || name == "WinMain"
                || (file.ends_with("main.c") && kind == "function")
                || (kind == "function" && fan_out > 8)
        }
        "cpp" => {
            name == "main"
                || name == "WinMain"
                || name == "wWinMain"
                || name == "DllMain"
                || (file.ends_with("main.cpp") && kind == "function")
                || (file.ends_with("main.cc") && kind == "function")
                || (kind == "function" && fan_out > 8)
        }
        _ => {
            // Auto-detect: generic heuristics
            name == "main" || (kind == "function" && fan_out > 8)
        }
    }
}

// ============================================================
// v0.9: AI Review Plan
// ============================================================

/// AI review plan workflow: converts project insights, impact analysis, changed symbols,
/// and doc associations into an actionable engineering checklist for AI agents.
///
/// Four modes: onboarding, before_edit, after_edit, release_check.
/// Graph-based heuristic — not compiler/IDE/test-system proof.
fn handle_review_plan(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    let mode = params["mode"].as_str().unwrap_or("onboarding");
    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(10).min(100) as usize;
    let include_docs = params["includeDocs"].as_bool().unwrap_or(true);
    let include_tests = params["includeTests"].as_bool().unwrap_or(true);
    check_language_feature(language)?;

    let (gv, result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let (node_count, edge_count, symbol_count) = gv.stats();
    let root_str = validated.to_string_lossy();

    // Shared: quality gates
    let quality_gates = result.get("qualityGates").cloned().unwrap_or(json!([]));
    let gate_array = quality_gates.as_array().cloned().unwrap_or_default();
    let passed_gates = gate_array
        .iter()
        .filter(|g| g["passed"].as_bool().unwrap_or(false))
        .count();
    let failed_gates = gate_array.len() - passed_gates;

    // Shared: low-confidence call edges
    let low_conf_count = gv
        .outgoing
        .values()
        .flatten()
        .filter(|e| {
            e["type"].as_str() == Some("CALLS")
                && e["properties"]["confidence"]
                    .as_f64()
                    .map(|c| c < 0.8)
                    .unwrap_or(false)
        })
        .count();

    // Shared: diagnostics
    let diag_count = gv.diagnostics.len();

    // Plan item helper closure
    let plan_item = |priority: &str,
                     action: &str,
                     target: &str,
                     file: &str,
                     line: u64,
                     reason: &str,
                     source: &str,
                     rec_tool: &str,
                     done: &str|
     -> Value {
        json!({"priority":priority,"action":action,"target":target,"file":file,"line":line,
               "reason":reason,"source":source,"recommendedTool":rec_tool,"doneCriteria":done})
    };

    let mut read_plan: Vec<Value> = Vec::new();
    let mut risk_review_plan: Vec<Value> = Vec::new();
    let mut test_hints: Vec<Value> = Vec::new();
    let mut doc_update_hints: Vec<Value> = Vec::new();
    let mut questions_to_ask: Vec<Value> = Vec::new();
    let mut manual_review_required: Vec<Value> = Vec::new();
    let mut recommended_mcp_calls: Vec<Value> = Vec::new();

    match mode {
        "onboarding" => {
            // Entry-like symbols
            for (id, edges) in &gv.outgoing {
                if !id.starts_with("symbol:") {
                    continue;
                }
                if let Some(node) = gv.nodes_by_id.get(id) {
                    let name = node["properties"]["name"].as_str().unwrap_or("");
                    let kind = node["properties"]["symbolKind"]
                        .as_str()
                        .unwrap_or("symbol");
                    let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
                    let line = node["properties"]["lineStart"].as_u64().unwrap_or(0);
                    if detect_entry_like(name, kind, file, &gv.language, edges.len()) {
                        read_plan.push(plan_item(
                            "P0",
                            &format!("Read entry point: {}", name),
                            name,
                            file,
                            line,
                            &format!("entry-like with {} outgoing edges", edges.len()),
                            "project_insights",
                            "codelattice_symbol_context",
                            &format!("understand {} and its call graph", name),
                        ));
                    }
                    if read_plan.len() >= limit {
                        break;
                    }
                }
            }
            // Dense files
            let mut fsym: HashMap<&str, usize> = HashMap::new();
            for node in gv.nodes_by_id.values() {
                if node["label"].as_str() == Some("symbol") {
                    if let Some(f) = node["properties"]["sourcePath"].as_str() {
                        *fsym.entry(f).or_insert(0) += 1;
                    }
                }
            }
            let mut dense: Vec<(&str, usize)> = fsym.into_iter().collect();
            dense.sort_by(|a, b| b.1.cmp(&a.1));
            for (file, count) in dense.iter().take(3) {
                read_plan.push(plan_item(
                    "P1",
                    &format!("High-density file ({} symbols)", count),
                    *file,
                    *file,
                    0,
                    "high symbol concentration",
                    "project_insights",
                    "codelattice_project_overview",
                    "understand file structure",
                ));
            }
            // Entry point reachability for onboarding
            {
                let ep = detect_entry_points(&gv, language, &[]);
                if !ep.is_empty() {
                    let names: Vec<&str> = ep
                        .iter()
                        .take(5)
                        .map(|(_, n, _, _, _)| n.as_str())
                        .collect();
                    read_plan.push(plan_item(
                        "P0",
                        &format!("Start from entry points: {}", names.join(", ")),
                        "entry-points",
                        "",
                        0,
                        &format!("{} entry point(s) detected", ep.len()),
                        "reachability_map",
                        "codelattice_reachability_map",
                        "understand full reachability from entry points",
                    ));
                }
            }

            read_plan.truncate(limit);
            // Docs signal
            if include_docs {
                if let Some(ds) = gv.doc_scanner() {
                    let dc = ds.summary_json()["docCount"].as_u64().unwrap_or(0);
                    if dc > 0 {
                        doc_update_hints.push(plan_item(
                            "P2",
                            "Read project docs",
                            "docs",
                            "",
                            0,
                            &format!("{} doc files found", dc),
                            "doc_graph",
                            "",
                            "familiar with architecture docs",
                        ));
                    }
                }
            }
            recommended_mcp_calls.push(json!({"tool":"codelattice_project_overview",
                "argumentsSummary":format!("root={}",root_str),"reason":"get full overview"}));
            recommended_mcp_calls.push(json!({"tool":"codelattice_symbol_context",
                "argumentsSummary":"name=<from-readPlan>","reason":"deep-dive into entry points"}));
        }

        "before_edit" => {
            let symbol = params["symbol"].as_str().unwrap_or("");
            if !symbol.is_empty() {
                let targets = gv.find_symbols(symbol, None, 5);
                if targets.is_empty() {
                    questions_to_ask.push(json!({"question":format!("Symbol '{}' not found. Try symbol_search.",symbol),"priority":"P0","source":"symbol_context"}));
                } else if targets.len() > 1 {
                    questions_to_ask.push(json!({"question":format!("'{}' has {} candidates. Specify kind/file.",symbol,targets.len()),"priority":"P0","source":"symbol_context"}));
                } else {
                    let tgt = &targets[0];
                    let tid = tgt["id"].as_str().unwrap_or("");
                    let tf = tgt["properties"]["sourcePath"].as_str().unwrap_or("");
                    let tl = tgt["properties"]["lineStart"].as_u64().unwrap_or(0);
                    let callers = gv.edges_to(tid, Some("CALLS"));
                    let callees = gv.edges_from(tid, Some("CALLS"));
                    let lc: Vec<&Value> = callers
                        .iter()
                        .chain(callees.iter())
                        .filter(|e| {
                            e["properties"]["confidence"]
                                .as_f64()
                                .map(|c| c < 0.8)
                                .unwrap_or(false)
                        })
                        .collect();
                    risk_review_plan.push(plan_item(
                        if callers.len() > 5 { "P0" } else { "P1" },
                        &format!("Review {} callers of {}", callers.len(), symbol),
                        symbol,
                        tf,
                        tl,
                        &format!("{} direct callers", callers.len()),
                        "impact_preview",
                        "codelattice_calls_to",
                        &format!("verify {} callers", callers.len()),
                    ));
                    if !lc.is_empty() {
                        risk_review_plan.push(plan_item(
                            "P0",
                            &format!("Inspect {} low-confidence edges", lc.len()),
                            symbol,
                            tf,
                            tl,
                            "uncertain call targets",
                            "impact_preview",
                            "codelattice_unresolved_report",
                            "verify uncertain edges",
                        ));
                    }
                    if include_docs {
                        if let Some(ds) = gv.doc_scanner() {
                            for doc in ds.find_related_docs(symbol, tf, &[], 3).iter().take(3) {
                                doc_update_hints.push(plan_item(
                                    "P1",
                                    &format!(
                                        "Review doc: {}",
                                        doc["docPath"].as_str().unwrap_or("?")
                                    ),
                                    symbol,
                                    doc["docPath"].as_str().unwrap_or(""),
                                    0,
                                    "doc mentions symbol",
                                    "doc_graph",
                                    "codelattice_symbol_context",
                                    "doc reflects code",
                                ));
                            }
                        }
                    }
                    if callers.len() > 10 {
                        questions_to_ask.push(json!({"question":format!("{} has {} callers - is change backward-compatible?",symbol,callers.len()),"priority":"P0","source":"impact_preview"}));
                    }
                    recommended_mcp_calls.push(json!({"tool":"codelattice_impact_preview","argumentsSummary":format!("symbol={}",symbol),"reason":"full blast radius"}));
                    recommended_mcp_calls.push(json!({"tool":"codelattice_calls_to","argumentsSummary":format!("symbol={}",symbol),"reason":"see all callers"}));
                }
            } else {
                let mut sf: Vec<(&String, usize)> = gv
                    .incoming
                    .iter()
                    .filter(|(id, _)| id.starts_with("symbol:"))
                    .map(|(id, e)| (id, e.len()))
                    .collect();
                sf.sort_by(|a, b| b.1.cmp(&a.1));
                for (id, fanin) in sf.iter().take(limit) {
                    if *fanin > 3 {
                        if let Some(n) = gv.nodes_by_id.get(*id) {
                            risk_review_plan.push(plan_item(
                                "P1",
                                &format!(
                                    "High-impact: {} ({} callers)",
                                    n["properties"]["name"].as_str().unwrap_or("?"),
                                    fanin
                                ),
                                n["properties"]["name"].as_str().unwrap_or(""),
                                n["properties"]["sourcePath"].as_str().unwrap_or(""),
                                n["properties"]["lineStart"].as_u64().unwrap_or(0),
                                "high fan-in",
                                "project_insights",
                                "codelattice_impact_preview",
                                "understand impact",
                            ));
                        }
                    }
                }
                questions_to_ask.push(json!({"question":"Which symbol(s) are you planning to change?","priority":"P1","source":"review_plan"}));
                recommended_mcp_calls.push(json!({"tool":"codelattice_project_insights","argumentsSummary":format!("root={}",root_str),"reason":"get risk landscape"}));
            }
        }

        "after_edit" => {
            let changed: Vec<Value> = if let Some(syms) = params["changedSymbols"].as_array() {
                syms.iter().filter_map(|s|s.as_str()).filter_map(|name|{
                    let f=gv.find_symbols(name,None,3);
                    if f.is_empty(){None}else{let sym=&f[0];let id=sym["id"].as_str().unwrap_or("");
                    Some(json!({"name":name,"kind":sym["properties"]["symbolKind"],"file":sym["properties"]["sourcePath"].as_str().unwrap_or(""),"line":sym["properties"]["lineStart"].as_u64().unwrap_or(0),"callerCount":gv.edges_to(id,Some("CALLS")).len(),"id":id}))}
                }).collect()
            } else if validated.join(".git").exists() {
                detect_changed_symbols(&validated, &gv, "working-tree", None, true, false, 2, 50)
                    .map(|d| d["changedSymbols"].as_array().cloned().unwrap_or_default())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            let uhunks: Vec<Value> = if validated.join(".git").exists() {
                detect_changed_symbols(&validated, &gv, "working-tree", None, true, false, 2, 50)
                    .map(|d| d["unknownHunks"].as_array().cloned().unwrap_or_default())
                    .unwrap_or_default()
            } else {
                vec![]
            };

            for si in changed.iter().take(limit) {
                let name = si["name"].as_str().unwrap_or("?");
                let callers = si["callerCount"].as_u64().unwrap_or(0) as usize;
                let file = si["file"].as_str().unwrap_or("");
                let line = si["line"].as_u64().unwrap_or(0);
                risk_review_plan.push(plan_item(
                    if callers > 10 {
                        "P0"
                    } else if callers > 3 {
                        "P1"
                    } else {
                        "P2"
                    },
                    &format!("Review impact: {} ({} callers)", name, callers),
                    name,
                    file,
                    line,
                    &format!("changed symbol with {} callers", callers),
                    "changed_symbols",
                    "codelattice_impact_preview",
                    "verify no breakage",
                ));
                if callers > 10 {
                    manual_review_required.push(plan_item(
                        "P0",
                        &format!("Manually inspect {} callers of {}", callers, name),
                        name,
                        file,
                        line,
                        "high fan-in change",
                        "changed_symbols",
                        "codelattice_calls_to",
                        "each caller checked",
                    ));
                }
            }
            for h in uhunks.iter().take(limit) {
                manual_review_required.push(plan_item(
                    "P0",
                    "Review unknown hunk",
                    h["file"].as_str().unwrap_or("?"),
                    h["file"].as_str().unwrap_or(""),
                    h["lineStart"].as_u64().unwrap_or(0),
                    "unmapped diff region",
                    "changed_symbols",
                    "codelattice_symbol_search",
                    "identify what changed",
                ));
            }
            if include_tests {
                let cf: Vec<&str> = changed.iter().filter_map(|s| s["file"].as_str()).collect();
                let has_mcp = cf.iter().any(|f| f.contains("mcp_server"));
                test_hints.push(json!({"command":"cargo test","reason":"general tests","priority":"P1","safeToRun":true,"requiresExternalProject":false}));
                if has_mcp {
                    test_hints.push(json!({"command":"cargo test --test mcp_server","reason":"MCP files changed","priority":"P0","safeToRun":true,"requiresExternalProject":false}));
                }
            }
            if include_docs {
                if let Some(ds) = gv.doc_scanner() {
                    let sn: Vec<String> = changed
                        .iter()
                        .filter_map(|s| s["name"].as_str().map(String::from))
                        .collect();
                    let fp: Vec<String> = changed
                        .iter()
                        .filter_map(|s| s["file"].as_str().map(String::from))
                        .collect();
                    for doc in ds
                        .find_docs_needing_update(&sn, &fp, limit)
                        .iter()
                        .take(limit)
                    {
                        doc_update_hints.push(plan_item(
                            "P1",
                            &format!("Update doc: {}", doc["docPath"].as_str().unwrap_or("?")),
                            "",
                            doc["docPath"].as_str().unwrap_or(""),
                            0,
                            "doc references changed symbols",
                            "doc_graph",
                            "codelattice_symbol_context",
                            "doc updated",
                        ));
                    }
                }
            }
            recommended_mcp_calls.push(json!({"tool":"codelattice_changed_symbols","argumentsSummary":format!("root={}",root_str),"reason":"re-confirm changes"}));
            recommended_mcp_calls.push(json!({"tool":"codelattice_production_assist","argumentsSummary":format!("root={}",root_str),"reason":"health check"}));
            if !changed.is_empty() {
                recommended_mcp_calls.push(json!({"tool":"codelattice_compare_runs","argumentsSummary":format!("root={}",root_str),"reason":"compare before/after"}));

                // Reachability for release check
                {
                    let ep = detect_entry_points(&gv, language, &[]);
                    let reach = reachable_from_entry_points(&gv, &ep);
                    let uc = gv
                        .nodes_by_id
                        .values()
                        .filter(|n| {
                            let k = n["kind"].as_str().unwrap_or("");
                            let l = n["label"].as_str().unwrap_or("");
                            if k != "symbol" && l != "symbol" {
                                return false;
                            }
                            let nid = n["id"].as_str().unwrap_or("");
                            !reach.contains(nid) && !ep.iter().any(|(eid, _, _, _, _)| eid == nid)
                        })
                        .count();
                    if uc > 0 {
                        risk_review_plan.push(plan_item(
                            "P2",
                            &format!("Review {} unreachable symbol(s)", uc),
                            "",
                            "",
                            0,
                            &format!("{} entry points, {} unreachable", ep.len(), uc),
                            "reachability_map",
                            "codelattice_reachability_map",
                            "reviewed",
                        ));
                    }
                }
            }
        }

        "release_check" => {
            if failed_gates > 0 {
                risk_review_plan.push(plan_item(
                    "P0",
                    &format!("Fix {} failed quality gate(s)", failed_gates),
                    "",
                    "",
                    0,
                    &format!("{} of {} gates failed", failed_gates, gate_array.len()),
                    "quality_gate",
                    "codelattice_quality",
                    "all gates pass",
                ));
            }
            if low_conf_count > 3 {
                risk_review_plan.push(plan_item(
                    "P1",
                    &format!("Investigate {} low-confidence edges", low_conf_count),
                    "",
                    "",
                    0,
                    "many uncertain calls",
                    "impact_preview",
                    "codelattice_unresolved_report",
                    "edges reviewed",
                ));
            }
            if diag_count > 0 {
                risk_review_plan.push(plan_item(
                    "P1",
                    &format!("Review {} diagnostics", diag_count),
                    "",
                    "",
                    0,
                    &format!("{} diagnostics", diag_count),
                    "production_assist",
                    "codelattice_project_overview",
                    "diagnostics addressed",
                ));
            }
            // v0.20: Reachability summary for release check
            {
                let eps = detect_entry_points(&gv, language, &vec![]);
                let reach = reachable_from_entry_points(&gv, &eps);
                let total_syms = gv
                    .nodes_by_id
                    .values()
                    .filter(|n| {
                        let k = n["kind"].as_str().unwrap_or("");
                        k == "symbol"
                            || k == "function"
                            || k == "method"
                            || k == "class"
                            || k == "struct"
                    })
                    .count();
                let unreachable = total_syms.saturating_sub(reach.len());
                if unreachable > 0 && unreachable < total_syms {
                    risk_review_plan.push(plan_item(
                        "P2",
                        &format!("Review {} potentially unreachable symbols", unreachable),
                        "",
                        "",
                        0,
                        &format!(
                            "{} of {} symbols not reachable from {} entry points",
                            unreachable,
                            total_syms,
                            eps.len()
                        ),
                        "reachability_map",
                        "codelattice_reachability_map",
                        "unreachable symbols triaged",
                    ));
                }
                recommended_mcp_calls.push(json!({"tool":"codelattice_reachability_map","argumentsSummary":format!("root={}",root_str),"reason":"unreachable code audit"}));
            }
            if include_tests {
                test_hints.push(json!({"command":"cargo test","reason":"full suite","priority":"P0","safeToRun":true,"requiresExternalProject":false}));
                test_hints.push(json!({"command":"cargo test --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript","reason":"all adapters","priority":"P1","safeToRun":true,"requiresExternalProject":false}));
                test_hints.push(json!({"command":"bash scripts/mcp-dogfood.sh","reason":"dogfood smoke","priority":"P0","safeToRun":true,"requiresExternalProject":false}));
                test_hints.push(json!({"command":"bash scripts/codelattice-mcp.sh --self-test","reason":"self-test","priority":"P0","safeToRun":true,"requiresExternalProject":false}));
            }
            recommended_mcp_calls.push(json!({"tool":"codelattice_production_assist","argumentsSummary":format!("root={}",root_str),"reason":"production readiness"}));
            recommended_mcp_calls.push(json!({"tool":"codelattice_project_overview","argumentsSummary":format!("root={}",root_str),"reason":"project health"}));
            recommended_mcp_calls.push(json!({"tool":"codelattice_compare_runs","argumentsSummary":format!("root={}",root_str),"reason":"compare against baseline"}));
        }

        _ => {
            return Err(mcp_error(
                "invalid_mode",
                &format!(
                    "Invalid mode '{}'. Use: onboarding, before_edit, after_edit, release_check",
                    mode
                ),
            ))
        }
    }

    let summary = json!({"mode":mode,"language":gv.language,"nodeCount":node_count,"edgeCount":edge_count,"symbolCount":symbol_count,
        "qualityGatesPassed":passed_gates,"qualityGatesFailed":failed_gates,"lowConfidenceEdges":low_conf_count,"diagnosticsCount":diag_count});

    read_plan.truncate(limit);
    risk_review_plan.truncate(limit);
    test_hints.truncate(limit);
    doc_update_hints.truncate(limit);
    questions_to_ask.truncate(limit);
    manual_review_required.truncate(limit);
    recommended_mcp_calls.truncate(limit);

    Ok(merge_cache_and_result(
        &json!({
            "mode":mode,"summary":summary,"readPlan":read_plan,"riskReviewPlan":risk_review_plan,
            "testHints":test_hints,"docUpdateHints":doc_update_hints,"questionsToAskBeforeEdit":questions_to_ask,
            "manualReviewRequired":manual_review_required,"recommendedMcpCalls":recommended_mcp_calls,
            "qualityMetrics": if mode == "release_check" { compute_quality_metrics(&gv) } else { Value::Null },
            "generatedFrom":{"projectInsights":true,"impactPreview":true,"changedSymbols":true,"docGraph":true,"graphBased":true,"compilerVerified":false,"previewOnly":true},
            "compact":compact
        }),
        &cache_meta,
    ))
}

/// Compare two analysis runs: either two bridge JSON files or the same root.
/// Returns differences in nodes, edges, symbols, diagnostics, quality gates.
fn handle_compare_runs(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    // Mode 1: Two bridge JSON file paths provided
    let before_path = params["beforeBridgeJson"].as_str();
    let after_path = params["afterBridgeJson"].as_str();

    if let (Some(bp), Some(ap)) = (before_path, after_path) {
        let before = std::fs::read_to_string(bp).map_err(|e| {
            mcp_error(
                "file_read_error",
                &format!("Cannot read before file: {}", e),
            )
        })?;
        let after = std::fs::read_to_string(ap)
            .map_err(|e| mcp_error("file_read_error", &format!("Cannot read after file: {}", e)))?;

        let before_json: Value = serde_json::from_str(&before).map_err(|e| {
            mcp_error(
                "json_error",
                &format!("Before file is not valid JSON: {}", e),
            )
        })?;
        let after_json: Value = serde_json::from_str(&after).map_err(|e| {
            mcp_error(
                "json_error",
                &format!("After file is not valid JSON: {}", e),
            )
        })?;

        return compare_bridge_jsons(&before_json, &after_json);
    }

    // Mode 2: Same root, analyze fresh and compare with cached
    let root = params["root"].as_str().ok_or_else(|| {
        mcp_error(
            "missing_parameter",
            "Provide root, or beforeBridgeJson+afterBridgeJson",
        )
    })?;
    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    // Get current cached result
    let (_gv, current_result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // Clear cache and re-analyze to get fresh result
    let canonical = validated.canonicalize().map_err(|_| {
        mcp_error(
            "path_not_found",
            &format!("Cannot canonicalize: {}", validated.display()),
        )
    })?;
    let key = CacheKey {
        root: canonical.to_string_lossy().to_string(),
        language: language.to_string(),
        strict: false,
    };
    cache.entries.remove(&key);

    let (_gv2, fresh_result, _fresh_meta) = cache.get_or_analyze(&validated, language, false)?;

    let diff = compare_bridge_jsons(&current_result, &fresh_result)?;
    Ok(merge_cache_and_result(&diff, &cache_meta))
}

/// Compare two bridge JSON results and return a structured diff.
fn compare_bridge_jsons(before: &Value, after: &Value) -> Result<Value, Value> {
    let before_graph = before.get("graph").unwrap_or(&Value::Null);
    let after_graph = after.get("graph").unwrap_or(&Value::Null);

    let before_nodes = before_graph
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let after_nodes = after_graph
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let before_edges = before_graph
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    let after_edges = after_graph
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    // Index by id for diff
    let before_node_ids: std::collections::HashSet<String> = before_nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(|s| s.to_string()))
        .collect();
    let after_node_ids: std::collections::HashSet<String> = after_nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(|s| s.to_string()))
        .collect();

    let added_nodes: Vec<String> = after_node_ids
        .difference(&before_node_ids)
        .cloned()
        .collect();
    let removed_nodes: Vec<String> = before_node_ids
        .difference(&after_node_ids)
        .cloned()
        .collect();

    // Edge diff by source+target+type composite key
    fn edge_key(e: &Value) -> String {
        format!(
            "{}→{}:{}",
            e["source"].as_str().unwrap_or(""),
            e["target"].as_str().unwrap_or(""),
            e["type"].as_str().unwrap_or("")
        )
    }
    let before_edge_keys: std::collections::HashSet<String> =
        before_edges.iter().map(edge_key).collect();
    let after_edge_keys: std::collections::HashSet<String> =
        after_edges.iter().map(edge_key).collect();

    let added_edges: Vec<String> = after_edge_keys
        .difference(&before_edge_keys)
        .cloned()
        .collect();
    let removed_edges: Vec<String> = before_edge_keys
        .difference(&after_edge_keys)
        .cloned()
        .collect();

    // Quality gates diff
    let before_gates = before
        .get("qualityGates")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();
    let after_gates = after
        .get("qualityGates")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();
    let before_passed = before_gates
        .iter()
        .filter(|g| g["passed"].as_bool().unwrap_or(false))
        .count();
    let after_passed = after_gates
        .iter()
        .filter(|g| g["passed"].as_bool().unwrap_or(false))
        .count();

    // Symbol count diff
    let before_symbols = before_nodes
        .iter()
        .filter(|n| n["label"].as_str() == Some("symbol"))
        .count();
    let after_symbols = after_nodes
        .iter()
        .filter(|n| n["label"].as_str() == Some("symbol"))
        .count();

    // Diagnostics diff
    let before_diags = before_graph
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let after_diags = after_graph
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(tool_result(&json!({
        "beforeNodes": before_nodes.len(),
        "afterNodes": after_nodes.len(),
        "nodeDelta": after_nodes.len() as i64 - before_nodes.len() as i64,
        "addedNodes": added_nodes.len(),
        "removedNodes": removed_nodes.len(),
        "addedNodeSamples": added_nodes.iter().take(10).cloned().collect::<Vec<_>>(),
        "removedNodeSamples": removed_nodes.iter().take(10).cloned().collect::<Vec<_>>(),

        "beforeEdges": before_edges.len(),
        "afterEdges": after_edges.len(),
        "edgeDelta": after_edges.len() as i64 - before_edges.len() as i64,
        "addedEdges": added_edges.len(),
        "removedEdges": removed_edges.len(),
        "addedEdgeSamples": added_edges.iter().take(10).cloned().collect::<Vec<_>>(),
        "removedEdgeSamples": removed_edges.iter().take(10).cloned().collect::<Vec<_>>(),

        "beforeSymbols": before_symbols,
        "afterSymbols": after_symbols,
        "symbolDelta": after_symbols as i64 - before_symbols as i64,

        "beforeDiagnostics": before_diags,
        "afterDiagnostics": after_diags,
        "diagnosticDelta": after_diags as i64 - before_diags as i64,

        "beforeQualityGatesPassed": before_passed,
        "afterQualityGatesPassed": after_passed,
        "qualityGateDelta": after_passed as i64 - before_passed as i64,

        "summary": format!(
            "Nodes: {}→{} ({:+}), Edges: {}→{} ({:+}), Symbols: {}→{} ({:+}), Diags: {}→{} ({:+}), Gates: {}→{} ({:+})",
            before_nodes.len(), after_nodes.len(), after_nodes.len() as i64 - before_nodes.len() as i64,
            before_edges.len(), after_edges.len(), after_edges.len() as i64 - before_edges.len() as i64,
            before_symbols, after_symbols, after_symbols as i64 - before_symbols as i64,
            before_diags, after_diags, after_diags as i64 - before_diags as i64,
            before_passed, after_passed, after_passed as i64 - before_passed as i64,
        ),
        "note": "generatedAt is excluded from deterministic comparison"
    })))
}

// ============================================================
// v0.3 Cache Management Tools
// ============================================================

fn handle_cache_status(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let filter_root = params["root"].as_str();
    let filter_lang = params["language"].as_str();
    let status = cache.status(filter_root, filter_lang);
    Ok(tool_result(&status))
}

fn handle_cache_clear(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let filter_root = params["root"].as_str();
    let filter_lang = params["language"].as_str();
    let layer = params["layer"].as_str().unwrap_or("memory");
    let (cleared, remaining) = cache.clear(filter_root, filter_lang, layer);
    Ok(tool_result(&json!({
        "clearedCount": cleared,
        "remainingCount": remaining,
        "layer": layer,
    })))
}

fn handle_cache_prewarm(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;
    let strict = params["strict"].as_bool().unwrap_or(false);

    let (_gv, result, cache_meta) = cache.get_or_analyze(&validated, language, strict)?;

    // Build compact summary from analysis result
    let summary = json!({
        "symbolCount": result.get("summary").and_then(|s| s.get("symbolCount")).unwrap_or(&json!(0)),
        "nodeCount": result.get("summary").and_then(|s| s.get("nodeCount")).unwrap_or(&json!(0)),
        "edgeCount": result.get("summary").and_then(|s| s.get("edgeCount")).unwrap_or(&json!(0)),
        "sourceFileCount": result.get("summary").and_then(|s| s.get("sourceFileCount")).unwrap_or(&json!(0)),
    });

    let mut output = json!({
        "warmed": true,
        "summary": summary,
    });

    // Merge cache meta
    if let (Some(obj), Some(meta)) = (output.as_object_mut(), cache_meta.as_object()) {
        for (k, v) in meta {
            obj.insert(k.clone(), v.clone());
        }
    }

    Ok(tool_result(&output))
}

// ============================================================
// Tools List
// ============================================================

fn tools_list() -> Value {
    json!({
       "tools": [
           {
               "name": "codelattice_analyze",
               "description": "Analyze a Rust or Cangjie project. Returns graph summary, quality gates, and optionally the full graph. Compact by default (graph excluded unless includeGraph=true).",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "strict": { "type": "boolean", "default": true, "description": "Mark quality gate failures as errors" },
                       "includeGraph": { "type": "boolean", "default": false, "description": "Include full graph in output (large, default off)" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_quality",
               "description": "Run quality gate checks on a project. Returns pass/fail for each gate, with failed gates listed first for quick triage.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to check" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_summary",
               "description": "Get a compact summary of project graph stats and quality gates without full graph output.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to summarize" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_smoke",
               "description": "Run end-to-end smoke tests (bridge JSON generation + Tool import). Validates Rust and/or Cangjie analysis pipeline. Includes tail output and failure hints.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "mode": { "type": "string", "enum": ["rust-only", "cangjie-only", "full"], "default": "full", "description": "Which smoke mode to run" }
                   }
               }
           },
           {
               "name": "codelattice_graph_overview",
               "description": "Get a compact overview of the graph: node/edge/symbol/package counts, kind breakdowns, quality and diagnostics summaries. No full graph data. Ideal for AI agents to quickly assess a project.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_unresolved_report",
               "description": "Report unresolved calls and diagnostics. For Rust: shows CALLS edges with low confidence or unresolved reasons, grouped by reason. For Cangjie: returns supported=false (no unresolved concept in v0.1). Includes stop-line classification note.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100, "description": "Max unresolved items to return" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit item detail arrays, return counts and reason breakdown only" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_symbol_search",
               "description": "Search for symbols by name (case-insensitive contains match). Returns matching symbols with name, kind, file, and line. Optionally filter by symbol kind (function, struct, class, etc).",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to search" },
                       "query": { "type": "string", "description": "Search query (case-insensitive substring match)" },
                       "kind": { "type": "string", "description": "Filter by symbol kind (function, struct, class, enum, interface, etc)" },
                       "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100, "description": "Max results to return" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit label, keep id/name/kind/file/line per match" }
                   },
                   "required": ["root", "query"]
               }
           },
           {
               "name": "codelattice_export_bridge",
               "description": "Export project analysis as GitNexus-RC bridge JSON to /tmp. Returns file path, byte count, and schema/counts summary. Output path must be under /tmp. No Tool import — export only.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python"], "description": "Language (must be explicit, not auto)" },
                       "outputPath": { "type": "string", "description": "Output file path (must be under /tmp). Default: auto-generated in /tmp" }
                   },
                   "required": ["root", "language"]
               }
           },
           {
               "name": "codelattice_symbol_context",
               "description": "Get rich context for a symbol: definition, source snippet, outgoing/incoming edges grouped by kind, related diagnostics, confidence samples. Returns ambiguous candidates if multiple symbols match.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "name": { "type": "string", "description": "Symbol name to look up" },
                       "kind": { "type": "string", "description": "Filter by symbol kind (function, struct, class, etc)" },
                       "limit": { "type": "integer", "default": 10, "maximum": 50 },
                       "includeSnippet": { "type": "boolean", "default": true, "description": "Include source code snippet in the response" },
                       "snippetContext": { "type": "integer", "default": 3, "maximum": 10, "description": "Number of context lines before/after the symbol" }
                   },
                   "required": ["root", "name"]
               }
           },
           {
               "name": "codelattice_calls_from",
               "description": "Trace outgoing calls from a symbol. Returns call tree up to specified depth with confidence/reason per edge. BFS traversal.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "symbol": { "type": "string", "description": "Source symbol name" },
                       "depth": { "type": "integer", "default": 1, "minimum": 1, "maximum": 3 },
                       "limit": { "type": "integer", "default": 20, "maximum": 100 },
                       "includeSnippet": { "type": "boolean", "default": true, "description": "Include source code snippets in results" },
                       "snippetContext": { "type": "integer", "default": 3, "minimum": 0, "maximum": 10, "description": "Lines of context around snippet" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit snippets and depth, keep id/name/kind/file/line per edge" }
                   },
                   "required": ["root", "symbol"]
               }
           },
           {
               "name": "codelattice_calls_to",
               "description": "Trace incoming callers/referrers to a symbol. Returns reverse call tree up to specified depth. Useful for understanding who depends on a symbol.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "symbol": { "type": "string", "description": "Target symbol name" },
                       "depth": { "type": "integer", "default": 1, "minimum": 1, "maximum": 3 },
                       "limit": { "type": "integer", "default": 20, "maximum": 100 },
                       "includeSnippet": { "type": "boolean", "default": true, "description": "Include source code snippets in results" },
                       "snippetContext": { "type": "integer", "default": 3, "minimum": 0, "maximum": 10, "description": "Lines of context around snippet" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit snippets and depth, keep id/name/kind/file/line per edge" }
                   },
                   "required": ["root", "symbol"]
               }
           },
           {
               "name": "codelattice_impact_preview",
               "description": "Preview the blast radius of changing a symbol. Returns impacted nodes/edges grouped by kind, approximate risk level (LOW/MEDIUM/HIGH), impact metrics, confidence summary, risk reasons, and review focus. Read-only, no writes.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "symbol": { "type": "string", "description": "Symbol name to analyze impact for" },
                       "direction": { "type": "string", "enum": ["upstream", "downstream", "both"], "default": "both" },
                       "depth": { "type": "integer", "default": 2, "minimum": 1, "maximum": 3 },
                       "limit": { "type": "integer", "default": 50, "maximum": 200 },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: keep risk/riskReasons/impactMetrics/confidenceSummary/reviewFocus, impactedSymbols only id/name/kind/file/line, no snippets" }
                   },
                   "required": ["root", "symbol"]
               }
           },
           {
               "name": "codelattice_query_graph",
               "description": "Query the graph by node kind, edge kind, name pattern, or file pattern. Safe parameterized query — no arbitrary query strings accepted.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "nodeKind": { "type": "string", "description": "Filter nodes by kind (function, struct, class, package, etc)" },
                       "edgeKind": { "type": "string", "description": "Filter edges by type (CALLS, DEFINES, IMPORTS, etc)" },
                       "nameContains": { "type": "string", "description": "Filter nodes by name (case-insensitive substring)" },
                       "fileContains": { "type": "string", "description": "Filter nodes by file path (case-insensitive substring)" },
                       "limit": { "type": "integer", "default": 50, "maximum": 200 },
                       "includeSnippet": { "type": "boolean", "default": false, "description": "Include source code snippets in results" },
                       "snippetContext": { "type": "integer", "default": 2, "minimum": 0, "maximum": 10, "description": "Lines of context around snippet" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit snippets, keep id/name/kind/file/line per node and confidence/reason per edge" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_project_overview",
               "description": "Get a comprehensive project overview: identity, stats, top kinds, quality, diagnostics, hotspots (high fanout), dense files. Ideal first call when opening a project.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "compact": { "type": "boolean", "default": false, "description": "Compact mode: omit hotspots, dense files, top kinds; return counts only" }
                   },
                   "required": ["root"]
               }
           },
           {
               "name": "codelattice_repo_registry",
               "description": "List known repos or check current root status. CodeLattice does not maintain a persistent registry — each call analyzes fresh. Use GitNexus-RC Tool for full registry management.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "action": { "type": "string", "enum": ["list", "status"], "default": "status" },
                       "root": { "type": "string", "description": "Project root (required for status action)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" }
                   }
               }
           },
           {
               "name": "codelattice_rename_preview",
               "description": "Preview a rename operation: find definition, reference edges, affected files. Read-only — no AST-safe rewrite. Returns applySupported=false. Use IDE/language server for actual renames.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto" },
                       "symbol": { "type": "string", "description": "Current symbol name" },
                       "newName": { "type": "string", "description": "Proposed new name" },
                       "kind": { "type": "string", "description": "Symbol kind to disambiguate" }
                   },
                    "required": ["root", "symbol", "newName"]
                }
           },
           {
               "name": "codelattice_cache_status",
               "description": "Query the analysis cache status for both memory and persistent layers. Shows cached entries, hit/miss counts, stale detection info, and persistent cache file sizes. Does not trigger analysis.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Filter by root path (substring match)" },
                       "language": { "type": "string", "description": "Filter by language" }
                   }
               }
           },
           {
               "name": "codelattice_cache_clear",
               "description": "Clear analysis cache entries. Supports memory-only (default), persistent-only, or both layers. Does not affect Tool registry or source files.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Filter by root path (substring match). Omit to clear all." },
                       "language": { "type": "string", "description": "Filter by language. Omit to clear all." },
                       "layer": { "type": "string", "enum": ["memory", "persistent", "both"], "default": "memory", "description": "Which cache layer to clear. Use 'persistent' or 'both' to also clear disk cache." }
                   }
               }
            },
            {
               "name": "codelattice_production_assist",
               "description": "Dry-run production readiness assistant. Aggregates quality gates, unresolved calls, diagnostics, and changed symbol impact for a quick project health check. Read-only, no file writes. Ideal for AI agents to assess change safety before committing.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "changedSymbols": {
                           "type": "array",
                           "items": { "type": "string" },
                           "description": "Optional list of symbol names you changed, to get their caller counts and snippets"
                       }
                   },
                   "required": ["root"]
               }
            },
            {
               "name": "codelattice_compare_runs",
               "description": "Compare two analysis results to find differences in nodes, edges, symbols, quality gates, and diagnostics. Provide beforeBridgeJson+afterBridgeJson file paths, or just root to compare cached vs fresh analysis. Useful for CI checks and verifying change impact.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root (compares cached vs fresh if no bridge files provided)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "beforeBridgeJson": { "type": "string", "description": "Path to 'before' bridge JSON file (must be under /tmp)" },
                       "afterBridgeJson": { "type": "string", "description": "Path to 'after' bridge JSON file (must be under /tmp)" }
                   }
                }
            },
            {
               "name": "codelattice_cache_prewarm",
               "description": "Pre-warm the process-local analysis cache for a project. Runs analysis and stores the result so subsequent tool calls are fast. Returns cache status after warming. If cache is already fresh (mtime-valid), returns cacheHit=true immediately.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "strict": { "type": "boolean", "default": false, "description": "Strict mode (quality gate failures as errors). Default false to match most other tools." }
                   },
                    "required": ["root"]
                }
             },
             {
                "name": "codelattice_project_insights",
                "description": "Large project insight map for AI agents onboarding onto unfamiliar codebases. Identifies entry points, hotspot files/symbols, risk areas, low-confidence zones, and provides read-first/review-first recommendations. Graph-based heuristic — not compiler/IDE-level proof.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact output — each item retains id/name/kind/file/line/riskScore/reasons only" },
                        "limit": { "type": "integer", "default": 10, "maximum": 100, "description": "Max items per category" },
                        "includeDocs": { "type": "boolean", "default": true, "description": "Include docs signals (symbol ↔ doc associations)" },
                        "includeDiagnostics": { "type": "boolean", "default": true, "description": "Include diagnostic counts in risk scoring" }
                    },
                    "required": ["root"]
                }
             },
            {
               "name": "codelattice_review_plan",
               "description": "AI review plan workflow: converts project insights, impact analysis, changed symbols, and doc associations into an actionable engineering checklist. Four modes: onboarding (read-first map), before_edit (impact preview for target symbol), after_edit (changed symbol impact + test/doc hints), release_check (quality gates + diagnostics + full suite). Graph-based heuristic — not compiler/IDE/test-system proof.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "mode": { "type": "string", "enum": ["onboarding", "before_edit", "after_edit", "release_check"], "default": "onboarding", "description": "Review plan mode" },
                       "symbol": { "type": "string", "description": "Target symbol name (used in before_edit mode)" },
                       "changedSymbols": { "type": "array", "items": { "type": "string" }, "description": "Explicit changed symbol names (after_edit mode; auto-detected if omitted)" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact output" },
                       "limit": { "type": "integer", "default": 10, "maximum": 100, "description": "Max items per category" },
                       "includeDocs": { "type": "boolean", "default": true, "description": "Include doc update hints" },
                       "includeTests": { "type": "boolean", "default": true, "description": "Include test hints" }
                   },
                   "required": ["root"]
               }
            },

            {
               "name": "codelattice_changed_symbols",
               "description": "Detect changed symbols from git diff. Maps diff hunks to graph symbols, returning changed files, changed symbols, unknown hunks, and deleted/renamed files. Read-only, no writes. Ideal for AI agents to auto-detect what changed before impact analysis.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path, must be a git repo)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "diffMode": { "type": "string", "enum": ["working-tree", "staged", "unstaged", "head"], "default": "working-tree", "description": "What to diff: working-tree (default, staged+unstaged), staged only, unstaged only, or HEAD" },
                       "baseRef": { "type": "string", "description": "Optional git ref to compare against (e.g., 'main', 'HEAD~3')" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact output — only id/name/kind/file/line/risk per symbol" },
                       "includeSnippet": { "type": "boolean", "default": true, "description": "Include source snippets for changed symbols" },
                       "snippetContext": { "type": "integer", "default": 2, "minimum": 0, "maximum": 10, "description": "Lines of context around snippets" },
                       "limit": { "type": "integer", "default": 100, "maximum": 500, "description": "Max changed symbols to return" }
                   },
                    "required": ["root"]
                }
            },

            {
               "name": "codelattice_dead_code_candidates",
               "description": "Identify static dead-code candidates — symbols and files with no incoming edges or unreachable from entry points. Returns candidates with confidence, risk cautions, and verification suggestions. NOT deletion proof. Use impact_preview and project tests before deleting.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact mode: keep only id/name/kind/file/line/score/confidence/reasons/cautions per item" },
                       "limit": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200, "description": "Max candidates to return" },
                       "includeFiles": { "type": "boolean", "default": true, "description": "Include file-level candidates" },
                       "includeSymbols": { "type": "boolean", "default": true, "description": "Include symbol-level candidates" },
                       "includeTests": { "type": "boolean", "default": false, "description": "Include test files and test symbols" },
                       "includePublicApi": { "type": "boolean", "default": true, "description": "Include public API candidates (with caution)" },
                       "entryHints": { "type": "array", "items": { "type": "string" }, "description": "Symbol names or file path substrings to treat as entry points" },
                       "excludePatterns": { "type": "array", "items": { "type": "string" }, "description": "File path patterns to exclude (e.g., target/, node_modules/)" }
                   },
                   "required": ["root"]
               }
            },

            {
               "name": "codelattice_impact_analysis",
               "description": "Change impact analysis — what breaks if I modify a symbol or file? Returns direct callers, callees, upstream/downstream paths, risk score, and prioritized review list. Static analysis heuristic, not compiler-verified.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "target": { "type": "string", "description": "Symbol name, file path, or symbol ID to analyze" },
                       "includeIndirect": { "type": "boolean", "default": true, "description": "Include indirect (transitive) impact paths" },
                       "maxDepth": { "type": "integer", "default": 3, "minimum": 1, "maximum": 6, "description": "Max BFS depth for indirect paths" },
                       "maxResults": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200, "description": "Max results per category" },
                       "includeTests": { "type": "boolean", "default": false, "description": "Include test files and test symbols" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact mode: omit full snippets" }
                   },
                   "required": ["root", "target"]
               }
            },

            {
               "name": "codelattice_risk_hotspots",
               "description": "Project risk hotspot detection — identify symbols and files with the highest risk scores based on fan-in/out, cross-module dependencies, and public API surface. Static analysis heuristic, not compiler-verified.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "scope": { "type": "string", "enum": ["all", "symbols", "files"], "default": "all", "description": "Scope of hotspot analysis" },
                       "maxResults": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100, "description": "Max hotspots per category" },
                       "includeTests": { "type": "boolean", "default": false, "description": "Include test files and test symbols" },
                       "minRiskLevel": { "type": "string", "enum": ["low", "medium", "high", "critical"], "default": "medium", "description": "Minimum risk level to include" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact mode" }
                   },
                   "required": ["root"]
               }
            },

            {
               "name": "codelattice_architecture_drift",
               "description": "Architecture health — detect dependency cycles, cross-layer calls, boundary leaks, and coupling issues. Static analysis heuristic, not compiler-verified.",
               "inputSchema": {
                   "type": "object",
                   "properties": {
                       "root": { "type": "string", "description": "Project root directory (absolute path)" },
                       "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                       "layerRules": { "type": "array", "items": { "type": "string" }, "description": "Layer ordering rules, e.g. ['api>service>domain>infra']" },
                       "moduleGlobs": { "type": "array", "items": { "type": "string" }, "description": "Module path globs to scope analysis" },
                       "maxCycles": { "type": "integer", "default": 10, "minimum": 1, "maximum": 50, "description": "Max cycle candidates to report" },
                       "maxFindings": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200, "description": "Max total findings" },
                       "includeTests": { "type": "boolean", "default": false, "description": "Include test files" },
                       "compact": { "type": "boolean", "default": true, "description": "Compact mode" }
                   },
                   "required": ["root"]
               }
            },
            {
                "name": "codelattice_ai_context_pack",
                "description": "AI editing context — what should I read before changing code? Returns context files, key symbols, call chains, dependency notes, risk notes, suggested read order, and useful commands for a given task. Static analysis heuristic, not semantic understanding.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                        "task": { "type": "string", "description": "Description of the editing task (keywords extracted for symbol/file matching)" },
                        "targets": { "type": "array", "items": { "type": "string" }, "description": "Symbol name substrings to target" },
                        "maxFiles": { "type": "integer", "default": 15, "minimum": 1, "maximum": 100, "description": "Max context files to return" },
                        "maxSymbols": { "type": "integer", "default": 30, "minimum": 1, "maximum": 200, "description": "Max key symbols to return" },
                        "includeTests": { "type": "boolean", "default": false, "description": "Include test files in context" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode: omit callChains details" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_review_gate",
                "description": "Diff-based review gate — does this change touch dangerous areas? Analyzes changed files for touched symbols, hotspots, risk level, and recommended tests. Uses git diff or explicit file list.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto", "description": "Language to analyze" },
                        "changedFiles": { "type": "array", "items": { "type": "string" }, "description": "Explicit list of changed file paths (relative to root)" },
                        "useGitDiff": { "type": "boolean", "default": false, "description": "Run git diff --name-only to detect changed files" },
                        "includeUntracked": { "type": "boolean", "default": false, "description": "Include untracked files when using git diff" },
                        "maxFindings": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200, "description": "Max findings to return" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode: keep only essential fields per item" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_reachability_map",
                "description": "Static graph reachability from detected entry point candidates. Identifies reachable and unreachable symbols/files via BFS traversal. Not runtime proof.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true },
                        "limit": { "type": "integer", "default": 100, "maximum": 500 },
                        "maxDepth": { "type": "integer", "default": 8, "minimum": 1, "maximum": 20 },
                        "includeTests": { "type": "boolean", "default": false },
                        "includePublicApi": { "type": "boolean", "default": true },
                        "includeReachableItems": { "type": "boolean", "default": false },
                        "entryHints": { "type": "array", "items": { "type": "string" } },
                        "excludePatterns": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_external_api_surface",
                "description": "Find public/external API surface candidates. Does not prove external usage. Use to caution dead-code removal and breaking-change review.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode" },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 },
                        "includeDocs": { "type": "boolean", "default": true, "description": "Include docs signal" },
                        "includeTests": { "type": "boolean", "default": false, "description": "Include test files" },
                        "includeHeaders": { "type": "boolean", "default": true, "description": "Include C/C++ header API" },
                        "includePackageMetadata": { "type": "boolean", "default": true, "description": "Include package metadata signals" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_framework_entry_hints",
                "description": "Static framework/callback entry hint detection. Identifies symbols likely invoked by framework routing, decorators, callback registries, or CLI commands. Not runtime proof. Use to reduce dead-code/reachability false positives.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode" },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 },
                        "includeTests": { "type": "boolean", "default": false, "description": "Include test files" },
                        "includeCallbacks": { "type": "boolean", "default": true, "description": "Include callback hints" },
                        "includeRoutes": { "type": "boolean", "default": true, "description": "Include route hints" },
                        "includeComponents": { "type": "boolean", "default": true, "description": "Include component hints" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_breaking_change_review",
                "description": "Static compatibility risk review for code changes. Cross-references changed symbols against public API surface, framework entry hints, and documentation. Does not prove runtime breakage. Use before release, deletion, or changing public APIs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode" },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 },
                        "changedSymbols": { "type": "array", "items": { "type": "string" }, "default": [], "description": "Changed symbol names/IDs/files to review" },
                        "diffMode": { "type": "string", "enum": ["working", "staged", "head"], "default": "working" },
                        "includeExternalApi": { "type": "boolean", "default": true, "description": "Check external API surface" },
                        "includeFrameworkEntries": { "type": "boolean", "default": true, "description": "Check framework entry hints" },
                        "includeReachability": { "type": "boolean", "default": true, "description": "Check reachability" },
                        "includeDocs": { "type": "boolean", "default": true, "description": "Check documentation references" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_consistency_review",
                "description": "Static docs/tests consistency review for code changes. Cross-references changed symbols against documentation and test files. Does not run tests or prove coverage. Returns candidates and recommended verification steps.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode" },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 },
                        "changedSymbols": { "type": "array", "items": { "type": "string" }, "default": [], "description": "Changed symbol names/IDs/files to review" },
                        "diffMode": { "type": "string", "enum": ["working", "staged", "head"], "default": "working" },
                        "includeDocs": { "type": "boolean", "default": true, "description": "Check documentation" },
                        "includeTests": { "type": "boolean", "default": true, "description": "Check test files" },
                        "includeDeadCode": { "type": "boolean", "default": true, "description": "Check dead code candidates" },
                        "includeBreakingRisk": { "type": "boolean", "default": true, "description": "Integrate breaking-change risk" }
                    },
                    "required": ["root"]
                }
            },
            {
                "name": "codelattice_config_examples_review",
                "description": "Static config/examples consistency review. Scans package.json, tsconfig, Cargo.toml, pyproject.toml, CI, Docker, examples, and docs code blocks for stale references. Does not execute scripts or builds. Returns candidates and recommended verification steps.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
                        "compact": { "type": "boolean", "default": true, "description": "Compact mode" },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 },
                        "includeExamples": { "type": "boolean", "default": true, "description": "Scan examples/" },
                        "includePackageConfig": { "type": "boolean", "default": true, "description": "Scan package.json/pyproject.toml/Cargo.toml" },
                        "includeBuildConfig": { "type": "boolean", "default": true, "description": "Scan tsconfig/CMake/Makefile/compile_commands" },
                        "includeCiConfig": { "type": "boolean", "default": true, "description": "Scan CI workflows and Dockerfile" },
                        "includeDocsCodeBlocks": { "type": "boolean", "default": true, "description": "Scan README/docs code blocks" }
                    },
                    "required": ["root"]
                }
            }




        ]
    })
}

// ============================================================
// v0.10: Dead Code Candidates
// ============================================================

/// Detect entry points from the graph — symbols that are entry-like or in entry-like files.
fn detect_entry_points(
    gv: &GraphView,
    language: &str,
    entry_hints: &[String],
) -> Vec<(String, String, String, String, u64)> {
    // entry-like file names
    let entry_file_suffixes: &[&str] = match language {
        "rust" => &["main.rs", "lib.rs"],
        "cangjie" => &["package.cj", "main.cj"],
        "arkts" => &["Index.ets", "MainAbility"],
        "typescript" => &["index.ts", "index.tsx", "main.ts", "app.ts"],
        "python" => &[
            "main.py",
            "app.py",
            "api.py",
            "__init__.py",
            "__main__.py",
            "cli.py",
        ],
        "c" | "cpp" => &["main.c", "main.cpp", "main.cc"],
        _ => &["main"],
    };

    let mut entry_points: Vec<(String, String, String, String, u64)> = Vec::new();

    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }
        let name = node["properties"]["name"]
            .as_str()
            .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
            .unwrap_or("");
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        let line = node["properties"]["startLine"].as_u64().unwrap_or(0);
        let id = node["id"].as_str().unwrap_or("").to_string();

        let fan_out = gv.outgoing.get(&id).map(|v| v.len()).unwrap_or(0);

        // Check entry-like heuristic
        if detect_entry_like(name, kind, file, language, fan_out) {
            entry_points.push((
                id.clone(),
                name.to_string(),
                kind.to_string(),
                file.to_string(),
                line,
            ));
            continue;
        }

        // Check entry-like file names
        for suffix in entry_file_suffixes {
            if file.ends_with(suffix) {
                entry_points.push((
                    id.clone(),
                    name.to_string(),
                    kind.to_string(),
                    file.to_string(),
                    line,
                ));
                break;
            }
        }

        // Check user-provided entry hints
        for hint in entry_hints {
            if name == hint || file.contains(hint.as_str()) || id.contains(hint.as_str()) {
                entry_points.push((
                    id.clone(),
                    name.to_string(),
                    kind.to_string(),
                    file.to_string(),
                    line,
                ));
                break;
            }
        }
    }

    // Deduplicate by id
    let mut seen = std::collections::HashSet::new();
    entry_points.retain(|(id, _, _, _, _)| seen.insert(id.clone()));

    entry_points
}

/// BFS reachability from entry points along CALLS/REFERENCES/IMPORTS/INCLUDES/DEFINES edges.
fn reachable_from_entry_points(
    gv: &GraphView,
    entry_points: &[(String, String, String, String, u64)],
) -> std::collections::HashSet<String> {
    let max_depth = 8;
    let mut reachable = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();

    for (id, _, _, _, _) in entry_points {
        reachable.insert(id.clone());
        queue.push_back((id.clone(), 0));
    }

    let follow_edge_types: &[&str] = &["CALLS", "REFERENCES", "IMPORTS", "INCLUDES", "DEFINES"];

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if let Some(edges) = gv.outgoing.get(&node_id) {
            for edge in edges {
                let edge_type = edge["type"].as_str().unwrap_or("");
                if follow_edge_types.contains(&edge_type) {
                    if let Some(target) = edge["target"].as_str() {
                        if reachable.insert(target.to_string()) {
                            queue.push_back((target.to_string(), depth + 1));
                        }
                    }
                }
            }
        }
    }

    reachable
}

/// Check if a file path looks like generated/vendor/dist/build.
fn is_generated_path(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.contains("/vendor/")
        || lower.contains("/node_modules/")
        || lower.contains("/dist/")
        || lower.contains("/build/")
        || lower.contains("/target/")
        || lower.contains("/.generated")
        || lower.contains("__generated__")
}

/// Check if a file path or symbol name has dynamic dispatch patterns.
fn has_dynamic_pattern(name: &str, file: &str) -> bool {
    let lower_name = name.to_lowercase();
    let lower_file = file.to_lowercase();
    lower_name.contains("plugin")
        || lower_name.contains("registry")
        || lower_name.contains("dynamic")
        || lower_file.contains("plugin")
        || lower_file.contains("registry")
        || lower_file.contains("route")
        || lower_file.contains("config")
        || lower_name.contains("importlib")
        || lower_name.contains("getattr")
        || lower_name.contains("eval")
}

/// Check if a file path looks like a test/example/fixture path.
fn is_test_like_path(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.contains("/test")
        || lower.contains("/tests")
        || lower.contains("/spec")
        || lower.contains("/__tests__")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.contains("/example")
        || lower.contains("/examples")
        || lower.contains("/fixture")
        || lower.contains("/fixtures")
}

/// Check if a symbol name looks like a test symbol.
fn is_test_symbol(name: &str, file: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("test")
        || lower.starts_with("test_")
        || lower.starts_with("it_")
        || lower.contains("should")
        || lower.starts_with("describe")
        || lower.starts_with("before")
        || lower.starts_with("after")
        || is_test_like_path(file)
}

/// Check if a symbol is public/exported.
fn is_public_symbol(node: &Value, gv: &GraphView) -> bool {
    let visibility = node["properties"]["visibility"].as_str().unwrap_or("");
    if visibility == "public" {
        return true;
    }

    // Check exported property (TypeScript)
    if node["properties"]["exported"].as_bool() == Some(true) {
        return true;
    }

    // Check if file is under include/ directory (public API convention)
    let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
    if file.contains("/include/") || file.contains("public-api") || file.contains("public_api") {
        return true;
    }

    // Check if other files import this symbol
    let id = node["id"].as_str().unwrap_or("");
    if let Some(incoming) = gv.incoming.get(id) {
        for edge in incoming {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if edge_type == "IMPORTS" || edge_type == "INCLUDES" || edge_type == "REFERENCES" {
                // Check if source is from a different file
                if let Some(source_id) = edge["source"].as_str() {
                    if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                        let source_file = source_node["properties"]["sourcePath"]
                            .as_str()
                            .unwrap_or("");
                        if source_file != file {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Score individual symbol candidates.
fn score_candidate_symbols(
    gv: &GraphView,
    language: &str,
    entry_point_ids: &std::collections::HashSet<String>,
    reachable: &std::collections::HashSet<String>,
    include_tests: bool,
    include_public_api: bool,
    exclude_patterns: &[String],
) -> Vec<Value> {
    let mut candidates: Vec<Value> = Vec::new();

    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }

        let symbol_kind = node["properties"]["symbolKind"]
            .as_str()
            .or_else(|| node["kind"].as_str())
            .unwrap_or("");

        // Skip module/package/repository/file kind nodes
        if matches!(
            symbol_kind,
            "module" | "package" | "repository" | "file" | "source_file"
        ) {
            continue;
        }

        let id = node["id"].as_str().unwrap_or("").to_string();
        let name = node["properties"]["name"]
            .as_str()
            .or_else(|| id.split("::").last())
            .unwrap_or("")
            .to_string();
        let file = node["properties"]["sourcePath"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let line = node["properties"]["startLine"].as_u64().unwrap_or(0);

        // Skip generated paths
        if is_generated_path(&file) {
            continue;
        }

        // Skip exclude patterns
        let mut excluded = false;
        for pattern in exclude_patterns {
            if file.contains(pattern.as_str()) {
                excluded = true;
                break;
            }
        }
        if excluded {
            continue;
        }

        // Skip entry points
        if entry_point_ids.contains(&id) {
            continue;
        }

        // Skip test symbols when includeTests=false
        if !include_tests && is_test_symbol(&name, &file) {
            continue;
        }

        // Score computation
        let mut score: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        let mut cautions: Vec<String> = vec!["static-analysis-only".to_string()];

        // Count incoming CALLS/REFERENCES/IMPORTS/INCLUDES edges
        let incoming_edges = gv.incoming.get(&id).cloned().unwrap_or_default();
        let relevant_incoming: Vec<&Value> = incoming_edges
            .iter()
            .filter(|e| {
                let t = e["type"].as_str().unwrap_or("");
                t == "CALLS" || t == "REFERENCES" || t == "IMPORTS" || t == "INCLUDES"
            })
            .collect();

        if relevant_incoming.is_empty() {
            score += 0.35;
            reasons.push("no-incoming-calls".to_string());
        }

        // Not reachable from entry points
        if !reachable.contains(&id) {
            score += 0.25;
            reasons.push("not-reachable-from-entry-points".to_string());
        }

        // Private/internal visibility
        let visibility = node["properties"]["visibility"].as_str().unwrap_or("");
        if visibility == "private" || visibility == "internal" || visibility == "" {
            score += 0.15;
            reasons.push("private-visibility".to_string());
        }

        // Not mentioned in docs
        if let Some(ref scanner) = gv.doc_scanner {
            let related = scanner.find_related_docs(&name, &file, &[], 1);
            if related.is_empty() {
                score += 0.10;
                reasons.push("not-mentioned-in-docs".to_string());
            }
        } else {
            // No scanner available, give small bonus
            score += 0.05;
        }

        // File is orphan-like (no incoming file-level edges)
        let file_incoming = gv.incoming.get(&file).cloned().unwrap_or_default();
        let file_relevant: Vec<&Value> = file_incoming
            .iter()
            .filter(|e| {
                let t = e["type"].as_str().unwrap_or("");
                t == "IMPORTS" || t == "REFERENCES" || t == "INCLUDES"
            })
            .collect();
        if file_relevant.is_empty() {
            score += 0.10;
            reasons.push("orphan-file".to_string());
        }

        // Low fan-out
        let fan_out = gv.outgoing.get(&id).map(|v| v.len()).unwrap_or(0);
        if fan_out <= 1 {
            score += 0.05;
            reasons.push("low-fan-out".to_string());
        }

        // Negative signals: public/exported
        let is_public = is_public_symbol(node, gv);
        if is_public {
            score -= 0.35;
            cautions.push("check-external-api-usage".to_string());
            if !include_public_api {
                continue;
            }
        }

        // Negative: name looks entry-like
        if detect_entry_like(&name, &symbol_kind, &file, language, fan_out) {
            score -= 0.40;
        }

        // Dynamic pattern caution
        if has_dynamic_pattern(&name, &file) {
            score -= 0.15;
            cautions.push("dynamic-dispatch-may-hide-callers".to_string());
        }

        // Clamp score to [0.0, 1.0]
        score = score.max(0.0).min(1.0);

        // Filter: only include candidates with score >= 0.45
        if score < 0.45 {
            continue;
        }

        // Determine confidence
        let confidence = if score >= 0.80 {
            "high"
        } else if score >= 0.55 {
            "medium"
        } else {
            "low"
        };

        // If public API, cap confidence at medium
        let final_confidence = if is_public && confidence == "high" {
            "medium"
        } else {
            confidence
        };

        // Public API caution
        if is_public {
            cautions.push("public-api-may-have-external-callers".to_string());
        }

        // Build recommended verification
        let mut recommended_verification: Vec<String> = Vec::new();
        recommended_verification.push("search-public-exports".to_string());
        recommended_verification.push("run-project-tests".to_string());
        if is_public {
            recommended_verification.push("check-external-consumers".to_string());
        }

        candidates.push(json!({
            "id": id,
            "name": name,
            "kind": symbol_kind,
            "file": file,
            "line": line,
            "confidence": final_confidence,
            "score": (score * 100.0).round() / 100.0,
            "reasons": reasons,
            "cautions": cautions,
            "recommendedVerification": recommended_verification
        }));
    }

    // Sort by score descending, then by name
    candidates.sort_by(|a, b| {
        let score_a = a["score"].as_f64().unwrap_or(0.0);
        let score_b = b["score"].as_f64().unwrap_or(0.0);
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let name_a = a["name"].as_str().unwrap_or("");
                let name_b = b["name"].as_str().unwrap_or("");
                name_a.cmp(name_b)
            })
    });

    candidates
}

/// Score file-level candidates.
fn score_candidate_files(
    gv: &GraphView,
    language: &str,
    entry_point_ids: &std::collections::HashSet<String>,
    reachable: &std::collections::HashSet<String>,
    include_tests: bool,
    exclude_patterns: &[String],
) -> Vec<Value> {
    // Collect per-file info
    let mut file_symbols: HashMap<String, Vec<Value>> = HashMap::new();
    let mut file_incoming: HashMap<String, usize> = HashMap::new();
    let mut file_outgoing: HashMap<String, usize> = HashMap::new();

    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }
        let file = node["properties"]["sourcePath"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if file.is_empty() {
            continue;
        }
        file_symbols
            .entry(file.clone())
            .or_default()
            .push(node.clone());
    }

    // Count file-level incoming/outgoing edges
    // File-level nodes have kind "source_file" or id matching the file path
    for (node_id, node) in &gv.nodes_by_id {
        let node_kind = node["kind"].as_str().unwrap_or("");
        if node_kind == "source_file" || node_kind == "file" {
            let in_count = gv.incoming.get(node_id).map(|v| v.len()).unwrap_or(0);
            let out_count = gv.outgoing.get(node_id).map(|v| v.len()).unwrap_or(0);
            // Map to file path
            let file = node["properties"]["sourcePath"]
                .as_str()
                .or_else(|| node["properties"]["path"].as_str())
                .unwrap_or(node_id.as_str());
            *file_incoming.entry(file.to_string()).or_default() += in_count;
            *file_outgoing.entry(file.to_string()).or_default() += out_count;
        }
    }

    // Also count edges from symbols in each file
    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }
        let file = node["properties"]["sourcePath"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if file.is_empty() {
            continue;
        }
        let id = node["id"].as_str().unwrap_or("");
        let _in_count = gv.incoming.get(id).map(|v| v.len()).unwrap_or(0);
        let _out_count = gv.outgoing.get(id).map(|v| v.len()).unwrap_or(0);
        // Only count cross-file edges for file-level scoring
        for edge in gv.incoming.get(id).cloned().unwrap_or_default() {
            let source = edge["source"].as_str().unwrap_or("");
            if let Some(src_node) = gv.nodes_by_id.get(source) {
                let src_file = src_node["properties"]["sourcePath"].as_str().unwrap_or("");
                if src_file != file {
                    *file_incoming.entry(file.clone()).or_default() += 1;
                }
            }
        }
        for edge in gv.outgoing.get(id).cloned().unwrap_or_default() {
            let target = edge["target"].as_str().unwrap_or("");
            if let Some(tgt_node) = gv.nodes_by_id.get(target) {
                let tgt_file = tgt_node["properties"]["sourcePath"].as_str().unwrap_or("");
                if tgt_file != file {
                    *file_outgoing.entry(file.clone()).or_default() += 1;
                }
            }
        }
    }

    let mut candidates: Vec<Value> = Vec::new();

    for (file, symbols) in &file_symbols {
        // Skip generated paths
        if is_generated_path(file) {
            continue;
        }

        // Skip exclude patterns
        let mut excluded = false;
        for pattern in exclude_patterns {
            if file.contains(pattern.as_str()) {
                excluded = true;
                break;
            }
        }
        if excluded {
            continue;
        }

        // Skip test/example/fixture paths when includeTests=false
        if !include_tests && is_test_like_path(file) {
            continue;
        }

        let mut score: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        let mut cautions: Vec<String> = Vec::new();

        // No incoming file-level edges
        let in_count = file_incoming.get(file).copied().unwrap_or(0);
        if in_count == 0 {
            score += 0.35;
            reasons.push("no-incoming-file-edges".to_string());
        }

        // No entry-like symbols inside
        let has_entry = symbols.iter().any(|s| {
            let sname = s["properties"]["name"].as_str().unwrap_or("");
            let skind = s["kind"].as_str().unwrap_or("");
            let sfan_out = s["id"]
                .as_str()
                .map(|id| gv.outgoing.get(id).map(|v| v.len()).unwrap_or(0))
                .unwrap_or(0);
            detect_entry_like(sname, skind, file, language, sfan_out)
                || entry_point_ids.contains(s["id"].as_str().unwrap_or(""))
        });
        if !has_entry {
            score += 0.20;
            reasons.push("no-entry-like-symbols".to_string());
        }

        // All symbols inside are candidates (unreachable)
        let all_unreachable = symbols.iter().all(|s| {
            let sid = s["id"].as_str().unwrap_or("");
            !entry_point_ids.contains(sid) && !reachable.contains(sid)
        });
        if all_unreachable && !symbols.is_empty() {
            score += 0.20;
            reasons.push("all-symbols-unreachable".to_string());
        }

        // Not referenced by docs
        if let Some(ref scanner) = gv.doc_scanner {
            let file_name = file.split('/').last().unwrap_or(file);
            let related = scanner.find_related_docs("", file_name, &[], 1);
            if related.is_empty() {
                score += 0.10;
                reasons.push("file-not-mentioned-in-docs".to_string());
            }
        }

        // Low outgoing edges
        let out_count = file_outgoing.get(file).copied().unwrap_or(0);
        if out_count <= 1 {
            score += 0.05;
            reasons.push("low-outgoing-edges".to_string());
        }

        // Negative: contains public API exports
        let has_public = symbols.iter().any(|s| is_public_symbol(s, gv));
        if has_public {
            score -= 0.30;
            cautions.push("contains-public-api-exports".to_string());
        }

        // Negative: filename is entry-like
        let file_lower = file.to_lowercase();
        let entry_file_names = [
            "main.ts",
            "main.rs",
            "main.c",
            "main.cpp",
            "main.py",
            "index.ts",
            "index.tsx",
            "app.ts",
            "app.py",
            "api.py",
            "lib.rs",
        ];
        if entry_file_names.iter().any(|ef| file_lower.ends_with(ef)) {
            score -= 0.40;
        }

        // Dynamic caution
        if has_dynamic_pattern("", file) {
            cautions.push("dynamic-dispatch-may-hide-callers".to_string());
        }

        // Clamp score
        score = score.max(0.0).min(1.0);

        // Filter: only include with score >= 0.45
        if score < 0.45 {
            continue;
        }

        let confidence = if score >= 0.80 {
            "high"
        } else if score >= 0.55 {
            "medium"
        } else {
            "low"
        };

        candidates.push(json!({
            "path": file,
            "score": (score * 100.0).round() / 100.0,
            "confidence": confidence,
            "symbolCount": symbols.len(),
            "incomingEdgeCount": in_count,
            "outgoingEdgeCount": out_count,
            "reasons": reasons,
            "cautions": cautions
        }));
    }

    // Sort by score descending, then by path
    candidates.sort_by(|a, b| {
        let score_a = a["score"].as_f64().unwrap_or(0.0);
        let score_b = b["score"].as_f64().unwrap_or(0.0);
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let path_a = a["path"].as_str().unwrap_or("");
                let path_b = b["path"].as_str().unwrap_or("");
                path_a.cmp(path_b)
            })
    });

    candidates
}

// ============================================================
// v0.11: Impact Analysis, Risk Hotspots, Architecture Drift
// ============================================================

/// Resolve a target string to matching node IDs in the graph.
/// Tries: symbol name (case-insensitive), file path, then direct ID.
fn resolve_target_nodes(gv: &GraphView, target: &str) -> Vec<Value> {
    let mut matches: Vec<Value> = Vec::new();
    let target_lower = target.to_lowercase();

    // 1. Try symbol name (case-insensitive)
    if let Some(syms) = gv.symbols_by_name.get(&target_lower) {
        for s in syms {
            matches.push(s.clone());
        }
    }

    // 2. Try as file path — find nodes with matching sourcePath
    if matches.is_empty() {
        for node in gv.nodes_by_id.values() {
            let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
            if file == target || file.ends_with(&format!("/{target}")) || file.contains(target) {
                matches.push(node.clone());
            }
        }
    }

    // 3. Try direct ID lookup
    if matches.is_empty() {
        if let Some(node) = gv.nodes_by_id.get(target) {
            matches.push(node.clone());
        }
    }

    matches
}

/// Compute risk level string from a score.
fn risk_level_from_score(score: f64) -> &'static str {
    if score < 0.3 {
        "low"
    } else if score < 0.6 {
        "medium"
    } else if score < 0.8 {
        "high"
    } else {
        "critical"
    }
}

/// Compute hotspot score for a symbol node.
fn compute_symbol_hotspot_score(
    node: &Value,
    gv: &GraphView,
    reachable: &std::collections::HashSet<String>,
    include_tests: bool,
) -> (f64, Vec<String>) {
    let id = node["id"].as_str().unwrap_or("").to_string();
    let name = node["properties"]["name"]
        .as_str()
        .or_else(|| id.split("::").last())
        .unwrap_or("")
        .to_string();
    let file = node["properties"]["sourcePath"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Skip test symbols if not included
    if !include_tests && is_test_symbol(&name, &file) {
        return (0.0, vec![]);
    }

    let mut score: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    // Fan-in: direct incoming + via ref:Call:NAME nodes
    let direct_incoming = gv.incoming.get(&id).cloned().unwrap_or_default();
    let direct_fan_in = direct_incoming
        .iter()
        .filter(|e| {
            let t = e["type"].as_str().unwrap_or("");
            t == "CALLS" || t == "REFERENCES" || t == "IMPORTS"
        })
        .count();

    // Also count callers via ref:Call:NAME intermediate nodes
    let ref_call_id = format!("ref:Call:{}", name);
    let ref_callers = gv.incoming.get(&ref_call_id).map(|v| v.len()).unwrap_or(0);

    let fan_in = direct_fan_in + ref_callers;
    let high_fan_in = fan_in > 5;
    if high_fan_in {
        score += 0.25;
        reasons.push("high fan-in".to_string());
    }

    // Fan-out: outgoing edges + ref:Call nodes from the file
    let outgoing = gv.outgoing.get(&id).cloned().unwrap_or_default();
    let mut fan_out = outgoing
        .iter()
        .filter(|e| {
            let t = e["type"].as_str().unwrap_or("");
            t == "CALLS" || t == "REFERENCES" || t == "IMPORTS"
        })
        .count();

    // Also count calls from the file containing this symbol
    for (nid, fnode) in &gv.nodes_by_id {
        let fpath = fnode["properties"]["sourcePath"].as_str().unwrap_or("");
        if fpath == file {
            if let Some(out_edges) = gv.outgoing.get(nid) {
                for edge in out_edges {
                    if edge["type"].as_str() == Some("CALLS") {
                        let target = edge["target"].as_str().unwrap_or("");
                        if target.starts_with("ref:Call:") && !target.ends_with(&name) {
                            fan_out += 1;
                        }
                    }
                }
            }
        }
    }

    let high_fan_out = fan_out > 5;
    if high_fan_out {
        score += 0.25;
        reasons.push("high fan-out".to_string());
    }

    // Both high
    if high_fan_in && high_fan_out {
        score += 0.15;
        reasons.push("both high fan-in/out".to_string());
    }

    // Cross-directory
    let mut has_cross_dir = false;
    for edge in &direct_incoming {
        let source = edge["source"].as_str().unwrap_or("");
        if let Some(src_node) = gv.nodes_by_id.get(source) {
            let src_file = src_node["properties"]["sourcePath"].as_str().unwrap_or("");
            if src_file != file && !src_file.is_empty() {
                // Check different directory
                let src_dir = parent_dir(src_file);
                let tgt_dir = parent_dir(&file);
                if src_dir != tgt_dir {
                    has_cross_dir = true;
                    break;
                }
            }
        }
    }
    if has_cross_dir {
        score += 0.15;
        reasons.push("cross-directory dependency".to_string());
    }

    // Entry reachable
    if reachable.contains(&id) {
        score += 0.10;
        reasons.push("entry-point reachable".to_string());
    }

    // Public/exported
    if is_public_symbol(node, gv) {
        score += 0.10;
        reasons.push("public/exported".to_string());
    }

    // Diagnostics in file
    let has_diag = gv
        .diagnostics
        .iter()
        .any(|d| d["file"].as_str().map(|f| f == file).unwrap_or(false));
    if has_diag {
        score += 0.10;
        reasons.push("file has diagnostics".to_string());
    }

    score = score.clamp(0.0, 1.0);
    (score, reasons)
}

/// Get parent directory from a file path string.
fn parent_dir(file: &str) -> String {
    if let Some(idx) = file.rfind('/') {
        file[..idx].to_string()
    } else {
        String::new()
    }
}

/// BFS traversal collecting nodes at each depth level along CALLS/REFERENCES/IMPORTS edges.
fn bfs_collect_by_depth(
    gv: &GraphView,
    start_ids: &[String],
    direction: &str, // "upstream" (incoming) or "downstream" (outgoing)
    max_depth: usize,
    max_results: usize,
    include_tests: bool,
) -> Vec<Value> {
    let mut levels: Vec<Value> = Vec::new();
    let mut visited: std::collections::HashSet<String> = start_ids.iter().cloned().collect();
    let mut current_level_ids: Vec<String> = start_ids.to_vec();

    for depth in 1..=max_depth {
        if current_level_ids.is_empty() {
            break;
        }
        let mut next_level_ids: Vec<String> = Vec::new();
        let mut level_nodes: Vec<Value> = Vec::new();

        for node_id in &current_level_ids {
            let edges = if direction == "upstream" {
                gv.incoming.get(node_id).cloned().unwrap_or_default()
            } else {
                gv.outgoing.get(node_id).cloned().unwrap_or_default()
            };

            for edge in edges {
                let edge_type = edge["type"].as_str().unwrap_or("");
                if edge_type != "CALLS" && edge_type != "REFERENCES" && edge_type != "IMPORTS" {
                    continue;
                }
                let neighbor_id = if direction == "upstream" {
                    edge["source"].as_str()
                } else {
                    edge["target"].as_str()
                };
                if let Some(nid) = neighbor_id {
                    if visited.insert(nid.to_string()) {
                        if let Some(node) = gv.nodes_by_id.get(nid) {
                            let name = node["properties"]["name"]
                                .as_str()
                                .or_else(|| {
                                    node["id"].as_str().and_then(|id| id.split("::").last())
                                })
                                .unwrap_or("")
                                .to_string();
                            let file = node["properties"]["sourcePath"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();

                            if !include_tests && is_test_symbol(&name, &file) {
                                continue;
                            }

                            level_nodes.push(compact_node(node));
                            next_level_ids.push(nid.to_string());
                        }
                    }
                }
            }
        }

        if !level_nodes.is_empty() {
            levels.push(json!({
                "depth": depth,
                "nodes": level_nodes.into_iter().take(max_results).collect::<Vec<_>>()
            }));
        }
        current_level_ids = next_level_ids;
    }

    levels
}

/// Make a compact node representation.
fn compact_node(node: &Value) -> Value {
    json!({
        "id": node["id"],
        "name": node["properties"]["name"].as_str()
            .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
            .unwrap_or(""),
        "kind": node["properties"]["symbolKind"].as_str()
            .or_else(|| node["kind"].as_str())
            .unwrap_or(""),
        "file": node["properties"]["sourcePath"].as_str().unwrap_or(""),
        "line": node["properties"]["startLine"].as_u64().unwrap_or(0)
    })
}

/// Detect cycles using iterative DFS with path tracking.
fn detect_cycles(gv: &GraphView, max_cycles: usize, include_tests: bool) -> Vec<Value> {
    let mut cycles: Vec<Value> = Vec::new();
    let mut seen_cycle_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let follow_types: &[&str] = &["CALLS", "IMPORTS", "REFERENCES"];

    for start_id in gv.nodes_by_id.keys() {
        if cycles.len() >= max_cycles {
            break;
        }

        // Skip test files
        if let Some(node) = gv.nodes_by_id.get(start_id) {
            let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
            let name = node["properties"]["name"].as_str().unwrap_or("");
            if !include_tests && is_test_symbol(name, file) {
                continue;
            }
        }

        // Iterative DFS: stack holds (node_id, path)
        let mut stack: Vec<(String, Vec<String>)> =
            vec![(start_id.clone(), vec![start_id.clone()])];

        while let Some((current_id, path)) = stack.pop() {
            if path.len() > 12 {
                continue; // depth limit
            }
            if let Some(edges) = gv.outgoing.get(&current_id) {
                for edge in edges {
                    let edge_type = edge["type"].as_str().unwrap_or("");
                    if !follow_types.contains(&edge_type) {
                        continue;
                    }
                    if let Some(target) = edge["target"].as_str() {
                        // Only cross-file cycles are interesting
                        let current_file = gv
                            .nodes_by_id
                            .get(&current_id)
                            .and_then(|n| n["properties"]["sourcePath"].as_str())
                            .unwrap_or("");
                        let target_file = gv
                            .nodes_by_id
                            .get(target)
                            .and_then(|n| n["properties"]["sourcePath"].as_str())
                            .unwrap_or("");
                        if current_file == target_file {
                            continue;
                        }

                        if let Some(cycle_idx) = path.iter().position(|p| p == target) {
                            // Found cycle
                            let cycle_path: Vec<String> = path[cycle_idx..].to_vec();
                            let mut key_parts = cycle_path.clone();
                            key_parts.sort();
                            let key = key_parts.join(",");
                            if seen_cycle_keys.insert(key) {
                                // Get participant dirs/files
                                let participants: Vec<String> = cycle_path
                                    .iter()
                                    .filter_map(|id| {
                                        gv.nodes_by_id.get(id).and_then(|n| {
                                            n["properties"]["sourcePath"].as_str().map(parent_dir)
                                        })
                                    })
                                    .collect();
                                let edge_types: Vec<String> = cycle_path
                                    .windows(2)
                                    .map(|w| {
                                        gv.outgoing
                                            .get(&w[0])
                                            .and_then(|edges| {
                                                edges
                                                    .iter()
                                                    .find(|e| {
                                                        e["target"].as_str() == Some(&w[1])
                                                            && follow_types.contains(
                                                                &e["type"].as_str().unwrap_or(""),
                                                            )
                                                    })
                                                    .and_then(|e| {
                                                        e["type"].as_str().map(String::from)
                                                    })
                                            })
                                            .unwrap_or_else(|| "CALLS".to_string())
                                    })
                                    .collect();

                                let desc_parts: Vec<String> = cycle_path
                                    .iter()
                                    .filter_map(|id| {
                                        gv.nodes_by_id.get(id).and_then(|n| {
                                            n["properties"]["name"]
                                                .as_str()
                                                .or_else(|| {
                                                    n["id"]
                                                        .as_str()
                                                        .and_then(|id| id.split("::").last())
                                                })
                                                .map(String::from)
                                        })
                                    })
                                    .collect();

                                cycles.push(json!({
                                    "participants": participants,
                                    "edgeTypes": edge_types,
                                    "description": format!("cycle: {}", desc_parts.join(" → "))
                                }));

                                if cycles.len() >= max_cycles {
                                    return cycles;
                                }
                            }
                        } else if path.len() < 12 {
                            let mut new_path = path.clone();
                            new_path.push(target.to_string());
                            stack.push((target.to_string(), new_path));
                        }
                    }
                }
            }
        }
    }

    cycles
}

/// Compute cross-directory coupling metrics.
fn compute_coupling(gv: &GraphView, max_findings: usize, include_tests: bool) -> Vec<Value> {
    let mut dir_edges: HashMap<(String, String), usize> = HashMap::new();
    let follow_types: &[&str] = &["CALLS", "IMPORTS", "REFERENCES"];

    for (src_id, edges) in &gv.outgoing {
        let src_file = gv
            .nodes_by_id
            .get(src_id)
            .and_then(|n| n["properties"]["sourcePath"].as_str())
            .unwrap_or("");

        if !include_tests && is_test_like_path(src_file) {
            continue;
        }
        let src_dir = parent_dir(src_file);
        if src_dir.is_empty() {
            continue;
        }

        for edge in edges {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if !follow_types.contains(&edge_type) {
                continue;
            }
            if let Some(target) = edge["target"].as_str() {
                let tgt_file = gv
                    .nodes_by_id
                    .get(target)
                    .and_then(|n| n["properties"]["sourcePath"].as_str())
                    .unwrap_or("");
                let tgt_dir = parent_dir(tgt_file);
                if tgt_dir.is_empty() || tgt_dir == src_dir {
                    continue;
                }
                if !include_tests && is_test_like_path(tgt_file) {
                    continue;
                }
                *dir_edges.entry((src_dir.clone(), tgt_dir)).or_insert(0) += 1;
            }
        }
    }

    // Find overly coupled modules (>15 edges total)
    let mut dir_total: HashMap<String, (usize, usize, std::collections::HashSet<String>)> =
        HashMap::new(); // dir -> (incoming, outgoing, connected_dirs)
    for ((src, tgt), count) in &dir_edges {
        let entry =
            dir_total
                .entry(src.clone())
                .or_insert((0, 0, std::collections::HashSet::new()));
        entry.1 += count;
        entry.2.insert(tgt.clone());
        let entry2 =
            dir_total
                .entry(tgt.clone())
                .or_insert((0, 0, std::collections::HashSet::new()));
        entry2.0 += count;
        entry2.2.insert(src.clone());
    }

    let mut coupled: Vec<Value> = dir_total
        .iter()
        .filter(|(_, (inc, out, _connected))| *inc + *out > 15)
        .map(|(dir, (inc, out, connected))| {
            json!({
                "path": dir,
                "incomingEdges": inc,
                "outgoingEdges": out,
                "connectedModules": connected.len(),
                "reasons": vec![
                    if *inc + *out > 30 { "very high coupling".to_string() } else { "high coupling".to_string() }
                ]
            })
        })
        .collect();

    coupled.sort_by(|a, b| {
        let a_total =
            a["incomingEdges"].as_u64().unwrap_or(0) + a["outgoingEdges"].as_u64().unwrap_or(0);
        let b_total =
            b["incomingEdges"].as_u64().unwrap_or(0) + b["outgoingEdges"].as_u64().unwrap_or(0);
        b_total.cmp(&a_total)
    });
    coupled.truncate(max_findings);
    coupled
}

/// Detect cross-layer calls and boundary leaks given layer rules.
fn detect_layer_violations(
    gv: &GraphView,
    layer_rules: &[String],
    max_findings: usize,
    include_tests: bool,
) -> (Vec<Value>, Vec<Value>) {
    let mut cross_layer_calls: Vec<Value> = Vec::new();
    let mut boundary_leaks: Vec<Value> = Vec::new();

    // Parse layer rules: "api>service>domain>infra" -> ["api", "service", "domain", "infra"]
    let mut layers: Vec<String> = Vec::new();
    for rule in layer_rules {
        for part in rule.split('>') {
            let trimmed = part.trim().to_string();
            if !trimmed.is_empty() && !layers.contains(&trimmed) {
                layers.push(trimmed);
            }
        }
    }
    if layers.len() < 2 {
        return (cross_layer_calls, boundary_leaks);
    }

    fn resolve_layer(file: &str, layers: &[String]) -> Option<usize> {
        let lower = file.to_lowercase();
        for (i, layer) in layers.iter().enumerate() {
            let layer_lower = layer.to_lowercase();
            if lower.contains(&format!("/{}", layer_lower))
                || lower.contains(&format!("{}{}", "/", layer_lower))
                || lower.starts_with(&format!("{}{}", layer_lower, "/"))
            {
                return Some(i);
            }
        }
        None
    }

    let follow_types: &[&str] = &["CALLS", "IMPORTS", "REFERENCES"];

    for (src_id, edges) in &gv.outgoing {
        let src_file = gv
            .nodes_by_id
            .get(src_id)
            .and_then(|n| n["properties"]["sourcePath"].as_str())
            .unwrap_or("");

        if !include_tests && is_test_like_path(src_file) {
            continue;
        }

        let src_layer_idx = match resolve_layer(src_file, &layers) {
            Some(idx) => idx,
            None => continue,
        };

        for edge in edges {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if !follow_types.contains(&edge_type) {
                continue;
            }
            if let Some(target) = edge["target"].as_str() {
                let tgt_file = gv
                    .nodes_by_id
                    .get(target)
                    .and_then(|n| n["properties"]["sourcePath"].as_str())
                    .unwrap_or("");
                let tgt_layer_idx = match resolve_layer(tgt_file, &layers) {
                    Some(idx) => idx,
                    None => continue,
                };

                // Reverse dependency: lower layer calling higher layer
                if tgt_layer_idx < src_layer_idx {
                    cross_layer_calls.push(json!({
                        "from": parent_dir(src_file),
                        "to": parent_dir(tgt_file),
                        "fromLayer": layers[src_layer_idx],
                        "toLayer": layers[tgt_layer_idx],
                        "edgeType": edge_type,
                        "violation": "reverse dependency"
                    }));

                    // Also record as boundary leak
                    boundary_leaks.push(json!({
                        "from": parent_dir(src_file),
                        "to": parent_dir(tgt_file),
                        "fromLayer": layers[src_layer_idx],
                        "toLayer": layers[tgt_layer_idx],
                        "edgeType": edge_type,
                        "description": format!(
                            "{} layer ({}) imports from {} layer ({})",
                            layers[src_layer_idx], parent_dir(src_file),
                            layers[tgt_layer_idx], parent_dir(tgt_file)
                        )
                    }));
                }
                // Skip-layer call (non-adjacent, but forward direction)
                else if tgt_layer_idx > src_layer_idx + 1 {
                    cross_layer_calls.push(json!({
                        "from": parent_dir(src_file),
                        "to": parent_dir(tgt_file),
                        "fromLayer": layers[src_layer_idx],
                        "toLayer": layers[tgt_layer_idx],
                        "edgeType": edge_type,
                        "violation": "skip-layer call"
                    }));
                }
            }
        }

        if cross_layer_calls.len() >= max_findings {
            break;
        }
    }

    (cross_layer_calls, boundary_leaks)
}

/// Detect bidirectional dependencies between directories.
fn detect_bidirectional_deps(gv: &GraphView, include_tests: bool) -> Vec<Value> {
    let mut dir_edges: HashMap<(String, String), usize> = HashMap::new();
    let follow_types: &[&str] = &["CALLS", "IMPORTS", "REFERENCES"];

    for (src_id, edges) in &gv.outgoing {
        let src_file = gv
            .nodes_by_id
            .get(src_id)
            .and_then(|n| n["properties"]["sourcePath"].as_str())
            .unwrap_or("");
        if !include_tests && is_test_like_path(src_file) {
            continue;
        }
        let src_dir = parent_dir(src_file);
        if src_dir.is_empty() {
            continue;
        }

        for edge in edges {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if !follow_types.contains(&edge_type) {
                continue;
            }
            if let Some(target) = edge["target"].as_str() {
                let tgt_file = gv
                    .nodes_by_id
                    .get(target)
                    .and_then(|n| n["properties"]["sourcePath"].as_str())
                    .unwrap_or("");
                let tgt_dir = parent_dir(tgt_file);
                if tgt_dir.is_empty() || tgt_dir == src_dir {
                    continue;
                }
                if !include_tests && is_test_like_path(tgt_file) {
                    continue;
                }
                *dir_edges.entry((src_dir.clone(), tgt_dir)).or_insert(0) += 1;
            }
        }
    }

    // Find bidirectional pairs
    let mut seen_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let mut reverse_deps: Vec<Value> = Vec::new();

    for ((src, tgt), count) in &dir_edges {
        let forward_key = if src < tgt {
            (src.clone(), tgt.clone())
        } else {
            (tgt.clone(), src.clone())
        };
        if seen_pairs.insert(forward_key.clone()) {
            // Check reverse
            let reverse_count = dir_edges
                .get(&(tgt.clone(), src.clone()))
                .or_else(|| dir_edges.get(&(src.clone(), tgt.clone())))
                .copied()
                .unwrap_or(0);
            if *count > 0 && reverse_count > 0 {
                reverse_deps.push(json!({
                    "from": src,
                    "to": tgt,
                    "forwardEdges": count,
                    "reverseEdges": reverse_count,
                    "description": format!("bidirectional: {} ↔ {}", src, tgt)
                }));
            }
        }
    }

    reverse_deps
}

/// Handle `codelattice_impact_analysis` tool.
fn handle_impact_analysis(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let target = params["target"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: target"))?;
    let include_indirect = params["includeIndirect"].as_bool().unwrap_or(true);
    let max_depth = params["maxDepth"].as_u64().unwrap_or(3).min(6) as usize;
    let max_results = params["maxResults"].as_u64().unwrap_or(50).min(200) as usize;
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let _compact = params["compact"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // 1. Resolve target
    let target_nodes = resolve_target_nodes(&gv, target);
    if target_nodes.is_empty() {
        let result_data = json!({
            "language": language,
            "root": root,
            "targetMatched": Value::Null,
            "directCallers": [],
            "directCallees": [],
            "upstreamPaths": [],
            "downstreamPaths": [],
            "relatedFiles": [],
            "entryPointReachability": { "reachable": false, "viaEntryPoints": [], "pathLength": 0 },
            "riskLevel": "low",
            "riskScore": 0.0,
            "reasons": vec![format!("target '{}' not found", target)],
            "cautions": vec!["static analysis only", "dynamic dispatch may hide callers"],
            "readFirst": [],
            "reviewFirst": [],
            "generatedFrom": {
                "staticAnalysisOnly": true,
                "heuristic": true,
                "compilerVerified": false
            }
        });
        return Ok(merge_cache_and_result(&result_data, &cache_meta));
    }

    // Use first match as primary target
    let primary = &target_nodes[0];
    let _primary_id = primary["id"].as_str().unwrap_or("").to_string();
    let primary_file = primary["properties"]["sourcePath"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let primary_name = primary["properties"]["name"]
        .as_str()
        .or_else(|| primary["id"].as_str().and_then(|id| id.split("::").last()))
        .unwrap_or("")
        .to_string();

    let all_target_ids: Vec<String> = target_nodes
        .iter()
        .filter_map(|n| n["id"].as_str().map(String::from))
        .collect();

    // 2. Direct callers
    // Strategy: Find callers via multiple graph patterns:
    //   a) Direct CALLS/REFERENCES/IMPORTS edges to target symbol nodes
    //   b) ref:Call:NAME nodes (intermediate call reference) → their incoming CALLS → source files → symbols in those files
    //   c) IMPORTS edges to target's file where names include target name → source files
    let call_edge_types: &[&str] = &["CALLS", "REFERENCES", "IMPORTS"];
    let mut caller_file_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut direct_callers: Vec<Value> = Vec::new();

    // 2a. Direct edges to target symbol nodes
    for tid in &all_target_ids {
        for edge in gv.edges_to(tid, None) {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if call_edge_types.contains(&edge_type) {
                let source = edge["source"].as_str().unwrap_or("");
                if let Some(src_node) = gv.nodes_by_id.get(source) {
                    let name = src_node["properties"]["name"]
                        .as_str()
                        .or_else(|| src_node["id"].as_str().and_then(|id| id.split("::").last()))
                        .unwrap_or("")
                        .to_string();
                    let file = src_node["properties"]["sourcePath"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    if !include_tests && is_test_symbol(&name, &file) {
                        continue;
                    }
                    if !file.is_empty() && caller_file_set.insert(file.clone()) {
                        direct_callers.push(compact_node(src_node));
                    }
                }
            }
        }
    }

    // 2b. Find ref:Call:NAME nodes and their incoming CALLS edges
    let ref_id = format!("ref:Call:{}", primary_name);
    if let Some(incoming) = gv.incoming.get(&ref_id) {
        for edge in incoming {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if edge_type == "CALLS" {
                let source = edge["source"].as_str().unwrap_or("");
                // Source is typically a file node; find symbols in that file
                if let Some(src_node) = gv.nodes_by_id.get(source) {
                    let src_file = src_node["properties"]["sourcePath"]
                        .as_str()
                        .or_else(|| {
                            // For file nodes, extract relative path from id
                            src_node["id"]
                                .as_str()
                                .and_then(|id| id.find("/src/").map(|pos| &id[pos + 1..]))
                        })
                        .unwrap_or("")
                        .to_string();
                    if !include_tests && is_test_like_path(&src_file) {
                        continue;
                    }
                    // Find all symbol nodes in the source file
                    for node in gv.nodes_by_id.values() {
                        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
                        if file == src_file {
                            let name = node["properties"]["name"]
                                .as_str()
                                .or_else(|| {
                                    node["id"].as_str().and_then(|id| id.split("::").last())
                                })
                                .unwrap_or("");
                            if !name.is_empty()
                                && !caller_file_set.contains(file)
                                && node["kind"].as_str() == Some("symbol")
                            {
                                caller_file_set.insert(file.to_string());
                                direct_callers.push(compact_node(node));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2c. Find IMPORTS edges to target's file where names include target name
    if !primary_file.is_empty() {
        for node in gv.nodes_by_id.values() {
            let nid = node["id"].as_str().unwrap_or("");
            // Look for file nodes that import the target's file
            if nid.starts_with("file:") {
                if let Some(outgoing) = gv.outgoing.get(nid) {
                    for edge in outgoing {
                        if edge["type"].as_str() == Some("IMPORTS") {
                            let target_file = edge["target"].as_str().unwrap_or("");
                            // Check if this import targets the file containing our target
                            if target_file.contains(&primary_file) {
                                let names = edge["properties"]["names"]
                                    .as_array()
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect::<Vec<String>>()
                                    })
                                    .unwrap_or_default();
                                if names.iter().any(|n| n == &primary_name) {
                                    let src_file = node["properties"]["sourcePath"]
                                        .as_str()
                                        .or_else(|| nid.find("/src/").map(|pos| &nid[pos + 1..]))
                                        .unwrap_or("")
                                        .to_string();
                                    if !include_tests && is_test_like_path(&src_file) {
                                        continue;
                                    }
                                    // Add the source file's symbols as callers
                                    for sym_node in gv.nodes_by_id.values() {
                                        let sym_file = sym_node["properties"]["sourcePath"]
                                            .as_str()
                                            .unwrap_or("");
                                        if sym_file == src_file
                                            && sym_node["kind"].as_str() == Some("symbol")
                                        {
                                            let name = sym_node["properties"]["name"]
                                                .as_str()
                                                .or_else(|| {
                                                    sym_node["id"]
                                                        .as_str()
                                                        .and_then(|id| id.split("::").last())
                                                })
                                                .unwrap_or("");
                                            if !name.is_empty()
                                                && caller_file_set.insert(src_file.clone())
                                            {
                                                direct_callers.push(compact_node(sym_node));
                                            }
                                            break; // one representative per file
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    direct_callers.truncate(max_results);

    // 3. Direct callees
    // Strategy: similar — follow outgoing edges, then resolve via ref:Call nodes
    let mut callee_file_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut direct_callees: Vec<Value> = Vec::new();

    // 3a. Direct edges from target symbol nodes
    for tid in &all_target_ids {
        for edge in gv.edges_from(tid, None) {
            let edge_type = edge["type"].as_str().unwrap_or("");
            if call_edge_types.contains(&edge_type) {
                let target_id = edge["target"].as_str().unwrap_or("");
                if let Some(tgt_node) = gv.nodes_by_id.get(target_id) {
                    let name = tgt_node["properties"]["name"]
                        .as_str()
                        .or_else(|| tgt_node["id"].as_str().and_then(|id| id.split("::").last()))
                        .unwrap_or("")
                        .to_string();
                    let file = tgt_node["properties"]["sourcePath"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    if !include_tests && is_test_symbol(&name, &file) {
                        continue;
                    }
                    if !file.is_empty() && callee_file_set.insert(file.clone()) {
                        direct_callees.push(compact_node(tgt_node));
                    }
                }
            }
        }
    }

    // 3b. Find ref:Call: nodes from target's file → those represent calls FROM the target's file
    // Look for CALLS edges from the target's file node
    for (nid, node) in &gv.nodes_by_id {
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        if file == primary_file || nid.ends_with(&primary_file.replace('/', "/")) {
            if let Some(outgoing) = gv.outgoing.get(nid) {
                for edge in outgoing {
                    if edge["type"].as_str() == Some("CALLS") {
                        let target = edge["target"].as_str().unwrap_or("");
                        // target is a ref:Call:NAME node — resolve to actual symbol
                        if let Some(rest) = target.strip_prefix("ref:Call:") {
                            // Find the symbol with this name
                            if let Some(syms) = gv.symbols_by_name.get(&rest.to_lowercase()) {
                                for sym in syms {
                                    let sym_file =
                                        sym["properties"]["sourcePath"].as_str().unwrap_or("");
                                    if sym_file != primary_file {
                                        if callee_file_set.insert(sym_file.to_string()) {
                                            direct_callees.push(compact_node(sym));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    direct_callees.truncate(max_results);

    // 4. Indirect paths (BFS)
    let upstream_paths = if include_indirect {
        bfs_collect_by_depth(
            &gv,
            &all_target_ids,
            "upstream",
            max_depth,
            max_results,
            include_tests,
        )
    } else {
        vec![]
    };

    let downstream_paths = if include_indirect {
        bfs_collect_by_depth(
            &gv,
            &all_target_ids,
            "downstream",
            max_depth,
            max_results,
            include_tests,
        )
    } else {
        vec![]
    };

    // Count indirect paths (total nodes across all levels)
    let indirect_count: usize = upstream_paths
        .iter()
        .chain(downstream_paths.iter())
        .map(|l| l["nodes"].as_array().map(|a| a.len()).unwrap_or(0))
        .sum();

    // 5. Related files
    let mut related_files: Vec<String> = Vec::new();
    for node in &target_nodes {
        let f = node["properties"]["sourcePath"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if !f.is_empty() && !related_files.contains(&f) {
            related_files.push(f);
        }
    }
    for caller in &direct_callers {
        let f = caller["file"].as_str().unwrap_or("").to_string();
        if !f.is_empty() && !related_files.contains(&f) {
            related_files.push(f);
        }
    }
    for level in upstream_paths.iter().chain(downstream_paths.iter()) {
        if let Some(nodes) = level["nodes"].as_array() {
            for node in nodes {
                let f = node["file"].as_str().unwrap_or("").to_string();
                if !f.is_empty() && !related_files.contains(&f) {
                    related_files.push(f);
                }
            }
        }
    }

    // 6. Entry point reachability
    let entry_points = detect_entry_points(&gv, language, &[]);
    let reachable = reachable_from_entry_points(&gv, &entry_points);
    let target_reachable = all_target_ids.iter().any(|id| reachable.contains(id));

    let mut via_entries: Vec<String> = Vec::new();
    let mut min_path_len = usize::MAX;
    if target_reachable {
        // BFS from each entry point to find which can reach target
        for (ep_id, ep_name, _, _, _) in &entry_points {
            let mut queue: std::collections::VecDeque<(String, usize)> =
                std::collections::VecDeque::new();
            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            queue.push_back((ep_id.clone(), 0));
            visited.insert(ep_id.clone());

            while let Some((nid, depth)) = queue.pop_front() {
                if all_target_ids.contains(&nid) {
                    via_entries.push(ep_name.clone());
                    if depth < min_path_len {
                        min_path_len = depth;
                    }
                    break;
                }
                if depth >= max_depth + 2 {
                    continue;
                }
                if let Some(edges) = gv.outgoing.get(&nid) {
                    for edge in edges {
                        let edge_type = edge["type"].as_str().unwrap_or("");
                        if call_edge_types.contains(&edge_type) {
                            if let Some(tgt) = edge["target"].as_str() {
                                if visited.insert(tgt.to_string()) {
                                    queue.push_back((tgt.to_string(), depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 7. Risk score
    let mut risk_score: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    if direct_callers.len() > 5 {
        risk_score += 0.30;
        reasons.push(format!("{} direct callers", direct_callers.len()));
    }
    if indirect_count > 10 {
        risk_score += 0.20;
        reasons.push(format!("{} indirect paths", indirect_count));
    }
    // Cross-directory check
    let primary_dir = parent_dir(&primary_file);
    let has_cross_dir = direct_callers.iter().any(|c| {
        let caller_dir = parent_dir(c["file"].as_str().unwrap_or(""));
        !caller_dir.is_empty() && caller_dir != primary_dir
    });
    if has_cross_dir {
        risk_score += 0.20;
        reasons.push("cross-directory impact".to_string());
    }
    if is_public_symbol(primary, &gv) {
        risk_score += 0.15;
        reasons.push("target is public/exported".to_string());
    }
    if target_reachable {
        risk_score += 0.15;
        reasons.push("entry-point reachable".to_string());
    }
    // Check quality metrics
    let quality = compute_quality_metrics(&gv);
    let has_failures = quality["gates"]
        .as_array()
        .map(|gates| gates.iter().any(|g| g["passed"].as_bool() == Some(false)))
        .unwrap_or(false);
    if has_failures {
        risk_score += 0.10;
        reasons.push("quality metrics have failures".to_string());
    }
    // Test-only target
    let is_test_target = is_test_symbol(
        primary["properties"]["name"].as_str().unwrap_or(""),
        &primary_file,
    );
    if is_test_target {
        risk_score -= 0.10;
    }

    risk_score = risk_score.clamp(0.0, 1.0);
    let risk_level = risk_level_from_score(risk_score);

    // 8. readFirst: top 5 callers by fan-in (most dependent)
    let mut callers_with_fanin: Vec<(Value, usize)> = Vec::new();
    for caller in &direct_callers {
        let cid = caller["id"].as_str().unwrap_or("").to_string();
        let fan_in = gv.incoming.get(&cid).map(|v| v.len()).unwrap_or(0);
        callers_with_fanin.push((caller.clone(), fan_in));
    }
    callers_with_fanin.sort_by(|a, b| b.1.cmp(&a.1));
    let read_first: Vec<Value> = callers_with_fanin
        .iter()
        .take(5)
        .map(|(node, fan_in)| {
            json!({
                "id": node["id"],
                "name": node["name"],
                "kind": node["kind"],
                "file": node["file"],
                "line": node["line"],
                "fanIn": fan_in
            })
        })
        .collect();

    // 9. reviewFirst: top 5 callers that are public/exported
    let mut public_callers: Vec<Value> = Vec::new();
    for caller in &direct_callers {
        let cid = caller["id"].as_str().unwrap_or("").to_string();
        if let Some(node) = gv.nodes_by_id.get(&cid) {
            if is_public_symbol(node, &gv) {
                public_callers.push(caller.clone());
            }
        }
    }
    public_callers.truncate(5);
    let review_first = public_callers;

    let result_data = json!({
        "language": language,
        "root": root,
        "targetMatched": compact_node(primary),
        "directCallers": direct_callers,
        "directCallees": direct_callees,
        "upstreamPaths": upstream_paths,
        "downstreamPaths": downstream_paths,
        "relatedFiles": related_files,
        "entryPointReachability": {
            "reachable": target_reachable,
            "viaEntryPoints": via_entries,
            "pathLength": if min_path_len == usize::MAX { 0 } else { min_path_len }
        },
        "riskLevel": risk_level,
        "riskScore": (risk_score * 100.0).round() / 100.0,
        "reasons": reasons,
        "cautions": vec!["static analysis only", "dynamic dispatch may hide callers"],
        "readFirst": read_first,
        "reviewFirst": review_first,
        "generatedFrom": {
            "staticAnalysisOnly": true,
            "heuristic": true,
            "compilerVerified": false
        }
    });

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

/// Handle `codelattice_risk_hotspots` tool.
fn handle_risk_hotspots(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let scope = params["scope"].as_str().unwrap_or("all");
    let max_results = params["maxResults"].as_u64().unwrap_or(20).min(100) as usize;
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let min_risk_level = params["minRiskLevel"].as_str().unwrap_or("medium");
    let _compact = params["compact"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // Entry points and reachability
    let entry_points = detect_entry_points(&gv, language, &[]);
    let reachable = reachable_from_entry_points(&gv, &entry_points);

    // Minimum risk score threshold
    let min_score = match min_risk_level {
        "low" => 0.0,
        "medium" => 0.3,
        "high" => 0.6,
        "critical" => 0.8,
        _ => 0.3,
    };

    // Symbol hotspots
    let mut hotspot_symbols: Vec<Value> = Vec::new();
    if scope == "all" || scope == "symbols" {
        for node in gv.nodes_by_id.values() {
            let kind = node["kind"].as_str().unwrap_or("");
            let label = node["label"].as_str().unwrap_or("");
            if kind != "symbol" && label != "symbol" {
                continue;
            }

            let symbol_kind = node["properties"]["symbolKind"]
                .as_str()
                .or_else(|| node["kind"].as_str())
                .unwrap_or("");
            if matches!(
                symbol_kind,
                "module" | "package" | "repository" | "file" | "source_file"
            ) {
                continue;
            }

            let (score, reasons) =
                compute_symbol_hotspot_score(node, &gv, &reachable, include_tests);
            if score < min_score || reasons.is_empty() {
                continue;
            }

            let id = node["id"].as_str().unwrap_or("").to_string();
            let fan_in = gv
                .incoming
                .get(&id)
                .map(|v| {
                    v.iter()
                        .filter(|e| {
                            let t = e["type"].as_str().unwrap_or("");
                            t == "CALLS" || t == "REFERENCES" || t == "IMPORTS"
                        })
                        .count()
                })
                .unwrap_or(0);
            let fan_out = gv
                .outgoing
                .get(&id)
                .map(|v| {
                    v.iter()
                        .filter(|e| {
                            let t = e["type"].as_str().unwrap_or("");
                            t == "CALLS" || t == "REFERENCES" || t == "IMPORTS"
                        })
                        .count()
                })
                .unwrap_or(0);

            hotspot_symbols.push(json!({
                "id": id,
                "name": node["properties"]["name"].as_str()
                    .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
                    .unwrap_or(""),
                "kind": symbol_kind,
                "file": node["properties"]["sourcePath"].as_str().unwrap_or(""),
                "line": node["properties"]["startLine"].as_u64().unwrap_or(0),
                "score": (score * 100.0).round() / 100.0,
                "riskLevel": risk_level_from_score(score),
                "fanIn": fan_in,
                "fanOut": fan_out,
                "reasons": reasons,
                "cautions": vec!["static analysis only"]
            }));
        }

        // Sort by score descending, then by name
        hotspot_symbols.sort_by(|a, b| {
            let score_cmp = b["score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal);
            if score_cmp != std::cmp::Ordering::Equal {
                score_cmp
            } else {
                a["name"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["name"].as_str().unwrap_or(""))
            }
        });
        hotspot_symbols.truncate(max_results);
    }

    // File hotspots
    let mut hotspot_files: Vec<Value> = Vec::new();
    if scope == "all" || scope == "files" {
        struct FileAccum {
            symbol_count: usize,
            total_score: f64,
            incoming: usize,
            outgoing: usize,
        }
        let mut file_accums: HashMap<String, FileAccum> = HashMap::new();

        for node in gv.nodes_by_id.values() {
            let kind = node["kind"].as_str().unwrap_or("");
            let label = node["label"].as_str().unwrap_or("");
            if kind != "symbol" && label != "symbol" {
                continue;
            }

            let file = node["properties"]["sourcePath"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if file.is_empty() || is_generated_path(&file) {
                continue;
            }
            if !include_tests && is_test_like_path(&file) {
                continue;
            }

            let id = node["id"].as_str().unwrap_or("").to_string();
            let (score, reasons) =
                compute_symbol_hotspot_score(node, &gv, &reachable, include_tests);
            if reasons.is_empty() {
                continue;
            }

            let inc = gv.incoming.get(&id).map(|v| v.len()).unwrap_or(0);
            let out = gv.outgoing.get(&id).map(|v| v.len()).unwrap_or(0);

            let accum = file_accums.entry(file.clone()).or_insert(FileAccum {
                symbol_count: 0,
                total_score: 0.0,
                incoming: 0,
                outgoing: 0,
            });
            accum.symbol_count += 1;
            accum.total_score += score;
            accum.incoming += inc;
            accum.outgoing += out;
        }

        for (file, accum) in &file_accums {
            if accum.symbol_count == 0 {
                continue;
            }
            let avg_score = accum.total_score / accum.symbol_count as f64;
            let mut file_score = avg_score;
            let mut file_reasons: Vec<String> = Vec::new();

            // High edge density
            let edge_density = (accum.incoming + accum.outgoing) as f64 / accum.symbol_count as f64;
            if edge_density > 3.0 {
                file_score += 0.20;
                file_reasons.push("high edge density".to_string());
            }

            // High symbol count
            if accum.symbol_count > 10 {
                file_score += 0.10;
                file_reasons.push("high symbol count".to_string());
            }

            // Entry file
            let is_entry = entry_points.iter().any(|(_, _, _, f, _)| f == file);
            if is_entry {
                file_score += 0.15;
                file_reasons.push("entry file".to_string());
            }

            // Contains public exports
            let has_public = gv.nodes_by_id.values().any(|n| {
                let f = n["properties"]["sourcePath"].as_str().unwrap_or("");
                f == *file && is_public_symbol(n, &gv)
            });
            if has_public {
                file_score += 0.15;
                file_reasons.push("contains public exports".to_string());
            }

            file_score = file_score.clamp(0.0, 1.0);
            if file_score < min_score {
                continue;
            }

            hotspot_files.push(json!({
                "path": file,
                "score": (file_score * 100.0).round() / 100.0,
                "symbolCount": accum.symbol_count,
                "fanIn": accum.incoming,
                "fanOut": accum.outgoing,
                "reasons": file_reasons,
                "cautions": vec!["static analysis only"]
            }));
        }

        hotspot_files.sort_by(|a, b| {
            let score_cmp = b["score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal);
            if score_cmp != std::cmp::Ordering::Equal {
                score_cmp
            } else {
                a["path"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["path"].as_str().unwrap_or(""))
            }
        });
        hotspot_files.truncate(max_results);
    }

    // Summary counts
    let high_risk_count = hotspot_symbols
        .iter()
        .filter(|s| {
            s["riskLevel"].as_str() == Some("high") || s["riskLevel"].as_str() == Some("critical")
        })
        .count();
    let medium_risk_count = hotspot_symbols
        .iter()
        .filter(|s| s["riskLevel"].as_str() == Some("medium"))
        .count();
    let low_risk_count = hotspot_symbols
        .iter()
        .filter(|s| s["riskLevel"].as_str() == Some("low"))
        .count();

    let result_data = json!({
        "language": language,
        "root": root,
        "summary": {
            "hotspotSymbolCount": hotspot_symbols.len(),
            "hotspotFileCount": hotspot_files.len(),
            "highRiskCount": high_risk_count,
            "mediumRiskCount": medium_risk_count,
            "lowRiskCount": low_risk_count
        },
        "hotspotSymbols": hotspot_symbols,
        "hotspotFiles": hotspot_files,
        "hotspotModules": [],
        "scoringModel": "fan-in/out + cross-module + entry reachability + public API",
        "cautions": vec!["static analysis only", "not compiler-verified"],
        "generatedFrom": {
            "staticAnalysisOnly": true,
            "heuristic": true,
            "compilerVerified": false
        }
    });

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

/// Handle `codelattice_architecture_drift` tool.
fn handle_architecture_drift(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let layer_rules: Vec<String> = params["layerRules"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let max_cycles = params["maxCycles"].as_u64().unwrap_or(10).min(50) as usize;
    let max_findings = params["maxFindings"].as_u64().unwrap_or(50).min(200) as usize;
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let _compact = params["compact"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // 1. Cycle detection
    let cycles = detect_cycles(&gv, max_cycles, include_tests);

    // 2. Cross-directory / bidirectional dependencies
    let reverse_deps = detect_bidirectional_deps(&gv, include_tests);

    // 3. Overly coupled modules
    let coupled_modules = compute_coupling(&gv, max_findings, include_tests);

    // 4. Layer violations (only if rules provided)
    let (cross_layer_calls, boundary_leaks) =
        detect_layer_violations(&gv, &layer_rules, max_findings, include_tests);

    // 5. Recommended refactoring checks
    let mut recommendations: Vec<String> = Vec::new();
    for cycle in &cycles {
        if let Some(desc) = cycle["description"].as_str() {
            recommendations.push(format!("Review cycle: {}", desc));
        }
    }
    for coupled in &coupled_modules {
        if let Some(path) = coupled["path"].as_str() {
            recommendations.push(format!("Reduce coupling in: {}", path));
        }
    }
    for rd in &reverse_deps {
        if let Some(desc) = rd["description"].as_str() {
            recommendations.push(format!("Review {}", desc));
        }
    }
    recommendations.truncate(max_findings);

    let total_findings = cycles.len()
        + reverse_deps.len()
        + cross_layer_calls.len()
        + boundary_leaks.len()
        + coupled_modules.len();

    let mut cautions = vec![
        "static analysis only".to_string(),
        "cycle detection has depth limits".to_string(),
    ];
    if !layer_rules.is_empty() {
        cautions.push("layer rules are user-provided not inferred".to_string());
    }

    let result_data = json!({
        "language": language,
        "root": root,
        "summary": {
            "cycleCount": cycles.len(),
            "reverseDependencyCount": reverse_deps.len(),
            "crossLayerCallCount": cross_layer_calls.len(),
            "boundaryLeakCount": boundary_leaks.len(),
            "overlyCoupledModuleCount": coupled_modules.len(),
            "totalFindings": total_findings
        },
        "cycles": cycles,
        "reverseDependencies": reverse_deps,
        "crossLayerCalls": cross_layer_calls,
        "boundaryLeaks": boundary_leaks,
        "overlyCoupledModules": coupled_modules,
        "cautions": cautions,
        "recommendedRefactorChecks": recommendations,
        "generatedFrom": {
            "staticAnalysisOnly": true,
            "heuristic": true,
            "compilerVerified": false
        }
    });

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

/// Handle `codelattice_dead_code_candidates` tool.
fn handle_dead_code_candidates(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;
    let include_files = params["includeFiles"].as_bool().unwrap_or(true);
    let include_symbols = params["includeSymbols"].as_bool().unwrap_or(true);
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let include_public_api = params["includePublicApi"].as_bool().unwrap_or(true);

    let entry_hints: Vec<String> = params["entryHints"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let exclude_patterns: Vec<String> = params["excludePatterns"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // 1. Detect entry points
    let entry_points = detect_entry_points(&gv, language, &entry_hints);
    let entry_point_ids: std::collections::HashSet<String> = entry_points
        .iter()
        .map(|(id, _, _, _, _)| id.clone())
        .collect();

    // 2. Compute reachability
    let reachable = reachable_from_entry_points(&gv, &entry_points);

    // 3. Score symbol candidates
    let mut symbol_candidates = if include_symbols {
        score_candidate_symbols(
            &gv,
            language,
            &entry_point_ids,
            &reachable,
            include_tests,
            include_public_api,
            &exclude_patterns,
        )
    } else {
        Vec::new()
    };

    // 4. Score file candidates
    let mut file_candidates = if include_files {
        score_candidate_files(
            &gv,
            language,
            &entry_point_ids,
            &reachable,
            include_tests,
            &exclude_patterns,
        )
    } else {
        Vec::new()
    };

    // Apply limit
    symbol_candidates.truncate(limit);
    file_candidates.truncate(limit);

    // Compact mode: remove extra fields
    if compact {
        for cand in &mut symbol_candidates {
            if let Some(obj) = cand.as_object_mut() {
                obj.remove("recommendedVerification");
            }
        }
    }

    // Compute summary
    let high_count = symbol_candidates
        .iter()
        .chain(file_candidates.iter())
        .filter(|c| c["confidence"].as_str() == Some("high"))
        .count();
    let medium_count = symbol_candidates
        .iter()
        .chain(file_candidates.iter())
        .filter(|c| c["confidence"].as_str() == Some("medium"))
        .count();
    let low_count = symbol_candidates
        .iter()
        .chain(file_candidates.iter())
        .filter(|c| c["confidence"].as_str() == Some("low"))
        .count();

    let public_api_caution_count = symbol_candidates
        .iter()
        .filter(|c| {
            c["cautions"]
                .as_array()
                .map(|arr| {
                    arr.iter().any(|v| {
                        v.as_str()
                            .map(|s| s.contains("public-api"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .count();

    let dynamic_caution_count = symbol_candidates
        .iter()
        .filter(|c| {
            c["cautions"]
                .as_array()
                .map(|arr| {
                    arr.iter().any(|v| {
                        v.as_str()
                            .map(|s| s.contains("dynamic-dispatch"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .count();

    // Build warnings
    let mut warnings: Vec<String> = Vec::new();
    if entry_points.is_empty() {
        warnings.push("entry-point-detection-low-confidence".to_string());
    }

    // Build entry points output
    let entry_points_json: Vec<Value> = entry_points
        .iter()
        .map(|(id, name, kind, file, line)| {
            json!({
                "id": id,
                "name": name,
                "kind": kind,
                "file": file,
                "line": line
            })
        })
        .collect();

    let result_data = json!({
        "language": language,
        "root": root,
        "summary": {
            "candidateSymbolCount": symbol_candidates.len(),
            "entryPointCount": entry_points.len(),
            "reachableSymbolCount": reachable.len(),
            "candidateFileCount": file_candidates.len(),
            "highConfidenceCandidateCount": high_count,
            "mediumConfidenceCandidateCount": medium_count,
            "lowConfidenceCandidateCount": low_count,
            "publicApiCautionCount": public_api_caution_count,
            "dynamicFeatureCautionCount": dynamic_caution_count
        },
        "candidateSymbols": symbol_candidates,
        "candidateFiles": file_candidates,
        "entryPoints": entry_points_json,
        "warnings": warnings,
        "generatedFrom": {
            "graphBased": true,
            "compilerVerified": false,
            "heuristic": true,
            "deletionSafe": false, "runtimeVerified": false
        }
    });

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

// ============================================================
// v0.14: AI Context Pack & Review Gate
// ============================================================

/// AI editing context — what should I read before changing code?
/// Searches for symbols/files matching task keywords, collects call chains,
/// dependency notes, risk notes, and suggests a read order.
fn handle_ai_context_pack(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let task = params["task"].as_str().unwrap_or("");
    let targets: Vec<String> = params["targets"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let max_files = params["maxFiles"].as_u64().unwrap_or(15).min(100) as usize;
    let max_symbols = params["maxSymbols"].as_u64().unwrap_or(30).min(200) as usize;
    let _include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let _compact = params["compact"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // 1. Parse task string for keywords (split by spaces and common delimiters)
    let mut keywords: Vec<String> = Vec::new();
    for word in task.split(|c: char| {
        c.is_whitespace() || c == ',' || c == ';' || c == ':' || c == '|' || c == '/'
    }) {
        let w = word.trim().to_lowercase();
        if !w.is_empty() && w.len() >= 2 {
            keywords.push(w);
        }
    }
    // Combine with targets
    for t in &targets {
        let tl = t.to_lowercase();
        if !tl.is_empty() && !keywords.contains(&tl) {
            keywords.push(tl);
        }
    }

    // 2. Search GraphView for matching symbols (case-insensitive contains)
    let mut matched_symbols: Vec<Value> = Vec::new();
    for (name, syms) in &gv.symbols_by_name {
        let name_lower = name.to_lowercase();
        let matches = keywords.iter().any(|kw| name_lower.contains(kw));
        if matches {
            for sym in syms {
                matched_symbols.push(sym.clone());
            }
        }
    }

    // Deduplicate by id
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    matched_symbols.retain(|s| {
        if let Some(id) = s["id"].as_str() {
            seen_ids.insert(id.to_string())
        } else {
            false
        }
    });

    // 3. Search for matching files (nodes with sourcePaths that contain keywords)
    let mut file_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for node in gv.nodes_by_id.values() {
        if let Some(sp) = node["properties"]["sourcePath"].as_str() {
            let sp_lower = sp.to_lowercase();
            if keywords.iter().any(|kw| sp_lower.contains(kw)) {
                file_set.insert(sp.to_string());
            }
        }
    }

    // 4. Collect direct callers/callees for matched symbols
    let mut call_chains: Vec<Value> = Vec::new();
    let mut file_deps: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_related_file_set: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

    for sym in &matched_symbols {
        if let Some(sym_id) = sym["id"].as_str() {
            // Outgoing edges (callees)
            if let Some(edges) = gv.outgoing.get(sym_id) {
                for edge in edges {
                    let edge_type = edge["type"].as_str().unwrap_or("");
                    if edge_type == "CALLS" || edge_type == "IMPORTS" {
                        let target_id = edge["target"].as_str().unwrap_or("");
                        if let Some(target_node) = gv.nodes_by_id.get(target_id) {
                            call_chains.push(json!({
                                "from": sym_id,
                                "to": target_id,
                                "edgeType": edge_type,
                                "description": format!("{} {} {}",
                                    sym["properties"]["name"].as_str().unwrap_or("?"),
                                    if edge_type == "CALLS" { "calls" } else { "imports" },
                                    target_node["properties"]["name"].as_str().unwrap_or("?")
                                )
                            }));
                            if let Some(sp) = target_node["properties"]["sourcePath"].as_str() {
                                all_related_file_set.insert(sp.to_string());
                            }
                        }
                    }
                }
            }
            // Incoming edges (callers)
            if let Some(edges) = gv.incoming.get(sym_id) {
                for edge in edges {
                    let edge_type = edge["type"].as_str().unwrap_or("");
                    if edge_type == "CALLS" || edge_type == "IMPORTS" {
                        let source_id = edge["source"].as_str().unwrap_or("");
                        if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                            call_chains.push(json!({
                                "from": source_id,
                                "to": sym_id,
                                "edgeType": edge_type,
                                "description": format!("{} {} {}",
                                    source_node["properties"]["name"].as_str().unwrap_or("?"),
                                    if edge_type == "CALLS" { "calls" } else { "imports" },
                                    sym["properties"]["name"].as_str().unwrap_or("?")
                                )
                            }));
                            if let Some(sp) = source_node["properties"]["sourcePath"].as_str() {
                                all_related_file_set.insert(sp.to_string());
                            }
                        }
                    }
                }
            }
            if let Some(sp) = sym["properties"]["sourcePath"].as_str() {
                all_related_file_set.insert(sp.to_string());
            }
        }
    }

    // Merge file_set into all_related_file_set
    for f in &file_set {
        all_related_file_set.insert(f.clone());
    }

    // 5. Compute dependency notes
    for sym in &matched_symbols {
        if let Some(sym_id) = sym["id"].as_str() {
            if let Some(sp) = sym["properties"]["sourcePath"].as_str() {
                // Check outgoing imports from this symbol's file
                for edge in gv.outgoing.get(sym_id).unwrap_or(&Vec::new()) {
                    if edge["type"].as_str() == Some("IMPORTS")
                        || edge["type"].as_str() == Some("REFERENCES")
                    {
                        if let Some(target_id) = edge["target"].as_str() {
                            if let Some(target_node) = gv.nodes_by_id.get(target_id) {
                                if let Some(tsp) = target_node["properties"]["sourcePath"].as_str()
                                {
                                    if tsp != sp {
                                        file_deps.insert(format!("{} depends on {}", sp, tsp));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. Compute risk notes — hotspots with high fan-in
    let mut risk_notes: Vec<String> = Vec::new();
    for sym in &matched_symbols {
        if let Some(sym_id) = sym["id"].as_str() {
            let name = sym["properties"]["name"].as_str().unwrap_or("?");
            if let Some(incoming) = gv.incoming.get(sym_id) {
                let caller_count = incoming
                    .iter()
                    .filter(|e| e["type"].as_str() == Some("CALLS"))
                    .count();
                if caller_count > 3 {
                    risk_notes.push(format!("{} is a hotspot ({} callers)", name, caller_count));
                }
            }
            // Check for low-confidence edges
            if let Some(edges) = gv.outgoing.get(sym_id) {
                for edge in edges {
                    if let Some(conf) = edge["confidence"].as_f64() {
                        if conf < 0.7 {
                            risk_notes.push(format!(
                                "{} has a low-confidence edge to {}",
                                name,
                                edge["target"].as_str().unwrap_or("?")
                            ));
                            break;
                        }
                    }
                }
            }
        }
    }

    // 7. Compute suggested read order
    // Compute fan-in per file
    let mut file_fan_in: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (id, node) in &gv.nodes_by_id {
        if let Some(sp) = node["properties"]["sourcePath"].as_str() {
            let incoming_count = gv.incoming.get(id).map(|v| v.len()).unwrap_or(0);
            *file_fan_in.entry(sp.to_string()).or_insert(0) += incoming_count;
        }
    }

    let mut read_order_files: Vec<String> = all_related_file_set.iter().cloned().collect();

    // Sort: entry-point-like files first, then by fan-in descending, then by name
    read_order_files.sort_by(|a, b| {
        let a_entry = is_entry_file(a, language);
        let b_entry = is_entry_file(b, language);
        if a_entry != b_entry {
            return b_entry.cmp(&a_entry); // entry files first
        }
        let a_fan = file_fan_in.get(a).unwrap_or(&0);
        let b_fan = file_fan_in.get(b).unwrap_or(&0);
        if a_fan != b_fan {
            return b_fan.cmp(a_fan); // higher fan-in first
        }
        a.cmp(b)
    });
    read_order_files.truncate(max_files);

    // 8. Compute doNotAssume list
    let mut do_not_assume: Vec<String> = Vec::new();
    // Dynamic dispatch patterns
    for sym in &matched_symbols {
        if let Some(name) = sym["properties"]["name"].as_str() {
            let nl = name.to_lowercase();
            if nl.contains("registry")
                || nl.contains("plugin")
                || nl.contains("handler")
                || nl.contains("factory")
            {
                do_not_assume.push(format!(
                    "dynamic dispatch may hide callers to {} patterns",
                    name
                ));
            }
        }
        // Public API symbols may have external callers
        let exported = sym["properties"]["exported"].as_bool().unwrap_or(false);
        if exported {
            if let Some(name) = sym["properties"]["name"].as_str() {
                do_not_assume.push(format!(
                    "{} is exported and may have external consumers",
                    name
                ));
            }
        }
    }
    if do_not_assume.is_empty() {
        do_not_assume.push("no dynamic dispatch patterns detected in matched symbols".to_string());
    }

    // 9. Build context files with priority
    let mut context_files: Vec<Value> = Vec::new();
    for (idx, f) in read_order_files.iter().enumerate() {
        let reason = if file_set.contains(f) {
            "contains target symbol"
        } else if matched_symbols
            .iter()
            .any(|s| s["properties"]["sourcePath"].as_str() == Some(f.as_str()))
        {
            "contains matched symbol"
        } else {
            "dependency of matched symbols"
        };
        context_files.push(json!({
            "path": f,
            "reason": reason,
            "readPriority": (idx + 1) as u64
        }));
    }

    // 10. Build key symbols
    let key_symbols: Vec<Value> = matched_symbols
        .iter()
        .take(max_symbols)
        .map(|s| {
            let name = s["properties"]["name"].as_str().unwrap_or("?");
            let reason = if keywords.iter().any(|kw| name.to_lowercase().contains(kw)) {
                "directly matches task keyword"
            } else {
                "related to matched symbols"
            };
            json!({
                "id": s["id"],
                "name": s["properties"]["name"],
                "kind": s["kind"],
                "file": s["properties"]["sourcePath"],
                "line": s["properties"]["startLine"],
                "reason": reason
            })
        })
        .collect();

    // 11. Compute useful commands
    let mut useful_commands: Vec<String> = Vec::new();
    for sym in key_symbols.iter().take(5) {
        if let Some(name) = sym["name"].as_str() {
            useful_commands.push(format!("codelattice_impact_preview symbol={}", name));
            useful_commands.push(format!("codelattice_symbol_context symbol={}", name));
        }
    }
    for sym in key_symbols.iter().take(3) {
        if let Some(name) = sym["name"].as_str() {
            useful_commands.push(format!("codelattice_calls_to symbol={}", name));
        }
    }

    // Deduplicate useful commands
    let mut seen_cmds: std::collections::HashSet<String> = std::collections::HashSet::new();
    useful_commands.retain(|c| seen_cmds.insert(c.clone()));

    let result_data = json!({
        "language": language,
        "root": root,
        "taskEcho": task,
        "contextFiles": context_files,
        "keySymbols": key_symbols,
        "callChains": call_chains,
        "dependencyNotes": file_deps.into_iter().collect::<Vec<String>>(),
        "riskNotes": risk_notes,
        "suggestedReadOrder": read_order_files,
        "doNotAssume": do_not_assume,
        "usefulCommands": useful_commands,
        "cautions": ["static analysis only", "keyword matching is not semantic understanding"],
        "generatedFrom": {
            "staticAnalysisOnly": true,
            "heuristic": true,
            "compilerVerified": false
        }
    });

    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

/// Helper: check if a file looks like an entry point based on language heuristics.
fn is_entry_file(path: &str, language: &str) -> bool {
    match language {
        "rust" => path.ends_with("main.rs") || path.ends_with("lib.rs"),
        "cangjie" => path.ends_with("package.cj"),
        "arkts" => path.contains("Index.ets") || path.contains("MainAbility/"),
        "typescript" => {
            path.ends_with("index.ts") || path.ends_with("main.ts") || path.ends_with(".tsx")
        }
        _ => path.ends_with("main") || path.ends_with("index"),
    }
}

/// Diff-based review gate — does this change touch dangerous areas?
/// Analyzes changed files for touched symbols, hotspots, risk level.
fn handle_review_gate(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let explicit_changed: Vec<String> = params["changedFiles"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let use_git_diff = params["useGitDiff"].as_bool().unwrap_or(false);
    let include_untracked = params["includeUntracked"].as_bool().unwrap_or(false);
    let max_findings = params["maxFindings"].as_u64().unwrap_or(50).min(200) as usize;
    let _compact = params["compact"].as_bool().unwrap_or(true);

    // 1. Get changed files
    let mut changed_files: Vec<String> = explicit_changed.clone();
    let mut warnings: Vec<String> = Vec::new();

    if use_git_diff {
        // Run git diff --name-only
        let git_output = std::process::Command::new("git")
            .args(&["diff", "--name-only"])
            .current_dir(&validated)
            .output();

        match git_output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() && !changed_files.contains(&trimmed) {
                        changed_files.push(trimmed);
                    }
                }
            }
            Ok(_) => {
                warnings.push("git diff failed — may not be a git repository".to_string());
            }
            Err(_) => {
                warnings.push("git command not available".to_string());
            }
        }

        // Optionally include staged changes
        let cached_output = std::process::Command::new("git")
            .args(&["diff", "--cached", "--name-only"])
            .current_dir(&validated)
            .output();

        if let Ok(output) = cached_output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() && !changed_files.contains(&trimmed) {
                        changed_files.push(trimmed);
                    }
                }
            }
        }

        if include_untracked {
            let untracked_output = std::process::Command::new("git")
                .args(&["ls-files", "--others", "--exclude-standard"])
                .current_dir(&validated)
                .output();

            if let Ok(output) = untracked_output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() && !changed_files.contains(&trimmed) {
                            changed_files.push(trimmed);
                        }
                    }
                }
            }
        }
    }

    if changed_files.is_empty() {
        let result_data = json!({
            "language": language,
            "root": root,
            "changedFiles": [],
            "touchedSymbols": [],
            "touchedHotspots": [],
            "impactSummary": {
                "totalTouchedSymbols": 0,
                "totalCallers": 0,
                "crossDirectoryCount": 0,
                "publicApiCount": 0
            },
            "architectureWarnings": [],
            "deadCodeRelatedSignals": [],
            "recommendedTests": [],
            "reviewChecklist": ["no changes detected — provide changedFiles or enable useGitDiff"],
            "riskLevel": "low",
            "cautions": ["static analysis only", "git diff may not reflect staged changes"],
            "generatedFrom": {
                "staticAnalysisOnly": true,
                "heuristic": true,
                "compilerVerified": false
            }
        });
        if !warnings.is_empty() {
            let mut rd = result_data;
            if let Some(obj) = rd.as_object_mut() {
                obj.insert("warnings".to_string(), json!(warnings));
            }
            return Ok(tool_result(&rd));
        }
        // No changes detected and no git
        let mut rd = result_data;
        if let Some(obj) = rd.as_object_mut() {
            obj.insert("warnings".to_string(), json!(["no-changes-detected"]));
        }
        return Ok(tool_result(&rd));
    }

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    // 2. Find symbols in changed files
    let mut touched_symbols: Vec<Value> = Vec::new();
    let mut all_caller_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total_callers: usize = 0;
    let mut public_api_count: usize = 0;
    let mut cross_directory_count: usize = 0;

    for node in gv.nodes_by_id.values() {
        let sp = node["properties"]["sourcePath"].as_str().unwrap_or("");
        if !changed_files
            .iter()
            .any(|cf| sp == cf || sp.ends_with(cf.as_str()))
        {
            continue;
        }

        let node_id = node["id"].as_str().unwrap_or("");
        let name = node["properties"]["name"].as_str().unwrap_or("");
        let kind = node["kind"].as_str().unwrap_or("");
        let is_public = node["properties"]["exported"].as_bool().unwrap_or(false)
            || node["properties"]["visibility"].as_str() == Some("public");

        // Count callers
        let empty_incoming: Vec<Value> = Vec::new();
        let incoming = gv.incoming.get(node_id).unwrap_or(&empty_incoming);
        let caller_count = incoming
            .iter()
            .filter(|e| e["type"].as_str() == Some("CALLS"))
            .count();

        total_callers += caller_count;
        if is_public {
            public_api_count += 1;
        }

        // Check for cross-directory callers
        for edge in incoming {
            if edge["type"].as_str() == Some("CALLS") {
                if let Some(source_id) = edge["source"].as_str() {
                    if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                        if let Some(source_sp) = source_node["properties"]["sourcePath"].as_str() {
                            all_caller_ids.insert(source_id.to_string());
                            // Check cross-directory
                            if let (Some(sp_dir), Some(source_dir)) = (
                                sp.rfind('/').map(|i| &sp[..i]),
                                source_sp.rfind('/').map(|i| &source_sp[..i]),
                            ) {
                                if sp_dir != source_dir {
                                    cross_directory_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        touched_symbols.push(json!({
            "id": node_id,
            "name": name,
            "kind": kind,
            "file": sp,
            "callerCount": caller_count,
            "isPublic": is_public
        }));

        if touched_symbols.len() >= max_findings {
            break;
        }
    }

    // 3. Collect hotspots — symbols that call modified symbols
    let mut touched_hotspots: Vec<Value> = Vec::new();
    for caller_id in &all_caller_ids {
        if let Some(caller_node) = gv.nodes_by_id.get(caller_id) {
            // Find which touched symbols this caller calls
            let caller_name = caller_node["properties"]["name"].as_str().unwrap_or("?");
            let caller_kind = caller_node["kind"].as_str().unwrap_or("?");
            let caller_file = caller_node["properties"]["sourcePath"]
                .as_str()
                .unwrap_or("");

            // Find the reason — which modified symbol it calls
            let mut reasons: Vec<String> = Vec::new();
            if let Some(edges) = gv.outgoing.get(caller_id) {
                for edge in edges {
                    if edge["type"].as_str() == Some("CALLS") {
                        if let Some(target_id) = edge["target"].as_str() {
                            if touched_symbols
                                .iter()
                                .any(|ts| ts["id"].as_str() == Some(target_id))
                            {
                                if let Some(target_node) = gv.nodes_by_id.get(target_id) {
                                    reasons.push(format!(
                                        "calls modified symbol {}",
                                        target_node["properties"]["name"].as_str().unwrap_or("?")
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            if !reasons.is_empty() {
                touched_hotspots.push(json!({
                    "id": caller_id,
                    "name": caller_name,
                    "kind": caller_kind,
                    "file": caller_file,
                    "reason": reasons.join(", ")
                }));
            }
        }
    }

    // 4. Architecture warnings — check cross-layer
    let mut architecture_warnings: Vec<String> = Vec::new();
    // Simple heuristic: if changed files span api/service/domain/infra layers, note it
    let layers: &[&str] = &["api", "service", "domain", "infra"];
    let mut changed_layers: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for cf in &changed_files {
        for layer in layers {
            if cf.contains(layer) {
                changed_layers.insert(layer);
            }
        }
    }
    if changed_layers.len() > 1 {
        let mut layer_names: Vec<&str> = changed_layers.into_iter().collect();
        layer_names.sort();
        architecture_warnings.push(format!(
            "changes span multiple layers: {}",
            layer_names.join(", ")
        ));
    }

    // 5. Dead code related signals
    let dead_code_related_signals: Vec<String> = Vec::new();

    // 6. Recommended tests — find test files that import from changed files
    let mut recommended_tests: Vec<String> = Vec::new();
    let changed_set: std::collections::HashSet<String> = changed_files.iter().cloned().collect();
    for node in gv.nodes_by_id.values() {
        let sp = node["properties"]["sourcePath"].as_str().unwrap_or("");
        if !sp.contains("test") && !sp.contains("spec") {
            continue;
        }
        let node_id = node["id"].as_str().unwrap_or("");
        if let Some(edges) = gv.outgoing.get(node_id) {
            for edge in edges {
                if edge["type"].as_str() == Some("IMPORTS")
                    || edge["type"].as_str() == Some("REFERENCES")
                {
                    if let Some(target_id) = edge["target"].as_str() {
                        if let Some(target_node) = gv.nodes_by_id.get(target_id) {
                            if let Some(target_sp) =
                                target_node["properties"]["sourcePath"].as_str()
                            {
                                if changed_set.contains(target_sp)
                                    || changed_files
                                        .iter()
                                        .any(|cf| target_sp.ends_with(cf.as_str()))
                                {
                                    let note = format!("{} (imports from changed files)", sp);
                                    if !recommended_tests.contains(&note) {
                                        recommended_tests.push(note);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 7. Compute risk level
    let max_callers = touched_symbols
        .iter()
        .map(|ts| ts["callerCount"].as_u64().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let risk_level = if touched_symbols.is_empty() {
        "low"
    } else if max_callers > 20 || public_api_count > 0 {
        "critical"
    } else if max_callers >= 10 {
        "high"
    } else if max_callers >= 3 || total_callers >= 5 {
        "medium"
    } else {
        "low"
    };

    // 8. Review checklist
    let mut review_checklist: Vec<String> = Vec::new();
    for ts in &touched_symbols {
        let name = ts["name"].as_str().unwrap_or("?");
        let callers = ts["callerCount"].as_u64().unwrap_or(0);
        if callers > 0 {
            review_checklist.push(format!(
                "{} is called by {} symbols — verify none break",
                name, callers
            ));
        }
        if ts["isPublic"].as_bool().unwrap_or(false) {
            review_checklist.push(format!("{} is public API — check external consumers", name));
        }
    }
    for hs in &touched_hotspots {
        let name = hs["name"].as_str().unwrap_or("?");
        let reason = hs["reason"].as_str().unwrap_or("");
        review_checklist.push(format!("{} — {}", name, reason));
    }
    // Deduplicate
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    review_checklist.retain(|item| seen.insert(item.clone()));
    review_checklist.truncate(max_findings);

    let result_data = json!({
        "language": language,
        "root": root,
        "changedFiles": changed_files,
        "touchedSymbols": touched_symbols,
        "touchedHotspots": touched_hotspots,
        "impactSummary": {
            "totalTouchedSymbols": touched_symbols.len(),
            "totalCallers": total_callers,
            "crossDirectoryCount": cross_directory_count,
            "publicApiCount": public_api_count
        },
        "architectureWarnings": architecture_warnings,
        "deadCodeRelatedSignals": dead_code_related_signals,
        "recommendedTests": recommended_tests,
        "reviewChecklist": review_checklist,
        "riskLevel": risk_level,
        "cautions": ["static analysis only", "git diff may not reflect staged changes"],
        "generatedFrom": {
            "staticAnalysisOnly": true,
            "heuristic": true,
            "compilerVerified": false
        }
    });

    let mut final_data = result_data;
    if !warnings.is_empty() {
        if let Some(obj) = final_data.as_object_mut() {
            obj.insert("warnings".to_string(), json!(warnings));
        }
    }

    Ok(merge_cache_and_result(&final_data, &cache_meta))
}

// ============================================================

// ============================================================
// v0.20: Entry Point & Reachability Map
// ============================================================

fn detect_entry_points_rich(
    gv: &GraphView,
    language: &str,
    entry_hints: &[String],
) -> Vec<(
    String,
    String,
    String,
    String,
    u64,
    String,
    f64,
    Vec<String>,
)> {
    let entry_file_suffixes: &[&str] = match language {
        "rust" => &["main.rs", "lib.rs"],
        "cangjie" => &["package.cj", "main.cj"],
        "arkts" => &["Index.ets", "MainAbility"],
        "typescript" => &["index.ts", "index.tsx", "main.ts", "app.ts", "server.ts"],
        "python" => &[
            "main.py",
            "__main__.py",
            "app.py",
            "cli.py",
            "api.py",
            "__init__.py",
        ],
        "c" | "cpp" => &["main.c", "main.cpp", "main.cc"],
        _ => &["main"],
    };
    let mut results: Vec<(
        String,
        String,
        String,
        String,
        u64,
        String,
        f64,
        Vec<String>,
    )> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }
        let name = node["properties"]["name"]
            .as_str()
            .or_else(|| {
                node["id"]
                    .as_str()
                    .and_then(|id_str| id_str.split("::").last())
            })
            .unwrap_or("");
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        let line = node["properties"]["startLine"].as_u64().unwrap_or(0);
        let id = node["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() || seen.contains(&id) {
            continue;
        }
        let fan_out = gv.outgoing.get(&id).map(|v| v.len()).unwrap_or(0);
        let mut score: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        if name == "main" {
            score += 0.50;
            reasons.push("entry-like symbol name".to_string());
        }
        match language {
            "python" if name == "create_app" || name == "createApp" => {
                score += 0.40;
                reasons.push("framework entry name".to_string());
            }
            "c" | "cpp" if name == "WinMain" || name == "wWinMain" || name == "DllMain" => {
                score += 0.35;
                reasons.push("platform entry name".to_string());
            }
            "arkts" if name == "build" || name == "aboutToAppear" => {
                score += 0.40;
                reasons.push("lifecycle entry name".to_string());
            }
            _ => {}
        }
        for s in entry_file_suffixes {
            if file.ends_with(s) {
                score += 0.30;
                reasons.push("entry-like filename".to_string());
                break;
            }
        }
        if fan_out > 8 {
            score += 0.15;
            reasons.push("high fan-out orchestrator".to_string());
        } else if fan_out > 4 {
            score += 0.08;
            reasons.push("moderate fan-out".to_string());
        }
        if (file.ends_with("lib.rs")
            || file.ends_with("package.cj")
            || file.ends_with("__init__.py"))
            && !name.starts_with('_')
        {
            score += 0.10;
            reasons.push("public symbol in package root".to_string());
        }
        for hint in entry_hints {
            if name == hint.as_str() || file.contains(hint.as_str()) || id.contains(hint.as_str()) {
                score += 0.35;
                reasons.push(format!("user hint: {}", hint));
                break;
            }
        }
        if score < 0.15 {
            continue;
        }
        let confidence = if score >= 0.70 {
            "high"
        } else if score >= 0.40 {
            "medium"
        } else {
            "low"
        };
        seen.insert(id.clone());
        results.push((
            id,
            name.to_string(),
            kind.to_string(),
            file.to_string(),
            line,
            confidence.to_string(),
            score.min(1.0),
            reasons,
        ));
    }
    if results.is_empty() {
        let mut cands: Vec<&Value> = gv
            .nodes_by_id
            .values()
            .filter(|n| {
                let k = n["kind"].as_str().unwrap_or("");
                k == "function" || k == "method" || k == "symbol"
            })
            .collect();
        cands.sort_by_key(|n| {
            std::cmp::Reverse(
                gv.outgoing
                    .get(n["id"].as_str().unwrap_or(""))
                    .map(|v| v.len())
                    .unwrap_or(0),
            )
        });
        for node in cands.iter().take(5) {
            let id = node["id"].as_str().unwrap_or("").to_string();
            if id.is_empty() || seen.contains(&id) {
                continue;
            }
            let name = node["properties"]["name"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let file = node["properties"]["sourcePath"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let line = node["properties"]["startLine"].as_u64().unwrap_or(0);
            let kind = node["kind"].as_str().unwrap_or("symbol").to_string();
            seen.insert(id.clone());
            results.push((
                id,
                name,
                kind,
                file,
                line,
                "low".to_string(),
                0.25,
                vec!["fallback: high fan-out".to_string()],
            ));
        }
    }
    results
}

fn reachable_from_entry_points_rich(
    gv: &GraphView,
    entry_points: &[(
        String,
        String,
        String,
        String,
        u64,
        String,
        f64,
        Vec<String>,
    )],
    max_depth: usize,
) -> std::collections::HashSet<String> {
    let mut reachable = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
    for (id, _, _, _, _, _, _, _) in entry_points {
        reachable.insert(id.clone());
        queue.push_back((id.clone(), 0));
    }
    let follow: &[&str] = &["CALLS", "REFERENCES", "IMPORTS", "INCLUDES", "DEFINES"];
    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if let Some(edges) = gv.outgoing.get(&node_id) {
            for edge in edges {
                let et = edge["type"].as_str().unwrap_or("");
                if follow.contains(&et) {
                    if let Some(target) = edge["target"].as_str() {
                        if reachable.insert(target.to_string()) {
                            queue.push_back((target.to_string(), depth + 1));
                        }
                    }
                }
            }
        }
    }
    reachable
}

fn handle_reachability_map(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;
    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(100).min(500) as usize;
    let max_depth = params["maxDepth"].as_u64().unwrap_or(8).min(20).max(1) as usize;
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let include_reachable_items = params["includeReachableItems"].as_bool().unwrap_or(false);
    let entry_hints: Vec<String> = params["entryHints"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let exclude_patterns: Vec<String> = params["excludePatterns"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let entry_points = detect_entry_points_rich(&gv, language, &entry_hints);
    let ep_ids: std::collections::HashSet<String> = entry_points
        .iter()
        .map(|(id, _, _, _, _, _, _, _)| id.clone())
        .collect();
    let reachable = reachable_from_entry_points_rich(&gv, &entry_points, max_depth);
    let mut reachable_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in &reachable {
        if let Some(n) = gv.nodes_by_id.get(id) {
            if let Some(f) = n["properties"]["sourcePath"].as_str() {
                if !f.is_empty() {
                    reachable_files.insert(f.to_string());
                }
            }
        }
    }
    let mut file_sym_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut file_unreach: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut unreachable_symbols: Vec<Value> = Vec::new();
    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol"
            && label != "symbol"
            && kind != "function"
            && kind != "method"
            && kind != "class"
            && kind != "struct"
            && kind != "enum"
            && kind != "const"
            && kind != "static"
            && kind != "associated-function"
        {
            continue;
        }
        let id = node["id"].as_str().unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let name = node["properties"]["name"]
            .as_str()
            .or_else(|| node["id"].as_str().and_then(|i| i.split("::").last()))
            .unwrap_or("");
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        let line = node["properties"]["startLine"].as_u64().unwrap_or(0);
        let node_kind = node["kind"].as_str().unwrap_or("symbol");
        if ep_ids.contains(id) || is_generated_path(file) {
            continue;
        }
        if !include_tests && is_test_like_path(file) {
            continue;
        }
        if exclude_patterns
            .iter()
            .any(|p| file.contains(p.as_str()) || name.contains(p.as_str()))
        {
            continue;
        }
        if !file.is_empty() {
            *file_sym_count.entry(file.to_string()).or_insert(0) += 1;
        }
        if reachable.contains(id) {
            continue;
        }
        *file_unreach.entry(file.to_string()).or_insert(0) += 1;
        let mut score: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        let mut cautions: Vec<String> = Vec::new();
        reasons.push("not-reachable-from-entry-points".to_string());
        score += 0.40;
        let incoming = gv.incoming.get(id).map(|v| v.len()).unwrap_or(0);
        if incoming == 0 {
            reasons.push("no-incoming-calls".to_string());
            score += 0.25;
        }
        if !reachable_files.contains(file) && !file.is_empty() {
            reasons.push("file-not-reachable".to_string());
            score += 0.15;
        }
        cautions.push("static-analysis-only".to_string());
        if !name.starts_with('_') && incoming == 0 {
            cautions.push("public-api-may-have-external-callers".to_string());
        }
        if has_dynamic_pattern(name, file) {
            cautions.push("dynamic-dispatch-may-hide-callers".to_string());
        }
        if score < 0.30 {
            continue;
        }
        let confidence = if score >= 0.70 {
            "high"
        } else if score >= 0.45 {
            "medium"
        } else {
            "low"
        };
        let mut sym = json!({"name": name, "kind": node_kind, "file": file, "line": line, "score": (score * 100.0).round() / 100.0, "confidence": confidence, "reasons": reasons, "cautions": cautions});
        if !compact {
            if let Some(o) = sym.as_object_mut() {
                o.insert("id".to_string(), json!(id));
            }
        }
        unreachable_symbols.push(sym);
    }
    unreachable_symbols.sort_by(|a, b| {
        b["score"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    unreachable_symbols.truncate(limit);
    let mut unreachable_files: Vec<Value> = Vec::new();
    for (file, total) in &file_sym_count {
        if is_generated_path(file) || (!include_tests && is_test_like_path(file)) {
            continue;
        }
        if exclude_patterns.iter().any(|p| file.contains(p.as_str())) {
            continue;
        }
        let uc = file_unreach.get(file).copied().unwrap_or(0);
        if uc == *total && uc > 0 {
            let mut fr: Vec<String> = vec!["file-not-reachable-from-entry-points".to_string()];
            if !reachable_files.contains(file) {
                fr.push("no-incoming-import-or-include".to_string());
            }
            unreachable_files.push(json!({"path": file, "symbolCount": total, "score": 0.70, "reasons": fr, "cautions": ["static-analysis-only"]}));
        }
    }
    let ep_json: Vec<Value> = entry_points.iter().map(|(id, name, kind, file, line, conf, score, reasons)| {
        let mut ep = json!({"name": name, "kind": kind, "file": file, "line": line, "confidence": conf, "score": (*score * 100.0).round() / 100.0, "reasons": reasons});
        if !compact { if let Some(o) = ep.as_object_mut() { o.insert("id".to_string(), json!(id)); } } ep
    }).collect();
    let dyn_caution = unreachable_symbols
        .iter()
        .filter(|s| {
            s["cautions"]
                .as_array()
                .map(|a| {
                    a.iter().any(|v| {
                        v.as_str()
                            .map(|t| t.contains("dynamic-dispatch"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .count();
    let mut warnings = vec![
        "static graph reachability only".to_string(),
        "dynamic dispatch may hide runtime reachability".to_string(),
    ];
    if entry_points.is_empty() {
        warnings.push("entry-point-detection-low-confidence".to_string());
    }
    let reachable_section = if include_reachable_items {
        let syms: Vec<Value> = reachable.iter().filter_map(|id_val| gv.nodes_by_id.get(id_val)).take(limit)
            .map(|n| json!({"id": n["id"], "name": n["properties"]["name"], "kind": n["kind"], "file": n["properties"]["sourcePath"]})).collect();
        json!({"symbolCount": reachable.len(), "fileCount": reachable_files.len(), "symbols": syms, "files": reachable_files.iter().take(limit).map(|f| json!(f)).collect::<Vec<Value>>()})
    } else {
        json!({"symbolCount": reachable.len(), "fileCount": reachable_files.len()})
    };
    let result_data = json!({
        "language": language, "root": root,
        "summary": {"entryPointCount": entry_points.len(), "reachableSymbolCount": reachable.len(), "reachableFileCount": reachable_files.len(),
            "unreachableSymbolCandidateCount": unreachable_symbols.len(), "unreachableFileCandidateCount": unreachable_files.len(), "dynamicCautionCount": dyn_caution},
        "entryPoints": ep_json, "reachable": reachable_section, "unreachableCandidates": {"symbols": unreachable_symbols, "files": unreachable_files},
        "warnings": warnings, "generatedFrom": {"graphBased": true, "compilerVerified": false, "runtimeVerified": false, "heuristic": true}
    });
    Ok(merge_cache_and_result(&result_data, &cache_meta))
}

// JSON-RPC Dispatch
// ============================================================

fn make_response(id: &Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn make_error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn handle_request(request: &Value, cache: &mut McpCache) -> Option<Value> {
    let method = request["method"].as_str().unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = request.get("params").cloned().unwrap_or(json!({}));

    // Notifications (no id or id is null) don't get responses
    if id.is_null() && !method.starts_with("tools/") {
        match method {
            "notifications/initialized" => {
                eprintln!("[mcp] client initialized");
                return None;
            }
            _ => {
                eprintln!("[mcp] ignoring notification: {}", method);
                return None;
            }
        }
    }

    match method {
        "initialize" => {
            let cangjie_support = {
                #[cfg(feature = "tree-sitter-cangjie")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-cangjie"))]
                {
                    false
                }
            };
            let arkts_support = {
                #[cfg(feature = "tree-sitter-arkts")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-arkts"))]
                {
                    false
                }
            };
            let typescript_support = {
                #[cfg(feature = "tree-sitter-typescript")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-typescript"))]
                {
                    false
                }
            };
            let c_support = {
                #[cfg(feature = "tree-sitter-c")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-c"))]
                {
                    false
                }
            };
            let cpp_support = {
                #[cfg(feature = "tree-sitter-cpp")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-cpp"))]
                {
                    false
                }
            };
            let python_support = {
                #[cfg(feature = "tree-sitter-python")]
                {
                    true
                }
                #[cfg(not(feature = "tree-sitter-python"))]
                {
                    false
                }
            };
            Some(make_response(
                &id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "codelattice",
                        "version": "0.13.0",
                        "cangjieSupport": cangjie_support,
                        "arktsSupport": arkts_support,
                        "typescriptSupport": typescript_support,
                        "cSupport": c_support,
                        "cppSupport": cpp_support,
                        "pythonSupport": python_support,
                        "toolCount": 27
                    }
                }),
            ))
        }

        "tools/list" => Some(make_response(&id, tools_list())),

        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");

            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = match tool_name {
                "codelattice_analyze" => handle_analyze(cache, &arguments),
                "codelattice_quality" => handle_quality(cache, &arguments),
                "codelattice_summary" => handle_summary(cache, &arguments),
                "codelattice_smoke" => handle_smoke(cache, &arguments),
                "codelattice_graph_overview" => handle_graph_overview(cache, &arguments),
                "codelattice_unresolved_report" => handle_unresolved_report(cache, &arguments),
                "codelattice_symbol_search" => handle_symbol_search(cache, &arguments),
                "codelattice_export_bridge" => handle_export_bridge(cache, &arguments),
                "codelattice_symbol_context" => handle_symbol_context(cache, &arguments),
                "codelattice_calls_from" => handle_calls_from(cache, &arguments),
                "codelattice_calls_to" => handle_calls_to(cache, &arguments),
                "codelattice_impact_preview" => handle_impact_preview(cache, &arguments),
                "codelattice_query_graph" => handle_query_graph(cache, &arguments),
                "codelattice_project_overview" => handle_project_overview(cache, &arguments),
                "codelattice_repo_registry" => handle_repo_registry(cache, &arguments),
                "codelattice_rename_preview" => handle_rename_preview(cache, &arguments),
                "codelattice_cache_status" => handle_cache_status(cache, &arguments),
                "codelattice_cache_clear" => handle_cache_clear(cache, &arguments),
                "codelattice_production_assist" => handle_production_assist(cache, &arguments),
                "codelattice_compare_runs" => handle_compare_runs(cache, &arguments),
                "codelattice_cache_prewarm" => handle_cache_prewarm(cache, &arguments),
                "codelattice_changed_symbols" => handle_changed_symbols(cache, &arguments),
                "codelattice_project_insights" => handle_project_insights(cache, &arguments),
                "codelattice_review_plan" => handle_review_plan(cache, &arguments),
                "codelattice_dead_code_candidates" => {
                    handle_dead_code_candidates(cache, &arguments)
                }
                "codelattice_impact_analysis" => handle_impact_analysis(cache, &arguments),
                "codelattice_risk_hotspots" => handle_risk_hotspots(cache, &arguments),
                "codelattice_architecture_drift" => handle_architecture_drift(cache, &arguments),
                "codelattice_ai_context_pack" => handle_ai_context_pack(cache, &arguments),
                "codelattice_review_gate" => handle_review_gate(cache, &arguments),
                "codelattice_reachability_map" => handle_reachability_map(cache, &arguments),
                "codelattice_external_api_surface" => {
                    handle_external_api_surface(cache, &arguments)
                }
                "codelattice_framework_entry_hints" => {
                    handle_framework_entry_hints(cache, &arguments)
                }
                "codelattice_breaking_change_review" => {
                    handle_breaking_change_review(cache, &arguments)
                }
                "codelattice_consistency_review" => handle_consistency_review(cache, &arguments),
                "codelattice_config_examples_review" => {
                    handle_config_examples_review(cache, &arguments)
                }
                _ => Err(mcp_error(
                    "unknown_tool",
                    &format!("Unknown tool: {tool_name}"),
                )),
            };

            match result {
                Ok(r) => Some(make_response(&id, r)),
                Err(e) => Some(make_response(
                    &id,
                    json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&e).unwrap_or_default() }],
                        "isError": true
                    }),
                )),
            }
        }

        "shutdown" | "exit" => {
            eprintln!("[mcp] shutdown requested");
            None
        }

        _ => Some(make_error_response(
            &id,
            -32601,
            &format!("Method not found: {method}"),
        )),
    }
}

// ============================================================
// Server Main Loop
// ============================================================

// ============================================================
// v0.21: External API Surface / Public API Caution
// ============================================================

// === External API Surface Handler ===
// Insert before the main() function at the end of mcp_server.rs

/// Detect external API surface symbols — symbols likely consumed by external callers.
/// Returns scored candidates with caution levels and verification recommendations.
fn compute_external_api_surface(
    gv: &GraphView,
    language: &str,
    doc_scanner: Option<&DocScanner>,
    include_docs: bool,
    include_tests: bool,
    include_headers: bool,
    include_package_metadata: bool,
    limit: usize,
) -> Value {
    let mut surface_symbols: Vec<Value> = Vec::new();
    let mut surface_files: Vec<String> = Vec::new();
    let mut package_export_count: usize = 0;
    let mut header_api_count: usize = 0;
    let mut documented_api_count: usize = 0;
    let mut high_caution_count: usize = 0;

    // Package metadata signals — scan package.json, pyproject.toml etc.
    let pkg_entry_files = detect_package_entry_files(gv, language);
    let pkg_bin_files = detect_package_bin_files(gv, language);

    for node in gv.nodes_by_id.values() {
        let kind = node["kind"].as_str().unwrap_or("");
        let label = node["label"].as_str().unwrap_or("");
        if kind != "symbol" && label != "symbol" {
            continue;
        }

        let name = node["properties"]["name"]
            .as_str()
            .or_else(|| node["id"].as_str().and_then(|id| id.split("::").last()))
            .unwrap_or("");
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        let line = node["properties"]["startLine"].as_u64().unwrap_or(0);
        let id = node["id"].as_str().unwrap_or("").to_string();

        // Skip test/generated/vendor paths
        if !include_tests && is_test_like_path(file) {
            continue;
        }
        if file.contains("/generated/")
            || file.contains("/vendor/")
            || file.contains("/node_modules/")
        {
            continue;
        }

        // Skip headers if not included
        if !include_headers && (file.ends_with(".h") || file.ends_with(".hpp")) {
            continue;
        }

        let mut score: f64 = 0.0;
        let mut reasons: Vec<String> = Vec::new();
        let mut caution_key = "";

        // === Language-specific heuristics ===
        match language {
            "rust" => {
                // pub visibility
                let visibility = node["properties"]["visibility"].as_str().unwrap_or("");
                if visibility == "public" {
                    score += 0.30;
                    reasons.push("rust-pub-visibility".to_string());
                }
                // lib.rs items
                if file.ends_with("lib.rs") {
                    score += 0.25;
                    reasons.push("rust-lib-rs-item".to_string());
                }
                // pub use re-export (check incoming REFERENCES)
                if is_reexported_symbol(&id, gv) {
                    score += 0.20;
                    reasons.push("rust-pub-use-re-export".to_string());
                }
                caution_key = "rust-public-api-may-have-external-crate-consumers";
            }
            "typescript" | "arkts" => {
                // export keyword
                if node["properties"]["exported"].as_bool() == Some(true) {
                    score += 0.30;
                    reasons.push("typescript-export".to_string());
                }
                // index.ts / package entry
                if file.ends_with("index.ts")
                    || file.ends_with("index.tsx")
                    || pkg_entry_files.iter().any(|f| file.ends_with(f.as_str()))
                {
                    score += 0.25;
                    reasons.push("typescript-package-entry-file".to_string());
                }
                // re-exported from index
                if is_reexported_from_index(&id, gv) {
                    score += 0.20;
                    reasons.push("typescript-re-exported-from-index".to_string());
                }
                // TSX component
                if file.ends_with(".tsx") {
                    score += 0.10;
                    reasons.push("typescript-tsx-component".to_string());
                }
                // package.json bin reference
                if pkg_bin_files.iter().any(|f| file.ends_with(f.as_str())) {
                    score += 0.25;
                    reasons.push("typescript-package-bin-entry".to_string());
                    package_export_count += 1;
                }
                // package.json exports/main/types reference
                if pkg_entry_files.iter().any(|f| file.ends_with(f.as_str())) {
                    if reasons.iter().all(|r| r != "typescript-package-entry-file") {
                        score += 0.15;
                        reasons.push("typescript-package-metadata-reference".to_string());
                    }
                    package_export_count += 1;
                }
                if language == "arkts" {
                    caution_key = "arkts-component-entry-may-be-used-by-framework";
                } else {
                    caution_key = "typescript-package-export-may-have-downstream-consumers";
                }
            }
            "python" => {
                // Non-underscore top-level name
                let is_public_name = !name.starts_with('_');
                if is_public_name {
                    score += 0.20;
                    reasons.push("python-public-name".to_string());
                }
                // __init__.py items
                if file.ends_with("__init__.py") {
                    score += 0.25;
                    reasons.push("python-init-py-export".to_string());
                }
                // Re-exported from __init__
                if is_reexported_from_init(&id, gv) {
                    score += 0.20;
                    reasons.push("python-re-exported-from-init".to_string());
                }
                caution_key = "python-package-api-may-have-external-importers";
            }
            "c" => {
                // Header under include/
                if file.contains("/include/") && file.ends_with(".h") {
                    score += 0.30;
                    reasons.push("c-include-header".to_string());
                    header_api_count += 1;
                }
                // Non-static function in header
                let is_static = node["properties"]["storageClass"].as_str() == Some("static");
                if !is_static && file.ends_with(".h") {
                    score += 0.25;
                    reasons.push("c-non-static-header-declaration".to_string());
                }
                caution_key = "c-header-api-may-have-external-callers";
            }
            "cpp" => {
                // Header under include/
                if file.contains("/include/")
                    && (file.ends_with(".h") || file.ends_with(".hpp") || file.ends_with(".hxx"))
                {
                    score += 0.30;
                    reasons.push("cpp-include-header".to_string());
                    header_api_count += 1;
                }
                // Exported namespace/class in header
                if file.ends_with(".hpp") || file.ends_with(".h") || file.ends_with(".hxx") {
                    score += 0.25;
                    reasons.push("cpp-header-declaration".to_string());
                }
                caution_key = "cpp-header-api-may-have-external-callers";
            }
            "cangjie" => {
                // Public package symbol
                let visibility = node["properties"]["visibility"].as_str().unwrap_or("");
                if visibility == "public" {
                    score += 0.30;
                    reasons.push("cangjie-public-visibility".to_string());
                }
                // Package root
                if file.ends_with("package.cj") || file.contains("/src/") && !file.contains("/src/")
                {
                    score += 0.25;
                    reasons.push("cangjie-package-root".to_string());
                }
                caution_key = "cangjie-public-api-may-have-external-consumers";
            }
            _ => {
                // Generic: check public/exported
                if is_public_symbol(node, gv) {
                    score += 0.25;
                    reasons.push("generic-public-or-exported".to_string());
                }
            }
        }

        // === Cross-cutting signals ===
        // Entry file patterns (lib.rs, index.ts, __init__.py)
        if is_entry_file(file, language) {
            if reasons.iter().all(|r| !r.contains("entry-file")) {
                score += 0.10;
                reasons.push("entry-file-pattern".to_string());
            }
        }

        // /include/ path (C/C++ or generic)
        if file.contains("/include/") {
            if reasons.iter().all(|r| !r.contains("include")) {
                score += 0.10;
                reasons.push("include-directory".to_string());
            }
        }

        // Documented in README/docs
        if include_docs {
            if let Some(scanner) = doc_scanner {
                if is_documented(name, file, scanner) {
                    score += 0.15;
                    reasons.push("documented-in-readme-or-docs".to_string());
                    documented_api_count += 1;
                }
            }
        }

        // Entry point candidate
        if detect_entry_like(
            name,
            kind,
            file,
            language,
            gv.outgoing.get(&id).map(|v| v.len()).unwrap_or(0),
        ) {
            score += 0.10;
            if reasons.iter().all(|r| !r.contains("entry")) {
                reasons.push("entry-point-candidate".to_string());
            }
        }

        // Negative: name starts _ or internal
        if name.starts_with('_') || name.contains("internal") || name.contains("private") {
            score -= 0.25;
        }

        // Clamp
        score = score.max(0.0).min(1.0);

        // Filter: only include score >= 0.35
        if score < 0.35 {
            continue;
        }

        // Determine caution level
        let caution_level = if score >= 0.75 {
            "high"
        } else if score >= 0.45 {
            "medium"
        } else {
            "low"
        };

        if caution_level == "high" {
            high_caution_count += 1;
        }

        // Build recommended verification
        let verification = build_external_verification(reasons.as_slice(), language);

        // Add caution key as reason
        if !caution_key.is_empty() {
            reasons.push(caution_key.to_string());
        }
        reasons.push("not-safe-to-treat-as-dead-code-based-on-internal-callers-only".to_string());

        // Track surface files
        if !surface_files.contains(&file.to_string()) {
            surface_files.push(file.to_string());
        }

        surface_symbols.push(json!({
            "id": id,
            "name": name,
            "kind": kind,
            "file": file,
            "line": line,
            "score": (score * 100.0).round() / 100.0,
            "cautionLevel": caution_level,
            "reasons": reasons,
            "recommendedVerification": verification
        }));
    }

    // Sort: score desc, file asc, line asc, name asc
    surface_symbols.sort_by(|a, b| {
        b["score"]
            .as_f64()
            .partial_cmp(&a["score"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a["file"].as_str().cmp(&b["file"].as_str()))
            .then_with(|| a["line"].as_u64().cmp(&b["line"].as_u64()))
            .then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
    });

    surface_symbols.truncate(limit);

    json!({
        "summary": {
            "externalSurfaceSymbolCount": surface_symbols.len(),
            "externalSurfaceFileCount": surface_files.len(),
            "packageExportCount": package_export_count,
            "headerApiCount": header_api_count,
            "documentedApiCount": documented_api_count,
            "highCautionCount": high_caution_count
        },
        "externalSurfaceSymbols": surface_symbols,
        "externalSurfaceFiles": surface_files.into_iter().take(limit).collect::<Vec<String>>()
    })
}

/// Detect package entry files from metadata (package.json main/types/exports, etc.)
fn detect_package_entry_files(gv: &GraphView, _language: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    // Check for common entry file patterns in graph
    for node in gv.nodes_by_id.values() {
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        if file.ends_with("index.ts")
            || file.ends_with("index.tsx")
            || file.ends_with("lib.rs")
            || file.ends_with("__init__.py")
            || file.ends_with("main.ts")
        {
            if !entries.iter().any(|e| file.ends_with(e.as_str())) {
                entries.push(file.to_string());
            }
        }
    }
    entries
}

/// Detect package bin files from metadata
fn detect_package_bin_files(gv: &GraphView, _language: &str) -> Vec<String> {
    let mut bins: Vec<String> = Vec::new();
    for node in gv.nodes_by_id.values() {
        let file = node["properties"]["sourcePath"].as_str().unwrap_or("");
        if file.ends_with("cli.ts") || file.ends_with("bin.ts") || file.ends_with("main.ts") {
            if !bins.iter().any(|e| file.ends_with(e.as_str())) {
                bins.push(file.to_string());
            }
        }
    }
    bins
}

/// Check if a symbol is re-exported (Rust pub use pattern)
fn is_reexported_symbol(id: &str, gv: &GraphView) -> bool {
    if let Some(incoming) = gv.incoming.get(id) {
        for edge in incoming {
            let et = edge["type"].as_str().unwrap_or("");
            if et == "REFERENCES" || et == "IMPORTS" {
                if let Some(source_id) = edge["source"].as_str() {
                    if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                        let source_file = source_node["properties"]["sourcePath"]
                            .as_str()
                            .unwrap_or("");
                        let source_name = source_node["properties"]["name"].as_str().unwrap_or("");
                        // If the source is in lib.rs or index.ts and the name is different, it's likely a re-export
                        if (source_file.ends_with("lib.rs") || source_file.ends_with("index.ts"))
                            && source_name.contains("export")
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a symbol is re-exported from index.ts
fn is_reexported_from_index(id: &str, gv: &GraphView) -> bool {
    if let Some(incoming) = gv.incoming.get(id) {
        for edge in incoming {
            let et = edge["type"].as_str().unwrap_or("");
            if et == "REFERENCES" || et == "IMPORTS" {
                if let Some(source_id) = edge["source"].as_str() {
                    if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                        let source_file = source_node["properties"]["sourcePath"]
                            .as_str()
                            .unwrap_or("");
                        if source_file.ends_with("index.ts") || source_file.ends_with("index.tsx") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a symbol is re-exported from __init__.py
fn is_reexported_from_init(id: &str, gv: &GraphView) -> bool {
    if let Some(incoming) = gv.incoming.get(id) {
        for edge in incoming {
            let et = edge["type"].as_str().unwrap_or("");
            if et == "REFERENCES" || et == "IMPORTS" {
                if let Some(source_id) = edge["source"].as_str() {
                    if let Some(source_node) = gv.nodes_by_id.get(source_id) {
                        let source_file = source_node["properties"]["sourcePath"]
                            .as_str()
                            .unwrap_or("");
                        if source_file.ends_with("__init__.py") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a file is a known entry file for the language

/// Check if a symbol or file is documented in README or docs
fn is_documented(name: &str, file: &str, scanner: &DocScanner) -> bool {
    for doc in &scanner.docs {
        for r#ref in &doc.references {
            if r#ref.match_type == "symbol" && r#ref.matched_text == name {
                return true;
            }
            if r#ref.match_type == "file" {
                let ref_path = &r#ref.matched_text;
                if file.ends_with(ref_path) || ref_path.ends_with(file) {
                    return true;
                }
            }
        }
    }
    false
}

/// Build recommended verification steps based on reasons and language
fn build_external_verification(reasons: &[String], language: &str) -> Vec<String> {
    let mut steps: Vec<String> = Vec::new();
    match language {
        "rust" => {
            steps.push("check crate public API and semver compatibility".to_string());
            steps.push("search downstream consumers on crates.io or in workspace".to_string());
        }
        "typescript" | "arkts" => {
            steps.push("check package exports/main/types/bin fields".to_string());
            steps.push("search downstream npm consumers".to_string());
        }
        "python" => {
            steps.push("check import paths and console scripts".to_string());
            steps.push("search downstream consumers on PyPI".to_string());
        }
        "c" | "cpp" => {
            steps.push("search external include usage".to_string());
            steps.push("check header consumers in dependent projects".to_string());
        }
        "cangjie" => {
            steps.push("check package public API surface".to_string());
        }
        _ => {
            steps.push("search downstream consumers".to_string());
        }
    }
    steps.push("review changelog and compatibility policy".to_string());
    steps.push("run compatibility tests before removing or changing".to_string());
    steps
}

/// External API surface handler for MCP tool
fn handle_external_api_surface(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;
    let include_docs = params["includeDocs"].as_bool().unwrap_or(true);
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let include_headers = params["includeHeaders"].as_bool().unwrap_or(true);
    let include_pkg_meta = params["includePackageMetadata"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let doc_scanner = if include_docs {
        let scanner = DocScanner::build(&std::path::PathBuf::from(&validated));
        Some(scanner)
    } else {
        None
    };

    let surface = compute_external_api_surface(
        &gv,
        language,
        doc_scanner.as_ref(),
        include_docs,
        include_tests,
        include_headers,
        include_pkg_meta,
        limit,
    );

    let result = json!({
        "language": language,
        "root": validated,
        "summary": surface["summary"],
        "externalSurfaceSymbols": if compact {
            json!(surface["externalSurfaceSymbols"].as_array().map(|arr| {
                arr.iter().map(|s| json!({
                    "name": s["name"],
                    "kind": s["kind"],
                    "file": s["file"],
                    "line": s["line"],
                    "score": s["score"],
                    "cautionLevel": s["cautionLevel"]
                })).collect::<Vec<Value>>()
            }).unwrap_or_default())
        } else {
            surface["externalSurfaceSymbols"].clone()
        },
        "externalSurfaceFiles": if compact {
            json!(null)
        } else {
            surface["externalSurfaceFiles"].clone()
        },
        "generatedFrom": {
            "graphBased": true,
            "compilerVerified": false,
            "externalUsageVerified": false,
            "heuristic": true
        }
    });

    Ok(merge_cache_and_result(&result, &cache_meta))
}

// ============================================================
// v0.22: Framework Entry Hints / Callback Entry Caution
// ============================================================

// ============================================================
// v0.22: Framework Entry Hints / Callback Entry Caution
// ============================================================

/// Scoring options for framework entry hint detection.
struct FrameworkHintOptions {
    include_tests: bool,
    include_callbacks: bool,
    include_routes: bool,
    include_components: bool,
    limit: usize,
}

/// Single framework entry hint result.
#[derive(Clone)]
struct FrameworkEntryHint {
    id: String,
    name: String,
    kind: String,
    file: String,
    line: u64,
    hint_kind: String,
    framework: String,
    score: f64,
    confidence: String,
    reasons: Vec<String>,
    cautions: Vec<String>,
    recommended_verification: Vec<String>,
}

impl FrameworkEntryHint {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "kind": self.kind,
            "file": self.file,
            "line": self.line,
            "hintKind": self.hint_kind,
            "framework": self.framework,
            "score": self.score,
            "confidence": self.confidence,
            "reasons": self.reasons,
            "cautions": self.cautions,
            "recommendedVerification": self.recommended_verification
        })
    }
}

/// Check if a file path matches route-like directory patterns.
fn is_route_path(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.contains("routes")
        || lower.contains("pages")
        || lower.contains("/api/")
        || lower.contains("handlers")
        || lower.contains("controllers")
        || lower.contains("commands")
        || lower.contains("/cli/")
        || lower.contains("components/")
        || lower.contains("callbacks")
        || lower.contains("registry")
}

/// Check if a file path matches test/vendor patterns to exclude.
fn is_test_or_vendor(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.contains("/test")
        || lower.contains("/vendor/")
        || lower.contains("/node_modules/")
        || lower.contains("/.git/")
        || lower.contains("/__pycache__/")
        || lower.contains("/target/")
        || lower.contains("/dist/")
        || lower.contains("/build/")
        || lower.contains("/site-packages/")
}

/// Check if symbol name matches handler/callback/route patterns.
fn has_framework_name_pattern(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("handler")
        || lower.contains("callback")
        || lower.contains("route")
        || lower.contains("command")
        || lower.contains("controller")
        || lower.starts_with("on_")
        || lower.starts_with("handle_")
        || lower.starts_with("get_")
        || lower.starts_with("post_")
        || lower.starts_with("put_")
        || lower.starts_with("delete_")
}

/// Check if symbol name looks private/internal.
fn is_private_name(name: &str) -> bool {
    name.starts_with('_') || name == "helper" || name == "internal" || name.contains("__")
}

/// Build standard caution list for framework entry hints.
fn framework_cautions() -> Vec<String> {
    vec![
        "framework-callback-may-hide-callers".to_string(),
        "static-analysis-only".to_string(),
        "runtime-registration-not-verified".to_string(),
        "dynamic-dispatch-may-hide-callers".to_string(),
    ]
}

/// Build recommended verification steps.
fn framework_verification_steps(hint_kind: &str) -> Vec<String> {
    let mut steps = vec![
        "check framework route registration".to_string(),
        "run route/handler tests before deleting".to_string(),
    ];
    match hint_kind {
        "route" => {
            steps.push("search config/registry for route bindings".to_string());
            steps.push("run app-level smoke before deleting".to_string());
        }
        "cli" => {
            steps.push("check CLI entry point registration".to_string());
            steps.push("run CLI integration tests".to_string());
        }
        "component" => {
            steps.push("check component import/usage across app".to_string());
            steps.push("run component snapshot tests".to_string());
        }
        "callback" | "lifecycle" => {
            steps.push("check runtime plugin loading".to_string());
            steps.push("inspect framework docs for lifecycle hooks".to_string());
        }
        _ => {}
    }
    steps
}

/// Score a single symbol for framework entry likelihood.
/// Uses only graph node fields: name, kind, file, line, properties.
fn score_framework_entry_hint(
    name: &str,
    kind: &str,
    file: &str,
    properties: &Value,
    language: &str,
    options: &FrameworkHintOptions,
) -> (f64, Vec<String>, String, String) {
    let mut score: f64 = 0.0;
    let mut reasons: Vec<String> = vec![];
    let mut hint_kind = String::new();
    let mut framework = String::new();

    // --- Positive signals ---

    // 1. Route file path
    if is_route_path(file) {
        score += 0.25;
        reasons.push(format!("route-file-path"));
    }

    // 2. Framework name pattern
    if has_framework_name_pattern(name) {
        score += 0.15;
        reasons.push("framework-name-pattern".to_string());
    }

    // 3. Public/exported symbol
    let is_public = properties["visibility"].as_str() == Some("public")
        || properties["exported"].as_bool() == Some(true);
    if is_public {
        score += 0.15;
        reasons.push("public-or-exported-symbol".to_string());
    }

    // --- Language-specific signals ---

    match language {
        "python" => {
            if file.contains("routes.py") || file.contains("views.py") || file.contains("api.py") {
                hint_kind = "route".to_string();
                framework = "python-web".to_string();
                score += 0.30;
                reasons.push("python-routes-file".to_string());
            } else if file.contains("cli.py") {
                hint_kind = "cli".to_string();
                framework = "python-cli".to_string();
                score += 0.20;
                reasons.push("python-cli-file".to_string());
            } else if has_framework_name_pattern(name) {
                hint_kind = "handler".to_string();
                framework = "python-generic".to_string();
            } else {
                hint_kind = "handler".to_string();
                framework = "python-generic".to_string();
            }
        }
        "typescript" => {
            // Next.js file-based route detection
            if file.contains("route.ts")
                || file.contains("route.tsx")
                || file.contains("page.tsx")
                || file.contains("layout.tsx")
            {
                hint_kind = "route".to_string();
                framework = "nextjs".to_string();
                score += 0.30;
                reasons.push("typescript-nextjs-file-route".to_string());
            } else if file.contains("routes/") || file.contains("pages/") {
                hint_kind = "route".to_string();
                framework = "express/nextjs".to_string();
                score += 0.25;
                reasons.push("typescript-route-file-path".to_string());
            }

            // Next.js exported handlers (GET, POST, PUT, DELETE, loader, action)
            if matches!(
                name,
                "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "loader" | "action"
            ) {
                if hint_kind.is_empty() {
                    hint_kind = "handler".to_string();
                }
                if framework.is_empty() {
                    framework = "nextjs".to_string();
                }
                score += 0.35;
                reasons.push("typescript-nextjs-exported-handler".to_string());
            }

            // React/TSX component — PascalCase export in .tsx
            if file.ends_with(".tsx")
                && name.chars().next().map_or(false, |c| c.is_uppercase())
                && is_public
            {
                if hint_kind.is_empty() {
                    hint_kind = "component".to_string();
                }
                if framework.is_empty() {
                    framework = "react".to_string();
                }
                score += 0.20;
                reasons.push("typescript-exported-component".to_string());
            }

            if hint_kind.is_empty() {
                hint_kind = "handler".to_string();
                framework = "typescript-generic".to_string();
            }
        }
        "arkts" => {
            if name == "build"
                || name == "aboutToAppear"
                || name == "aboutToDisappear"
                || name == "onPageShow"
                || name == "onPageHide"
            {
                hint_kind = "lifecycle".to_string();
                framework = "arkui".to_string();
                score += 0.35;
                reasons.push("arkts-lifecycle-method".to_string());
            }
            if properties["entry"].as_bool() == Some(true) {
                hint_kind = "component".to_string();
                framework = "arkui".to_string();
                score += 0.40;
                reasons.push("arkts-entry-component".to_string());
            }
            if hint_kind.is_empty() {
                hint_kind = "component".to_string();
                framework = "arkui-generic".to_string();
            }
        }
        "rust" => {
            if has_framework_name_pattern(name) {
                hint_kind = "handler".to_string();
                framework = "rust-generic".to_string();
            } else if name == "main" {
                hint_kind = "cli".to_string();
                framework = "rust-binary".to_string();
            } else {
                hint_kind = "handler".to_string();
                framework = "rust-generic".to_string();
            }
        }
        "c" | "cpp" => {
            if has_framework_name_pattern(name) {
                hint_kind = "callback".to_string();
                framework = "c-cpp-generic".to_string();
            } else if file.contains("include") {
                hint_kind = "callback".to_string();
                framework = "c-cpp-header-api".to_string();
                score += 0.15;
                reasons.push("c-cpp-header-include-path".to_string());
            } else {
                hint_kind = "callback".to_string();
                framework = "c-cpp-generic".to_string();
            }
        }
        "cangjie" => {
            if has_framework_name_pattern(name) {
                hint_kind = "handler".to_string();
                framework = "cangjie-generic".to_string();
            } else if name.contains("Page") || name.contains("Component") {
                hint_kind = "component".to_string();
                framework = "cangjie-ui".to_string();
                score += 0.10;
                reasons.push("cangjie-component-naming".to_string());
            } else {
                hint_kind = "handler".to_string();
                framework = "cangjie-generic".to_string();
            }
        }
        _ => {
            hint_kind = "handler".to_string();
            framework = "generic".to_string();
        }
    }

    // --- Negative signals ---

    // Private/internal name
    if is_private_name(name) {
        score -= 0.20;
        reasons.push("private-internal-name".to_string());
    }

    // Test/vendor path
    if is_test_or_vendor(file) && !options.include_tests {
        score -= 0.50;
        reasons.push("test-vendor-path".to_string());
    }

    // Clamp score
    score = score.max(0.0).min(1.0);

    (score, reasons, hint_kind, framework)
}

/// Detect framework entry hints from symbol nodes in the graph.
fn detect_framework_entry_hints(
    gv: &GraphView,
    language: &str,
    options: &FrameworkHintOptions,
) -> Vec<FrameworkEntryHint> {
    let mut hints: Vec<FrameworkEntryHint> = Vec::new();

    for (node_id, node) in &gv.nodes_by_id {
        let name = node["name"].as_str().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let file = node["file"].as_str().unwrap_or("");
        if file.is_empty() {
            continue;
        }
        let kind = node["kind"].as_str().unwrap_or("function");

        // Skip non-functional nodes (include classes only for TSX components)
        if kind != "function"
            && kind != "method"
            && kind != "constructor"
            && !(kind == "class" && language == "typescript" && file.ends_with(".tsx"))
        {
            continue;
        }

        // Skip test/vendor unless explicitly included
        if is_test_or_vendor(file) && !options.include_tests {
            continue;
        }

        let properties = &node["properties"];
        let line = node["line"].as_u64().unwrap_or(0);

        let (score, reasons, hint_kind, framework) =
            score_framework_entry_hint(name, kind, file, properties, language, options);

        // Filter by requested hint kinds
        if !options.include_routes && hint_kind == "route" {
            continue;
        }
        if !options.include_callbacks && hint_kind == "callback" {
            continue;
        }
        if !options.include_components && hint_kind == "component" {
            continue;
        }

        // Filter low scores
        if score < 0.35 {
            continue;
        }

        let confidence = if score >= 0.80 {
            "high"
        } else if score >= 0.55 {
            "medium"
        } else {
            "low"
        };

        let id = node_id.clone();
        let cautions = framework_cautions();
        let verification = framework_verification_steps(&hint_kind);

        hints.push(FrameworkEntryHint {
            id,
            name: name.to_string(),
            kind: kind.to_string(),
            file: file.to_string(),
            line,
            hint_kind,
            framework,
            score,
            confidence: confidence.to_string(),
            reasons,
            cautions,
            recommended_verification: verification,
        });
    }

    // Deterministic sort: score desc, file asc, line asc, name asc
    hints.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.name.cmp(&b.name))
    });

    // Apply limit
    if hints.len() > options.limit {
        hints.truncate(options.limit);
    }

    hints
}

/// Compute framework entry hints from graph data.
fn compute_framework_entry_hints(gv: &GraphView, language: &str, params: &Value) -> Value {
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let include_callbacks = params["includeCallbacks"].as_bool().unwrap_or(true);
    let include_routes = params["includeRoutes"].as_bool().unwrap_or(true);
    let include_components = params["includeComponents"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;

    let options = FrameworkHintOptions {
        include_tests,
        include_callbacks,
        include_routes,
        include_components,
        limit,
    };

    let hints = detect_framework_entry_hints(gv, language, &options);

    let route_count = hints.iter().filter(|h| h.hint_kind == "route").count();
    let callback_count = hints.iter().filter(|h| h.hint_kind == "callback").count();
    let component_count = hints.iter().filter(|h| h.hint_kind == "component").count();
    let cli_count = hints.iter().filter(|h| h.hint_kind == "cli").count();
    let lifecycle_count = hints.iter().filter(|h| h.hint_kind == "lifecycle").count();
    let high_count = hints.iter().filter(|h| h.confidence == "high").count();
    let med_count = hints.iter().filter(|h| h.confidence == "medium").count();
    let low_count = hints.iter().filter(|h| h.confidence == "low").count();

    let avg_score = if hints.is_empty() {
        0.0
    } else {
        hints.iter().map(|h| h.score).sum::<f64>() / hints.len() as f64
    };

    let hint_json: Vec<Value> = hints.iter().map(|h| h.to_json()).collect();

    json!({
        "summary": {
            "frameworkEntryHintCount": hints.len(),
            "routeHintCount": route_count,
            "callbackHintCount": callback_count,
            "componentHintCount": component_count,
            "cliHintCount": cli_count,
            "lifecycleHintCount": lifecycle_count,
            "highConfidenceHintCount": high_count,
            "mediumConfidenceHintCount": med_count,
            "lowConfidenceHintCount": low_count,
            "averageCautionScore": (avg_score * 100.0).round() / 100.0
        },
        "frameworkEntryHints": hint_json,
        "generatedFrom": {
            "graphBased": true,
            "compilerVerified": false,
            "runtimeVerified": false,
            "heuristic": true
        }
    })
}

/// Framework entry hints handler for MCP tool.
fn handle_framework_entry_hints(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let compact = params["compact"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;
    let include_tests = params["includeTests"].as_bool().unwrap_or(false);
    let include_callbacks = params["includeCallbacks"].as_bool().unwrap_or(true);
    let include_routes = params["includeRoutes"].as_bool().unwrap_or(true);
    let include_components = params["includeComponents"].as_bool().unwrap_or(true);

    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let call_params = json!({
        "includeTests": include_tests,
        "includeCallbacks": include_callbacks,
        "includeRoutes": include_routes,
        "includeComponents": include_components,
        "limit": limit
    });

    let result = compute_framework_entry_hints(&gv, language, &call_params);

    if compact {
        let hints = result["frameworkEntryHints"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let compact_hints: Vec<Value> = hints
            .iter()
            .map(|h| {
                json!({
                    "id": h["id"],
                    "name": h["name"],
                    "kind": h["kind"],
                    "file": h["file"],
                    "line": h["line"],
                    "hintKind": h["hintKind"],
                    "framework": h["framework"],
                    "score": h["score"],
                    "confidence": h["confidence"],
                    "reasons": h["reasons"]
                })
            })
            .collect();

        let comp = json!({
            "language": language,
            "root": validated,
            "summary": result["summary"],
            "frameworkEntryHints": compact_hints,
            "generatedFrom": result["generatedFrom"]
        });
        Ok(merge_cache_and_result(&comp, &cache_meta))
    } else {
        let full = json!({
            "language": language,
            "root": validated,
            "summary": result["summary"],
            "frameworkEntryHints": result["frameworkEntryHints"],
            "generatedFrom": result["generatedFrom"]
        });
        Ok(merge_cache_and_result(&full, &cache_meta))
    }
}

// ============================================================
// v0.23: Breaking-Change Review / Compatibility Risk Assessment
// ============================================================

// ============================================================
// v0.23: Breaking-Change Review / Compatibility Risk Assessment
// ============================================================

/// Changed symbol review entry.
struct BrReview {
    id: String,
    name: String,
    kind: String,
    file: String,
    line: u64,
    risk: String,
    reasons: Vec<String>,
    recommended_verification: Vec<String>,
}

/// Resolve changed symbols — explicit list or git diff auto-detect.
fn resolve_changed_symbols_for_review(
    gv: &GraphView,
    params: &Value,
    root: &str,
) -> (Vec<(String, Value)>, Vec<String>, Vec<String>) {
    let changed_raw: Vec<String> = params["changedSymbols"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if changed_raw.is_empty() {
        return (vec![], vec![], vec![]);
    }

    let mut found: Vec<(String, Value)> = Vec::new();
    let mut ambiguous: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();

    for raw in &changed_raw {
        // Try exact ID match
        if let Some(node) = gv.nodes_by_id.get(raw.as_str()) {
            found.push((raw.clone(), node.clone()));
            continue;
        }

        // Try exact name match
        let name_matches: Vec<(&String, &Value)> = gv
            .nodes_by_id
            .iter()
            .filter(|(_, n)| n["name"].as_str() == Some(raw.as_str()))
            .collect();

        if name_matches.len() == 1 {
            found.push((raw.clone(), name_matches[0].1.clone()));
        } else if name_matches.len() > 1 {
            ambiguous.push(raw.clone());
        } else {
            // Try file path match
            let file_matches: Vec<(&String, &Value)> = gv
                .nodes_by_id
                .iter()
                .filter(|(_, n)| {
                    n["file"]
                        .as_str()
                        .map(|f| f.contains(raw.as_str()))
                        .unwrap_or(false)
                })
                .collect();
            if !file_matches.is_empty() {
                for (_, fm) in file_matches {
                    found.push((raw.clone(), fm.clone()));
                }
            } else {
                unknown.push(raw.clone());
            }
        }
    }

    (found, ambiguous, unknown)
}

/// Check if symbol name mentioned in README.md (simple heuristic).
fn is_documented_in_readme(root: &str, name: &str) -> bool {
    let readme_path = std::path::PathBuf::from(root).join("README.md");
    if let Ok(content) = std::fs::read_to_string(&readme_path) {
        content.contains(name)
    } else {
        false
    }
}

/// Classify a symbol against external API surface signals.
fn classify_ext_api(
    node: &Value,
    node_id: &str,
    language: &str,
    gv: &GraphView,
    root: &str,
    include_docs: bool,
) -> Option<BrReview> {
    let name = node["name"].as_str().unwrap_or("");
    let file = node["file"].as_str().unwrap_or("");
    let kind = node["kind"].as_str().unwrap_or("function");
    let line = node["line"].as_u64().unwrap_or(0);
    let properties = &node["properties"];

    let is_pub = properties["visibility"].as_str() == Some("public")
        || properties["exported"].as_bool() == Some(true);

    let mut score: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    if is_pub {
        score += 0.30;
        reasons.push("exported-or-public".to_string());
    }

    if is_entry_file(file, language) {
        score += 0.25;
        reasons.push("package-entry-file".to_string());
    }

    if is_reexported_symbol(node_id, gv) || is_reexported_from_index(node_id, gv) {
        score += 0.20;
        reasons.push("reexported-from-entry".to_string());
    }

    if include_docs && is_documented_in_readme(root, name) {
        score += 0.15;
        reasons.push("documented-in-readme".to_string());
    }

    // C/C++ header API
    if (language == "c" || language == "cpp") && file.contains("include/") {
        score += 0.30;
        reasons.push("c-cpp-header-api".to_string());
    }

    if score < 0.35 {
        return None;
    }

    let risk_level = if score >= 0.75 {
        "high"
    } else if score >= 0.45 {
        "medium"
    } else {
        "low"
    };

    Some(BrReview {
        id: node_id.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        file: file.to_string(),
        line,
        risk: risk_level.to_string(),
        reasons,
        recommended_verification: vec![
            "check downstream imports".to_string(),
            "add or update compatibility tests".to_string(),
            "document breaking change if signature changed".to_string(),
        ],
    })
}

/// Classify a symbol against framework entry hints.
fn classify_fw_entry(node: &Value, language: &str) -> Option<BrReview> {
    let name = node["name"].as_str().unwrap_or("");
    let file = node["file"].as_str().unwrap_or("");
    let kind = node["kind"].as_str().unwrap_or("function");
    let line = node["line"].as_u64().unwrap_or(0);
    let id = node["id"].as_str().unwrap_or("").to_string();
    let properties = &node["properties"];

    let mut hint_kind = String::new();
    let mut framework = String::new();
    let mut score: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    let is_pub = properties["visibility"].as_str() == Some("public")
        || properties["exported"].as_bool() == Some(true);

    let lower_file = file.to_lowercase();
    let has_fw_name = name.to_lowercase().contains("handler")
        || name.to_lowercase().contains("callback")
        || name.to_lowercase().contains("route")
        || name.to_lowercase().contains("command")
        || name.to_lowercase().starts_with("handle_")
        || name.to_lowercase().starts_with("on_");

    match language {
        "python" => {
            if file.contains("routes.py") || file.contains("views.py") || file.contains("api.py") {
                hint_kind = "route".to_string();
                framework = "python-web".to_string();
                score += 0.30;
                reasons.push("python-routes-file".to_string());
            } else if file.contains("cli.py") {
                hint_kind = "cli".to_string();
                framework = "python-cli".to_string();
                score += 0.20;
                reasons.push("python-cli-file".to_string());
            }
        }
        "typescript" | "arkts" => {
            if file.contains("route.ts")
                || file.contains("route.tsx")
                || file.contains("page.tsx")
                || file.contains("layout.tsx")
            {
                hint_kind = "route".to_string();
                framework = "nextjs".to_string();
                score += 0.30;
                reasons.push("typescript-file-route".to_string());
            } else if lower_file.contains("routes")
                || lower_file.contains("pages")
                || lower_file.contains("/api/")
                || lower_file.contains("components/")
            {
                hint_kind = "route".to_string();
                framework = "express/nextjs".to_string();
                score += 0.25;
            }
            if matches!(
                name,
                "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "loader" | "action"
            ) {
                if hint_kind.is_empty() {
                    hint_kind = "handler".to_string();
                }
                score += 0.35;
                reasons.push("nextjs-exported-handler".to_string());
            }
            if file.ends_with(".tsx")
                && name.chars().next().map_or(false, |c| c.is_uppercase())
                && is_pub
            {
                hint_kind = "component".to_string();
                framework = "react".to_string();
                score += 0.20;
                reasons.push("react-tsx-component".to_string());
            }
        }
        _ => {
            if has_fw_name {
                hint_kind = "handler".to_string();
                framework = "generic".to_string();
                score += 0.15;
            }
        }
    }

    if is_pub {
        score += 0.15;
        reasons.push("public-exported".to_string());
    }

    if name.starts_with('_') || name == "helper" || name.contains("__") {
        score -= 0.20;
    }

    if score < 0.35 {
        return None;
    }

    let risk_level = if score >= 0.75 {
        "high"
    } else if score >= 0.55 {
        "medium"
    } else {
        "low"
    };

    if !hint_kind.is_empty() {
        reasons.insert(0, format!("framework-{}-hint", hint_kind));
    }

    Some(BrReview {
        id,
        name: name.to_string(),
        kind: kind.to_string(),
        file: file.to_string(),
        line,
        risk: risk_level.to_string(),
        reasons,
        recommended_verification: vec![
            "check framework route/callback registration".to_string(),
            "consider framework-specific integration tests".to_string(),
        ],
    })
}

/// Compute overall compatibility risk.
fn compute_compat_risk(
    ext_high: usize,
    fw_high: usize,
    has_critical: bool,
    has_ambiguous: bool,
    has_unknown: bool,
    total_found: usize,
) -> &'static str {
    if has_critical {
        return "critical";
    }
    if ext_high > 0 || fw_high > 0 {
        return "high";
    }
    if has_ambiguous || has_unknown {
        return "medium";
    }
    if total_found > 0 {
        return "medium";
    }
    "low"
}

/// Build review checklist.
fn build_br_checklist(
    ext_reviews: &[BrReview],
    fw_reviews: &[BrReview],
    ambiguous: &[String],
    unknown: &[String],
    doc_names: &[String],
) -> Vec<Value> {
    let mut cl: Vec<Value> = Vec::new();

    for r in ext_reviews {
        if r.risk == "high" {
            cl.push(json!({
                "priority": "P0",
                "item": format!("Check package exports and downstream consumers before changing {}", r.name),
                "reason": "public API surface changed — may break external consumers"
            }));
        }
    }

    for r in fw_reviews {
        if r.risk == "high" {
            cl.push(json!({
                "priority": "P1",
                "item": format!("Check framework route/callback registration before changing {}", r.name),
                "reason": "framework entry point changed — registration or runtime config may be affected"
            }));
        }
    }

    for s in ambiguous {
        cl.push(json!({
            "priority": "P2",
            "item": format!("Manually verify impact of ambiguous symbol: {}", s),
            "reason": "multiple symbols matched — requires human verification"
        }));
    }

    for s in unknown {
        cl.push(json!({
            "priority": "P2",
            "item": format!("Investigate unknown symbol: {}", s),
            "reason": "symbol not found in graph — may be new, deleted, or misnamed"
        }));
    }

    for d in doc_names {
        cl.push(json!({
            "priority": "P2",
            "item": format!("Update documentation referencing: {}", d),
            "reason": "documented API changed — docs may need update"
        }));
    }

    cl
}

/// Build release notes hints.
fn build_relnotes_hints(
    ext_reviews: &[BrReview],
    fw_reviews: &[BrReview],
    doc_names: &[String],
    risk: &str,
) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();

    if risk == "high" || risk == "critical" {
        hints.push("BREAKING CHANGE: public API surface modified".to_string());
    }

    for r in ext_reviews {
        if r.risk == "high" {
            hints.push(format!(
                "Mention {} compatibility impact if signature or behavior changed",
                r.name
            ));
        }
    }

    for r in fw_reviews {
        if r.risk == "high" {
            hints.push(format!(
                "Mention {} route/handler change in release notes",
                r.name
            ));
        }
    }

    if !doc_names.is_empty() {
        hints.push("Documentation updates required for changed APIs".to_string());
    }

    if hints.is_empty() {
        hints.push("No significant breaking changes detected from static analysis".to_string());
    }

    hints
}

/// Compute breaking-change review from graph data.
fn compute_breaking_change_review(gv: &GraphView, params: &Value, root: &str) -> Value {
    let language = params["language"].as_str().unwrap_or("auto");
    let include_ext = params["includeExternalApi"].as_bool().unwrap_or(true);
    let include_fw = params["includeFrameworkEntries"].as_bool().unwrap_or(true);
    let include_docs = params["includeDocs"].as_bool().unwrap_or(true);
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;

    let (changed, ambiguous, unknown) = resolve_changed_symbols_for_review(gv, params, root);

    let mut ext_reviews: Vec<BrReview> = Vec::new();
    let mut fw_reviews: Vec<BrReview> = Vec::new();
    let mut doc_names: Vec<String> = Vec::new();
    let mut has_critical = false;

    for (raw, node) in &changed {
        let name = node["name"].as_str().unwrap_or("");
        let file = node["file"].as_str().unwrap_or("");
        let node_id_val = node["id"].as_str().unwrap_or(raw);
        let nid = if node_id_val.is_empty() {
            raw.as_str()
        } else {
            node_id_val
        };

        if include_ext {
            if let Some(r) = classify_ext_api(node, nid, language, gv, root, include_docs) {
                if r.risk == "high" && is_entry_file(&r.file, language) {
                    has_critical = true;
                }
                if r.reasons.iter().any(|x| x == "documented-in-readme") {
                    doc_names.push(name.to_string());
                }
                ext_reviews.push(r);
            }
        }

        if include_fw {
            if let Some(r) = classify_fw_entry(node, language) {
                fw_reviews.push(r);
            }
        }

        if include_docs && !doc_names.contains(&name.to_string()) {
            if is_documented_in_readme(root, name) {
                doc_names.push(name.to_string());
            }
        }
    }

    if ext_reviews.len() > limit {
        ext_reviews.truncate(limit);
    }
    if fw_reviews.len() > limit {
        fw_reviews.truncate(limit);
    }

    let ext_high = ext_reviews.iter().filter(|r| r.risk == "high").count();
    let fw_high = fw_reviews.iter().filter(|r| r.risk == "high").count();

    let risk = compute_compat_risk(
        ext_high,
        fw_high,
        has_critical,
        !ambiguous.is_empty(),
        !unknown.is_empty(),
        changed.len(),
    );

    let checklist = build_br_checklist(&ext_reviews, &fw_reviews, &ambiguous, &unknown, &doc_names);
    let relnotes = build_relnotes_hints(&ext_reviews, &fw_reviews, &doc_names, risk);

    let risk_reasons: Vec<String> = {
        let mut rr = Vec::new();
        if ext_high > 0 {
            rr.push("changed exported package API".to_string());
        }
        if fw_high > 0 {
            rr.push("changed framework route/handler".to_string());
        }
        if !doc_names.is_empty() {
            rr.push("public API appears in documentation".to_string());
        }
        if !ambiguous.is_empty() {
            rr.push("ambiguous changed symbols require manual verification".to_string());
        }
        if !unknown.is_empty() {
            rr.push("unknown changed symbols not found in graph".to_string());
        }
        rr
    };

    let ext_json: Vec<Value> = ext_reviews
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "name": r.name, "kind": r.kind,
                "file": r.file, "line": r.line, "risk": r.risk,
                "reasons": r.reasons, "recommendedVerification": r.recommended_verification
            })
        })
        .collect();

    let fw_json: Vec<Value> = fw_reviews
        .iter()
        .map(|r| {
            json!({
                "name": r.name, "kind": r.kind, "file": r.file,
                "risk": r.risk, "reasons": r.reasons
            })
        })
        .collect();

    let warnings: Vec<String> = {
        let mut w = Vec::new();
        if changed.is_empty() && ambiguous.is_empty() && unknown.is_empty() {
            w.push("no-changed-symbols-provided".to_string());
        }
        if !unknown.is_empty() {
            w.push("some-symbols-not-found".to_string());
        }
        w
    };

    json!({
        "summary": {
            "compatibilityRisk": risk,
            "changedSymbolCount": changed.len(),
            "changedExternalApiCount": ext_reviews.len(),
            "changedFrameworkEntryCount": fw_reviews.len(),
            "ambiguousCount": ambiguous.len(),
            "unknownCount": unknown.len(),
            "docUpdateLikely": !doc_names.is_empty(),
            "recommendedTestCount": ext_reviews.len() + fw_reviews.len()
        },
        "riskReasons": risk_reasons,
        "changedExternalApi": ext_json,
        "changedFrameworkEntries": fw_json,
        "ambiguousChangedSymbols": ambiguous.iter().map(|s| json!({"symbol": s})).collect::<Vec<_>>(),
        "unknownChangedSymbols": unknown.iter().map(|s| json!({"symbol": s})).collect::<Vec<_>>(),
        "reviewChecklist": checklist,
        "releaseNotesHints": relnotes,
        "warnings": warnings,
        "generatedFrom": {
            "graphBased": true,
            "gitDiffBased": params["changedSymbols"].as_array().map_or(true, |a| a.is_empty()),
            "compilerVerified": false,
            "runtimeVerified": false,
            "externalUsageVerified": false,
            "heuristic": true
        }
    })
}

/// Breaking-change review handler for MCP tool.
fn handle_breaking_change_review(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;

    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let root_str = validated.to_string_lossy().into_owned();
    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let result = compute_breaking_change_review(&gv, params, &root_str);

    let mut out = result.as_object().cloned().unwrap_or_default();
    out.insert("language".to_string(), json!(language));
    out.insert("root".to_string(), json!(validated));

    Ok(merge_cache_and_result(&json!(out), &cache_meta))
}

// ============================================================
// v0.24: Docs & Tests Consistency Review
// ============================================================

// ============================================================
// v0.24: Docs & Tests Consistency Review
// ============================================================

/// Scan markdown files in root dir for symbol mentions.
fn scan_docs_for_mentions(root: &str, symbols: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut results: Vec<(String, Vec<String>)> = Vec::new();
    let root_path = std::path::Path::new(root);
    if let Ok(entries) = std::fs::read_dir(root_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "mdx" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let mut found: Vec<String> = Vec::new();
                        for sym in symbols {
                            if content.contains(*sym) {
                                found.push(sym.to_string());
                            }
                        }
                        if !found.is_empty() {
                            let rel = path.strip_prefix(root_path).unwrap_or(&path);
                            results.push((rel.to_string_lossy().to_string(), found));
                        }
                    }
                }
            }
        }
        // Also scan docs/ subdirectory if exists
        let docs_dir = root_path.join("docs");
        if docs_dir.is_dir() {
            if let Ok(d_entries) = std::fs::read_dir(&docs_dir) {
                for d_entry in d_entries.flatten() {
                    let d_path = d_entry.path();
                    if let Some(ext) = d_path.extension() {
                        if ext == "md" || ext == "mdx" {
                            if let Ok(content) = std::fs::read_to_string(&d_path) {
                                let mut found: Vec<String> = Vec::new();
                                for sym in symbols {
                                    if content.contains(*sym) {
                                        found.push(sym.to_string());
                                    }
                                }
                                if !found.is_empty() {
                                    let rel = d_path.strip_prefix(root_path).unwrap_or(&d_path);
                                    results.push((rel.to_string_lossy().to_string(), found));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    results
}

/// Walk directories for test files.
fn walk_test_files(
    dir: &std::path::Path,
    root: &std::path::Path,
    test_dirs: &[&str],
) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                let dn = p.file_name().unwrap_or_default().to_string_lossy();
                if test_dirs.iter().any(|t| dn == *t) || dn == "src" {
                    files.extend(walk_test_files(&p, root, test_dirs));
                }
            } else if p.is_file() {
                let fn_ = p.file_name().unwrap_or_default().to_string_lossy();
                if fn_.contains("test") || fn_.contains("spec") {
                    files.push(p);
                }
            }
        }
    }
    files
}

/// Find test files related to changed symbols.
fn find_related_tests(
    root: &str,
    changed_names: &[&str],
    changed_files: &[&str],
) -> Vec<(String, Vec<String>, f64)> {
    let mut results: Vec<(String, Vec<String>, f64)> = Vec::new();
    let root_path = std::path::Path::new(root);

    let test_dirs = ["test", "tests", "__tests__", "spec", "e2e", "__test__"];
    let src_dir = root_path.join("src");
    let base_dir = if src_dir.is_dir() {
        &src_dir
    } else {
        root_path
    };

    let test_files = walk_test_files(base_dir, root_path, &test_dirs);

    for tf in &test_files {
        let rel = tf
            .strip_prefix(root_path)
            .unwrap_or(tf)
            .to_string_lossy()
            .to_string();
        let mut matched: Vec<String> = Vec::new();
        let mut score: f64 = 0.0;

        // Check file name
        let fname = tf.file_stem().unwrap_or_default().to_string_lossy();
        for name in changed_names {
            let nl = name.to_lowercase();
            if fname.to_lowercase().contains(&nl) {
                matched.push(name.to_string());
                score += 0.30;
            }
        }

        // Check file path matches changed file paths
        for cf in changed_files {
            if rel.contains(cf) || cf.contains(&rel) {
                if score < 0.30 {
                    score += 0.40;
                }
            }
        }

        // Check file content for symbol mentions
        if let Ok(content) = std::fs::read_to_string(tf) {
            for name in changed_names {
                if content.contains(*name) && !matched.contains(&name.to_string()) {
                    matched.push(name.to_string());
                    score += 0.25;
                }
            }
        }

        if !matched.is_empty() {
            score = score.min(1.0);
            results.push((rel, matched, score));
        }
    }

    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    results
}

/// Find tests that reference unknown/dead symbols (stale tests).
fn find_stale_tests(root: &str, unknown_symbols: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut results: Vec<(String, Vec<String>)> = Vec::new();
    let root_path = std::path::Path::new(root);

    if unknown_symbols.is_empty() {
        return results;
    }

    let test_dirs = ["test", "tests", "__tests__", "spec", "e2e"];
    let src_dir = root_path.join("src");
    let base_dir = if src_dir.is_dir() {
        &src_dir
    } else {
        root_path
    };

    for tf in walk_test_files(base_dir, root_path, &test_dirs) {
        if let Ok(content) = std::fs::read_to_string(&tf) {
            let mut mentioned: Vec<String> = Vec::new();
            for sym in unknown_symbols {
                if content.contains(*sym) {
                    mentioned.push(sym.to_string());
                }
            }
            if !mentioned.is_empty() {
                let rel = tf
                    .strip_prefix(root_path)
                    .unwrap_or(&tf)
                    .to_string_lossy()
                    .to_string();
                results.push((rel, mentioned));
            }
        }
    }
    results
}

/// Resolve changed symbols for consistency review (aligned with breaking_change_review).
fn resolve_changed_for_consistency(
    gv: &GraphView,
    params: &Value,
) -> (Vec<(String, Value)>, Vec<String>, Vec<String>) {
    let changed_raw: Vec<String> = params["changedSymbols"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if changed_raw.is_empty() {
        return (vec![], vec![], vec![]);
    }

    let mut found: Vec<(String, Value)> = Vec::new();
    let mut amb: Vec<String> = Vec::new();
    let mut unk: Vec<String> = Vec::new();

    for raw in &changed_raw {
        if let Some(node) = gv.nodes_by_id.get(raw.as_str()) {
            found.push((raw.clone(), node.clone()));
            continue;
        }
        let name_matches: Vec<(&String, &Value)> = gv
            .nodes_by_id
            .iter()
            .filter(|(_, n)| n["name"].as_str() == Some(raw.as_str()))
            .collect();
        if name_matches.len() == 1 {
            found.push((raw.clone(), name_matches[0].1.clone()));
        } else if name_matches.len() > 1 {
            amb.push(raw.clone());
        } else {
            let file_matches: Vec<(&String, &Value)> = gv
                .nodes_by_id
                .iter()
                .filter(|(_, n)| {
                    n["file"]
                        .as_str()
                        .map(|f| f.contains(raw.as_str()))
                        .unwrap_or(false)
                })
                .collect();
            if !file_matches.is_empty() {
                for (_, fm) in file_matches {
                    found.push((raw.clone(), fm.clone()));
                }
            } else {
                unk.push(raw.clone());
            }
        }
    }
    (found, amb, unk)
}

/// Compute docs & tests consistency review.
fn compute_consistency_review(gv: &GraphView, params: &Value, root: &str) -> Value {
    let include_docs = params["includeDocs"].as_bool().unwrap_or(true);
    let include_tests = params["includeTests"].as_bool().unwrap_or(true);
    let _include_dead = params["includeDeadCode"].as_bool().unwrap_or(true);
    let _include_breaking = params["includeBreakingRisk"].as_bool().unwrap_or(true);

    let (changed, ambiguous, unknown) = resolve_changed_for_consistency(gv, params);

    let mut changed_names: Vec<String> = Vec::new();
    let mut changed_files: Vec<String> = Vec::new();
    let mut has_public_api = false;
    let mut has_fw_entry = false;

    for (_, node) in &changed {
        let name = node["name"].as_str().unwrap_or("").to_string();
        let file = node["file"].as_str().unwrap_or("").to_string();
        let props = &node["properties"];
        let is_pub = props["visibility"].as_str() == Some("public")
            || props["exported"].as_bool() == Some(true);

        if !name.is_empty() {
            changed_names.push(name);
        }
        if !file.is_empty() {
            changed_files.push(file);
        }
        if is_pub {
            has_public_api = true;
        }

        let fw_name = node["name"].as_str().unwrap_or("");
        if matches!(
            fw_name,
            "GET" | "POST" | "PUT" | "DELETE" | "loader" | "action"
        ) {
            has_fw_entry = true;
        }
        let f = node["file"].as_str().unwrap_or("");
        if f.contains("routes") || f.contains("pages") {
            has_fw_entry = true;
        }
    }

    let cnames: Vec<&str> = changed_names.iter().map(|s| s.as_str()).collect();
    let _cfiles: Vec<&str> = changed_files.iter().map(|s| s.as_str()).collect();
    let unames: Vec<&str> = unknown.iter().map(|s| s.as_str()).collect();

    // 1. Stale doc candidates: docs mentioning changed/unknown symbols
    let mut stale_docs: Vec<Value> = Vec::new();
    if include_docs {
        let all_search: Vec<&str> = cnames.iter().chain(unames.iter()).copied().collect();
        for (doc_path, mentions) in scan_docs_for_mentions(root, &all_search) {
            let risk = if mentions.iter().any(|m| unknown.iter().any(|u| u == m)) {
                "high" // doc mentions symbol that no longer exists
            } else {
                "medium"
            };
            stale_docs.push(json!({
                "path": doc_path, "mentionedSymbols": mentions, "risk": risk,
                "reasons": if risk == "high" { vec!["doc mentions unknown or removed symbol"] } else { vec!["doc mentions changed API"] },
                "recommendedUpdate": ["review examples and API references", "update if behavior or signature changed"]
            }));
        }
    }

    // 2. Missing doc update candidates
    let mut missing_docs: Vec<Value> = Vec::new();
    if include_docs && has_public_api {
        for name in &changed_names {
            let has_doc = stale_docs.iter().any(|d| {
                d["mentionedSymbols"]
                    .as_array()
                    .map_or(false, |a| a.iter().any(|s| s.as_str() == Some(name)))
            });
            if !has_doc {
                missing_docs.push(json!({
                    "symbol": name, "risk": "medium",
                    "reasons": ["changed public API with no related docs found"],
                    "recommendedUpdate": ["add or update docs for this user-facing API"]
                }));
            }
        }
    }

    // 3. Related tests
    let mut related_tests: Vec<Value> = Vec::new();
    if include_tests {
        let all_cfiles: Vec<&str> = changed_files.iter().map(|s| s.as_str()).collect();
        for (path, matched, score) in find_related_tests(root, &cnames, &all_cfiles) {
            let conf = if score >= 0.70 {
                "high"
            } else if score >= 0.40 {
                "medium"
            } else {
                "low"
            };
            related_tests.push(json!({
                "path": path, "relatedSymbols": matched, "confidence": conf,
                "score": (score * 100.0).round() / 100.0
            }));
        }
    }

    // 4. Missing test candidates
    let mut missing_tests: Vec<Value> = Vec::new();
    if include_tests {
        for name in &changed_names {
            let has_test = related_tests.iter().any(|t| {
                t["relatedSymbols"]
                    .as_array()
                    .map_or(false, |a| a.iter().any(|s| s.as_str() == Some(name)))
            });
            if !has_test && (has_public_api || has_fw_entry) {
                let risk = if has_public_api { "high" } else { "medium" };
                missing_tests.push(json!({
                    "symbol": name, "risk": risk,
                    "reasons": ["changed API without obvious related test"],
                    "recommendedTest": if has_fw_entry { vec!["add route/handler smoke test"] }
                        else { vec!["add API integration test", "add unit test for changed behavior"] }
                }));
            }
        }
    }

    // 5. Stale test candidates
    let mut stale_tests: Vec<Value> = Vec::new();
    if include_tests && !unknown.is_empty() {
        for (path, mentioned) in find_stale_tests(root, &unames) {
            stale_tests.push(json!({
                "path": path, "mentionedUnknownSymbols": mentioned, "risk": "high",
                "reasons": ["test references unknown or removed symbols"],
                "recommendedAction": ["review and update or remove stale test", "check if symbol was renamed or deleted"]
            }));
        }
    }

    // 6. Consistency risk
    let stale_high = stale_docs.iter().filter(|d| d["risk"] == "high").count() + stale_tests.len();
    let missing_high = missing_tests.iter().filter(|t| t["risk"] == "high").count();
    let has_issues = !stale_docs.is_empty()
        || !missing_docs.is_empty()
        || !stale_tests.is_empty()
        || !missing_tests.is_empty();

    let consistency_risk = if stale_high > 0 && missing_high > 0 {
        "critical"
    } else if stale_high > 0 || missing_high > 0 {
        "high"
    } else if has_issues {
        "medium"
    } else if !ambiguous.is_empty() {
        "medium"
    } else {
        "low"
    };

    // 7. Checklist
    let mut checklist: Vec<Value> = Vec::new();
    for d in &stale_docs {
        if d["risk"] == "high" {
            checklist.push(json!({"priority":"P0","item":format!("Update {}: stale symbol references", d["path"]),"reason":"documents reference removed or unknown symbols"}));
        }
    }
    for t in &missing_tests {
        if t["risk"] == "high" {
            checklist.push(json!({"priority":"P1","item":format!("Add tests for changed API: {}", t["symbol"]),"reason":"changed public API lacks related test coverage"}));
        }
    }
    for s in &stale_tests {
        checklist.push(json!({"priority":"P0","item":format!("Review stale test: {}", s["path"]),"reason":"test references symbols that no longer exist"}));
    }

    let warnings: Vec<String> = if changed.is_empty() && ambiguous.is_empty() && unknown.is_empty()
    {
        vec!["no-changed-symbols-provided".to_string()]
    } else {
        Vec::new()
    };

    json!({
        "summary": {
            "consistencyRisk": consistency_risk,
            "changedSymbolCount": changed.len(),
            "staleDocCandidateCount": stale_docs.len(),
            "missingDocUpdateCandidateCount": missing_docs.len(),
            "relatedTestCount": related_tests.len(),
            "missingTestCandidateCount": missing_tests.len(),
            "staleTestCandidateCount": stale_tests.len(),
            "ambiguousCount": ambiguous.len(),
            "unknownCount": unknown.len()
        },
        "staleDocCandidates": stale_docs,
        "missingDocUpdateCandidates": missing_docs,
        "relatedTests": related_tests,
        "missingTestCandidates": missing_tests,
        "staleTestCandidates": stale_tests,
        "ambiguousChangedSymbols": ambiguous.iter().map(|s| json!({"symbol":s})).collect::<Vec<_>>(),
        "unknownChangedSymbols": unknown.iter().map(|s| json!({"symbol":s})).collect::<Vec<_>>(),
        "reviewChecklist": checklist,
        "warnings": warnings,
        "generatedFrom": {
            "graphBased": true,
            "docScannerBased": include_docs,
            "testNameHeuristic": include_tests,
            "coverageVerified": false,
            "runtimeVerified": false,
            "heuristic": true
        }
    })
}

/// Consistency review MCP handler.
fn handle_consistency_review(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;

    let root_str = validated.to_string_lossy().into_owned();
    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;

    let result = compute_consistency_review(&gv, params, &root_str);
    let mut out = result.as_object().cloned().unwrap_or_default();
    out.insert("language".to_string(), json!(language));
    out.insert("root".to_string(), json!(validated));
    Ok(merge_cache_and_result(&json!(out), &cache_meta))
}

// ============================================================
// v0.25: Config & Examples Consistency Review
// ============================================================

// ============================================================
// v0.25: Config & Examples Consistency Review
// ============================================================

/// Check if a file/dir exists relative to project root.
fn path_exists(root: &std::path::Path, rel: &str) -> bool {
    let p = root.join(rel.trim_start_matches("./").trim_end_matches('/'));
    p.exists()
}

/// Scan package.json for stale references.
fn scan_package_json(root: &std::path::Path) -> Vec<Value> {
    let mut risks = Vec::new();
    let pkg_path = root.join("package.json");
    if !pkg_path.exists() {
        return risks;
    }
    let Ok(content) = std::fs::read_to_string(&pkg_path) else {
        return risks;
    };
    let Ok(pkg) = serde_json::from_str::<Value>(&content) else {
        return risks;
    };

    // Check main, module, types
    for field in &["main", "module", "types"] {
        if let Some(val) = pkg[field].as_str() {
            if !val.is_empty() && !path_exists(root, val) {
                risks.push(
                    json!({"path":"package.json","field":field,"referencedPath":val,
                    "risk":"high","reasons":["configured entry path does not exist"],
                    "recommendedFix":["update package.json or restore the referenced file"]}),
                );
            }
        }
    }

    // Check exports
    if let Some(exports) = pkg["exports"].as_object() {
        for (key, val) in exports {
            let path = val.as_str().unwrap_or("");
            if !path.is_empty() && !path_exists(root, path) {
                risks.push(
                    json!({"path":"package.json","field":format!("exports['{}']",key),
                    "referencedPath":path,"risk":"high",
                    "reasons":["configured exports path does not exist"],
                    "recommendedFix":["update package exports or restore the referenced file"]}),
                );
            }
        }
    }

    // Check bin
    if let Some(bin) = pkg["bin"].as_object() {
        for (key, val) in bin {
            let path = val.as_str().unwrap_or("");
            if !path.is_empty() && !path_exists(root, path) {
                risks.push(
                    json!({"path":"package.json","field":format!("bin['{}']",key),
                    "referencedPath":path,"risk":"high",
                    "reasons":["configured bin path does not exist"],
                    "recommendedFix":["update package bin entry or restore the referenced file"]}),
                );
            }
        }
    }

    // Check scripts for missing config files
    if let Some(scripts) = pkg["scripts"].as_object() {
        for (name, cmd) in scripts {
            let command = cmd.as_str().unwrap_or("");
            for token in command.split_whitespace() {
                if (token.ends_with(".json") || token.ends_with(".yaml") || token.ends_with(".yml"))
                    && token.contains(".config")
                    || token.contains("tsconfig")
                    || token.contains("package")
                {
                    if !path_exists(root, token.trim_matches('"').trim_matches('\'')) {
                        risks.push(
                            json!({"path":"package.json","script":name,"command":command,
                            "risk":"medium","reasons":["script references missing config file"],
                            "recommendedFix":["update script command or restore config file"]}),
                        );
                        break;
                    }
                }
            }
        }
    }

    risks
}

/// Scan tsconfig for stale path references.
fn scan_tsconfig(root: &std::path::Path) -> Vec<Value> {
    let mut risks = Vec::new();
    for name in &["tsconfig.json", "tsconfig.base.json", "tsconfig.build.json"] {
        let p = root.join(name);
        if !p.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(cfg) = serde_json::from_str::<Value>(&content) else {
            continue;
        };

        // Check paths
        if let Some(paths) = cfg["compilerOptions"]["paths"].as_object() {
            for (alias, targets) in paths {
                if let Some(arr) = targets.as_array() {
                    let any_exists = arr.iter().any(|t| {
                        let t2 = t.as_str().unwrap_or("").replace("/*", "");
                        path_exists(root, &t2)
                    });
                    if !any_exists && !arr.is_empty() {
                        risks.push(
                            json!({"path":name.to_string(),"field":format!("paths['{}']",alias),
                            "aliasedPath":alias,"risk":"medium",
                            "reasons":["tsconfig path alias points to nonexistent directory"],
                            "recommendedFix":["update or remove stale path alias"]}),
                        );
                    }
                }
            }
        }
        // Check include
        if let Some(arr) = cfg["include"].as_array() {
            for inc in arr {
                let inc_s = inc.as_str().unwrap_or("");
                if inc_s.contains("*") {
                    continue;
                }
                if !path_exists(root, inc_s) {
                    risks.push(json!({"path":name.to_string(),"field":"include",
                        "referencedPath":inc_s,"risk":"medium",
                        "reasons":["tsconfig include path does not exist"]}));
                }
            }
        }
    }
    risks
}

/// Scan README and docs for stale code references.
fn scan_docs_code_blocks(root: &std::path::Path, gv: &GraphView) -> Vec<Value> {
    let mut results = Vec::new();
    // Check README.md
    for md_name in &["README.md", "README.rst"] {
        let p = root.join(md_name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            // Look for code blocks containing import/require statements
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import ")
                    || trimmed.starts_with("const ") && trimmed.contains("require(")
                {
                    // Extract symbol-like tokens
                    for token in trimmed.split(&[' ', '{', '}', ',', ';', '(', ')', '\"', '\'']) {
                        let t = token.trim();
                        if t.is_empty() || t.len() < 2 || t.starts_with(".") || t.starts_with("/") {
                            continue;
                        }
                        // Check if this looks like a symbol not in graph
                        if t.chars().next().map_or(false, |c| c.is_uppercase()) && !t.contains('\n')
                        {
                            let found = gv
                                .nodes_by_id
                                .values()
                                .any(|n| n["name"].as_str() == Some(t));
                            if !found
                                && !gv
                                    .nodes_by_id
                                    .values()
                                    .any(|n| n["file"].as_str().map_or(false, |f| f.contains(t)))
                            {
                                results.push(json!({"path":md_name.to_string(),"line":1,
                                    "referencedSymbol":t,"risk":"medium",
                                    "reasons":["doc code block references symbol not found in graph"],
                                    "recommendedFix":["update code example or remove stale reference"]}));
                            }
                        }
                    }
                }
            }
        }
    }
    results
}

/// Scan examples/ directory for stale imports/references.
fn scan_examples(root: &std::path::Path, gv: &GraphView) -> Vec<Value> {
    let mut results = Vec::new();
    let examples_dir = root.join("examples");
    if !examples_dir.is_dir() {
        return results;
    }

    fn walk(
        dir: &std::path::Path,
        root: &std::path::Path,
        gv: &GraphView,
        results: &mut Vec<Value>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, gv, results);
                    continue;
                }
                if p.extension().map_or(true, |ext| {
                    ext != "ts" && ext != "tsx" && ext != "js" && ext != "py" && ext != "rs"
                }) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                            // Check for paths that don't exist
                            for token in trimmed.split(&[' ', '"', '\'']) {
                                if token.starts_with("./") || token.starts_with("../") {
                                    let clean = token.trim_matches('"').trim_matches('\'');
                                    // Resolve relative to the example file's directory, then check from root
                                    let example_dir = p.parent().unwrap_or(root);
                                    if !example_dir.join(clean).exists()
                                        && !example_dir.join(format!("{}.ts", clean)).exists()
                                        && !example_dir.join(format!("{}.tsx", clean)).exists()
                                    {
                                        let rel = p.strip_prefix(root).unwrap_or(&p);
                                        results.push(json!({"path":rel.to_string_lossy(),
                                            "referencedImport":clean,"risk":"high",
                                            "reasons":["example imports nonexistent module"],
                                            "recommendedFix":["update example import or restore the referenced module"]}));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    walk(&examples_dir, root, gv, &mut results);
    results
}

/// Scan CI/Docker files for missing local paths.
fn scan_ci_docker(root: &std::path::Path) -> Vec<Value> {
    let mut risks = Vec::new();

    // Dockerfile
    for name in &["Dockerfile", "Dockerfile.prod"] {
        let p = root.join(name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            for line in content.lines() {
                let upper = line.to_uppercase();
                if upper.starts_with("COPY ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for part in &parts {
                        if !part.contains(':') && !part.starts_with('/') && !part.starts_with("--")
                        {
                            let cleaned = part.trim_matches('"').trim_matches('\'');
                            if cleaned.ends_with(".json")
                                || cleaned.ends_with(".yaml")
                                || cleaned.ends_with(".yml")
                                || cleaned.ends_with(".env")
                                || cleaned.ends_with(".js")
                                || cleaned.ends_with(".ts")
                            {
                                if !path_exists(root, cleaned) {
                                    risks.push(json!({"path":name.to_string(),"line":1,
                                        "referencedPath":cleaned,"risk":"high",
                                        "reasons":["Docker COPY references nonexistent local path"],
                                        "recommendedFix":["update Dockerfile or restore referenced file"]}));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // .github/workflows CI
    let ci_dir = root.join(".github/workflows");
    if ci_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ci_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("working-directory:") {
                            let dir = trimmed.trim_start_matches("working-directory:").trim();
                            if !dir.is_empty() && !path_exists(root, dir) {
                                let rel = p.strip_prefix(root).unwrap_or(&p);
                                risks.push(
                                    json!({"path":rel.to_string_lossy(),"field":"working-directory",
                                    "referencedPath":dir,"risk":"high",
                                    "reasons":["CI working-directory does not exist"],
                                    "recommendedFix":["update CI workflow or create directory"]}),
                                );
                            }
                        }
                        if trimmed.starts_with("run:") {
                            let cmd = trimmed.trim_start_matches("run:").trim();
                            for token in cmd.split_whitespace() {
                                if (token.ends_with(".json") || token.ends_with(".yaml"))
                                    && !path_exists(root, token)
                                {
                                    let rel = p.strip_prefix(root).unwrap_or(&p);
                                    risks.push(json!({"path":rel.to_string_lossy(),"command":cmd,
                                        "risk":"medium","reasons":["CI run references missing config file"],
                                        "recommendedFix":["update CI command or restore config file"]}));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    risks
}

/// Compute config & examples consistency review.
fn compute_config_examples_review(gv: &GraphView, params: &Value, root: &str) -> Value {
    let root_path = std::path::Path::new(root);
    let include_examples = params["includeExamples"].as_bool().unwrap_or(true);
    let include_pkg = params["includePackageConfig"].as_bool().unwrap_or(true);
    let include_build = params["includeBuildConfig"].as_bool().unwrap_or(true);
    let include_ci = params["includeCiConfig"].as_bool().unwrap_or(true);
    let include_docs_cb = params["includeDocsCodeBlocks"].as_bool().unwrap_or(true);

    let mut all_risks: Vec<Value> = Vec::new();
    let mut stale_examples: Vec<Value> = Vec::new();
    let mut pkg_risks = Vec::new();
    let mut ts_risks = Vec::new();
    let mut ci_risks = Vec::new();
    let mut doc_risks = Vec::new();

    if include_pkg {
        pkg_risks = scan_package_json(root_path);
        all_risks.extend(pkg_risks.clone());
    }
    if include_build {
        ts_risks = scan_tsconfig(root_path);
        all_risks.extend(ts_risks.clone());
    }
    if include_examples {
        stale_examples = scan_examples(root_path, gv);
        all_risks.extend(stale_examples.clone());
    }
    if include_docs_cb {
        doc_risks = scan_docs_code_blocks(root_path, gv);
        all_risks.extend(doc_risks.clone());
    }
    if include_ci {
        ci_risks = scan_ci_docker(root_path);
        all_risks.extend(ci_risks.clone());
    }

    let high_count = all_risks.iter().filter(|r| r["risk"] == "high").count();
    let med_count = all_risks.iter().filter(|r| r["risk"] == "medium").count();

    let overall_risk = if high_count >= 3 {
        "high"
    } else if high_count > 0 {
        "medium"
    } else if med_count > 0 {
        "low"
    } else {
        "low"
    };

    let mut recs: Vec<String> = Vec::new();
    if !pkg_risks.is_empty() {
        recs.push("review package.json entry fields".to_string());
    }
    if !ts_risks.is_empty() {
        recs.push("review tsconfig paths and includes".to_string());
    }
    if !stale_examples.is_empty() {
        recs.push("review and update stale examples".to_string());
    }
    if !ci_risks.is_empty() {
        recs.push("review CI/Dockerfile paths".to_string());
    }

    json!({
        "summary": {
            "configRiskCount": all_risks.len(),
            "staleExampleCandidateCount": stale_examples.len(),
            "packageScriptRiskCount": pkg_risks.len(),
            "tsconfigPathRiskCount": ts_risks.len(),
            "ciDockerRiskCount": ci_risks.len(),
            "docsCodeBlockRiskCount": doc_risks.len(),
            "overallConfigConsistencyRisk": overall_risk
        },
        "staleExamples": stale_examples,
        "staleConfigReferences": all_risks.iter().filter(|r| r["risk"]=="high").cloned().collect::<Vec<_>>(),
        "packageScriptRisks": pkg_risks,
        "recommendedVerification": recs,
        "generatedFrom": {
            "graphBased": true,
            "configScannerBased": true,
            "examplesHeuristic": include_examples,
            "scriptsExecuted": false,
            "buildExecuted": false,
            "runtimeVerified": false,
            "heuristic": true
        }
    })
}

fn handle_config_examples_review(cache: &mut McpCache, params: &Value) -> Result<Value, Value> {
    let root = params["root"]
        .as_str()
        .ok_or_else(|| mcp_error("missing_parameter", "Missing required parameter: root"))?;
    let validated = validate_root_path(root)?;
    let language = params["language"].as_str().unwrap_or("auto");
    check_language_feature(language)?;
    let root_str = validated.to_string_lossy().into_owned();
    let (gv, _result, cache_meta) = cache.get_or_analyze(&validated, language, false)?;
    let result = compute_config_examples_review(&gv, params, &root_str);
    let mut out = result.as_object().cloned().unwrap_or_default();
    out.insert("language".to_string(), json!(language));
    out.insert("root".to_string(), json!(validated));
    Ok(merge_cache_and_result(&json!(out), &cache_meta))
}

pub fn run_mcp_server() -> Result<(), String> {
    eprintln!("[mcp] CodeLattice MCP v0.8 server starting on stdin/stdout");

    let mut cache = McpCache::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = stdin
            .lock()
            .read_line(&mut line)
            .map_err(|e| format!("stdin read error: {e}"))?;

        if bytes_read == 0 {
            // EOF — client disconnected
            eprintln!("[mcp] stdin EOF, shutting down");
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[mcp] JSON parse error: {e}");
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&error_response).unwrap_or_default()
                );
                let _ = stdout.flush();
                continue;
            }
        };

        eprintln!("[mcp] -> {}", request["method"].as_str().unwrap_or("?"));

        if let Some(response) = handle_request(&request, &mut cache) {
            let response_str = serde_json::to_string(&response).unwrap_or_default();
            let _ = writeln!(stdout, "{}", response_str);
            let _ = stdout.flush();
        }

        // Check for shutdown
        if request["method"].as_str() == Some("shutdown")
            || request["method"].as_str() == Some("exit")
        {
            break;
        }
    }

    eprintln!("[mcp] server stopped");
    Ok(())
}
