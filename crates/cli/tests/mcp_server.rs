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
        let bin = cli_binary();
        let mut child = Command::new(bin)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start MCP server");

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
fn mcp_tools_list_returns_eighteen_tools() {
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
    assert_eq!(tools.len(), 18, "expected 18 tools, got {}", tools.len());

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
    assert!(data["symbolCount"].as_u64().unwrap_or(0) > 0);
    assert!(data["topNodeKinds"].is_object());
    assert!(data["topEdgeKinds"].is_object());
    assert!(data["qualitySummary"].is_object());
    assert!(data["diagnosticsSummary"].is_object());
    assert!(data["hotspots"].is_array());
    assert!(data["denseFiles"].is_array());
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
    assert_eq!(data["entryCount"], 0);
    assert_eq!(data["totalHits"], 0);
    assert_eq!(data["totalMisses"], 0);
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
    assert_eq!(data["entryCount"], 1);
    assert_eq!(data["totalMisses"], 1);
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
    assert_eq!(data["cacheHit"], true, "cross-tool call should be cache hit");
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
    assert_eq!(data2["entryCount"], 0, "cache should be empty after clear");
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
    assert_eq!(data["entryCount"], 1);
    assert_eq!(data["totalHits"], 2, "should have 2 hits after 3 analyze calls");
    assert_eq!(data["totalMisses"], 1, "should have 1 miss");
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
    assert!(data2["entryCount"].as_u64().unwrap_or(0) >= 2, "should have at least 2 cache entries");
}
