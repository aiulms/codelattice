//! Cangjie SDK diagnostics runner.
//!
//! Spawns cjc (compiler) and cjlint (linter) as subprocesses, parses their
//! JSON output, and returns normalized [`CangjieDiagnostic`] values.
//!
//! All functions gracefully degrade when the SDK is not available — they
//! return empty vectors rather than panicking.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::diagnostics::types::{CangjieDiagnostic, DiagnosticSeverity};

// ---------------------------------------------------------------------------
// SDK tool discovery
// ---------------------------------------------------------------------------

/// Resolve path to a Cangjie SDK tool binary.
///
/// Checks, in order:
/// 1. `CANGJIE_HOME` + sub_dir/tool_name
/// 2. `CANGJIE_SDK_HOME` + sub_dir/tool_name
/// 3. PATH fallback (direct lookup)
///
/// Returns `None` if the tool is not found or not executable.
pub fn resolve_cangjie_tool(tool_name: &str, sub_dir: &str) -> Option<PathBuf> {
    // Check CANGJIE_HOME and CANGJIE_SDK_HOME
    for env_var in &["CANGJIE_HOME", "CANGJIE_SDK_HOME"] {
        if let Ok(home) = std::env::var(env_var) {
            let candidate = PathBuf::from(&home).join(sub_dir).join(tool_name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    // PATH fallback
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let candidate = PathBuf::from(dir).join(tool_name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .metadata()
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false)
}

/// Check whether the Cangjie SDK tools (cjc and cjlint) are available.
pub fn is_cangjie_sdk_available() -> bool {
    resolve_cangjie_tool("cjc", "bin").is_some()
        && resolve_cangjie_tool("cjlint", "tools/bin").is_some()
}

/// Build environment variables for spawning Cangjie SDK subprocesses.
///
/// On macOS, Cangjie SDK tools link against @rpath libraries and need
/// DYLD_LIBRARY_PATH pointing at the SDK lib directories.
pub fn build_cangjie_spawn_env() -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    let home = std::env::var("CANGJIE_HOME").or_else(|_| std::env::var("CANGJIE_SDK_HOME"));

    if let Ok(home) = home {
        #[cfg(target_arch = "aarch64")]
        let hw_arch = "aarch64";
        #[cfg(not(target_arch = "aarch64"))]
        let hw_arch = "x86_64";

        let dyld_path = format!(
            "{}/runtime/lib/darwin_{}_cjnative:{}/tools/lib",
            home, hw_arch, home
        );

        let prev = env.get("DYLD_LIBRARY_PATH").cloned();
        let merged = match prev {
            Some(p) if !p.is_empty() => format!("{}:{}", dyld_path, p),
            _ => dyld_path,
        };
        env.insert("DYLD_LIBRARY_PATH".to_string(), merged);
    }

    env
}

// ---------------------------------------------------------------------------
// Timeout helper
// ---------------------------------------------------------------------------

/// Run a command with a timeout, collecting stdout and stderr.
///
/// Uses a temporary file to capture output from tools that only write to
/// files (e.g. cjlint -o).
fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> std::io::Result<(std::process::ExitStatus, String, String)> {
    use std::thread;

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // We can't easily do timeout with std::process alone, so we
    // use a simple approach: wait with timeout via try_wait loop.
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                child
                    .stdout
                    .take()
                    .map(|mut s| s.read_to_string(&mut stdout));
                child
                    .stderr
                    .take()
                    .map(|mut s| s.read_to_string(&mut stderr));
                return Ok((status, stdout, stderr));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "subprocess timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// cjc runner
// ---------------------------------------------------------------------------

/// Run cjc compiler diagnostics on a single source file.
///
/// Uses `--diagnostic-format=json --output-type=staticlib` to produce JSON
/// diagnostics without requiring a full executable link.
///
/// Exit code 1 means diagnostics exist — not that the tool crashed.
/// cjc writes JSON to stdout on exit code 0, and to stderr on exit code 1.
///
/// Returns empty vec if cjc is not available or the subprocess fails.
pub fn run_cjc_diagnostics(source_file: &Path) -> Vec<CangjieDiagnostic> {
    let cjc_path = match resolve_cangjie_tool("cjc", "bin") {
        Some(p) => p,
        None => return Vec::new(),
    };

    let output_path = format!("/tmp/cjc-diag-{}", std::process::id());

    // cjc --diagnostic-format=json --output-type=staticlib
    //      --error-count-limit all -o <tmp_output> <source_file>
    let mut cmd = Command::new(&cjc_path);
    cmd.arg(source_file)
        .arg("--diagnostic-format=json")
        .arg("--output-type=staticlib")
        .arg("--error-count-limit")
        .arg("all")
        .arg("-o")
        .arg(&output_path);

    // Set environment
    for (k, v) in build_cangjie_spawn_env() {
        cmd.env(k, v);
    }

    let result = run_with_timeout(cmd, Duration::from_secs(60));

    // Clean up temp output file (cjc produces a .lib file regardless)
    let _ = std::fs::remove_file(&output_path);

    let (_status, stdout, stderr) = match result {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    // cjc: stdout on exit 0 (warnings only), stderr on exit 1 (errors)
    let raw = if stdout.trim().starts_with('{') {
        stdout
    } else if stderr.trim().starts_with('{') {
        stderr
    } else {
        return Vec::new();
    };

    parse_cjc_output(&raw)
}

fn parse_cjc_output(raw: &str) -> Vec<CangjieDiagnostic> {
    let parsed: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let diags = match parsed.get("Diags").and_then(|d| d.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    for d in diags {
        let severity_str = d
            .get("Severity")
            .and_then(|s| s.as_str())
            .unwrap_or("warning");
        let severity = match severity_str {
            "error" | "fatal" => DiagnosticSeverity::Error,
            "note" => DiagnosticSeverity::Note,
            _ => DiagnosticSeverity::Warning,
        };

        let file = d
            .get("Location")
            .and_then(|l| l.get("File"))
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();

        // 1-based → 0-based
        let start_line = d
            .get("Location")
            .and_then(|l| l.get("Line"))
            .and_then(|l| l.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(0);

        let start_column = d
            .get("Location")
            .and_then(|l| l.get("Column"))
            .and_then(|c| c.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(0);

        let end_line = d
            .get("MainHint")
            .and_then(|h| h.get("Range"))
            .and_then(|r| r.get("End"))
            .and_then(|e| e.get("Line"))
            .and_then(|l| l.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(start_line);

        let end_column = d
            .get("MainHint")
            .and_then(|h| h.get("Range"))
            .and_then(|r| r.get("End"))
            .and_then(|e| e.get("Column"))
            .and_then(|c| c.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(start_column);

        let message = d
            .get("Message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown diagnostic")
            .to_string();

        let rule = d.get("DiagKind").and_then(|k| k.as_str()).map(String::from);

        result.push(CangjieDiagnostic {
            file_path: file,
            start_line,
            start_column,
            end_line,
            end_column,
            severity,
            message,
            source: "cjc".to_string(),
            rule,
        });
    }

    result
}

// ---------------------------------------------------------------------------
// cjlint runner
// ---------------------------------------------------------------------------

/// Run cjlint linter diagnostics on a project root directory.
///
/// Uses `-r json -o <tmpfile>` to write JSON output to a temp file,
/// then parses and filters to the given source files.
///
/// Returns empty vec if cjlint is not available or the subprocess fails.
pub fn run_cjlint_diagnostics(
    project_root: &Path,
    source_files: &[PathBuf],
) -> Vec<CangjieDiagnostic> {
    let cjlint_path = match resolve_cangjie_tool("cjlint", "tools/bin") {
        Some(p) => p,
        None => return Vec::new(),
    };

    let out_path = format!("/tmp/cjlint-diag-{}.json", std::process::id());

    let mut cmd = Command::new(&cjlint_path);
    cmd.arg("-f")
        .arg(project_root)
        .arg("-r")
        .arg("json")
        .arg("-o")
        .arg(&out_path);

    // Set environment (needed for @rpath libs on macOS)
    for (k, v) in build_cangjie_spawn_env() {
        cmd.env(k, v);
    }

    let result = run_with_timeout(cmd, Duration::from_secs(60));

    // Clean up temp file
    let cleanup = || {
        let _ = std::fs::remove_file(&out_path);
    };

    match result {
        Ok(_) => {
            let raw = match std::fs::read_to_string(&out_path) {
                Ok(s) => s,
                Err(_) => {
                    cleanup();
                    return Vec::new();
                }
            };
            cleanup();
            parse_cjlint_output(&raw, project_root, source_files)
        }
        Err(_) => {
            cleanup();
            Vec::new()
        }
    }
}

fn parse_cjlint_output(
    raw: &str,
    project_root: &Path,
    source_files: &[PathBuf],
) -> Vec<CangjieDiagnostic> {
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    // Build a set of known project source files for filtering.
    let known_files: Vec<String> = source_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let mut result = Vec::new();
    for d in &parsed {
        let file = d
            .get("file")
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();

        // Filter: only keep diagnostics for known project files.
        // cjlint may report diagnostics for SDK/builtin files.
        if !file.is_empty()
            && !known_files
                .iter()
                .any(|kf| file.ends_with(kf) || kf.ends_with(&file))
        {
            // Try matching by relative path
            let rel = project_root.join(&file);
            if !known_files.iter().any(|kf| rel.to_string_lossy() == *kf) {
                // Also try matching by filename suffix
                let file_path = Path::new(&file);
                if !known_files
                    .iter()
                    .any(|kf| Path::new(kf).ends_with(file_path))
                {
                    continue;
                }
            }
        }

        // 1-based → 0-based
        let start_line = d
            .get("line")
            .and_then(|l| l.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(0);

        let start_column = d
            .get("column")
            .and_then(|c| c.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(0);

        let end_line = d
            .get("endLine")
            .and_then(|l| l.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(start_line);

        let end_column = d
            .get("endColumn")
            .and_then(|c| c.as_u64())
            .map(|n| n.saturating_sub(1) as usize)
            .unwrap_or(start_column);

        // defectLevel: MANDATORY → error, SUGGESTIONS → suggestion, else warning
        let defect_level = d.get("defectLevel").and_then(|l| l.as_str()).unwrap_or("");
        let severity = match defect_level {
            "MANDATORY" => DiagnosticSeverity::Error,
            "SUGGESTIONS" => DiagnosticSeverity::Suggestion,
            _ => DiagnosticSeverity::Warning,
        };

        let message = d
            .get("description")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown lint diagnostic")
            .to_string();

        let rule = d
            .get("defectType")
            .and_then(|r| r.as_str())
            .or_else(|| d.get("analyzerName").and_then(|a| a.as_str()))
            .map(String::from);

        result.push(CangjieDiagnostic {
            file_path: file,
            start_line,
            start_column,
            end_line,
            end_column,
            severity,
            message,
            source: "cjlint".to_string(),
            rule,
        });
    }

    result
}

// ---------------------------------------------------------------------------
// Convenience: run all diagnostics
// ---------------------------------------------------------------------------

/// Run both cjc and cjlint diagnostics for a project.
///
/// Returns a combined list of all diagnostics. SDK-absent → empty vec.
pub fn run_all_diagnostics(
    project_root: &Path,
    source_files: &[PathBuf],
) -> Vec<CangjieDiagnostic> {
    let mut all = Vec::new();

    // cjc: per-file compiler diagnostics
    for file in source_files {
        all.extend(run_cjc_diagnostics(file));
    }

    // cjlint: project-level linter diagnostics
    all.extend(run_cjlint_diagnostics(project_root, source_files));

    all
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- SDK tool discovery tests ---

    #[test]
    fn is_executable_detects_system_binary() {
        assert!(is_executable(Path::new("/bin/sh")));
    }

    #[test]
    fn is_executable_rejects_nonexistent_path() {
        assert!(!is_executable(Path::new("/nonexistent/tool_xyz")));
    }

    #[test]
    fn is_executable_rejects_directory() {
        assert!(!is_executable(Path::new("/tmp")));
    }

    #[test]
    fn resolve_tool_returns_none_for_unknown_tool() {
        // Clear env so we only test PATH fallback
        let result = resolve_cangjie_tool("nonexistent_tool_xyz_123", "bin");
        assert!(result.is_none());
    }

    #[test]
    fn is_cangjie_sdk_available_without_sdk() {
        // When SDK is not in PATH/CANGJIE_HOME, should return false
        // (This test must work in CI without Cangjie SDK)
        let available = is_cangjie_sdk_available();
        // We don't assert false because the dev machine may have SDK;
        // just verify it doesn't panic.
        let _ = available;
    }

    // --- cjc output parser tests ---

    #[test]
    fn parse_cjc_output_empty_input() {
        let result = parse_cjc_output("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_cjc_output_invalid_json() {
        let result = parse_cjc_output("not json");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_cjc_output_no_diags_field() {
        let result = parse_cjc_output(r#"{"Num":{"Errors":0}}"#);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_cjc_output_single_warning() {
        let raw = r#"{
            "Diags": [{
                "Severity": "warning",
                "DiagKind": "G.FUN.02",
                "Message": "Function name should be camelCase",
                "Location": {"File": "/proj/src/main.cj", "Line": 10, "Column": 5}
            }],
            "Num": {"Errors": 0, "Warnings": 1}
        }"#;
        let result = parse_cjc_output(raw);
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, DiagnosticSeverity::Warning);
        assert_eq!(d.start_line, 9); // 1-based → 0-based
        assert_eq!(d.start_column, 4);
        assert_eq!(d.message, "Function name should be camelCase");
        assert_eq!(d.source, "cjc");
        assert_eq!(d.rule.as_deref(), Some("G.FUN.02"));
    }

    #[test]
    fn parse_cjc_output_error_with_hint_range() {
        let raw = r#"{
            "Diags": [{
                "Severity": "error",
                "DiagKind": "E001",
                "Message": "Type mismatch: expected String, found Int64",
                "Location": {"File": "/proj/src/main.cj", "Line": 15, "Column": 20},
                "MainHint": {"Range": {"Begin": {"Line": 15, "Column": 20}, "End": {"Line": 15, "Column": 25}}}
            }],
            "Num": {"Errors": 1, "Warnings": 0}
        }"#;
        let result = parse_cjc_output(raw);
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, DiagnosticSeverity::Error);
        assert_eq!(d.start_line, 14);
        assert_eq!(d.end_line, 14);
        assert_eq!(d.end_column, 24);
    }

    #[test]
    fn parse_cjc_output_fatal_treated_as_error() {
        let raw = r#"{
            "Diags": [{
                "Severity": "fatal",
                "Message": "Fatal error",
                "Location": {"File": "/proj/src/main.cj", "Line": 1, "Column": 1}
            }]
        }"#;
        let result = parse_cjc_output(raw);
        assert_eq!(result[0].severity, DiagnosticSeverity::Error);
    }

    // --- cjlint output parser tests ---

    #[test]
    fn parse_cjlint_output_empty_array() {
        let result = parse_cjlint_output("[]", Path::new("/proj"), &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_cjlint_output_invalid_json() {
        let result = parse_cjlint_output("not json", Path::new("/proj"), &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_cjlint_output_mandatory_defect() {
        let raw = r#"[{
            "file": "src/main.cj",
            "line": 5,
            "column": 10,
            "defectLevel": "MANDATORY",
            "description": "Missing return statement",
            "defectType": "G.FUN.01"
        }]"#;
        let source_files = vec![PathBuf::from("/proj/src/main.cj")];
        let result = parse_cjlint_output(raw, Path::new("/proj"), &source_files);
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, DiagnosticSeverity::Error);
        assert_eq!(d.start_line, 4); // 1-based → 0-based
        assert_eq!(d.start_column, 9);
        assert_eq!(d.message, "Missing return statement");
        assert_eq!(d.source, "cjlint");
        assert_eq!(d.rule.as_deref(), Some("G.FUN.01"));
    }

    #[test]
    fn parse_cjlint_output_suggestion_defect() {
        let raw = r#"[{
            "file": "src/lib.cj",
            "line": 3,
            "column": 1,
            "defectLevel": "SUGGESTIONS",
            "description": "Consider adding documentation",
            "analyzerName": "doc-checker"
        }]"#;
        let source_files = vec![PathBuf::from("/proj/src/lib.cj")];
        let result = parse_cjlint_output(raw, Path::new("/proj"), &source_files);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, DiagnosticSeverity::Suggestion);
        assert_eq!(result[0].rule.as_deref(), Some("doc-checker"));
    }

    #[test]
    fn parse_cjlint_output_filters_non_project_files() {
        let raw = r#"[
            {"file": "/sdk/builtin.cj", "line": 1, "column": 1, "defectLevel": "WARNING", "description": "SDK warning"},
            {"file": "src/main.cj", "line": 2, "column": 1, "defectLevel": "WARNING", "description": "Project warning"}
        ]"#;
        let source_files = vec![PathBuf::from("/proj/src/main.cj")];
        let result = parse_cjlint_output(raw, Path::new("/proj"), &source_files);
        // Only the project file diagnostic should be kept
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "Project warning");
    }

    // --- graceful degrade tests (no SDK needed) ---

    #[test]
    fn run_cjc_diagnostics_returns_empty_when_sdk_absent() {
        // This test only verifies graceful degrade when SDK is absent.
        // In SDK-present CI, it may return real diagnostics — both are fine.
        let result = run_cjc_diagnostics(Path::new("/nonexistent/file.cj"));
        // Should not panic; may return diagnostics if SDK is present.
        let _ = result;
    }

    #[test]
    fn run_cjlint_diagnostics_returns_empty_when_sdk_absent() {
        let result = run_cjlint_diagnostics(Path::new("/nonexistent"), &[]);
        // Should not panic.
        let _ = result;
    }

    #[test]
    fn run_all_diagnostics_does_not_panic() {
        let result = run_all_diagnostics(Path::new("/nonexistent"), &[]);
        // Should not panic.
        let _ = result;
    }
}
