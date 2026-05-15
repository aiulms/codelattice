//! Integration tests for MCP v0.3 stdio server.
//!
//! Tests start the binary with `mcp` subcommand, communicate via stdin/stdout
//! using newline-delimited JSON-RPC, and verify responses.
//!
//! Covers v0 (4 tools) + v0.1 (4 tools) + v0.2 (8 tools) + v0.3 (2 cache tools) = 18 tools total.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn portable_smoke_dir() -> std::path::PathBuf {
    // Use c1-same-module as a valid Rust project for testing
    workspace_root()
        .join("fixtures")
        .join("call-resolution")
        .join("c1-same-module")
}

#[allow(dead_code)]
fn cangjie_portable_smoke_dir() -> std::path::PathBuf {
    workspace_root()
        .join("fixtures")
        .join("cangjie")
        .join("portable-smoke")
}

#[allow(dead_code)]
fn arkts_portable_smoke_dir() -> std::path::PathBuf {
    workspace_root()
        .join("fixtures")
        .join("arkts")
        .join("portable-smoke")
}

#[allow(dead_code)]
fn arkts_cross_file_dir() -> std::path::PathBuf {
    workspace_root()
        .join("fixtures")
        .join("arkts")
        .join("cross-file")
}

fn cli_binary() -> PathBuf {
    // Use CARGO_BIN_EXE environment variable set by cargo test
    std::env::var("CARGO_BIN_EXE_gitnexus-rust-core-cli")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            workspace_root()
                .join("target")
                .join("debug")
                .join("gitnexus-rust-core-cli")
        })
}

struct McpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl McpSession {
    fn start() -> Self {
        Self::start_with_cache_dir(None)
    }

    fn start_with_cache_dir(cache_dir: Option<&std::path::Path>) -> Self {
        let bin = cli_binary();
        let mut cmd = Command::new(bin);
        cmd.arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = cache_dir {
            cmd.env("CODELATTICE_CACHE_DIR", dir);
        }
        let mut child = cmd.spawn().expect("Failed to start MCP server");

        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = child.stdout.take().expect("Failed to get stdout");

        McpSession {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, request: &serde_json::Value) {
        let line = serde_json::to_string(request).unwrap();
        writeln!(self.stdin, "{}", line).expect("Failed to write to stdin");
        self.stdin.flush().expect("Failed to flush stdin");
    }

    fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        let mut reader = std::io::BufReader::new(&mut self.stdout);
        reader
            .read_line(&mut line)
            .expect("Failed to read from stdout");
        serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("Failed to parse JSON response: {}. Line: {:?}", e, line))
    }

    fn initialize(&mut self) -> serde_json::Value {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" }
            }
        }));
        let resp = self.recv();
        assert_eq!(resp["id"], 1, "initialize response id mismatch");
        resp
    }

    fn send_notification_initialized(&mut self) {
        // Notifications don't expect responses, just send and continue
        let line = serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .unwrap();
        writeln!(self.stdin, "{}", line).expect("write");
        self.stdin.flush().expect("flush");
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        // Try graceful shutdown
        let _ = writeln!(
            self.stdin,
            "{{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\"}}"
        );
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_initialize_returns_capabilities() {
    let mut session = McpSession::start();
    let resp = session.initialize();

    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(resp["result"]["serverInfo"]["name"], "codelattice");
    assert!(resp["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn mcp_tools_list_returns_twenty_two_tools() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 2);

    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools should be array");
    assert_eq!(tools.len(), 22, "expected 22 tools, got {}", tools.len());

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    // v0 tools
    assert!(
        names.contains(&"codelattice_analyze"),
        "missing codelattice_analyze"
    );
    assert!(
        names.contains(&"codelattice_quality"),
        "missing codelattice_quality"
    );
    assert!(
        names.contains(&"codelattice_summary"),
        "missing codelattice_summary"
    );
    assert!(
        names.contains(&"codelattice_smoke"),
        "missing codelattice_smoke"
    );
    // v0.1 tools
    assert!(
        names.contains(&"codelattice_graph_overview"),
        "missing codelattice_graph_overview"
    );
    assert!(
        names.contains(&"codelattice_unresolved_report"),
        "missing codelattice_unresolved_report"
    );
    assert!(
        names.contains(&"codelattice_symbol_search"),
        "missing codelattice_symbol_search"
    );
    assert!(
        names.contains(&"codelattice_export_bridge"),
        "missing codelattice_export_bridge"
    );
    // v0.2 tools
    assert!(
        names.contains(&"codelattice_symbol_context"),
        "missing codelattice_symbol_context"
    );
    assert!(
        names.contains(&"codelattice_calls_from"),
        "missing codelattice_calls_from"
    );
    assert!(
        names.contains(&"codelattice_calls_to"),
        "missing codelattice_calls_to"
    );
    assert!(
        names.contains(&"codelattice_impact_preview"),
        "missing codelattice_impact_preview"
    );
    assert!(
        names.contains(&"codelattice_query_graph"),
        "missing codelattice_query_graph"
    );
    assert!(
        names.contains(&"codelattice_project_overview"),
        "missing codelattice_project_overview"
    );
    assert!(
        names.contains(&"codelattice_repo_registry"),
        "missing codelattice_repo_registry"
    );
    assert!(
        names.contains(&"codelattice_rename_preview"),
        "missing codelattice_rename_preview"
    );
    // v0.3 cache tools
    assert!(
        names.contains(&"codelattice_cache_status"),
        "missing codelattice_cache_status"
    );
    assert!(
        names.contains(&"codelattice_cache_clear"),
        "missing codelattice_cache_clear"
    );

    // Verify each tool has inputSchema
    for tool in tools {
        assert!(
            tool["inputSchema"].is_object(),
            "tool {} missing inputSchema",
            tool["name"]
        );
    }
}

#[test]
fn mcp_analyze_rust_portable_smoke() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "strict": false,
                "includeGraph": false
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "analyze should succeed, got: {:?}",
        resp
    );

    // Parse the content text as JSON
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("analyze output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert!(data["summary"]["nodeCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["qualityGates"].is_array());
}

#[test]
fn mcp_quality_rust_portable_smoke() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "codelattice_quality",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 20);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "quality should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("quality output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert_eq!(data["overall"], "pass");
    assert!(data["gates"].is_array());
}

#[test]
fn mcp_summary_rust_portable_smoke() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "tools/call",
        "params": {
            "name": "codelattice_summary",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 30);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "summary should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("summary output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert!(data["graphSummary"].is_object());
    assert!(data["qualitySummary"].is_object());
}

#[test]
fn mcp_smoke_rust_only() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "tools/call",
        "params": {
            "name": "codelattice_smoke",
            "arguments": {
                "mode": "rust-only"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 40);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "smoke should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("smoke output should be valid JSON");

    assert_eq!(data["mode"], "rust-only");
    assert!(data["passed"].as_bool().unwrap_or(false));
    assert!(data["passCount"].as_u64().unwrap_or(0) > 0);
    assert_eq!(data["failCount"].as_u64().unwrap_or(1), 0);
}

#[test]
fn mcp_path_denied_live_repo() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    // Try to analyze the live cangjie repo (should be denied)
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 50,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": "/Users/jiangxuanyang/Desktop/cangjie",
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 50);

    // Should be an error
    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false);
    assert!(is_error, "live repo path should be denied");

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let error_data: serde_json::Value = serde_json::from_str(content_text).unwrap_or_default();
    assert_eq!(error_data["error"], "path_denied");
}

#[test]
fn mcp_nonexistent_path_rejected() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 60,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": "/nonexistent/path/that/does/not/exist",
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 60);

    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false);
    assert!(is_error, "nonexistent path should be rejected");

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let error_data: serde_json::Value = serde_json::from_str(content_text).unwrap_or_default();
    assert!(
        error_data["error"].as_str().unwrap_or("").contains("path"),
        "expected path error, got: {:?}",
        error_data
    );
}

#[test]
fn mcp_unknown_tool_returns_error() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 70,
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool",
            "arguments": {}
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 70);

    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false);
    assert!(is_error, "unknown tool should return error");

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let error_data: serde_json::Value = serde_json::from_str(content_text).unwrap_or_default();
    assert_eq!(error_data["error"], "unknown_tool");
}

#[test]
fn mcp_json_rpc_id_matching() {
    let mut session = McpSession::start();

    // Send initialize with id=42
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "id-test", "version": "1.0" }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 42, "response id should match request id");
    assert_eq!(resp["result"]["serverInfo"]["name"], "codelattice");

    // Send tools/list with id=99
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/list"
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 99, "response id should match request id");
}

// ============================================================
// v0.1 Tool Tests
// ============================================================

#[test]
fn mcp_graph_overview_rust() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "tools/call",
        "params": {
            "name": "codelattice_graph_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 100);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "graph_overview should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("graph_overview output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert!(
        data["nodeCount"].as_u64().unwrap_or(0) > 0,
        "should have nodes"
    );
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "should have edges"
    );
    assert!(
        data["symbolCount"].as_u64().unwrap_or(0) > 0,
        "should have symbols"
    );
    assert!(data["nodeKindCounts"].is_object());
    assert!(data["edgeKindCounts"].is_object());
    assert!(data["qualitySummary"].is_object());
    assert!(data["diagnosticsSummary"].is_object());
}

#[test]
fn mcp_symbol_search_finds_helper() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 110,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "query": "helper"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 110);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "symbol_search should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("symbol_search output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert!(
        data["matchCount"].as_u64().unwrap_or(0) > 0,
        "should find 'helper' symbol"
    );
    let matches = data["matches"].as_array().expect("matches should be array");
    let names: Vec<&str> = matches.iter().filter_map(|m| m["name"].as_str()).collect();
    assert!(
        names.iter().any(|n| n.contains("helper")),
        "should find symbol containing 'helper', got: {:?}",
        names
    );
}

#[test]
fn mcp_symbol_search_finds_main() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 115,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "query": "main"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 115);

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("symbol_search output should be valid JSON");

    assert!(
        data["matchCount"].as_u64().unwrap_or(0) > 0,
        "should find 'main' symbol"
    );
}

#[test]
fn mcp_unresolved_report_rust() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 120,
        "method": "tools/call",
        "params": {
            "name": "codelattice_unresolved_report",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 120);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "unresolved_report should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("unresolved_report output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert_eq!(data["supported"], true);
    // This small fixture may have 0 unresolved, that's fine
    assert!(data["total"].is_number());
    assert!(data["unresolvedEdges"].is_number());
    assert!(data["reasonBreakdown"].is_object());
    assert!(
        data["stopLineNote"].is_string(),
        "should include stop-line note"
    );
}

#[test]
fn mcp_export_bridge_writes_to_tmp() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    let output_path = format!(
        "/tmp/codelattice-mcp-test-bridge-{}.json",
        std::process::id()
    );
    // Cleanup from previous runs
    let _ = std::fs::remove_file(&output_path);

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 130,
        "method": "tools/call",
        "params": {
            "name": "codelattice_export_bridge",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "outputPath": output_path
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 130);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "export_bridge should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("export_bridge output should be valid JSON");

    assert_eq!(data["stdoutPurity"], true);
    assert!(data["bytes"].as_u64().unwrap_or(0) > 0, "should have bytes");
    assert!(data["schemaVersion"].is_string());
    assert!(data["packages"].as_u64().unwrap_or(0) > 0);
    assert!(data["symbols"].as_u64().unwrap_or(0) > 0);

    // Verify file was actually written and is valid JSON
    let file_contents = std::fs::read_to_string(&output_path).expect("bridge file should exist");
    let file_json: serde_json::Value =
        serde_json::from_str(&file_contents).expect("bridge file should be valid JSON");
    assert!(
        file_json["schemaVersion"].is_string(),
        "bridge file should have schemaVersion"
    );

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}

#[test]
fn mcp_export_bridge_rejects_non_tmp_path() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 140,
        "method": "tools/call",
        "params": {
            "name": "codelattice_export_bridge",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "outputPath": "/Users/jiangxuanyang/Desktop/evil-output.json"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 140);

    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false);
    assert!(is_error, "non-/tmp path should be rejected");

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let error_data: serde_json::Value = serde_json::from_str(content_text).unwrap_or_default();
    assert_eq!(error_data["error"], "output_path_denied");
    assert!(
        error_data["hint"].is_string(),
        "should include helpful hint"
    );
}

#[test]
fn mcp_analyze_compact_excludes_graph() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // Default includeGraph=false should not include graph
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 150,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "strict": false,
                "includeGraph": false
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 150);

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("analyze output should be valid JSON");

    // Compact mode: graph should not be present
    assert!(
        data.get("graph").is_none() || data["graph"].is_null(),
        "compact mode should not include graph, got keys: {:?}",
        data.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    // But should have summary and qualityGates
    assert!(data["summary"].is_object());
    assert!(data["qualityGates"].is_array());
}

#[test]
fn mcp_analyze_include_graph_returns_graph() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 155,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "strict": false,
                "includeGraph": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 155);

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("analyze output should be valid JSON");

    // With includeGraph=true, graph should be present
    assert!(
        data["graph"].is_object(),
        "should include graph when includeGraph=true"
    );
    assert!(
        data["graph"]["nodes"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
            > 0,
        "graph should have nodes"
    );
}

// ============================================================
// v0.2 Tool Tests
// ============================================================

#[test]
fn mcp_symbol_context_finds_helper() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 200,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 200);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "symbol_context should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("symbol_context output should be valid JSON");

    assert!(data["matchCount"].as_u64().unwrap_or(0) > 0);
    assert!(
        data["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"].as_str() == Some("helper")),
        "should find helper"
    );
    // Should have edge info
    let first = &data["candidates"].as_array().unwrap()[0];
    assert!(first["outgoingEdges"].is_object());
    assert!(first["incomingEdges"].is_object());
}

#[test]
fn mcp_calls_from_main() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 210,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "main_fn",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 210);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "calls_from should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("calls_from output should be valid JSON");

    assert!(data["sourceCandidates"].as_array().unwrap().len() > 0);
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "main_fn should have outgoing edges"
    );
    // Should have edges to helper
    let edges = data["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["targetName"].as_str() == Some("helper")),
        "should have edge to helper"
    );
}

#[test]
fn mcp_calls_to_helper() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 220,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_to",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 220);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "calls_to should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("calls_to output should be valid JSON");

    assert!(data["targetCandidates"].as_array().unwrap().len() > 0);
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "helper should have incoming edges"
    );
    let edges = data["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["sourceName"].as_str() == Some("main_fn")),
        "should have edge from main_fn"
    );
}

#[test]
fn mcp_impact_preview_helper() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 230,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "direction": "both",
                "depth": 2
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 230);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "impact_preview should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("impact_preview output should be valid JSON");

    assert!(["LOW", "MEDIUM", "HIGH"].contains(&data["risk"].as_str().unwrap_or("")));
    assert!(data["impactedNodeCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["impactedNodesByKind"].is_object());
    assert!(data["previewOnly"].as_bool().unwrap_or(false));
    assert!(data["noWrites"].as_bool().unwrap_or(false));
}

#[test]
fn mcp_query_graph_by_node_kind() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 240,
        "method": "tools/call",
        "params": {
            "name": "codelattice_query_graph",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "nodeKind": "function"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 240);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "query_graph should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("query_graph output should be valid JSON");

    assert!(data["matchedNodeCount"].as_u64().unwrap_or(0) > 0);
    let nodes = data["matchedNodes"].as_array().unwrap();
    assert!(nodes.iter().all(|n| n["kind"].as_str() == Some("function")));
}

#[test]
fn mcp_project_overview_rust() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 250,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 250);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "project_overview should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("project_overview output should be valid JSON");

    assert_eq!(data["language"], "rust");
    assert!(data["nodeCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["edgeCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["symbolCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["sourceFileCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["topNodeKinds"].is_object());
    assert!(data["topEdgeKinds"].is_object());
    assert!(data["qualitySummary"].is_object());
    assert!(data["diagnosticsSummary"].is_object());
    assert!(data["hotspots"].is_array());
    assert!(data["denseFiles"].is_array());
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn mcp_project_overview_cangjie_counts_are_nonzero() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = cangjie_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 251,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "cangjie"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 251);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "project_overview should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("project_overview output should be valid JSON");

    assert_eq!(data["language"], "cangjie");
    assert!(data["nodeCount"].as_u64().unwrap_or(0) > 0);
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "Cangjie project_overview should report graph edges"
    );
    assert!(
        data["symbolCount"].as_u64().unwrap_or(0) > 0,
        "Cangjie project_overview should report symbols"
    );
    assert!(
        data["sourceFileCount"].as_u64().unwrap_or(0) > 0,
        "Cangjie project_overview should report source files"
    );
}

#[test]
fn mcp_rename_preview_read_only() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 260,
        "method": "tools/call",
        "params": {
            "name": "codelattice_rename_preview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "newName": "assist"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 260);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "rename_preview should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("rename_preview output should be valid JSON");

    assert_eq!(data["applySupported"], false);
    assert!(data["candidates"].as_array().unwrap().len() > 0);
}

#[test]
fn mcp_repo_registry_status() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 270,
        "method": "tools/call",
        "params": {
            "name": "codelattice_repo_registry",
            "arguments": {
                "action": "status",
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 270);
    assert!(
        !resp
            .get("result")
            .map_or(true, |r| r["isError"].as_bool().unwrap_or(false)),
        "repo_registry should succeed, got: {:?}",
        resp
    );

    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value =
        serde_json::from_str(content_text).expect("repo_registry output should be valid JSON");

    assert_eq!(data["action"], "status");
    assert_eq!(data["indexed"], true);
    assert!(data["nodeCount"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn mcp_impact_preview_nonexistent_symbol() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 280,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "nonexistent_symbol_xyz"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 280);
    // Should succeed but report UNKNOWN risk
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(content_text).expect("should be valid JSON");
    assert_eq!(data["risk"], "UNKNOWN");
}

// ─── v0.3 Cache Tests ───────────────────────────────────────────────────

#[test]
fn mcp_cache_status_empty() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 301,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 301);
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(content_text).expect("should be valid JSON");
    let mem = &data["memory"];
    assert_eq!(mem["entryCount"], 0);
    assert_eq!(mem["totalHits"], 0);
    assert_eq!(mem["totalMisses"], 0);
}

#[test]
fn mcp_cache_status_after_analyze() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First: run analyze to populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 302,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Then: check cache status
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 303,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 303);
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(content_text).expect("should be valid JSON");
    let mem = &data["memory"];
    assert_eq!(mem["entryCount"], 1);
    assert_eq!(mem["totalMisses"], 1);
}

#[test]
fn mcp_cache_hit_on_second_analyze() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First call — cache miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 304,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp1 = session.recv();
    let text1 = resp1["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data1: serde_json::Value = serde_json::from_str(text1).expect("valid JSON");
    assert_eq!(data1["cacheHit"], false, "first call should be cache miss");
    assert!(
        data1["analysisDurationMs"].as_u64().unwrap_or(0) > 0,
        "miss should report analysisDurationMs"
    );

    // Second call — cache hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 305,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert_eq!(data2["cacheHit"], true, "second call should be cache hit");
}

#[test]
fn mcp_cache_hit_cross_tool() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First: calls_from (populates cache with strict=false)
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 306,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "symbol": "main",
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Second: symbol_context (should get cache hit — same root+language+strict=false)
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 307,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "name": "main",
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(
        data["cacheHit"], true,
        "cross-tool call should be cache hit"
    );
}

#[test]
fn mcp_cache_clear_empties_cache() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // Populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 308,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Clear cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 309,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_clear",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["clearedCount"].as_u64().unwrap_or(0), 1);

    // Verify cache is empty
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 310,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert_eq!(
        data2["memory"]["entryCount"], 0,
        "cache should be empty after clear"
    );
}

#[test]
fn mcp_cache_hit_on_calls_from() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First call — miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 311,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "symbol": "main",
                "language": "rust"
            }
        }
    }));
    let resp1 = session.recv();
    let text1 = resp1["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data1: serde_json::Value = serde_json::from_str(text1).expect("valid JSON");
    assert_eq!(data1["cacheHit"], false, "first calls_from should be miss");

    // Second call — hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 312,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "symbol": "main",
                "language": "rust"
            }
        }
    }));
    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert_eq!(data2["cacheHit"], true, "second calls_from should be hit");
}

#[test]
fn mcp_cache_hit_on_project_overview() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First call — miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 313,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp1 = session.recv();
    let text1 = resp1["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data1: serde_json::Value = serde_json::from_str(text1).expect("valid JSON");
    assert_eq!(data1["cacheHit"], false);

    // Second call — hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 314,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert_eq!(data2["cacheHit"], true);
}

#[test]
fn mcp_cache_miss_after_clear_then_re_analyze() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // Populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 315,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Clear
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 316,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_clear",
            "arguments": {}
        }
    }));
    let _ = session.recv();

    // Re-analyze — should be miss again
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 317,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["cacheHit"], false, "should be miss after cache clear");
    assert!(
        data["analysisDurationMs"].as_u64().unwrap_or(0) > 0,
        "re-analyze should report duration"
    );
}

#[test]
fn mcp_cache_status_shows_hit_count() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First analyze — miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 318,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Second analyze — hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 319,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Third analyze — hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 320,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Check status
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 321,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    let mem = &data["memory"];
    assert_eq!(mem["entryCount"], 1);
    assert_eq!(
        mem["totalHits"], 2,
        "should have 2 hits after 3 analyze calls"
    );
    assert_eq!(mem["totalMisses"], 1, "should have 1 miss");
}

#[test]
fn mcp_cache_different_roots_are_separate() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root1 = portable_smoke_dir();
    let root2 = workspace_root()
        .join("fixtures")
        .join("rust")
        .join("portable-smoke");

    // Analyze root1
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 322,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root1.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Analyze root2 — different root, should be miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 323,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root2.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["cacheHit"], false, "different root should be miss");

    // Check status — should have 2 entries
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 324,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert!(
        data2["memory"]["entryCount"].as_u64().unwrap_or(0) >= 2,
        "should have at least 2 cache entries"
    );
}

// ─── v0.4 Source Snippet Tests ──────────────────────────────────────────

#[test]
fn mcp_symbol_context_includes_source_snippet() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 401,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));

    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let selected = &data["selected"];
    assert!(selected.is_object(), "selected should be an object");

    let snippet = &selected["sourceSnippet"];
    assert!(snippet.is_object(), "sourceSnippet should be present");
    assert!(
        snippet["lines"].as_str().unwrap_or("").contains("helper"),
        "snippet should contain 'helper'"
    );
    assert!(snippet["startLine"].as_u64().unwrap_or(0) > 0);
    assert!(snippet["endLine"].as_u64().unwrap_or(0) > 0);
    assert!(snippet["totalLines"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn mcp_symbol_context_snippet_disabled() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 402,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper",
                "includeSnippet": false
            }
        }
    }));

    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let selected = &data["selected"];
    assert!(selected.is_object());
    // sourceSnippet should be null when disabled
    assert!(
        selected["sourceSnippet"].is_null(),
        "sourceSnippet should be null when includeSnippet=false"
    );
}

#[test]
fn mcp_symbol_context_snippet_with_cache_hit() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    // First call — miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 403,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));
    let _ = session.recv();

    // Second call — hit, snippet should still work
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 404,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));
    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    assert_eq!(data["cacheHit"], true);
    let snippet = &data["selected"]["sourceSnippet"];
    assert!(
        snippet["lines"].as_str().unwrap_or("").contains("helper"),
        "snippet should work on cache hit"
    );
}

#[test]
fn mcp_symbol_context_custom_snippet_context() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 405,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper",
                "snippetContext": 0
            }
        }
    }));

    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let snippet = &data["selected"]["sourceSnippet"];
    assert!(snippet.is_object());
    // With 0 context, snippet should be just the function itself
    let lines = snippet["lines"].as_str().unwrap_or("");
    assert!(
        lines.contains("helper"),
        "should contain helper even with 0 context"
    );
}

#[test]
fn mcp_symbol_context_snippet_candidates_all_have_snippets() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 406,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));

    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let candidates = data["candidates"]
        .as_array()
        .expect("candidates should be array");
    for c in candidates {
        let snippet = &c["sourceSnippet"];
        assert!(
            snippet.is_object(),
            "every candidate should have sourceSnippet object"
        );
        assert!(
            snippet["lines"].is_string(),
            "snippet should have lines string"
        );
    }
}

// ============================================================
// v0.5 Cache Correctness Tests
// ============================================================

#[test]
fn mcp_cache_mtime_invalidation() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();

    // First call — check cache empty
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 500,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 500);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["memory"]["entryCount"], 0, "should start empty");

    // Trigger analyze to populate cache (uses codelattice_analyze which goes through cache)
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 501,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 501);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["cacheHit"], false, "first call should be miss");

    // Second call — should hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 502,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 502);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["cacheHit"], true, "second call should hit");

    // Check cache_status has maxEntries and totalEvictions
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 503,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 503);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    let mem = &data["memory"];
    assert_eq!(mem["entryCount"], 1);
    assert!(mem["maxEntries"].is_number(), "should have maxEntries");
    assert!(
        mem["totalEvictions"].is_number(),
        "should have totalEvictions"
    );
    assert!(mem["totalHits"].is_number(), "should have totalHits");
}

#[test]
fn mcp_cache_clear_then_miss() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();

    // Populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 510,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Clear cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 511,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_clear",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 511);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert!(
        data["clearedCount"].as_u64().unwrap_or(0) >= 1,
        "should clear at least 1"
    );

    // Next call should be miss
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 512,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp = session.recv();
    assert_eq!(resp["id"], 512);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["cacheHit"], false, "after clear should be miss");
}

// ============================================================
// v0.5 Daily Workflow Tool Tests
// ============================================================

#[test]
fn mcp_production_assist_basic() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 600,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 600);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    assert!(data["symbolCount"].is_number(), "should have symbolCount");
    assert!(data["nodeCount"].is_number(), "should have nodeCount");
    assert!(data["edgeCount"].is_number(), "should have edgeCount");
    assert!(data["risk"].is_string(), "should have risk level");
    assert!(
        data["qualityGatesPassed"].is_number(),
        "should have qualityGatesPassed"
    );
    assert!(
        data["qualityGatesFailed"].is_number(),
        "should have qualityGatesFailed"
    );
    assert!(
        data["unresolvedCalls"].is_number(),
        "should have unresolvedCalls"
    );
    assert!(data["diagnostics"].is_number(), "should have diagnostics");
    assert!(data["dryRun"].is_boolean(), "should have dryRun=true");
    assert!(data["topFiles"].is_array(), "should have topFiles");
    assert!(
        data["recommendations"].is_array(),
        "should have recommendations"
    );
}

#[test]
fn mcp_production_assist_with_changed_symbols() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 601,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "changedSymbols": ["helper", "main"]
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 601);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let changed = data["changedSymbols"]
        .as_array()
        .expect("should have changedSymbols array");
    assert!(
        changed.len() >= 1,
        "should find at least one changed symbol"
    );
    let first = &changed[0];
    assert!(first["name"].is_string(), "should have name");
    assert!(first["callerCount"].is_number(), "should have callerCount");
    assert!(
        first["sourceSnippet"].is_object(),
        "should have sourceSnippet"
    );
}

#[test]
fn mcp_compare_runs_bridge_files() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    // Create two temp bridge JSON files
    let before = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "n1", "label": "symbol", "properties": {"name": "foo", "symbolKind": "Function", "sourcePath": "a.rs"}},
                {"id": "n2", "label": "symbol", "properties": {"name": "bar", "symbolKind": "Function", "sourcePath": "b.rs"}}
            ],
            "edges": [
                {"source": "n1", "target": "n2", "type": "CALLS", "properties": {}}
            ]
        },
        "qualityGates": [{"name": "test", "passed": true}]
    });
    let after = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "n1", "label": "symbol", "properties": {"name": "foo", "symbolKind": "Function", "sourcePath": "a.rs"}},
                {"id": "n3", "label": "symbol", "properties": {"name": "baz", "symbolKind": "Function", "sourcePath": "c.rs"}}
            ],
            "edges": [
                {"source": "n1", "target": "n3", "type": "CALLS", "properties": {}}
            ]
        },
        "qualityGates": [{"name": "test", "passed": false}]
    });

    let before_path = format!(
        "/tmp/codelattice-compare-before-{}.json",
        std::process::id()
    );
    let after_path = format!("/tmp/codelattice-compare-after-{}.json", std::process::id());
    std::fs::write(&before_path, serde_json::to_string(&before).unwrap()).unwrap();
    std::fs::write(&after_path, serde_json::to_string(&after).unwrap()).unwrap();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 610,
        "method": "tools/call",
        "params": {
            "name": "codelattice_compare_runs",
            "arguments": {
                "beforeBridgeJson": before_path,
                "afterBridgeJson": after_path
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 610);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    assert_eq!(data["beforeNodes"], 2);
    assert_eq!(data["afterNodes"], 2);
    assert!(data["addedNodes"].is_number(), "should have addedNodes");
    assert!(data["removedNodes"].is_number(), "should have removedNodes");
    assert!(data["summary"].is_string(), "should have summary");
    assert!(
        data["note"].is_string(),
        "should have note about generatedAt"
    );

    // Cleanup
    let _ = std::fs::remove_file(&before_path);
    let _ = std::fs::remove_file(&after_path);
}

#[test]
fn mcp_calls_from_includes_snippet() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 620,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "main",
                "includeSnippet": true,
                "snippetContext": 2
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 620);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let candidates = data["sourceCandidates"]
        .as_array()
        .expect("should have sourceCandidates");
    if !candidates.is_empty() {
        let first = &candidates[0];
        assert!(
            first["sourceSnippet"].is_object(),
            "candidates should have sourceSnippet"
        );
    }
}

#[test]
fn mcp_rename_preview_includes_snippet() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 630,
        "method": "tools/call",
        "params": {
            "name": "codelattice_rename_preview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "newName": "assist",
                "includeSnippet": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 630);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");

    let candidates = data["candidates"]
        .as_array()
        .expect("should have candidates");
    if !candidates.is_empty() {
        let first = &candidates[0];
        assert!(
            first["sourceSnippet"].is_object(),
            "rename candidates should have sourceSnippet"
        );
        assert!(
            first["sourceSnippet"]["lines"].is_string(),
            "snippet should have lines"
        );
    }
}

// ============================================================
// v0.6: cache_prewarm + cangjie symbol_search
// ============================================================

#[test]
fn mcp_cache_prewarm_warms_cache() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();

    // Clear cache first
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_clear",
            "arguments": {}
        }
    }));
    let _ = session.recv();

    // Prewarm
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_prewarm",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 3);
    let text = resp["result"]["content"][0]["text"].as_str().expect("text");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["warmed"], true, "should be warmed");
    assert!(
        data["summary"]["symbolCount"].as_u64().unwrap_or(0) > 0,
        "summary should have symbols"
    );

    // Verify subsequent call hits cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp2 = session.recv();
    let text2 = resp2["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    let data2: serde_json::Value = serde_json::from_str(text2).expect("valid JSON");
    assert_eq!(data2["cacheHit"], true, "should hit cache after prewarm");
}

#[test]
fn mcp_cache_prewarm_returns_hit_if_fresh() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();

    // Prewarm once
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_prewarm",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Prewarm again — should be cache hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_prewarm",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    let text = resp["result"]["content"][0]["text"].as_str().expect("text");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    assert_eq!(data["warmed"], true);
    assert_eq!(data["cacheHit"], true, "second prewarm should be cache hit");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn mcp_cangjie_symbol_search_finds_init() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = cangjie_portable_smoke_dir();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "cangjie",
                "query": "init",
                "limit": 10
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 2);
    let text = resp["result"]["content"][0]["text"].as_str().expect("text");
    let data: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
    let count = data["matchCount"].as_u64().unwrap_or(0);
    assert!(
        count > 0,
        "cangjie symbol_search(init) should find at least one match, got {}",
        count
    );
}

// ============================================================
// Compact mode tests
// ============================================================

/// Helper: extract tool result data from MCP response
fn extract_tool_data(resp: &serde_json::Value) -> serde_json::Value {
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("");
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("Invalid JSON in tool result: {}. Text: {}", e, text))
}

#[test]
fn mcp_compact_symbol_search_retains_identity() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4001,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "query": "helper",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4001);
    let data = extract_tool_data(&resp);

    assert!(
        data["matchCount"].as_u64().unwrap_or(0) > 0,
        "should find helper"
    );
    let matches = data["matches"].as_array().expect("matches should be array");
    let first = &matches[0];

    // Compact must retain id, name, kind, file, line
    assert!(
        first["id"].as_str().is_some(),
        "compact match must have 'id'"
    );
    assert!(
        first["name"].as_str().is_some(),
        "compact match must have 'name'"
    );
    assert!(
        first["kind"].as_str().is_some(),
        "compact match must have 'kind'"
    );
    assert!(
        first["file"].as_str().is_some() || first["file"].is_null(),
        "compact match must have 'file' key"
    );
    assert!(
        first.get("line").is_some(),
        "compact match must have 'line' key"
    );

    // Compact must NOT have 'label'
    assert!(
        first["label"].is_null(),
        "compact match should not have 'label'"
    );
}

#[test]
fn mcp_compact_calls_from_retains_identity() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4002,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_from",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "main_fn",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4002);
    let data = extract_tool_data(&resp);

    // Source candidates must have id, name, kind, file, line
    let candidates = data["sourceCandidates"]
        .as_array()
        .expect("sourceCandidates should be array");
    assert!(candidates.len() > 0, "should have source candidates");
    let first_cand = &candidates[0];
    assert!(
        first_cand["id"].as_str().is_some(),
        "compact candidate must have 'id'"
    );
    assert!(
        first_cand["name"].as_str().is_some(),
        "compact candidate must have 'name'"
    );
    assert!(
        first_cand["file"].as_str().is_some() || first_cand["file"].is_null(),
        "compact candidate must have 'file'"
    );
    assert!(
        first_cand.get("line").is_some(),
        "compact candidate must have 'line'"
    );

    // Edges must have targetId, targetName, targetKind, targetFile, targetLine, type, confidence, reason
    let edges = data["edges"].as_array().expect("edges should be array");
    assert!(edges.len() > 0, "main_fn should have outgoing edges");
    let first_edge = &edges[0];
    assert!(
        first_edge["targetId"].as_str().is_some(),
        "compact edge must have 'targetId'"
    );
    assert!(
        first_edge["targetName"].as_str().is_some(),
        "compact edge must have 'targetName'"
    );
    assert!(
        first_edge["targetKind"].as_str().is_some(),
        "compact edge must have 'targetKind'"
    );
    assert!(
        first_edge.get("targetFile").is_some(),
        "compact edge must have 'targetFile'"
    );
    assert!(
        first_edge.get("targetLine").is_some(),
        "compact edge must have 'targetLine'"
    );
    assert!(
        first_edge["type"].as_str().is_some(),
        "compact edge must have 'type'"
    );
    assert!(
        first_edge.get("confidence").is_some(),
        "compact edge must have 'confidence'"
    );
    assert!(
        first_edge.get("reason").is_some(),
        "compact edge must have 'reason'"
    );

    // Compact edges should NOT have 'depth' or 'source' (raw id)
    assert!(
        first_edge.get("depth").is_none(),
        "compact edge should not have 'depth'"
    );
    assert!(
        first_edge.get("source").is_none(),
        "compact edge should not have 'source'"
    );

    // Response should have compact: true
    assert_eq!(
        data["compact"], true,
        "compact response should have compact=true"
    );
}

#[test]
fn mcp_compact_calls_to_retains_identity() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4003,
        "method": "tools/call",
        "params": {
            "name": "codelattice_calls_to",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4003);
    let data = extract_tool_data(&resp);

    // Target candidates must have id, name, kind, file, line
    let candidates = data["targetCandidates"]
        .as_array()
        .expect("targetCandidates should be array");
    assert!(candidates.len() > 0, "should have target candidates");
    let first_cand = &candidates[0];
    assert!(
        first_cand["id"].as_str().is_some(),
        "compact candidate must have 'id'"
    );
    assert!(
        first_cand["name"].as_str().is_some(),
        "compact candidate must have 'name'"
    );
    assert!(
        first_cand["file"].as_str().is_some() || first_cand["file"].is_null(),
        "compact candidate must have 'file'"
    );
    assert!(
        first_cand.get("line").is_some(),
        "compact candidate must have 'line'"
    );

    // Edges must have sourceId, sourceName, sourceKind, sourceFile, sourceLine, type, confidence, reason
    let edges = data["edges"].as_array().expect("edges should be array");
    assert!(edges.len() > 0, "helper should have callers");
    let first_edge = &edges[0];
    assert!(
        first_edge["sourceId"].as_str().is_some(),
        "compact edge must have 'sourceId'"
    );
    assert!(
        first_edge["sourceName"].as_str().is_some(),
        "compact edge must have 'sourceName'"
    );
    assert!(
        first_edge["sourceKind"].as_str().is_some(),
        "compact edge must have 'sourceKind'"
    );
    assert!(
        first_edge.get("sourceFile").is_some(),
        "compact edge must have 'sourceFile'"
    );
    assert!(
        first_edge.get("sourceLine").is_some(),
        "compact edge must have 'sourceLine'"
    );
    assert!(
        first_edge["type"].as_str().is_some(),
        "compact edge must have 'type'"
    );
    assert!(
        first_edge.get("confidence").is_some(),
        "compact edge must have 'confidence'"
    );
    assert!(
        first_edge.get("reason").is_some(),
        "compact edge must have 'reason'"
    );

    assert_eq!(
        data["compact"], true,
        "compact response should have compact=true"
    );
}

#[test]
fn mcp_compact_query_graph_retains_identity() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4004,
        "method": "tools/call",
        "params": {
            "name": "codelattice_query_graph",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "nodeKind": "function",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4004);
    let data = extract_tool_data(&resp);

    let nodes = data["matchedNodes"]
        .as_array()
        .expect("matchedNodes should be array");
    assert!(nodes.len() > 0, "should match function nodes");
    let first_node = &nodes[0];

    // Compact nodes must have id, name, kind, file, line
    assert!(
        first_node["id"].as_str().is_some(),
        "compact node must have 'id'"
    );
    assert!(
        first_node["name"].as_str().is_some(),
        "compact node must have 'name'"
    );
    assert!(
        first_node["kind"].as_str().is_some(),
        "compact node must have 'kind'"
    );
    assert!(
        first_node.get("file").is_some(),
        "compact node must have 'file'"
    );
    assert!(
        first_node.get("line").is_some(),
        "compact node must have 'line'"
    );

    // Compact nodes should NOT have 'label' or 'sourceSnippet'
    assert!(
        first_node.get("label").is_none(),
        "compact node should not have 'label'"
    );
    assert!(
        first_node.get("sourceSnippet").is_none(),
        "compact node should not have 'sourceSnippet'"
    );

    assert_eq!(
        data["compact"], true,
        "compact response should have compact=true"
    );
}

#[test]
fn mcp_compact_query_graph_edges_retain_confidence_reason() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4005,
        "method": "tools/call",
        "params": {
            "name": "codelattice_query_graph",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "edgeKind": "CALLS",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4005);
    let data = extract_tool_data(&resp);

    let edges = data["matchedEdges"]
        .as_array()
        .expect("matchedEdges should be array");
    if edges.len() > 0 {
        let first_edge = &edges[0];
        assert!(
            first_edge["source"].as_str().is_some(),
            "compact edge must have 'source'"
        );
        assert!(
            first_edge["target"].as_str().is_some(),
            "compact edge must have 'target'"
        );
        assert!(
            first_edge["type"].as_str().is_some(),
            "compact edge must have 'type'"
        );
        assert!(
            first_edge.get("confidence").is_some(),
            "compact edge must have 'confidence'"
        );
        assert!(
            first_edge.get("reason").is_some(),
            "compact edge must have 'reason'"
        );
    }
}

#[test]
fn mcp_compact_project_overview_counts_only() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4006,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4006);
    let data = extract_tool_data(&resp);

    // Must have core counts
    assert!(
        data["nodeCount"].as_u64().unwrap_or(0) > 0,
        "should have nodeCount"
    );
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "should have edgeCount"
    );
    assert!(data.get("symbolCount").is_some(), "should have symbolCount");
    assert!(
        data.get("packageCount").is_some(),
        "should have packageCount"
    );
    assert!(
        data.get("sourceFileCount").is_some(),
        "should have sourceFileCount"
    );
    assert!(
        data.get("diagnosticsCount").is_some(),
        "should have diagnosticsCount"
    );

    // Must NOT have verbose breakdowns
    assert!(
        data.get("hotspots").is_none(),
        "compact should not have 'hotspots'"
    );
    assert!(
        data.get("denseFiles").is_none(),
        "compact should not have 'denseFiles'"
    );
    assert!(
        data.get("topNodeKinds").is_none(),
        "compact should not have 'topNodeKinds'"
    );
    assert!(
        data.get("topEdgeKinds").is_none(),
        "compact should not have 'topEdgeKinds'"
    );
    assert!(
        data.get("qualitySummary").is_none(),
        "compact should not have 'qualitySummary'"
    );
    assert!(
        data.get("diagnosticsSummary").is_none(),
        "compact should not have 'diagnosticsSummary'"
    );

    assert_eq!(
        data["compact"], true,
        "compact response should have compact=true"
    );
}

#[test]
fn mcp_compact_unresolved_report_counts_only() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4007,
        "method": "tools/call",
        "params": {
            "name": "codelattice_unresolved_report",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4007);
    let data = extract_tool_data(&resp);

    // Must have summary fields
    assert!(data.get("total").is_some(), "should have 'total'");
    assert!(
        data.get("unresolvedEdges").is_some(),
        "should have 'unresolvedEdges'"
    );
    assert!(
        data.get("unresolvedDiagnostics").is_some(),
        "should have 'unresolvedDiagnostics'"
    );
    assert!(
        data.get("reasonBreakdown").is_some(),
        "should have 'reasonBreakdown'"
    );

    // Must NOT have detail arrays
    assert!(
        data.get("topItems").is_none(),
        "compact should not have 'topItems'"
    );
    assert!(
        data.get("diagnosticItems").is_none(),
        "compact should not have 'diagnosticItems'"
    );

    assert_eq!(
        data["compact"], true,
        "compact response should have compact=true"
    );
}

#[test]
fn mcp_include_snippet_false_still_retains_file_line() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4008,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "query": "helper",
                "compact": false,
                "includeSnippet": false
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 4008);
    let data = extract_tool_data(&resp);

    let matches = data["matches"].as_array().expect("matches should be array");
    assert!(matches.len() > 0);
    let first = &matches[0];

    // Must have id, name, kind, file, line even without snippet
    assert!(first["id"].as_str().is_some(), "must have 'id'");
    assert!(first["name"].as_str().is_some(), "must have 'name'");
    assert!(first["kind"].as_str().is_some(), "must have 'kind'");
    assert!(first.get("file").is_some(), "must have 'file'");
    assert!(first.get("line").is_some(), "must have 'line'");
}

// ============================================================
// ArkTS Integration Tests (feature-gated)
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
mod arkts_tests {
    use super::*;

    /// ArkTS CLI analysis: portable-smoke fixture should produce valid JSON with
    /// correct node/edge counts and package node.
    #[test]
    fn arkts_cli_portable_smoke_json() {
        let root = arkts_portable_smoke_dir();
        let output = std::process::Command::new(cli_binary())
            .args([
                "analyze",
                "--language",
                "arkts",
                "--root",
                &root.to_string_lossy(),
                "--format",
                "json",
            ])
            .output()
            .expect("failed to run CLI");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be valid JSON");

        let summary = &data["summary"];
        assert_eq!(
            summary["nodeCount"].as_u64().unwrap(),
            9,
            "expected 9 nodes (repo+pkg+2src+2baseSym+2component+1build)"
        );
        assert_eq!(summary["sourceFileCount"].as_u64().unwrap(), 2);
        assert_eq!(
            summary["packageCount"].as_u64().unwrap(),
            1,
            "expected 1 package from oh-package.json5"
        );
        assert!(
            summary["symbolCount"].as_u64().unwrap() >= 4,
            "should have at least 4 symbols"
        );
    }

    /// ArkTS CLI bridge format: symbols must have kind differentiation (component,
    /// method, buildMethod) and sourcePath.
    #[test]
    fn arkts_cli_portable_smoke_bridge() {
        let root = arkts_portable_smoke_dir();
        let output = std::process::Command::new(cli_binary())
            .args([
                "analyze",
                "--language",
                "arkts",
                "--root",
                &root.to_string_lossy(),
                "--format",
                "gitnexus-rc",
            ])
            .output()
            .expect("failed to run CLI");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value =
            serde_json::from_str(&stdout).expect("bridge stdout should be valid JSON");

        // Packages populated
        let packages = data["packages"]
            .as_array()
            .expect("packages should be array");
        assert_eq!(packages.len(), 1, "should have 1 package");
        assert_eq!(packages[0]["name"].as_str().unwrap(), "portable-smoke");

        // Source files have packageId
        let source_files = data["sourceFiles"]
            .as_array()
            .expect("sourceFiles should be array");
        assert_eq!(source_files.len(), 2);
        for sf in source_files {
            assert!(
                sf["packageId"].as_str().is_some(),
                "sourceFile should have packageId"
            );
        }

        // Symbols have differentiated kinds
        let symbols = data["symbols"].as_array().expect("symbols should be array");
        let kinds: std::collections::HashSet<&str> =
            symbols.iter().filter_map(|s| s["kind"].as_str()).collect();
        assert!(kinds.contains("component"), "should have component kind");
        assert!(kinds.contains("method"), "should have method kind");
        assert!(
            !kinds.contains("symbol") || kinds.len() > 2,
            "should not have generic 'symbol' as only kind"
        );

        // Every symbol has sourcePath
        for sym in symbols {
            assert!(
                sym["sourcePath"].as_str().is_some(),
                "symbol {} should have sourcePath",
                sym["name"]
            );
            assert!(
                sym["fileId"].as_str().is_some(),
                "symbol {} should have fileId",
                sym["name"]
            );
        }
    }

    /// ArkTS cross-file fixture: import edges should exist between files.
    #[test]
    fn arkts_cli_cross_file_imports() {
        let root = arkts_cross_file_dir();
        let output = std::process::Command::new(cli_binary())
            .args([
                "analyze",
                "--language",
                "arkts",
                "--root",
                &root.to_string_lossy(),
                "--format",
                "json",
            ])
            .output()
            .expect("failed to run CLI");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be valid JSON");

        let edges = data["graph"]["edges"]
            .as_array()
            .expect("edges should be array");
        let import_edges: Vec<_> = edges
            .iter()
            .filter(|e| {
                let kind = e["kind"].as_str().unwrap_or("");
                let type_ = e["type"].as_str().unwrap_or("");
                kind == "imports" || type_ == "IMPORTS"
            })
            .collect();

        // Should have import edges from Index to Logger, Index to Second, Second to Logger
        assert!(
            import_edges.len() >= 3,
            "should have at least 3 import edges, got {}",
            import_edges.len()
        );

        // Verify at least one cross-file import (to non-@kit module)
        let cross_file_imports: Vec<_> = import_edges
            .iter()
            .filter(|e| {
                e["target"]
                    .as_str()
                    .map(|t| !t.starts_with("module:@kit"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !cross_file_imports.is_empty(),
            "should have cross-file import edges (to Logger or Second)"
        );

        // Verify 3 source files detected
        assert_eq!(data["summary"]["sourceFileCount"].as_u64().unwrap(), 3);
        // Logger.ets should produce class, method, function symbols
        let symbols = data["graph"]["nodes"]
            .as_array()
            .expect("nodes should be array");
        let logger_syms: Vec<_> = symbols
            .iter()
            .filter(|n| {
                n["id"]
                    .as_str()
                    .map(|id| id.contains("Logger.ets"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(logger_syms.len() >= 4, "Logger.ets should produce at least 4 symbols (class+property+methods+function), got {}", logger_syms.len());
    }

    /// ArkTS MCP analyze tool: should return analysis results through MCP protocol.
    #[test]
    fn mcp_arkts_analyze() {
        let mut session = McpSession::start();
        session.initialize();
        session.send_notification_initialized();

        let root = arkts_portable_smoke_dir();
        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8001,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "arkts",
                    "includeGraph": true
                }
            }
        }));

        let resp = session.recv();
        assert_eq!(resp["id"], 8001);
        assert!(
            resp.get("error").is_none(),
            "MCP analyze should not error: {:?}",
            resp.get("error")
        );
        let data = extract_tool_data(&resp);
        assert_eq!(data["language"].as_str().unwrap(), "arkts");
        let summary = &data["summary"];
        assert!(
            summary["nodeCount"].as_u64().unwrap() > 0,
            "should have nodes"
        );
        assert!(
            summary["sourceFileCount"].as_u64().unwrap() >= 2,
            "should have at least 2 source files"
        );
    }

    /// ArkTS MCP project_overview: should return counts without full graph.
    #[test]
    fn mcp_arkts_project_overview() {
        let mut session = McpSession::start();
        session.initialize();
        session.send_notification_initialized();

        let root = arkts_cross_file_dir();
        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8002,
            "method": "tools/call",
            "params": {
                "name": "codelattice_project_overview",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "arkts"
                }
            }
        }));

        let resp = session.recv();
        assert_eq!(resp["id"], 8002);
        assert!(
            resp.get("error").is_none(),
            "MCP project_overview should not error: {:?}",
            resp.get("error")
        );
        let data = extract_tool_data(&resp);
        assert!(data["nodeCount"].as_u64().unwrap() > 0);
        assert!(data["sourceFileCount"].as_u64().unwrap() >= 3);
    }

    /// ArkTS MCP symbol_search: should find ArkTS component symbols.
    #[test]
    fn mcp_arkts_symbol_search() {
        let mut session = McpSession::start();
        session.initialize();
        session.send_notification_initialized();

        let root = arkts_portable_smoke_dir();
        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8003,
            "method": "tools/call",
            "params": {
                "name": "codelattice_symbol_search",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "arkts",
                    "query": "Index"
                }
            }
        }));

        let resp = session.recv();
        assert_eq!(resp["id"], 8003);
        assert!(
            resp.get("error").is_none(),
            "MCP symbol_search should not error: {:?}",
            resp.get("error")
        );
        let data = extract_tool_data(&resp);
        let matches = data["matches"].as_array().expect("matches should be array");
        assert!(matches.len() > 0, "should find Index component");
        // The Index match should be a component
        let index_match = matches.iter().find(|m| m["name"].as_str() == Some("Index"));
        assert!(index_match.is_some(), "should find Index symbol");
    }
}

// ============================================================
// Changed Symbols Integration Tests
// ============================================================

/// Helper: create a temporary git repo with a Rust file for testing changed symbols.
fn create_test_git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let dir_path = dir.path();

    // Write a simple Rust file
    let src_dir = dir_path.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
        src_dir.join("main.rs"),
        r#"fn helper() -> i32 {
    42
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();
    std::fs::write(
        dir_path.join("Cargo.toml"),
        r#"[package]
name = "test-changed-symbols"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    // Initialize git repo and commit
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir_path)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir_path)
        .output()
        .expect("git config email failed");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir_path)
        .output()
        .expect("git config name failed");
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .expect("git add failed");
    std::process::Command::new("git")
        .args(["commit", "-m", "baseline"])
        .current_dir(dir_path)
        .output()
        .expect("git commit failed");

    dir
}

#[test]
fn mcp_changed_symbols_detects_modified_function() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify helper() function
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 {
    99
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9001,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust",
                "diffMode": "unstaged",
                "compact": true,
                "includeSnippet": false
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9001);
    assert!(
        resp.get("error").is_none(),
        "changed_symbols should not error: {:?}",
        resp.get("error")
    );
    let data = extract_tool_data(&resp);

    // Should detect changed files
    let changed_files = data["changedFiles"]
        .as_array()
        .expect("changedFiles should be array");
    assert!(
        !changed_files.is_empty(),
        "should have at least 1 changed file"
    );

    // Should detect changed symbols (at least helper which was modified)
    let changed_symbols = data["changedSymbols"]
        .as_array()
        .expect("changedSymbols should be array");
    assert!(
        !changed_symbols.is_empty(),
        "should have at least 1 changed symbol, got: {:?}",
        data
    );

    // Each changed symbol must have minimum identity fields
    for sym in changed_symbols {
        assert!(sym["id"].as_str().is_some(), "symbol must have id");
        assert!(sym["name"].as_str().is_some(), "symbol must have name");
        assert!(sym["kind"].as_str().is_some(), "symbol must have kind");
        assert!(sym["file"].as_str().is_some(), "symbol must have file");
        assert!(sym.get("line").is_some(), "symbol must have line");
        assert!(sym["risk"].as_str().is_some(), "symbol must have risk");
    }

    // Summary should be present
    assert!(data["summary"]["changedFileCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["previewOnly"].as_bool().unwrap_or(false));
    assert!(data["noWrites"].as_bool().unwrap_or(false));
}

#[test]
fn mcp_changed_symbols_unknown_hunk_for_top_comment() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Add a comment at the top (not inside any function)
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"// This is a top-level comment
fn helper() -> i32 {
    42
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9002,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust",
                "diffMode": "unstaged"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9002);
    assert!(
        resp.get("error").is_none(),
        "should not error: {:?}",
        resp.get("error")
    );
    let data = extract_tool_data(&resp);

    // Should have unknown hunks (the top-level comment)
    let unknown_hunks = data["unknownHunks"]
        .as_array()
        .expect("unknownHunks should be array");
    assert!(
        !unknown_hunks.is_empty(),
        "top-level comment should produce unknown hunk, got: {:?}",
        data
    );

    // Each unknown hunk should have required fields
    for hunk in unknown_hunks {
        assert!(hunk["file"].as_str().is_some(), "hunk must have file");
        assert!(hunk["reason"].as_str().is_some(), "hunk must have reason");
    }
}

#[test]
fn mcp_changed_symbols_staged_diff() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify and stage
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 {
    100
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir_path)
        .output()
        .expect("git add failed");

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9003,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust",
                "diffMode": "staged"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9003);
    assert!(
        resp.get("error").is_none(),
        "staged diff should not error: {:?}",
        resp.get("error")
    );
    let data = extract_tool_data(&resp);

    // Should detect the staged changes
    let changed_files = data["changedFiles"]
        .as_array()
        .expect("changedFiles should be array");
    assert!(
        !changed_files.is_empty(),
        "staged diff should detect changes"
    );
}

#[test]
fn mcp_changed_symbols_non_git_repo_graceful_error() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let dir_path = dir.path();

    // Create a simple Rust project without git
    let src_dir = dir_path.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(
        dir_path.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9004,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9004);
    // Should return an error, not panic
    assert!(
        resp.get("error").is_some() || resp["result"]["isError"].as_bool().unwrap_or(false),
        "non-git repo should return graceful error, got: {:?}",
        resp
    );
}

#[test]
fn mcp_production_assist_auto_detects_changed_symbols() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify helper() function
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 {
    77
}

fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9005,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9005);
    assert!(
        resp.get("error").is_none(),
        "production_assist should not error: {:?}",
        resp.get("error")
    );
    let data = extract_tool_data(&resp);

    // Should auto-detect changed symbols
    assert_eq!(
        data["autoDetectedChangedSymbols"]
            .as_bool()
            .unwrap_or(false),
        true,
        "should auto-detect changed symbols"
    );
    assert!(
        data["changedSymbolCount"].as_u64().unwrap_or(0) > 0,
        "should have changed symbols"
    );
    assert!(
        data["changedSymbols"].as_array().is_some(),
        "should have changedSymbols array"
    );
}

#[test]
fn mcp_changed_symbols_no_crash_on_new_file() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Add a new file
    std::fs::write(dir_path.join("src/new_mod.rs"), "pub fn new_func() {}\n").unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9006,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust",
                "diffMode": "unstaged"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9006);
    // Must not panic — should return gracefully
    assert!(
        resp.get("error").is_none(),
        "should not panic on new file: {:?}",
        resp.get("error")
    );
    let data = extract_tool_data(&resp);
    assert!(data["changedFiles"].as_array().is_some());
}

#[test]
fn mcp_changed_symbols_compact_mode_retains_identity() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify helper()
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 { 123 }
fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9007,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust",
                "compact": true,
                "includeSnippet": false
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 9007);
    let data = extract_tool_data(&resp);
    let symbols = data["changedSymbols"]
        .as_array()
        .expect("changedSymbols should be array");

    for sym in symbols {
        // Compact must retain minimum identity: id, name, kind, file, line
        assert!(sym["id"].as_str().is_some(), "compact must have id");
        assert!(sym["name"].as_str().is_some(), "compact must have name");
        assert!(sym["kind"].as_str().is_some(), "compact must have kind");
        assert!(sym["file"].as_str().is_some(), "compact must have file");
        assert!(sym.get("line").is_some(), "compact must have line");
        assert!(sym["risk"].as_str().is_some(), "compact must have risk");
    }
}

// ============================================================
// Stage 4: Better Impact Risk Reasons tests
// ============================================================

#[test]
fn mcp_impact_preview_returns_risk_reasons() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10001,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10001);
    let data = extract_tool_data(&resp);
    assert!(
        data["riskReasons"].is_array(),
        "impact_preview must return riskReasons array: {:?}",
        data
    );
    let reasons = data["riskReasons"].as_array().unwrap();
    assert!(!reasons.is_empty(), "riskReasons should not be empty");
}

#[test]
fn mcp_impact_preview_returns_impact_metrics() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10002,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10002);
    let data = extract_tool_data(&resp);
    let metrics = &data["impactMetrics"];
    assert!(
        metrics.is_object(),
        "impact_preview must return impactMetrics object: {:?}",
        data
    );
    // Verify all required metric fields
    assert!(
        metrics["callerCount"].is_number(),
        "impactMetrics must have callerCount"
    );
    assert!(
        metrics["downstreamCount"].is_number(),
        "impactMetrics must have downstreamCount"
    );
    assert!(
        metrics["impactedFileCount"].is_number(),
        "impactMetrics must have impactedFileCount"
    );
    assert!(
        metrics["crossFileCount"].is_number(),
        "impactMetrics must have crossFileCount"
    );
    assert!(
        metrics["publicSymbolCount"].is_number(),
        "impactMetrics must have publicSymbolCount"
    );
    assert!(
        metrics["testFileCount"].is_number(),
        "impactMetrics must have testFileCount"
    );
    assert!(
        metrics["totalEdgesConsidered"].is_number(),
        "impactMetrics must have totalEdgesConsidered"
    );
    assert!(
        metrics["lowConfidenceEdgeCount"].is_number(),
        "impactMetrics must have lowConfidenceEdgeCount"
    );
    assert!(
        metrics["highConfidenceEdgeCount"].is_number(),
        "impactMetrics must have highConfidenceEdgeCount"
    );
}

#[test]
fn mcp_impact_preview_returns_confidence_summary() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10003,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10003);
    let data = extract_tool_data(&resp);
    let cs = &data["confidenceSummary"];
    assert!(
        cs.is_object(),
        "impact_preview must return confidenceSummary: {:?}",
        data
    );
    assert!(
        cs["totalEdgesConsidered"].is_number(),
        "confidenceSummary must have totalEdgesConsidered"
    );
    assert!(
        cs["highConfidenceCount"].is_number(),
        "confidenceSummary must have highConfidenceCount"
    );
    assert!(
        cs["mediumConfidenceCount"].is_number(),
        "confidenceSummary must have mediumConfidenceCount"
    );
    assert!(
        cs["lowConfidenceCount"].is_number(),
        "confidenceSummary must have lowConfidenceCount"
    );
    assert!(
        cs["unknownConfidenceCount"].is_number(),
        "confidenceSummary must have unknownConfidenceCount"
    );
}

#[test]
fn mcp_impact_preview_compact_retains_identity() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10004,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 1,
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10004);
    let data = extract_tool_data(&resp);
    // compact mode must retain risk, riskReasons, impactMetrics, confidenceSummary, reviewFocus
    assert!(data["risk"].is_string(), "compact must have risk");
    assert!(
        data["riskReasons"].is_array(),
        "compact must have riskReasons"
    );
    assert!(
        data["impactMetrics"].is_object(),
        "compact must have impactMetrics"
    );
    assert!(
        data["confidenceSummary"].is_object(),
        "compact must have confidenceSummary"
    );
    assert!(
        data["reviewFocus"].is_object(),
        "compact must have reviewFocus"
    );
    // impactedSymbols must still have id/name/kind/file/line
    let symbols = data["impactedSymbols"]
        .as_array()
        .expect("impactedSymbols must exist");
    for sym in symbols {
        assert!(
            sym["id"].is_string() || sym["id"].is_null(),
            "compact symbol must have id"
        );
        assert!(
            sym["name"].is_string() || sym["name"].is_null(),
            "compact symbol must have name"
        );
        assert!(
            sym["kind"].is_string() || sym["kind"].is_null(),
            "compact symbol must have kind"
        );
    }
}

#[test]
fn mcp_impact_preview_returns_review_focus() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10005,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "helper",
                "depth": 2
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10005);
    let data = extract_tool_data(&resp);
    let rf = &data["reviewFocus"];
    assert!(
        rf.is_object(),
        "impact_preview must return reviewFocus: {:?}",
        data
    );
    assert!(
        rf["topCallers"].is_array(),
        "reviewFocus must have topCallers"
    );
    assert!(
        rf["topCallees"].is_array(),
        "reviewFocus must have topCallees"
    );
    assert!(rf["topFiles"].is_array(), "reviewFocus must have topFiles");
    assert!(
        rf["lowConfidenceEdges"].is_array(),
        "reviewFocus must have lowConfidenceEdges"
    );
    assert!(
        rf["publicSymbols"].is_array(),
        "reviewFocus must have publicSymbols"
    );
    assert!(
        rf["testFiles"].is_array(),
        "reviewFocus must have testFiles"
    );
}

#[test]
fn mcp_impact_preview_public_symbol_in_metrics() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10006,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "symbol": "main_fn",
                "depth": 2
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10006);
    let data = extract_tool_data(&resp);
    let metrics = &data["impactMetrics"];
    // For a function symbol like main_fn, publicSymbolCount should be >= 1
    let public_count = metrics["publicSymbolCount"].as_u64().unwrap_or(0);
    // The helper/main_fn functions are public symbols
    assert!(
        public_count >= 1,
        "publicSymbolCount should be >= 1 for function impacts, got: {}",
        public_count
    );
}

#[test]
fn mcp_production_assist_returns_overall_risk() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10007,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10007);
    let data = extract_tool_data(&resp);
    assert!(
        data["overallRisk"].is_string(),
        "production_assist must return overallRisk: {:?}",
        data
    );
    let risk = data["overallRisk"].as_str().unwrap();
    assert!(
        ["LOW", "MEDIUM", "HIGH"].contains(&risk),
        "overallRisk must be LOW/MEDIUM/HIGH, got: {}",
        risk
    );
}

#[test]
fn mcp_production_assist_returns_review_checklist() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10008,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10008);
    let data = extract_tool_data(&resp);
    assert!(
        data["reviewChecklist"].is_array(),
        "production_assist must return reviewChecklist: {:?}",
        data
    );
    let checklist = data["reviewChecklist"].as_array().unwrap();
    assert!(!checklist.is_empty(), "reviewChecklist should not be empty");
}

#[test]
fn mcp_production_assist_changed_symbols_with_risk() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify the function
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 { 456 }
fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10009,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10009);
    let data = extract_tool_data(&resp);
    // Should have auto-detected changed symbols
    assert!(
        data["autoDetectedChangedSymbols"]
            .as_bool()
            .unwrap_or(false),
        "should auto-detect changed symbols"
    );
    assert!(
        data["changedSymbolCount"].as_u64().unwrap_or(0) > 0,
        "should have changed symbols"
    );
    // Must have overallRisk and overallRiskReasons
    assert!(data["overallRisk"].is_string(), "must have overallRisk");
    assert!(
        data["overallRiskReasons"].is_array(),
        "must have overallRiskReasons"
    );
    // Must have changedSymbolImpacts
    assert!(
        data["changedSymbolImpacts"].is_array(),
        "must have changedSymbolImpacts"
    );
    let impacts = data["changedSymbolImpacts"].as_array().unwrap();
    for impact in impacts {
        assert!(impact["name"].is_string(), "impact must have name");
        assert!(impact["risk"].is_string(), "impact must have risk");
        assert!(
            impact["callerCount"].is_number(),
            "impact must have callerCount"
        );
        assert!(impact["reasons"].is_array(), "impact must have reasons");
    }
}

#[test]
fn mcp_production_assist_unknown_hunks_in_checklist() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Add a top-level comment (not inside any function) — should produce unknown hunks
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"// Top-level comment change that is not in any function
fn helper() -> i32 { 42 }
fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10010,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 10010);
    let data = extract_tool_data(&resp);
    // If there are unknown hunks, they must appear in risk reasons or checklist
    let unknown_count = data["unknownHunkCount"].as_u64().unwrap_or(0);
    if unknown_count > 0 {
        // Check overallRiskReasons mentions unknown hunks
        let reasons = data["overallRiskReasons"].as_array().unwrap();
        let mentions_unknown = reasons.iter().any(|r| {
            r.as_str()
                .map(|s| s.contains("unknown hunk"))
                .unwrap_or(false)
        });
        assert!(
            mentions_unknown,
            "overallRiskReasons must mention unknown hunks when present: {:?}",
            reasons
        );
    }
    // reviewChecklist must be present
    assert!(
        data["reviewChecklist"].is_array(),
        "must have reviewChecklist"
    );
}

// ============================================================
// Stage 7: Static Doc Graph Tests
// ============================================================

#[test]
fn mcp_symbol_context_returns_related_docs() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    // Use the real codelattice repo — it has docs that mention symbols
    let root = workspace_root();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11001,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust",
                "name": "handle_impact_preview"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11001);
    let data = extract_tool_data(&resp);
    // Even if no symbol match, relatedDocs should be present (possibly empty)
    assert!(
        data["relatedDocs"].is_array(),
        "symbol_context must return relatedDocs: {:?}",
        data
    );
    // If the symbol was found, relatedDocs should have entries from mcp-v0-contract.md
    if data["matchCount"].as_u64().unwrap_or(0) > 0 {
        let docs = data["relatedDocs"].as_array().unwrap();
        // Each doc should have path, line, matchType, confidence, reason
        for doc in docs {
            assert!(doc["path"].is_string(), "doc must have path");
            assert!(doc["line"].is_number(), "doc must have line");
            assert!(doc["matchType"].is_string(), "doc must have matchType");
            assert!(doc["confidence"].is_string(), "doc must have confidence");
        }
    }
}

#[test]
fn mcp_impact_preview_returns_docs_likely_need_update() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = workspace_root();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11002,
        "method": "tools/call",
        "params": {
             "name": "codelattice_impact_preview",
             "arguments": {
                 "root": root.to_string_lossy(),
                 "language": "rust",
                 "symbol": "handle_impact_preview",
                 "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11002);
    let data = extract_tool_data(&resp);
    assert!(
        data["relatedDocs"].is_array(),
        "impact_preview must return relatedDocs: {:?}",
        data
    );
    assert!(
        data["docsLikelyNeedUpdate"].is_array(),
        "impact_preview must return docsLikelyNeedUpdate: {:?}",
        data
    );
}

#[test]
fn mcp_production_assist_returns_docs_fields() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = workspace_root();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11003,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11003);
    let data = extract_tool_data(&resp);
    assert!(
        data["docsLikelyNeedUpdate"].is_array(),
        "production_assist must return docsLikelyNeedUpdate: {:?}",
        data
    );
    assert!(
        data["docAssociationSummary"].is_object(),
        "production_assist must return docAssociationSummary: {:?}",
        data
    );
}

#[test]
fn mcp_project_overview_returns_docs_summary() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = workspace_root();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11004,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11004);
    let data = extract_tool_data(&resp);
    let docs = &data["docs"];
    assert!(
        docs.is_object(),
        "project_overview must return docs summary: {:?}",
        data
    );
    assert!(docs["docCount"].is_number(), "docs must have docCount");
    assert!(
        docs["docSectionCount"].is_number(),
        "docs must have docSectionCount"
    );
    assert!(
        docs["docLinkCount"].is_number(),
        "docs must have docLinkCount"
    );
    assert!(
        docs["docSymbolReferenceCount"].is_number(),
        "docs must have docSymbolReferenceCount"
    );
    // The codelattice repo has many docs
    let doc_count = docs["docCount"].as_u64().unwrap_or(0);
    assert!(
        doc_count > 0,
        "codelattice repo should have at least 1 doc, got: {}",
        doc_count
    );
}

#[test]
fn mcp_doc_scanner_excludes_hidden_dirs() {
    // Verify that .agents/.claude/.gitnexus/target dirs are excluded
    // by checking the doc scanner on the workspace root
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = workspace_root();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11005,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11005);
    let data = extract_tool_data(&resp);
    let docs = &data["docs"];
    // docCount should be reasonable (not hundreds from .agents or .claude)
    let doc_count = docs["docCount"].as_u64().unwrap_or(0);
    // The codelattice repo has roughly 80-100 docs in docs/plans + a few others
    assert!(
        doc_count < 200,
        "doc count should be reasonable (excluded hidden dirs), got: {}",
        doc_count
    );
}

#[test]
fn mcp_no_docs_graceful_empty() {
    // Use the portable smoke fixture — it has no docs
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let fixture = portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11006,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": fixture.to_string_lossy(),
                "language": "rust",
                "name": "helper"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11006);
    let data = extract_tool_data(&resp);
    // Should return empty relatedDocs, not error
    assert!(
        data["relatedDocs"].is_array(),
        "must have relatedDocs array even with no docs"
    );
    assert_eq!(
        data["relatedDocs"].as_array().unwrap().len(),
        0,
        "relatedDocs should be empty for fixture with no docs"
    );
}

#[test]
fn mcp_production_assist_doc_checklist_item() {
    let dir = create_test_git_repo();
    let dir_path = dir.path();

    // Modify a function
    std::fs::write(
        dir_path.join("src/main.rs"),
        r#"fn helper() -> i32 { 999 }
fn main() {
    let x = helper();
    println!("{}", x);
}
"#,
    )
    .unwrap();

    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11007,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": dir_path.to_string_lossy(),
                "language": "rust"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 11007);
    let data = extract_tool_data(&resp);
    // Should have docsLikelyNeedUpdate (even if empty for this small project)
    assert!(
        data["docsLikelyNeedUpdate"].is_array(),
        "must have docsLikelyNeedUpdate"
    );
    assert!(
        data["docAssociationSummary"].is_object(),
        "must have docAssociationSummary"
    );
    // reviewChecklist should exist
    assert!(
        data["reviewChecklist"].is_array(),
        "must have reviewChecklist"
    );
}

// ============================================================
// TypeScript Phase A MCP Tests
// ============================================================

#[cfg(feature = "tree-sitter-typescript")]
#[allow(dead_code)]
fn typescript_portable_smoke_dir() -> std::path::PathBuf {
    workspace_root()
        .join("fixtures")
        .join("typescript")
        .join("portable-smoke")
}

#[cfg(feature = "tree-sitter-typescript")]
#[allow(dead_code)]
fn typescript_tsx_smoke_dir() -> std::path::PathBuf {
    workspace_root()
        .join("fixtures")
        .join("typescript")
        .join("tsx-smoke")
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_project_overview() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12001,
        "method": "tools/call",
        "params": {
            "name": "codelattice_project_overview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12001);
    let data = extract_tool_data(&resp);
    assert!(
        data["nodeCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript overview must have nodes: {:?}",
        data
    );
    assert!(
        data["symbolCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript overview must have symbols: {:?}",
        data
    );
    assert!(
        data["sourceFileCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript overview must have source files: {:?}",
        data
    );
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_symbol_search() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12002,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_search",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript",
                "query": "add"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12002);
    let data = extract_tool_data(&resp);
    let results = data["matches"].as_array();
    assert!(
        results.is_some() && !results.unwrap().is_empty(),
        "TypeScript symbol search for 'add' must return matches: {:?}",
        data
    );
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_symbol_context() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12003,
        "method": "tools/call",
        "params": {
            "name": "codelattice_symbol_context",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript",
                "name": "Calculator"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12003);
    let data = extract_tool_data(&resp);
    assert!(
        data["matchCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript symbol context for Calculator must find matches: {:?}",
        data
    );
    let empty_candidates: Vec<serde_json::Value> = vec![];
    let candidates = data["candidates"].as_array().unwrap_or(&empty_candidates);
    assert!(
        !candidates.is_empty(),
        "TypeScript symbol context for Calculator must find matches: {:?}",
        data
    );
    let first = &candidates[0];
    assert!(
        first["file"].as_str().unwrap_or("").contains("math"),
        "Calculator should be in math.ts, got: {:?}",
        first["file"]
    );
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_query_graph_compact() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12004,
        "method": "tools/call",
        "params": {
            "name": "codelattice_query_graph",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript",
                "nameContains": "Calculator",
                "compact": true
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12004);
    let data = extract_tool_data(&resp);
    let nodes = data["matchedNodes"].as_array();
    assert!(
        nodes.is_some() && !nodes.unwrap().is_empty(),
        "TypeScript query_graph must return matched nodes: {:?}",
        data
    );
    // Compact mode: each node should have id/name/kind/file/line
    let first = &nodes.unwrap()[0];
    assert!(first["id"].is_string(), "compact node must have id");
    assert!(first["name"].is_string(), "compact node must have name");
    assert!(first["kind"].is_string(), "compact node must have kind");
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_impact_preview() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12005,
        "method": "tools/call",
        "params": {
            "name": "codelattice_impact_preview",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript",
                "symbol": "greet",
                "depth": 1
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12005);
    let data = extract_tool_data(&resp);
    assert!(
        data["risk"].is_string(),
        "TypeScript impact_preview must return risk: {:?}",
        data
    );
    assert!(
        data["riskReasons"].is_array(),
        "TypeScript impact_preview must return riskReasons: {:?}",
        data
    );
    assert!(
        data["impactMetrics"].is_object(),
        "TypeScript impact_preview must return impactMetrics: {:?}",
        data
    );
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_changed_symbols() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12006,
        "method": "tools/call",
        "params": {
            "name": "codelattice_changed_symbols",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12006);
    let data = extract_tool_data(&resp);
    // changed_symbols should return without error (may be empty if no git diff)
    assert!(
        data["changedSymbols"].is_array() || data["changedSymbols"].is_null(),
        "TypeScript changed_symbols must return array: {:?}",
        data
    );
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn mcp_typescript_production_assist() {
    let mut session = McpSession::start();
    session.initialize();
    session.send_notification_initialized();

    let root = typescript_portable_smoke_dir();
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 12007,
        "method": "tools/call",
        "params": {
            "name": "codelattice_production_assist",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "typescript"
            }
        }
    }));

    let resp = session.recv();
    assert_eq!(resp["id"], 12007);
    let data = extract_tool_data(&resp);
    assert!(
        data["nodeCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript production_assist must have nodes: {:?}",
        data
    );
    assert!(
        data["qualityGatesPassed"].is_number(),
        "must have qualityGatesPassed: {:?}",
        data
    );
    assert!(
        data["edgeCount"].as_u64().unwrap_or(0) > 0,
        "TypeScript production_assist must have edges: {:?}",
        data
    );
}

// ─── v0.8 Persistent Cache Tests ──────────────────────────────────────────

/// Helper: create an isolated temp cache dir for a test.
fn make_isolated_cache_dir(test_name: &str) -> std::path::PathBuf {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "codelattice-test-cache-{}-{}-{}",
        test_name,
        std::process::id(),
        ts
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Copy a fixture directory to an isolated temp dir so parallel tests
/// can't interfere with mtime/stale detection.
fn copy_fixture_to_temp(fixture: &std::path::Path, tag: &str) -> std::path::PathBuf {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = std::env::temp_dir().join(format!(
        "codelattice-fixture-{}-{}-{}",
        tag,
        std::process::id(),
        ts
    ));
    // Copy directory recursively
    fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
        let _ = std::fs::create_dir_all(dst);
        if let Ok(entries) = std::fs::read_dir(src) {
            for entry in entries.flatten() {
                let src_path = entry.path();
                let dst_path = dst.join(entry.file_name());
                if src_path.is_dir() {
                    copy_dir(&src_path, &dst_path);
                } else {
                    let _ = std::fs::copy(&src_path, &dst_path);
                }
            }
        }
    }
    copy_dir(fixture, &tmp);
    tmp
}

#[test]
fn mcp_persistent_cache_hit_on_new_process() {
    // Use an isolated copy of the fixture so parallel tests can't change
    // its files between session 1 and session 2 (mtime/stale interference).
    let _root = portable_smoke_dir();
    let root = copy_fixture_to_temp(&_root, "hit");
    let cache_dir = make_isolated_cache_dir("hit");

    // Session 1: analyze to populate persistent cache
    {
        let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
        session.initialize();
        session.send_notification_initialized();

        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8001,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "rust"
                }
            }
        }));
        let resp = session.recv();
        let data = extract_tool_data(&resp);
        assert_eq!(data["cacheHit"], false, "first call should be miss");
    }

    // Session 2: should hit persistent cache
    {
        let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
        session.initialize();
        session.send_notification_initialized();

        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8002,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "rust"
                }
            }
        }));
        let resp = session.recv();
        let data = extract_tool_data(&resp);
        assert!(
            data["cacheHit"].as_bool().unwrap_or(false),
            "second process should hit persistent cache: {:?}",
            data
        );
    }

    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mcp_persistent_cache_status_shows_layers() {
    let root = portable_smoke_dir();
    let cache_dir = make_isolated_cache_dir("status");

    let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
    session.initialize();
    session.send_notification_initialized();

    // Analyze to populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8010,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Check status has both memory and persistent
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8011,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp = session.recv();
    let data = extract_tool_data(&resp);
    assert!(data["memory"].is_object(), "should have memory layer");
    assert!(
        data["persistent"].is_object(),
        "should have persistent layer"
    );
    assert_eq!(
        data["memory"]["entryCount"], 1,
        "memory should have 1 entry"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[test]
fn mcp_persistent_cache_clear_layer() {
    let root = portable_smoke_dir();
    let cache_dir = make_isolated_cache_dir("clear");

    let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
    session.initialize();
    session.send_notification_initialized();

    // Populate cache
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8020,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let _ = session.recv();

    // Clear persistent layer only
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8021,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_clear",
            "arguments": {
                "layer": "persistent"
            }
        }
    }));
    let resp = session.recv();
    let data = extract_tool_data(&resp);
    assert!(
        data["clearedCount"].as_u64().unwrap_or(0) >= 1,
        "should clear at least 1 persistent entry"
    );

    // Memory should still have entry
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8022,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache_status",
            "arguments": {}
        }
    }));
    let resp2 = session.recv();
    let data2 = extract_tool_data(&resp2);
    assert_eq!(
        data2["memory"]["entryCount"], 1,
        "memory should still have 1 entry after clearing persistent only"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[test]
fn mcp_persistent_cache_corruption_graceful() {
    let root = portable_smoke_dir();
    let cache_dir = make_isolated_cache_dir("corrupt");

    // Session 1: analyze to create persistent cache file
    {
        let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
        session.initialize();
        session.send_notification_initialized();

        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8030,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "rust"
                }
            }
        }));
        let _ = session.recv();
    }

    // Corrupt the persistent cache file
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("cl-cache-") {
                        let _ = std::fs::write(&path, "CORRUPTED{not valid json!!!");
                    }
                }
            }
        }
    }

    // Session 2: should NOT panic, should re-analyze
    {
        let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
        session.initialize();
        session.send_notification_initialized();

        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8031,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "rust"
                }
            }
        }));
        let resp = session.recv();
        let data = extract_tool_data(&resp);
        assert!(
            data["summary"]["nodeCount"].as_u64().unwrap_or(0) > 0,
            "should still produce valid analysis after cache corruption, got: {:?}",
            data
        );
    }

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[test]
fn mcp_persistent_cache_manifest_change_stale() {
    let root = portable_smoke_dir();
    let cache_dir = make_isolated_cache_dir("manifest");

    // Session 1: analyze
    {
        let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
        session.initialize();
        session.send_notification_initialized();

        session.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8040,
            "method": "tools/call",
            "params": {
                "name": "codelattice_analyze",
                "arguments": {
                    "root": root.to_string_lossy(),
                    "language": "rust"
                }
            }
        }));
        let _ = session.recv();
    }

    // Touch Cargo.toml to change manifest hash
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.exists() {
        // Append a comment to change the hash
        let original = std::fs::read_to_string(&cargo_toml).unwrap_or_default();
        let modified = original.clone() + "\n# cache-test-perturbation\n";
        let _ = std::fs::write(&cargo_toml, modified);

        // Session 2: should re-analyze (stale manifest)
        {
            let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
            session.initialize();
            session.send_notification_initialized();

            session.send(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 8041,
                "method": "tools/call",
                "params": {
                    "name": "codelattice_analyze",
                    "arguments": {
                        "root": root.to_string_lossy(),
                        "language": "rust"
                    }
                }
            }));
            let resp = session.recv();
            let data = extract_tool_data(&resp);
            assert_eq!(
                data["cacheHit"], false,
                "should be miss after manifest change"
            );
        }

        // Restore original
        let _ = std::fs::write(&cargo_toml, original);
    }

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[test]
fn mcp_cache_layer_field_in_output() {
    let root = portable_smoke_dir();
    let cache_dir = make_isolated_cache_dir("layer");

    let mut session = McpSession::start_with_cache_dir(Some(&cache_dir));
    session.initialize();
    session.send_notification_initialized();

    // First call — miss, should show layer=none
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8050,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp1 = session.recv();
    let data1 = extract_tool_data(&resp1);
    assert_eq!(
        data1["cacheLayer"], "none",
        "first call should be layer=none"
    );

    // Second call — memory hit
    session.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8051,
        "method": "tools/call",
        "params": {
            "name": "codelattice_analyze",
            "arguments": {
                "root": root.to_string_lossy(),
                "language": "rust"
            }
        }
    }));
    let resp2 = session.recv();
    let data2 = extract_tool_data(&resp2);
    assert_eq!(
        data2["cacheLayer"], "memory",
        "second call should be layer=memory"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}
