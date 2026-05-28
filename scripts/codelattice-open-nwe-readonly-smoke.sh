#!/usr/bin/env bash
# Read-only installed MCP smoke for /Users/jiangxuanyang/Desktop/open-nwe.
#
# This script does not write to open-nwe and does not execute its project code.
# It verifies the installed CodeLattice wrapper through real MCP JSON-RPC.

set -euo pipefail

TARGET_ROOT="${OPEN_NWE_ROOT:-/Users/jiangxuanyang/Desktop/open-nwe}"
WRAPPER="${CODELATTICE_MCP_WRAPPER:-/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh}"

if [[ ! -d "$TARGET_ROOT" ]]; then
    echo "SKIP: open-nwe not found at $TARGET_ROOT"
    exit 0
fi
if [[ ! -x "$WRAPPER" ]]; then
    echo "FAIL: installed wrapper is not executable: $WRAPPER" >&2
    exit 1
fi

BEFORE_STATUS="$(git -C "$TARGET_ROOT" status --short 2>/dev/null || true)"
export BEFORE_STATUS

python3 - "$TARGET_ROOT" "$WRAPPER" <<'PY'
import json
import os
import subprocess
import sys
import time

root = sys.argv[1]
wrapper = sys.argv[2]

env = os.environ.copy()
env.pop("CODELATTICE_MCP_TOOLSET", None)
env["CODELATTICE_MCP_TOOLSET"] = "ai"
proc = subprocess.Popen(
    ["bash", wrapper],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    env=env,
)
next_id = 1


def send(payload):
    assert proc.stdin is not None
    proc.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
    proc.stdin.flush()


def recv_id(wanted_id, timeout=180):
    assert proc.stdout is not None
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = proc.stdout.readline()
        if not line:
            break
        doc = json.loads(line)
        if doc.get("id") == wanted_id:
            return doc
    stderr = proc.stderr.read() if proc.stderr is not None else ""
    raise AssertionError(f"missing response id={wanted_id}; stderr={stderr}")


def request(method, params=None, timeout=180):
    global next_id
    request_id = next_id
    next_id += 1
    payload = {"jsonrpc": "2.0", "id": request_id, "method": method}
    if params is not None:
        payload["params"] = params
    send(payload)
    return recv_id(request_id, timeout=timeout)


def call_tool(name, arguments, timeout=180):
    resp = request(
        "tools/call",
        {"name": name, "arguments": arguments},
        timeout=timeout,
    )
    content = resp.get("result", {}).get("content", [])
    if not content:
        raise AssertionError(f"tool response has no content: {resp}")
    return json.loads(content[0]["text"])


try:
    init = request(
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "open-nwe-readonly-smoke", "version": "1.0"},
        },
        timeout=30,
    )
    assert init["result"]["serverInfo"]["toolset"] == "ai", init
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    tools = request("tools/list", timeout=30)["result"]["tools"]
    assert len(tools) == 6, len(tools)

    job = call_tool(
        "codelattice_workspace",
        {"mode": "job", "root": root, "language": "auto", "compact": True},
        timeout=240,
    )
    serialized = json.dumps(job, separators=(",", ":"))
    summary = job.get("summary") or {}
    if summary.get("projects") is not None:
        raise AssertionError("workspace job response embedded full projects list")
    if len(serialized) > 60000:
        raise AssertionError(f"workspace compact job response too large: {len(serialized)} bytes")
    job_id = job.get("jobId")
    if not job_id:
        raise AssertionError(f"workspace job response missing jobId: {job}")
    print(f"PASS: open-nwe workspace job compact response bytes={len(serialized)} jobId={job_id}")

    status = call_tool("codelattice_workspace", {"mode": "job_status", "jobId": job_id}, timeout=60)
    assert status.get("jobId") == job_id, status
    assert "status" in status, status
    print(f"PASS: job_status without root status={status.get('status')}")

    detail = call_tool(
        "codelattice_workspace",
        {"mode": "job_detail", "jobId": job_id, "page": 0, "pageSize": 5},
        timeout=60,
    )
    required = ["page", "pageSize", "totalItems", "totalPages", "hasMore", "items"]
    missing = [key for key in required if key not in detail]
    if missing:
        raise AssertionError(f"job_detail missing paging fields {missing}: {detail}")
    if len(detail.get("items", [])) > 5:
        raise AssertionError(f"job_detail pageSize not honored: {detail}")
    print(f"PASS: job_detail paging totalItems={detail.get('totalItems')} pageSize={detail.get('pageSize')}")

    cache = call_tool("codelattice_cache", {"mode": "status", "compact": True}, timeout=60)
    if cache.get("error") == "mcp_server_busy":
        raise AssertionError(f"cache status stayed busy after job finished: {cache}")
    if "schemaVersion" not in cache:
        raise AssertionError(f"cache status did not return normal facade payload: {cache}")
    print("PASS: codelattice_cache(mode=status) recovered after workspace job")
finally:
    try:
        send({"jsonrpc": "2.0", "method": "shutdown"})
        if proc.stdin is not None:
            proc.stdin.close()
    except BrokenPipeError:
        pass
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise
PY

AFTER_STATUS="$(git -C "$TARGET_ROOT" status --short 2>/dev/null || true)"

echo "open-nwe git status before:"
printf '%s\n' "${BEFORE_STATUS:-<clean>}"
echo "open-nwe git status after:"
printf '%s\n' "${AFTER_STATUS:-<clean>}"

if [[ "$BEFORE_STATUS" != "$AFTER_STATUS" ]]; then
    echo "FAIL: open-nwe git status changed during read-only smoke" >&2
    exit 1
fi

echo "Open-nwe read-only smoke passed."
