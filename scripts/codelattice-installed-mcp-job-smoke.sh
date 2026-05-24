#!/usr/bin/env bash
# CodeLattice installed MCP job runtime smoke.
#
# Uses the promoted stable wrapper and a real newline-delimited MCP JSON-RPC
# session. Every session performs initialize, notifications/initialized, and
# then tools/list or tools/call.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL_DIR="${CODELATTICE_TOOL_DIR:-/Users/jiangxuanyang/Desktop/CodeLattice-Tool}"
WRAPPER="${CODELATTICE_MCP_WRAPPER:-$TOOL_DIR/codelattice-mcp.sh}"

if [[ ! -x "$WRAPPER" ]]; then
    echo "FAIL: installed wrapper is not executable: $WRAPPER" >&2
    exit 1
fi

python3 - "$REPO_ROOT" "$WRAPPER" <<'PY'
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

repo = Path(sys.argv[1])
wrapper = Path(sys.argv[2])
fixture = repo / "fixtures" / "call-resolution" / "c1-same-module"
expected_facades = [
    "codelattice_workflow",
    "codelattice_project",
    "codelattice_symbol",
    "codelattice_change_review",
    "codelattice_workspace",
    "codelattice_cache",
]


class McpSession:
    def __init__(self, toolset="ai"):
        env = os.environ.copy()
        env.pop("CODELATTICE_MCP_TOOLSET", None)
        env["CODELATTICE_MCP_TOOLSET"] = toolset
        self.proc = subprocess.Popen(
            ["bash", str(wrapper)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
        self.next_id = 1

    def send(self, payload):
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        self.proc.stdin.flush()

    def recv_id(self, wanted_id, timeout=60):
        assert self.proc.stdout is not None
        deadline = time.time() + timeout
        while time.time() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                break
            doc = json.loads(line)
            if doc.get("id") == wanted_id:
                return doc
        stderr = self.proc.stderr.read() if self.proc.stderr is not None else ""
        raise AssertionError(f"missing response id={wanted_id}; stderr={stderr}")

    def initialize(self):
        request_id = self.next_id
        self.next_id += 1
        self.send({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "installed-job-smoke", "version": "1.0"},
            },
        })
        resp = self.recv_id(request_id)
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        return resp

    def tools_list(self):
        request_id = self.next_id
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": request_id, "method": "tools/list"})
        return self.recv_id(request_id)["result"]["tools"]

    def call_tool(self, name, arguments, timeout=90):
        request_id = self.next_id
        self.next_id += 1
        self.send({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        })
        resp = self.recv_id(request_id, timeout=timeout)
        content = resp.get("result", {}).get("content", [])
        if not content:
            raise AssertionError(f"tool response has no content: {resp}")
        return json.loads(content[0]["text"])

    def close(self):
        try:
            self.send({"jsonrpc": "2.0", "method": "shutdown"})
            if self.proc.stdin is not None:
                self.proc.stdin.close()
        except BrokenPipeError:
            pass
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            raise


def assert_paging(data):
    required = ["page", "pageSize", "totalItems", "totalPages", "hasMore", "items"]
    missing = [key for key in required if key not in data]
    if missing:
        raise AssertionError(f"missing paging fields {missing}: {data}")
    if not isinstance(data["items"], list):
        raise AssertionError(f"paging items must be an array: {data}")


def make_workspace():
    temp = tempfile.TemporaryDirectory(prefix="codelattice-installed-job-")
    root = Path(temp.name)
    rust_src = root / "rust-app" / "src"
    rust_src.mkdir(parents=True)
    (root / "rust-app" / "Cargo.toml").write_text(
        "[package]\nname=\"rust-app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        encoding="utf-8",
    )
    (rust_src / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    py_root = root / "python-tool"
    py_root.mkdir()
    (py_root / "pyproject.toml").write_text("[project]\nname=\"python-tool\"\n", encoding="utf-8")
    (py_root / "main.py").write_text("def main():\n    return 1\n", encoding="utf-8")
    return temp, root


session = McpSession("ai")
try:
    init = session.initialize()
    info = init["result"]["serverInfo"]
    assert info["toolset"] == "ai", info
    tools = session.tools_list()
    names = [tool["name"] for tool in tools]
    assert names == expected_facades, names
    print(f"PASS: default AI tools/list exposes exactly {len(names)} facade tools")

    workspace_tmp, workspace_root = make_workspace()
    try:
        job_specs = [
            ("codelattice_project", {"root": str(fixture), "language": "rust", "mode": "job", "compact": True}),
            ("codelattice_symbol", {"root": str(fixture), "language": "rust", "mode": "job", "compact": True}),
            ("codelattice_change_review", {"root": str(fixture), "language": "rust", "mode": "job", "compact": True}),
            ("codelattice_workspace", {"root": str(workspace_root), "language": "auto", "mode": "job", "compact": True}),
        ]
        job_ids = {}
        for facade, args in job_specs:
            job = session.call_tool(facade, args)
            job_id = job.get("jobId")
            if not job_id:
                raise AssertionError(f"{facade} job did not return jobId: {job}")
            if not job.get("compactResult"):
                raise AssertionError(f"{facade} job was not compact: {job}")
            if facade == "codelattice_workspace" and job.get("summary", {}).get("projects") is not None:
                raise AssertionError(f"workspace job response embedded projects: {job}")
            job_ids[facade] = job_id

            status = session.call_tool(facade, {"mode": "job_status", "jobId": job_id})
            assert status.get("jobId") == job_id, status
            assert "status" in status, status

            detail = session.call_tool(
                facade,
                {"mode": "job_detail", "jobId": job_id, "page": 0, "pageSize": 2},
            )
            assert detail.get("jobId") == job_id, detail
            assert_paging(detail)
        print("PASS: facade job/job_status/job_detail keep jobIds within one MCP session")

        invalid = session.call_tool(
            "codelattice_workspace",
            {"mode": "job_status", "jobId": "job_engine_missing"},
        )
        assert invalid.get("error") == "job_not_found", invalid
        print("PASS: invalid jobId returns structured job_not_found")
    finally:
        workspace_tmp.cleanup()
finally:
    session.close()

full = McpSession("full")
try:
    init = full.initialize()
    info = init["result"]["serverInfo"]
    tools = full.tools_list()
    assert len(tools) == int(info["toolCount"]), (len(tools), info)
    print(f"PASS: full tools/list count = {len(tools)}")
finally:
    full.close()
PY
