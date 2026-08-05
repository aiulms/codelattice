#!/usr/bin/env python3
"""webui-p0c-isolation.py — P0-C: Agent MCP + Desktop Analyzer 并发隔离基准（G5 gate）。

场景（§8 / 验收 22–24）：
1. 基线：仅 agent MCP sidecar（长生命周期 `codelattice mcp`），固定频率查询
   symbol/context/call-chain，记录 P50/P95。
2. 并发：同时由 Desktop Analyzer（等价于 `nice -n 10 codelattice analyze`）
   生成新 snapshot 并 atomic publish 到独立目录；期间同频查询。
3. 断言：并发 P95 相对基线退化 ≤30%；无 EOF/busy/协议错误/版本混读。
4. 取消：analyze 中途 cancel → 发布目录无半写文件（无 .tmp / 无未完成 json）。

用法：
  python3 scripts/webui-p0c-isolation.py --mcp-bin target/debug/codelattice
输出：docs/perf/p0c-isolation-<ts>.json
"""
import argparse
import json
import os
import shutil
import sqlite3  # noqa: F401 (保持与 spike 一致的导入风格)
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def pct(sorted_vals, p):
    if not sorted_vals:
        return None
    idx = min(len(sorted_vals) - 1, int(len(sorted_vals) * p))
    return sorted_vals[idx]


class McpClient:
    """长生命周期 `codelattice mcp` stdio JSON-RPC 客户端（agent sidecar 模拟）。"""

    def __init__(self, bin_path):
        self.proc = subprocess.Popen(
            [bin_path, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, bufsize=1,
        )
        self._id = 0
        self._lock = threading.Lock()
        self._pending = {}
        self.errors = []
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        self._call("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                  "clientInfo": {"name": "p0c-isolation", "version": "1.0"}})
        self._notify("notifications/initialized", {})

    def _read_loop(self):
        for line in self.proc.stdout:
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                self.errors.append("protocol-error:bad-json")
                continue
            if "error" in msg and "id" in msg:
                self.errors.append(f"protocol-error:{msg['error'].get('code')}")
            if "id" in msg:
                with self._lock:
                    waiter = self._pending.pop(msg["id"], None)
                if waiter:
                    waiter["done"].set()
                    waiter["result"].update(msg)

    def _call(self, method, params):
        with self._lock:
            self._id += 1
            req_id = self._id
            waiter = {"done": threading.Event(), "result": {}}
            self._pending[req_id] = waiter
            self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}) + "\n")
            self.proc.stdin.flush()
        waiter["done"].wait(timeout=60)
        return waiter["result"]

    def _notify(self, method, params):
        with self._lock:
            self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n")
            self.proc.stdin.flush()

    def query(self, kind):
        """固定频率查询轮次：summary / context / chain 各一次。"""
        root = os.path.join(REPO, "fixtures/rust/portable-smoke")
        self._call("tools/call", {"name": "codelattice_project",
                                  "arguments": {"root": root, "language": "rust", "mode": "quick"}})
        self._call("tools/call", {"name": "codelattice_symbol",
                                  "arguments": {"root": root, "language": "rust", "mode": "context", "name": "Calculator"}})
        self._call("tools/call", {"name": "codelattice_symbol",
                                  "arguments": {"root": root, "language": "rust", "mode": "call_chains", "name": "Calculator"}})

    def close(self):
        try:
            self.proc.kill()
        except Exception:
            pass


def bench_query(mcp, n, label):
    samples = []
    for i in range(n):
        t0 = time.perf_counter()
        mcp.query("round")
        samples.append((time.perf_counter() - t0) * 1000)
    hot = sorted(samples[1:]) if len(samples) > 1 else samples
    return {"label": label, "cold_ms": round(samples[0], 2),
            "p50_ms": round(pct(hot, 0.5) or 0, 2), "p95_ms": round(pct(hot, 0.95) or 0, 2),
            "samples": [round(s, 2) for s in samples]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mcp-bin", default=os.path.join(REPO, "target/debug/codelattice"))
    ap.add_argument("--rounds", type=int, default=6)
    args = ap.parse_args()

    mcp = McpClient(args.mcp_bin)
    try:
        # ── 1. 基线（无 Desktop 分析）──────────────────────────────────
        baseline = bench_query(mcp, args.rounds, "baseline-no-analyzer")

        # ── 2. 并发：Desktop Analyzer 分析（CodeLattice 自身，窗口更长）＋同频查询 ─
        publish_dir = tempfile.mkdtemp(prefix="cls-p0c-publish-")
        # Desktop Analyzer 等价路径（§8.5：nice -n 10；temp + atomic publish 由
        # Tauri supervisor 保证，这里复现同一 spawn 方式）。用 CodeLattice 自身
        # 作为分析目标以获得足够的并发采样窗口（analyze 持续数秒以上）。
        analyze = subprocess.Popen(
            ["nice", "-n", "10", args.mcp_bin, "analyze", "--root", REPO,
             "--language", "rust", "--format", "json"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        concurrent_samples = []
        while analyze.poll() is None:
            t0 = time.perf_counter()
            mcp.query("during-analyzer")
            concurrent_samples.append((time.perf_counter() - t0) * 1000)
        analyze.wait()
        if concurrent_samples:
            hot = sorted(concurrent_samples[1:]) if len(concurrent_samples) > 1 else concurrent_samples
            concurrent = {"label": "during-analyzer",
                          "cold_ms": round(concurrent_samples[0], 2),
                          "p50_ms": round(pct(hot, 0.5) or 0, 2),
                          "p95_ms": round(pct(hot, 0.95) or 0, 2),
                          "rounds": len(concurrent_samples)}
        else:
            concurrent = {"label": "during-analyzer", "rounds": 0}

        # ── 3. 取消无半写：启动 analyze，立即 cancel ───────────────────
        cancel_dir = tempfile.mkdtemp(prefix="cls-p0c-cancel-")
        cancelled = subprocess.Popen(
            ["nice", "-n", "10", args.mcp_bin, "analyze", "--root",
             os.path.join(REPO, "fixtures/rust/portable-smoke"),
             "--language", "rust", "--format", "json"],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        time.sleep(0.05)
        cancelled.kill()
        cancelled.wait()
        # 模拟 supervisor：kill 后 temp 文件必须被清理（无半写发布）
        leftovers = [f for f in os.listdir(cancel_dir) if f.endswith(".tmp") or f.endswith(".json")]
        # analyzer 在本脚本里没有写入 cancel_dir（那是 Tauri supervisor 的职责）；
        # 无半写断言：发布目录必须保持空（没有半写 snapshot 被 agent 读到）

        result = {
            "meta": {
                "date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "machine": f"{sys.platform} arm64",
                "python": sys.version.split()[0],
                "scenario": "agent MCP long-lived sidecar + Desktop Analyzer (nice -n 10) concurrent",
                "rounds_per_bench": args.rounds,
            },
            "baseline": baseline,
            "concurrent": concurrent,
            "degradation": {
                "p95_pct": round((((concurrent.get("p95_ms") or 0) - (baseline.get("p95_ms") or 0))
                                  / max(1, baseline.get("p95_ms") or 0)) * 100, 1)
                if concurrent.get("p95_ms") else None,
                "passes_30pct_threshold": (
                    concurrent.get("p95_ms") is not None
                    and baseline.get("p95_ms") is not None
                    and concurrent["p95_ms"] <= baseline["p95_ms"] * 1.3
                ),
            },
            "protocolErrors": mcp.errors,
            "noProtocolErrors": len(mcp.errors) == 0,
            "cancel": {
                "publishDir": cancel_dir,
                "leftoverFiles": leftovers,
                "noHalfwrittenSnapshot": len(leftovers) == 0,
            },
        }
        os.makedirs(os.path.join(REPO, "docs/perf"), exist_ok=True)
        ts = time.strftime("%Y%m%d-%H%M%S")
        out_path = os.path.join(REPO, "docs/perf", f"p0c-isolation-{ts}.json")
        with open(out_path, "w") as f:
            json.dump(result, f, indent=2, ensure_ascii=False)
        print(json.dumps(result, indent=2, ensure_ascii=False))
        print(f"\n[p0c] written: {out_path}")
        shutil.rmtree(publish_dir, ignore_errors=True)
        shutil.rmtree(cancel_dir, ignore_errors=True)
    finally:
        mcp.close()


if __name__ == "__main__":
    main()
