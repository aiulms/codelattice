#!/usr/bin/env bash
# CodeLattice MCP concurrency smoke
# 验证同一 MCP stdio session 收到多个未完成 tools/call 时不会断连。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALLED_WRAPPER="${CODELATTICE_MCP_WRAPPER:-/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh}"
DEFAULT_BINARY="$REPO_ROOT/target/release/codelattice"
if [ ! -x "$DEFAULT_BINARY" ]; then
    DEFAULT_BINARY="$REPO_ROOT/target/debug/codelattice"
fi
BINARY="${CODELATTICE_MCP_BIN:-$DEFAULT_BINARY}"
if [[ -x "$INSTALLED_WRAPPER" && -z "${CODELATTICE_MCP_BIN:-}" ]]; then
    MCP_EXEC="$INSTALLED_WRAPPER"
    MCP_EXEC_KIND="wrapper"
else
    MCP_EXEC="$BINARY"
    MCP_EXEC_KIND="binary"
fi

OUT="$(mktemp -t codelattice-mcp-concurrency-out.XXXXXX)"
ERR="$(mktemp -t codelattice-mcp-concurrency-err.XXXXXX)"
REQ="$(mktemp -t codelattice-mcp-concurrency-req.XXXXXX)"
trap 'rm -f "$OUT" "$ERR" "$REQ"' EXIT

python3 - "$REPO_ROOT" > "$REQ" <<'PY'
import json
import sys

root = sys.argv[1]
requests = [
    {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "concurrency-smoke", "version": "1.0"},
        },
    },
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "codelattice_workspace",
            "arguments": {"root": root, "mode": "graph", "compact": True},
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "codelattice_cache",
            "arguments": {"mode": "status", "compact": True},
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "codelattice_workflow",
            "arguments": {"root": root, "mode": "onboarding", "compact": True},
        },
    },
]
for request in requests:
    print(json.dumps(request, separators=(",", ":")))
PY

CODELATTICE_MCP_EXEC="$MCP_EXEC" CODELATTICE_MCP_EXEC_KIND="$MCP_EXEC_KIND" CODELATTICE_MCP_REQ="$REQ" CODELATTICE_MCP_OUT="$OUT" CODELATTICE_MCP_ERR="$ERR" python3 - <<'PY'
import os
import subprocess
import sys

env = os.environ.copy()
env["CODELATTICE_MCP_TOOLSET"] = "ai"
exec_path = os.environ["CODELATTICE_MCP_EXEC"]
cmd = ["bash", exec_path] if os.environ["CODELATTICE_MCP_EXEC_KIND"] == "wrapper" else [exec_path, "mcp"]
with open(os.environ["CODELATTICE_MCP_REQ"], "r", encoding="utf-8") as stdin, \
     open(os.environ["CODELATTICE_MCP_OUT"], "w", encoding="utf-8") as stdout, \
     open(os.environ["CODELATTICE_MCP_ERR"], "w", encoding="utf-8") as stderr:
    try:
        proc = subprocess.run(
            cmd,
            stdin=stdin,
            stdout=stdout,
            stderr=stderr,
            env=env,
            timeout=30,
            text=True,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit("MCP server timed out during concurrency smoke")
if proc.returncode != 0:
    raise SystemExit(f"MCP server exited {proc.returncode}")
PY

python3 - "$OUT" <<'PY'
import json
import sys

responses = []
busy = 0
for line in open(sys.argv[1], encoding="utf-8"):
    if not line.strip():
        continue
    doc = json.loads(line)
    if doc.get("id") in (2, 3, 4):
        responses.append(doc)
        text = ""
        if "result" in doc:
            content = doc["result"].get("content", [])
            if content:
                text = content[0].get("text", "")
        if "mcp_server_busy" in text:
            busy += 1

ids = sorted(doc.get("id") for doc in responses)
assert ids, "no tools/call responses returned"
assert 2 in ids, f"primary request did not return: ids={ids}"
assert len(ids) >= 2, f"server returned too few responses before disconnect: ids={ids}"
assert set(ids) == {2, 3, 4}, f"server did not return all concurrent responses: ids={ids}"
print(f"PASS: responses={ids} busy={busy}")
PY

CODELATTICE_MCP_EXEC="$MCP_EXEC" CODELATTICE_MCP_EXEC_KIND="$MCP_EXEC_KIND" CODELATTICE_REPO_ROOT="$REPO_ROOT" python3 - <<'PY'
import json
import os
import subprocess
import sys
import time

exec_path = os.environ["CODELATTICE_MCP_EXEC"]
cmd = ["bash", exec_path] if os.environ["CODELATTICE_MCP_EXEC_KIND"] == "wrapper" else [exec_path, "mcp"]
root = os.environ["CODELATTICE_REPO_ROOT"]
env = os.environ.copy()
env["CODELATTICE_MCP_TOOLSET"] = "ai"

proc = subprocess.Popen(
    cmd,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    env=env,
)

def send(payload):
    assert proc.stdin is not None
    proc.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
    proc.stdin.flush()

def read_until(wanted, timeout=30):
    assert proc.stdout is not None
    deadline = time.time() + timeout
    seen = {}
    while time.time() < deadline and wanted - set(seen):
        line = proc.stdout.readline()
        if not line:
            break
        doc = json.loads(line)
        if doc.get("id") in wanted:
            seen[doc["id"]] = doc
    missing = wanted - set(seen)
    if missing:
        raise AssertionError(f"missing MCP responses after busy check: {sorted(missing)}")
    return seen

try:
    send({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "concurrency-recovery-smoke", "version": "1.0"},
        },
    })
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    first_batch = [
        (2, "codelattice_workspace", {"root": root, "mode": "graph", "compact": True}),
        (3, "codelattice_cache", {"mode": "status", "compact": True}),
        (4, "codelattice_workflow", {"root": root, "mode": "onboarding", "compact": True}),
    ]
    for request_id, tool, arguments in first_batch:
        send({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {"name": tool, "arguments": arguments},
        })

    responses = read_until({2, 3, 4})
    busy_count = 0
    for doc in responses.values():
        content = doc.get("result", {}).get("content", [])
        text = content[0].get("text", "") if content else ""
        if "mcp_server_busy" in text:
            busy_count += 1
    send({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {"name": "codelattice_cache", "arguments": {"mode": "status", "compact": True}},
    })
    recovery = read_until({5}, timeout=15)[5]
    content = recovery.get("result", {}).get("content", [])
    recovery_text = content[0].get("text", "") if content else ""
    if "mcp_server_busy" in recovery_text:
        raise AssertionError("server stayed busy after primary call finished")
    if "schemaVersion" not in recovery_text:
        raise AssertionError("post-busy recovery call did not return a normal tool payload")
    if "Connection closed" in recovery_text or "No such tool available" in recovery_text:
        raise AssertionError(f"unexpected client-facing transport/tool error: {recovery_text}")
    if busy_count:
        print(f"PASS: live session recovered after busy responses (busy={busy_count})")
    else:
        print("PASS: live session returned concurrent responses without busy and stayed usable")
finally:
    if proc.stdin is not None:
        try:
            proc.stdin.close()
        except BrokenPipeError:
            pass
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise
    if proc.returncode not in (0, None):
        stderr = proc.stderr.read() if proc.stderr is not None else ""
        raise SystemExit(f"MCP recovery process exited {proc.returncode}\n{stderr}")
PY

echo "MCP concurrency smoke passed."
