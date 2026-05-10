//! Integration tests for MCP v0.1 stdio server.
//!
//! Tests start the binary with `mcp` subcommand, communicate via stdin/stdout
//! using newline-delimited JSON-RPC, and verify responses.
//!
//! Covers v0 (4 tools) + v0.1 (4 tools) = 8 tools total.

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
fn mcp_tools_list_returns_eight_tools() {
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
    assert_eq!(tools.len(), 8, "expected 8 tools, got {}", tools.len());

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
