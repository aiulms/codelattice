//! MCP v0.5 Daily-use Candidate Pack for CodeLattice CLI
//!
//! Implements a MCP JSON-RPC server over stdin/stdout.
//! Provides 20 tools:
//!   v0:  codelattice_analyze, codelattice_quality, codelattice_summary, codelattice_smoke
//!   v0.1: codelattice_graph_overview, codelattice_unresolved_report,
//!         codelattice_symbol_search, codelattice_export_bridge
//!   v0.2: codelattice_symbol_context, codelattice_calls_from, codelattice_calls_to,
//!         codelattice_impact_preview, codelattice_query_graph, codelattice_project_overview,
//!         codelattice_repo_registry, codelattice_rename_preview
//!   v0.3: codelattice_cache_status, codelattice_cache_clear
//!   v0.5: codelattice_production_assist, codelattice_compare_runs
//!   v0.6: codelattice_cache_prewarm
//!
//! Transport: newline-delimited JSON-RPC.
//! Approach: subprocess — spawns the CLI binary for analyze/quality/summary,
//!           and the smoke script for smoke.
//! Cache: process-local analysis cache with mtime-based staleness detection
//!        and LRU eviction (max 16 entries).
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
// Process-Local Analysis Cache (v0.3)
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
/// Context lines are added before/after the symbol range (default 3).
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
}

/// Default maximum cache entries (LRU eviction kicks in above this).
const CACHE_MAX_ENTRIES: usize = 16;

/// Process-local cache for MCP server. Not persisted, not shared across processes.
struct McpCache {
    entries: HashMap<CacheKey, CacheEntry>,
    total_hits: u64,
    total_misses: u64,
    total_evictions: u64,
}

/// Scan source files under root and collect their mtimes.
/// Returns a map of relative_path → mtime_ms.
/// Only scans common source file extensions (.rs, .cj, .toml, .json).
fn scan_file_mtimes(root: &Path) -> HashMap<String, u64> {
    let mut mtimes = HashMap::new();
    let extensions = ["rs", "cj", "toml", "json"];

    fn walk_dir(dir: &Path, root: &Path, mtimes: &mut HashMap<String, u64>, extensions: &[&str]) {
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
                    walk_dir(&path, root, mtimes, extensions);
                } else {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if extensions.contains(&ext) {
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

    walk_dir(root, root, &mut mtimes, &extensions);
    mtimes
}

/// Check if cached mtimes are still fresh by comparing with current filesystem.
/// Returns true if any file was added, removed, or modified.
fn mtimes_are_stale(root: &Path, cached_mtimes: &HashMap<String, u64>) -> bool {
    let current = scan_file_mtimes(root);
    if current.len() != cached_mtimes.len() {
        return true; // files added or removed
    }
    for (path, mtime) in cached_mtimes {
        match current.get(path) {
            Some(current_mtime) if *current_mtime == *mtime => {}
            _ => return true, // modified or removed
        }
    }
    false
}

impl McpCache {
    fn new() -> Self {
        McpCache {
            entries: HashMap::new(),
            total_hits: 0,
            total_misses: 0,
            total_evictions: 0,
        }
    }

    /// Get cached analysis or run fresh analyze subprocess.
    /// Returns (graph_view_clone, analyze_result_clone, cache_meta_json).
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

        if let Some(entry) = self.entries.get_mut(&key) {
            // Check mtime freshness
            let root_path = Path::new(&entry.root_canonical);
            if mtimes_are_stale(root_path, &entry.file_mtimes) {
                // Invalidate stale entry — remove it and fall through to re-analyze
                self.entries.remove(&key);
                // Don't count as hit or miss yet; the re-analyze below will count as miss
            } else {
                entry.hit_count += 1;
                entry.last_used_at = Instant::now();
                self.total_hits += 1;
                let meta = json!({
                    "cacheHit": true,
                    "cacheKey": format!("{}:{}:{}", key.root, key.language, key.strict),
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

        // Cache miss — run analyze
        let start = Instant::now();
        let result = run_analyze_subprocess(root, language, "json", strict)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let graph_view = GraphView::build(&result);

        // Scan file mtimes for future freshness checks
        let file_mtimes = scan_file_mtimes(&canonical);

        let cache_key_str = format!("{}:{}:{}", key.root, key.language, key.strict);
        self.entries.insert(
            key,
            CacheEntry {
                analyze_result: result.clone(),
                graph_view: graph_view.clone_shallow(),
                created_at: Instant::now(),
                last_used_at: Instant::now(),
                hit_count: 0,
                analysis_duration_ms: duration_ms,
                file_mtimes,
                root_canonical: canonical.to_string_lossy().to_string(),
            },
        );
        self.total_misses += 1;

        let meta = json!({
            "cacheHit": false,
            "cacheKey": cache_key_str,
            "analysisDurationMs": duration_ms,
        });
        Ok((graph_view, result, meta))
    }

    /// Get cache status, optionally filtered by root/language.
    fn status(&self, filter_root: Option<&str>, filter_lang: Option<&str>) -> Value {
        let mut entries = Vec::new();
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
            entries.push(json!({
                "root": key.root,
                "language": key.language,
                "strict": key.strict,
                "cacheKey": format!("{}:{}:{}", key.root, key.language, key.strict),
                "createdAtMs": entry.created_at.elapsed().as_millis() as u64,
                "lastUsedAtMs": entry.last_used_at.elapsed().as_millis() as u64,
                "hitCount": entry.hit_count,
                "analysisDurationMs": entry.analysis_duration_ms,
                "trackedFiles": entry.file_mtimes.len(),
            }));
        }
        json!({
            "entryCount": entries.len(),
            "maxEntries": CACHE_MAX_ENTRIES,
            "entries": entries,
            "totalHits": self.total_hits,
            "totalMisses": self.total_misses,
            "totalEvictions": self.total_evictions,
        })
    }

    /// Clear cache entries, optionally filtered by root/language.
    fn clear(&mut self, filter_root: Option<&str>, filter_lang: Option<&str>) -> (usize, usize) {
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
        let cleared = before - self.entries.len();
        (cleared, self.entries.len())
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

/// Check if cangjie/arkts language is requested but feature is not compiled.
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

    Ok(merge_cache_and_result(
        &json!({
            "query": name,
            "matchCount": matches.len(),
            "ambiguous": ambiguous,
            "selected": selected,
            "candidates": match_summaries,
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
    let include_snippet = params["includeSnippet"].as_bool().unwrap_or(true);
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

    // Risk heuristic
    let total_impacted = impacted_nodes.len();
    let caller_count = impacted_edge_types.get("CALLS").copied().unwrap_or(0);

    let (risk, reasons) = if total_impacted <= 3 && caller_count <= 2 {
        (
            "LOW".to_string(),
            vec!["Small blast radius, few callers".to_string()],
        )
    } else if total_impacted <= 15 && caller_count <= 10 {
        (
            "MEDIUM".to_string(),
            vec![format!(
                "Moderate fanout: {} impacted nodes, {} CALLS edges",
                total_impacted, caller_count
            )],
        )
    } else {
        (
            "HIGH".to_string(),
            vec![format!(
                "High fanout: {} impacted nodes, {} CALLS edges — change requires careful review",
                total_impacted, caller_count
            )],
        )
    };

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
                "denseFiles": dense_files
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

    // Changed symbols lookup if provided
    let changed_symbols_info: Vec<Value> =
        if let Some(symbols) = params["changedSymbols"].as_array() {
            symbols
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
                            "callerCount": callers,
                            "sourceSnippet": read_source_snippet(&root_str, file, start, end, 3),
                        }))
                    }
                })
                .collect()
        } else {
            vec![]
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
            "changedSymbols": changed_symbols_info,
            "recommendations": recommendations,
            "dryRun": true,
            "noWrites": true,
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
    let (cleared, remaining) = cache.clear(filter_root, filter_lang);
    Ok(tool_result(&json!({
        "clearedCount": cleared,
        "remainingCount": remaining,
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to check" }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to summarize" }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to search" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript"], "description": "Language (must be explicit, not auto)" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
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
                "description": "Preview the blast radius of changing a symbol. Returns impacted nodes/edges grouped by kind, approximate risk level (LOW/MEDIUM/HIGH), and top affected files. Read-only, no writes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Project root directory (absolute path)" },
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
                        "symbol": { "type": "string", "description": "Symbol name to analyze impact for" },
                        "direction": { "type": "string", "enum": ["upstream", "downstream", "both"], "default": "both" },
                        "depth": { "type": "integer", "default": 2, "minimum": 1, "maximum": 3 },
                        "limit": { "type": "integer", "default": 50, "maximum": 200 }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
                        "symbol": { "type": "string", "description": "Current symbol name" },
                        "newName": { "type": "string", "description": "Proposed new name" },
                        "kind": { "type": "string", "description": "Symbol kind to disambiguate" }
                    },
                     "required": ["root", "symbol", "newName"]
                 }
            },
            {
                "name": "codelattice_cache_status",
                "description": "Query the process-local analysis cache status. Shows cached entries, hit/miss counts. Does not trigger analysis.",
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
                "description": "Clear the process-local analysis cache. Does not delete disk files or affect Tool registry. Only clears cache in the current MCP server process.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Filter by root path (substring match). Omit to clear all." },
                        "language": { "type": "string", "description": "Filter by language. Omit to clear all." }
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" },
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
                        "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto", "description": "Language to analyze" },
                        "strict": { "type": "boolean", "default": false, "description": "Strict mode (quality gate failures as errors). Default false to match most other tools." }
                    },
                    "required": ["root"]
                }
             }
         ]
    })
}

// ============================================================
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
            Some(make_response(
                &id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "codelattice",
                        "version": "0.7.0",
                        "cangjieSupport": cangjie_support,
                        "toolCount": 21
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

pub fn run_mcp_server() -> Result<(), String> {
    eprintln!("[mcp] CodeLattice MCP v0.3 server starting on stdin/stdout");

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
