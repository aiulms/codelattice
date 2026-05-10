//! Integration tests for MCP v0 stdio server.
//!
//! Tests start the binary with `mcp` subcommand, communicate via stdin/stdout
//! using newline-delimited JSON-RPC, and verify responses.

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
fn mcp_tools_list_returns_four_tools() {
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
    assert_eq!(tools.len(), 4, "expected 4 tools, got {}", tools.len());

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
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
