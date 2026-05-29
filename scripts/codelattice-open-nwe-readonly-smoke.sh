#!/usr/bin/env bash
# Read-only MCP smoke for /Users/jiangxuanyang/Desktop/open-nwe.
#
# This script does not write to open-nwe and does not execute its project code.
# It verifies the selected CodeLattice wrapper through real MCP JSON-RPC.
# Uses a temporary CODELATTICE_CACHE_DIR to force clean-cache behavior.

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

# Force clean cache via temp directory
CACHE_DIR="$(mktemp -d /tmp/codelattice-smoke-cache.XXXXXX)"
trap 'rm -rf "$CACHE_DIR"' EXIT
export CODELATTICE_CACHE_DIR="$CACHE_DIR"

python3 - "$TARGET_ROOT" "$WRAPPER" <<'PY'
import json
import os
import select
import subprocess
import sys
import time

root = sys.argv[1]
wrapper = sys.argv[2]

class McpClient:
    def __init__(self, toolset=None):
        env = os.environ.copy()
        if toolset is None:
            env.pop("CODELATTICE_MCP_TOOLSET", None)
        else:
            env["CODELATTICE_MCP_TOOLSET"] = toolset
        self.proc = subprocess.Popen(
            ["bash", wrapper],
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

    def recv_id(self, wanted_id, timeout=180):
        assert self.proc.stdout is not None
        deadline = time.time() + timeout
        while time.time() < deadline:
            remaining = max(0.0, deadline - time.time())
            ready, _, _ = select.select([self.proc.stdout], [], [], min(0.25, remaining))
            if not ready:
                if self.proc.poll() is not None:
                    break
                continue
            line = self.proc.stdout.readline()
            if not line:
                break
            doc = json.loads(line)
            if doc.get("id") == wanted_id:
                return doc
        stderr = ""
        if self.proc.stderr is not None:
            ready, _, _ = select.select([self.proc.stderr], [], [], 0)
            if ready:
                stderr = self.proc.stderr.read()
        raise AssertionError(f"missing response id={wanted_id}; stderr={stderr}")

    def request(self, method, params=None, timeout=180):
        request_id = self.next_id
        self.next_id += 1
        payload = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            payload["params"] = params
        self.send(payload)
        return self.recv_id(request_id, timeout=timeout)

    def call_tool(self, name, arguments, timeout=180):
        resp = self.request(
            "tools/call",
            {"name": name, "arguments": arguments},
            timeout=timeout,
        )
        content = resp.get("result", {}).get("content", [])
        if not content:
            raise AssertionError(f"tool response has no content: {resp}")
        return json.loads(content[0]["text"])

    def close(self):
        try:
            self.send({"jsonrpc": "2.0", "method": "shutdown"})
            if self.proc.stdin is not None:
                self.proc.stdin.close()
        except (BrokenPipeError, OSError):
            pass
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=10)


def initialized_client(toolset=None):
    client = McpClient(toolset=toolset)
    init = client.request(
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "open-nwe-readonly-smoke", "version": "1.0"},
        },
        timeout=30,
    )
    client.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    return client, init


def tool_count(toolset):
    client, init = initialized_client(toolset)
    try:
        tools = client.request("tools/list", timeout=30)["result"]["tools"]
        return init["result"]["serverInfo"], len(tools)
    finally:
        client.close()


def unwrap_tool(data):
    return data.get("result", data)


def digest_counts(summary):
    facade_digest = summary.get("facadeDigest") or {}
    ai_digest = summary.get("aiDigest") or {}
    digest = facade_digest if facade_digest.get("symbolCount", 0) else ai_digest
    return {
        "facadeSymbolCount": summary.get("facadeSymbolCount")
            or facade_digest.get("facadeSymbolCount")
            or ai_digest.get("facadeSymbolCount")
            or 0,
        "symbolCount": digest.get("symbolCount", 0),
        "callEdgeCount": digest.get("callEdgeCount", 0),
        "topSymbolsPreview": digest.get("topSymbolsPreview") or [],
        "topFiles": digest.get("topFiles") or [],
        "facadeWarmDurationMs": summary.get("facadeWarmDurationMs")
            or digest.get("warmDurationMs")
            or 0,
    }


try:
    default_info, default_count = tool_count(None)
    assert default_info["toolset"] == "ai", default_info
    assert default_count == 6, default_count
    print("PASS: default toolset exposes 6 tools")

    full_info, full_count = tool_count("full")
    assert full_info["toolset"] == "full", full_info
    assert full_count == 49, full_count
    print("PASS: full toolset exposes 49 tools")

    client, init = initialized_client(None)
    assert init["result"]["serverInfo"]["toolset"] == "ai", init

    # ─── Symbol search 闭环测试 (backend, Rust) ───
    backend_root = root + "/backend"
    if os.path.isdir(backend_root):
        # Step 1: clean-cache symbol search must return analyzing + jobId
        search_started = time.monotonic()
        search1 = client.call_tool(
            "codelattice_symbol",
            {"mode": "search", "root": backend_root, "language": "rust",
             "query": "preview_delegation_context_snapshot", "compact": True},
            timeout=60,
        )
        inner1 = unwrap_tool(search1)
        if inner1.get("matchCount", 0) > 0:
            raise AssertionError(f"clean-cache first search unexpectedly hit cache: {search1}")
        job_id = inner1.get("jobId") or search1.get("jobId")
        status = inner1.get("status") or search1.get("status")
        assert status == "analyzing" and job_id, \
            f"clean-cache first search should return analyzing + jobId, got {search1}"
        print(f"PASS: symbol search clean-cache miss status=analyzing jobId={job_id}")

        # Step 2: poll job_status until succeeded and ensure progress elapsedMs follows wall-clock
        last_status = None
        warming_samples = []
        for attempt in range(600):
            time.sleep(0.5)
            js = client.call_tool(
                "codelattice_project",
                {"mode": "job_status", "jobId": job_id, "compact": True},
                timeout=60,
            )
            js_inner = unwrap_tool(js)
            st = js_inner.get("status", js.get("status"))
            progress = js_inner.get("progress") or js.get("progress") or {}
            stage = progress.get("stage")
            elapsed_ms = int(progress.get("elapsedMs") or 0)
            wall_ms = int((time.monotonic() - search_started) * 1000)
            if stage == "warming_facade_cache":
                warming_samples.append((wall_ms, elapsed_ms, progress.get("wallClockMs")))
                if wall_ms >= 5000 and elapsed_ms < 4000:
                    raise AssertionError(
                        f"warming progress elapsedMs is stale: wallMs={wall_ms} progress={progress}"
                    )
            last_status = js_inner
            if st == "succeeded":
                break
            if st == "failed":
                raise AssertionError(f"job failed: {js}")
        else:
            raise AssertionError(f"job did not complete in 300s; last={last_status}")

        warm_wall_seconds = time.monotonic() - search_started
        summary = last_status.get("summary") or {}
        counts = digest_counts(summary)
        assert summary.get("facadeCacheReady") is True, summary
        assert counts["facadeSymbolCount"] > 0, summary
        assert counts["symbolCount"] > 0, summary
        assert counts["callEdgeCount"] > 0, summary
        assert counts["topSymbolsPreview"], summary
        top_file = counts["topFiles"][0].get("file", "") if counts["topFiles"] else ""
        assert top_file and top_file != "rust" and top_file.endswith(".rs"), \
            f"topFiles should contain real source paths, got {counts['topFiles']}"
        print(
            "PASS: job succeeded "
            f"wallClockSeconds={warm_wall_seconds:.2f} "
            f"facadeSymbolCount={counts['facadeSymbolCount']} "
            f"symbolCount={counts['symbolCount']} "
            f"callEdgeCount={counts['callEdgeCount']}"
        )
        print(
            "METRIC: backend_warm_wall_clock_seconds="
            f"{warm_wall_seconds:.2f} facadeSymbolCount={counts['facadeSymbolCount']} "
            f"symbolCount={counts['symbolCount']} callEdgeCount={counts['callEdgeCount']} "
            f"facadeWarmDurationMs={counts['facadeWarmDurationMs']}"
        )
        if warming_samples:
            wall_ms, elapsed_ms, reported_wall = warming_samples[-1]
            print(
                "PASS: warming progress sample "
                f"wallMs={wall_ms} elapsedMs={elapsed_ms} reportedWallMs={reported_wall}"
            )

        # Step 3: retry symbol search (should hit cache)
        search2 = client.call_tool(
            "codelattice_symbol",
            {"mode": "search", "root": backend_root, "language": "rust",
             "query": "preview_delegation_context_snapshot", "compact": True},
            timeout=60,
        )
        inner2 = unwrap_tool(search2)
        match_count = inner2.get("matchCount", 0)
        assert match_count >= 1, f"symbol search after job should find >= 1 match, got {search2}"
        summary_match_count = search2.get("summary", {}).get("matchCount")
        assert summary_match_count == match_count, \
            f"compact summary.matchCount should mirror result.matchCount: {search2}"
        assert 1 <= len(search2.get("summary", {}).get("topMatches", [])) <= 5, \
            f"compact summary.topMatches should contain bounded preview: {search2}"
        matches = inner2.get("matches", [])
        first = matches[0] if matches else {}
        name = first.get("name", "?")
        file = first.get("file", "?")
        line = first.get("line", "?")
        assert "delegation_context_snapshot_handlers" in str(file), \
            f"expected file to include delegation_context_snapshot_handlers.rs, got {file}"
        assert str(line) == "55", f"expected line 55, got {line}"
        print(f"PASS: symbol search found {match_count} match(es): name={name} file={file} line={line}")
        print(f"METRIC: symbol_search_hit name={name} file={file} line={line} matchCount={match_count}")

        # Step 4: change_review impact must resolve the warmed symbol
        impact = client.call_tool(
            "codelattice_change_review",
            {"mode": "impact", "root": backend_root, "language": "rust",
             "symbol": "preview_delegation_context_snapshot", "compact": True},
            timeout=60,
        )
        impact_inner = unwrap_tool(impact)
        risk = impact_inner.get("risk", "UNKNOWN")
        assert risk != "UNKNOWN", f"impact should not be UNKNOWN: {impact}"
        assert "Symbol not found" not in json.dumps(impact, ensure_ascii=False), impact
        print(f"PASS: change_review impact risk={risk}")
        print(f"METRIC: impact_risk={risk}")
    else:
        print(f"SKIP: backend root not found at {backend_root}")
finally:
    if "client" in locals():
        client.close()
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
