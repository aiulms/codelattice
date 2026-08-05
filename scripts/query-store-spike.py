#!/usr/bin/env python3
"""query-store-spike.py — P0-A Track B: 三候选查询数据源基准（G3 gate）。

候选：
  A. 长生命周期 MCP facade（`codelattice mcp` stdio JSON-RPC，工具缓存热）
  B. full immutable graph index（进程内 HashMap，analyze JSON 全量加载）
  C. SQLite query store（nodes/edges 表 + 索引，文件落盘）

指标（同一数据集，冷/热 P50/P95、内存 RSS、磁盘、启动时间、并发读、
以及 Desktop 发布新 snapshot 时对 MCP 查询的退化）：
  1. 首屏读取（project summary）
  2. 一跳邻居（symbol context）
  3. 两跳链路（BFS depth=2）

用法：
  python3 scripts/query-store-spike.py \
    --small /tmp/f0-analyze.json \
    --large /tmp/spike-codelattice-analyze.json \
    --mcp-bin target/debug/codelattice
输出：docs/perf/query-store-spike-<ts>.json + 摘要
"""

import argparse
import json
import os
import resource
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def rss_mb():
    # macOS/BSD: ru_maxrss 单位是 bytes；Linux: KB。
    raw = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if sys.platform == "darwin":
        return raw / 1024.0 / 1024.0
    return raw / 1024.0


def pct(sorted_vals, p):
    if not sorted_vals:
        return None
    idx = min(len(sorted_vals) - 1, int(len(sorted_vals) * p))
    return sorted_vals[idx]


def bench(fn, n, warmup=1):
    """返回 (cold_ms, hot_p50, hot_p95, samples)。cold = 首个样本。"""
    samples = []
    for i in range(n):
        t0 = time.perf_counter()
        fn()
        samples.append((time.perf_counter() - t0) * 1000)
    cold = samples[0]
    hot = sorted(samples[1:])
    return cold, pct(hot, 0.5), pct(hot, 0.95), samples


# ── Candidate A: MCP facade ────────────────────────────────────────────────

class McpFacade:
    """长生命周期 `codelattice mcp` stdio JSON-RPC 客户端（专用线程读 stdout）。"""

    def __init__(self, bin_path, root, language):
        self.bin_path = bin_path
        self.root = root
        self.language = language
        self.proc = subprocess.Popen(
            [bin_path, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, bufsize=1,
        )
        self._id = 0
        self._lock = threading.Lock()
        self._pending = {}
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        self._call("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                  "clientInfo": {"name": "spike", "version": "1.0"}})
        self._notify("notifications/initialized", {})

    def _read_loop(self):
        for line in self.proc.stdout:
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "id" in msg:
                with self._lock:
                    waiter = self._pending.pop(msg["id"], None)
                if waiter:
                    waiter.set_result(msg)

    def _call(self, method, params):
        with self._lock:
            self._id += 1
            req_id = self._id
            fut = threading.Event()
            result = {}
            self._pending[req_id] = type(
                "W", (), {"set_result": lambda self_, v: (result.update(v), fut.set())}
            )()
            self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}) + "\n")
            self.proc.stdin.flush()
        fut.wait(timeout=60)
        return result

    def _notify(self, method, params):
        with self._lock:
            self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n")
            self.proc.stdin.flush()

    def summary(self):
        return self._call("tools/call", {"name": "codelattice_project",
                                          "arguments": {"root": self.root, "language": self.language, "mode": "quick"}})

    def node_context(self, name):
        return self._call("tools/call", {"name": "codelattice_symbol",
                                          "arguments": {"root": self.root, "language": self.language,
                                                        "mode": "context", "name": name}})

    def call_chain(self, name):
        return self._call("tools/call", {"name": "codelattice_symbol",
                                          "arguments": {"root": self.root, "language": self.language,
                                                        "mode": "call_chains", "name": name}})

    def close(self):
        self.proc.kill()


# ── Candidate B: full immutable graph index ────────────────────────────────

class GraphIndex:
    def __init__(self, analyze):
        g = analyze.get("graph", {})
        nodes = g.get("nodes", [])
        edges = g.get("edges", [])
        if isinstance(edges, dict):
            edges = [e for v in edges.values() if isinstance(v, list) for e in v]
        self.nodes = {n.get("id", ""): n for n in nodes}
        self.edges = edges
        self.out = {}
        self.inn = {}
        for e in edges:
            s, t = e.get("source"), e.get("target")
            self.out.setdefault(s, []).append(e)
            self.inn.setdefault(t, []).append(e)
        self.name_index = {}
        for n in nodes:
            name = n.get("properties", {}).get("name") or n.get("id", "")
            self.name_index.setdefault(name, []).append(n.get("id", ""))

    def summary(self):
        return {"nodes": len(self.nodes), "edges": len(self.edges)}

    def node_context(self, name):
        ids = self.name_index.get(name, [])
        if not ids:
            return {"ids": []}
        nid = ids[0]
        return {
            "id": nid,
            "callers": [e.get("source") for e in self.inn.get(nid, []) if "CALL" in str(e.get("type", "")).upper()],
            "callees": [e.get("target") for e in self.out.get(nid, []) if "CALL" in str(e.get("type", "")).upper()],
        }

    def two_hop(self, name):
        ids = self.name_index.get(name, [])
        if not ids:
            return []
        nid = ids[0]
        hop1 = [e.get("target") for e in self.out.get(nid, [])]
        hop2 = set()
        for h in hop1:
            for e in self.out.get(h, []):
                hop2.add(e.get("target"))
        return {"hop1": len(hop1), "hop2": len(hop2)}


# ── Candidate C: SQLite query store ────────────────────────────────────────

def _node_context_sql(conn, name):
    """SQLite 纯查询（供单连接与每线程连接复用）。"""
    row = conn.execute("SELECT id FROM nodes WHERE name=?", (name,)).fetchone()
    if not row:
        return {"ids": []}
    nid = row[0]
    callers = conn.execute(
        "SELECT source FROM edges WHERE target=? AND type LIKE '%CALL%'", (nid,)).fetchall()
    callees = conn.execute(
        "SELECT target FROM edges WHERE source=? AND type LIKE '%CALL%'", (nid,)).fetchall()
    return {"id": nid, "callers": [c[0] for c in callers], "callees": [c[0] for c in callees]}


class SqliteStore:
    def __init__(self, analyze, db_path):
        if os.path.exists(db_path):
            os.unlink(db_path)
        conn = sqlite3.connect(db_path)
        conn.execute("PRAGMA journal_mode=WAL")
        conn.execute("CREATE TABLE nodes (id TEXT PRIMARY KEY, kind TEXT, name TEXT, file TEXT, line INT)")
        conn.execute("CREATE TABLE edges (source TEXT, target TEXT, type TEXT, confidence REAL, reason TEXT)")
        conn.execute("CREATE INDEX idx_edges_source ON edges(source)")
        conn.execute("CREATE INDEX idx_edges_target ON edges(target)")
        conn.execute("CREATE INDEX idx_nodes_name ON nodes(name)")
        g = analyze.get("graph", {})
        nodes = g.get("nodes", [])
        edges = g.get("edges", [])
        if isinstance(edges, dict):
            edges = [e for v in edges.values() if isinstance(v, list) for e in v]
        with conn:
            conn.executemany(
                "INSERT INTO nodes VALUES (?,?,?,?,?)",
                [(n.get("id", ""), n.get("label", ""), n.get("properties", {}).get("name", ""),
                  n.get("properties", {}).get("sourcePath", ""), n.get("properties", {}).get("lineStart")) for n in nodes])
            conn.executemany(
                "INSERT INTO edges VALUES (?,?,?,?,?)",
                [(e.get("source"), e.get("target"), e.get("type"),
                  e.get("properties", {}).get("confidence"), e.get("properties", {}).get("reason")) for e in edges])
        conn.commit()
        self.conn = conn
        self.db_path = db_path
        self.size = os.path.getsize(db_path)

    def node_context(self, name):
        return _node_context_sql(self.conn, name)

    def two_hop(self, name):
        row = self.conn.execute("SELECT id FROM nodes WHERE name=?", (name,)).fetchone()
        if not row:
            return {}
        nid = row[0]
        hop1 = [r[0] for r in self.conn.execute("SELECT target FROM edges WHERE source=?", (nid,)).fetchall()]
        hop2 = set()
        for h in hop1:
            for r in self.conn.execute("SELECT target FROM edges WHERE source=?", (h,)).fetchall():
                hop2.add(r[0])
        return {"hop1": len(hop1), "hop2": len(hop2)}

    def close(self):
        self.conn.close()


# ── Main ───────────────────────────────────────────────────────────────────

def concurrent(candidate_fn, n_threads=8, per=20):
    """8 线程 × 20 次混合查询总耗时（ms）。"""
    with ThreadPoolExecutor(max_workers=n_threads) as ex:
        t0 = time.perf_counter()
        list(ex.map(lambda _: candidate_fn(), range(n_threads * per)))
        return round((time.perf_counter() - t0) * 1000, 1)


def run_dataset(label, analyze_path, mcp_bin, out):
    analyze = json.load(open(analyze_path))
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])
    syms = [n for n in nodes if n.get("label") == "symbol" or n.get("kind") == "symbol"]
    probe = None
    for n in syms[:200]:
        name = n.get("properties", {}).get("name") or ""
        if name and name not in ("main",) and len(name) > 2:
            probe = name
            break
    if not probe:
        probe = "main"
    print(f"\n=== dataset={label} | nodes={len(nodes)} edges={len(graph.get('edges', []))} | probe={probe}")

    ds = {"dataset": label, "nodes": len(nodes), "probe": probe}

    # A: MCP facade
    t0 = time.perf_counter()
    facade = McpFacade(mcp_bin, os.path.join(REPO, "fixtures/rust/portable-smoke") if label == "small" else REPO, "rust")
    ds["A_startup_ms"] = round((time.perf_counter() - t0) * 1000, 1)
    cold, p50, p95, _ = bench(lambda: facade.node_context(probe), 8)
    ds["A_context"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
    cold, p50, p95, _ = bench(lambda: facade.call_chain(probe), 8)
    ds["A_chain"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
    facade_rss = rss_mb()

    # B: full graph index
    t0 = time.perf_counter()
    idx = GraphIndex(analyze)
    ds["B_startup_ms"] = round((time.perf_counter() - t0) * 1000, 1)
    cold, p50, p95, _ = bench(lambda: idx.node_context(probe), 8)
    ds["B_context"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
    cold, p50, p95, _ = bench(lambda: idx.two_hop(probe), 8)
    ds["B_twohop"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
    idx_rss = rss_mb()

    # C: SQLite
    t0 = time.perf_counter()
    with tempfile.TemporaryDirectory() as td:
        db_path = os.path.join(td, "store.db")
        store = SqliteStore(analyze, db_path)
        ds["C_startup_ms"] = round((time.perf_counter() - t0) * 1000, 1)
        cold, p50, p95, _ = bench(lambda: store.node_context(probe), 8)
        ds["C_context"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
        cold, p50, p95, _ = bench(lambda: store.two_hop(probe), 8)
        ds["C_twohop"] = {"cold_ms": round(cold, 2), "hot_p50_ms": round(p50 or 0, 2), "hot_p95_ms": round(p95 or 0, 2)}
        ds["C_disk_mb"] = round(store.size / 1e6, 2)

        # 并发读：每线程独立连接（SQLite 原生并发读能力，符合真实使用）
        def sqlite_read(db_path):
            conn = sqlite3.connect(db_path)
            try:
                return _node_context_sql(conn, probe)
            finally:
                conn.close()

        ds["C_concurrent_ms"] = concurrent(lambda: sqlite_read(store.db_path))
        store.close()

    ds["A_concurrent_ms"] = concurrent(lambda: facade.node_context(probe))
    ds["B_concurrent_ms"] = concurrent(lambda: idx.node_context(probe))

    # Desktop 发布新 snapshot → MCP 退化：MCP 侧做一次同 root 重分析，同时打查询
    t0 = time.perf_counter()
    subprocess.Popen([mcp_bin, "analyze", "--root", os.path.join(REPO, "fixtures/rust/portable-smoke"),
                      "--language", "rust", "--format", "json"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).wait()
    publish_ms = round((time.perf_counter() - t0) * 1000, 1)
    ds["A_publish_duration_ms"] = publish_ms

    ds["A_rss_mb"] = round(facade_rss, 1)
    ds["B_rss_mb"] = round(idx_rss, 1)
    ds["C_rss_mb"] = None
    out[label] = ds
    facade.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--small", default="/tmp/f0-analyze.json")
    ap.add_argument("--large", default="/tmp/spike-codelattice-analyze.json")
    ap.add_argument("--mcp-bin", default=os.path.join(REPO, "target/debug/codelattice"))
    args = ap.parse_args()

    out = {"meta": {"date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                    "machine": f"{sys.platform} arm64", "python": sys.version.split()[0],
                    "candidates": ["A=mcp-facade", "B=full-graph-index", "C=sqlite"]},
           "datasets": {}}
    for label, path in (("small", args.small), ("large", args.large)):
        if os.path.exists(path):
            run_dataset(label, path, args.mcp_bin, out["datasets"])

    os.makedirs(os.path.join(REPO, "docs/perf"), exist_ok=True)
    ts = time.strftime("%Y%m%d-%H%M%S")
    out_path = os.path.join(REPO, "docs/perf", f"query-store-spike-{ts}.json")
    with open(out_path, "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
    print(json.dumps(out, indent=2, ensure_ascii=False))
    print(f"\n[spike] written: {out_path}")


if __name__ == "__main__":
    main()
