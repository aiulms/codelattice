// P0-F0 characterization: snapshot JSON structure, truncation markers, key counts.
// Freezes the observable contract of CodeLatticeWebSnapshotV1 (webui.snapshot.v1)
// as produced by scripts/webui-snapshot.sh + scripts/codelattice-snapshot-gen.py.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "../../..");
const SNAPSHOT = path.join(WS, "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");

const REQUIRED_TOP_KEYS = [
  "schemaVersion", "generatedAt", "generatedFrom", "summary",
  "quality", "limitations", "explore", "cleanup", "releaseReview",
  "insights", "workflowPresets", "graph",
];

function loadFixture() {
  assert.ok(existsSync(SNAPSHOT), `fixture snapshot missing: ${SNAPSHOT}`);
  return JSON.parse(readFileSync(SNAPSHOT, "utf8"));
}

test("fixture snapshot top-level structure matches webui.snapshot.v1", () => {
  const snap = loadFixture();
  assert.equal(snap.schemaVersion, "webui.snapshot.v1");
  for (const k of REQUIRED_TOP_KEYS) {
    assert.ok(k in snap, `missing top-level key: ${k}`);
  }
  // generatedFrom invariant: static-only; runtimeVerified is always false.
  // NOTE: current generator emits {tool, toolVersion, snapshotSchema,
  // staticAnalysis, runtimeVerified, generationMethod} — external/coverage/
  // deletion verified flags are NOT emitted (absent, not false).
  assert.equal(snap.generatedFrom.staticAnalysis, true);
  assert.equal(snap.generatedFrom.runtimeVerified, false);
  assert.ok(!("coverageVerified" in snap.generatedFrom) || snap.generatedFrom.coverageVerified === false,
    "no coverage proof may ever be claimed");
  // limitations: ACTUAL generator emits an object {verified flags, notes[]},
  // while the contract doc (webui-snapshot-contract.md §3.10) documents an
  // array. P0-F0 characterizes the actual shape and records the divergence.
  assert.ok(snap.limitations && typeof snap.limitations === "object");
  assert.ok(Array.isArray(snap.limitations.notes) && snap.limitations.notes.length > 0,
    "limitations must carry a non-empty notes[]");
});

test("graph section carries bounded preview semantics and summary counts", () => {
  const snap = loadFixture();
  const g = snap.graph;
  assert.equal(g.status, "collected");
  assert.equal(g.stability, "preview");
  assert.ok(Array.isArray(g.nodes) && g.nodes.length > 0);
  assert.ok(Array.isArray(g.edges));
  assert.equal(typeof g.truncated, "boolean");
  // summary counts must match the actual arrays (no fabricated numbers)
  assert.equal(g.summary.nodeCount, g.nodes.length);
  assert.equal(g.summary.edgeCount, g.edges.length);
  assert.equal(
    g.summary.symbolNodeCount,
    g.nodes.filter((n) => n.kind === "symbol").length
  );
  assert.equal(
    g.summary.callEdgeCount,
    g.edges.filter((e) => e.kind === "calls").length
  );
});

test("graph node entries carry stable id/label/kind and optional provenance", () => {
  const snap = loadFixture();
  const ids = new Set();
  for (const n of snap.graph.nodes) {
    assert.equal(typeof n.id, "string");
    assert.ok(n.id.length > 0);
    assert.ok(!ids.has(n.id), `duplicate node id: ${n.id}`);
    ids.add(n.id);
    assert.equal(typeof n.label, "string");
    assert.ok(["symbol", "file", "package", "entry", "risk", "related"].includes(n.kind),
      `unexpected node kind: ${n.kind}`);
    if ("line" in n) assert.equal(typeof n.line, "number");
  }
});

test("graph edge entries carry source/target/kind with optional confidence/reason", () => {
  const snap = loadFixture();
  const nodeIds = new Set(snap.graph.nodes.map((n) => n.id));
  for (const e of snap.graph.edges) {
    assert.equal(typeof e.source, "string");
    assert.equal(typeof e.target, "string");
    // Graph schema v0.2 invariant: no dangling CALLS edges
    assert.ok(nodeIds.has(e.source), `edge source not in node set: ${e.source}`);
    assert.ok(nodeIds.has(e.target), `edge target not in node set: ${e.target}`);
    assert.ok(["calls", "imports", "defines", "owns", "related"].includes(e.kind),
      `unexpected edge kind: ${e.kind}`);
    if ("confidence" in e) assert.equal(typeof e.confidence, "number");
    if ("reason" in e) assert.equal(typeof e.reason, "string");
  }
});

test("G2 identity: relationKey = sha256(source + kind + target) is stable and present", () => {
  const snap = loadFixture();
  assert.ok(snap.graph.edges.length > 0, "fixture has edges");
  for (const e of snap.graph.edges) {
    assert.ok("relationKey" in e, "relationKey must exist after P0-A identity work (G2)");
    const expect = `rel:sha256:${sha256Hex(`${e.source}\u0000${e.kind}\u0000${e.target}`)}`;
    assert.equal(e.relationKey, expect, `relationKey mismatch for ${e.source}->${e.target}`);
    assert.ok(!("occurrenceKey" in e), "occurrenceKey absent (relation-level frozen at G2)");
  }
});

test("G2 regeneration determinism: relationKey survives re-analysis unchanged", () => {
  const script = path.join(WS, "scripts/webui-snapshot.sh");
  const out = execFileSync("bash", [script, "--root", "fixtures/rust/portable-smoke",
    "--language", "rust", "--output", "-", "--redact-root"],
    { cwd: WS, encoding: "utf8", timeout: 120000 });
  const fresh = JSON.parse(out);
  const baseline = loadFixture();
  const rk = (e) => `${e.relationKey}`;
  const freshKeys = new Map(fresh.graph.edges.map((e) => [rk(e), e]));
  for (const e of baseline.graph.edges) {
    assert.ok(freshKeys.has(rk(e)), `relationKey lost on regeneration: ${rk(e)}`);
  }
});

function sha256Hex(s) {
  // Node 同步 sha256 到 hex（node:crypto）
  return createHash("sha256").update(s, "utf8").digest("hex");
}

test("regeneration determinism: same fixture yields same counts and edge set (modulo timestamps)", () => {
  const script = path.join(WS, "scripts/webui-snapshot.sh");
  assert.ok(existsSync(script), "webui-snapshot.sh missing");
  const out = execFileSync("bash", [script, "--root", "fixtures/rust/portable-smoke",
    "--language", "rust", "--output", "-", "--redact-root"],
    { cwd: WS, encoding: "utf8", timeout: 120000 });
  const fresh = JSON.parse(out);
  const baseline = loadFixture();
  assert.equal(fresh.schemaVersion, baseline.schemaVersion);
  assert.equal(fresh.summary.symbolCount, baseline.summary.symbolCount);
  assert.equal(fresh.summary.sourceFileCount, baseline.summary.sourceFileCount);
  assert.equal(fresh.graph.summary.nodeCount, baseline.graph.summary.nodeCount);
  assert.equal(fresh.graph.summary.edgeCount, baseline.graph.summary.edgeCount);
  const key = (e) => `${e.kind}|${e.source}|${e.target}`;
  const freshEdges = new Set(fresh.graph.edges.map(key));
  for (const e of baseline.graph.edges) {
    assert.ok(freshEdges.has(key(e)), `edge lost on regeneration: ${key(e)}`);
  }
});

test("truncation markers: explore section carries truncated flag consistent with limits", () => {
  const snap = loadFixture();
  if (snap.explore && snap.explore.status === "collected") {
    assert.equal(typeof snap.explore.truncated, "boolean");
    assert.ok(Array.isArray(snap.explore.symbols));
    assert.ok(snap.explore.symbols.length <= 500, "symbols capped at MAX_SYMBOLS_DEFAULT");
  }
});

test("insights/cleanup heuristic sections keep stable envelope", () => {
  const snap = loadFixture();
  for (const sec of ["insights", "cleanup", "releaseReview"]) {
    assert.ok(snap[sec] && typeof snap[sec] === "object", `${sec} must be an object`);
    if ("status" in snap[sec]) {
      assert.ok(["collected", "not_collected", "partial"].includes(snap[sec].status),
        `${sec}.status unexpected: ${snap[sec].status}`);
    }
  }
  if (snap.insights.status !== "not_collected") {
    assert.ok(Array.isArray(snap.insights.entryPoints));
    assert.ok(Array.isArray(snap.insights.hotspots));
  }
});

test("moduleGraph is present and count-sum equals aggregatable edges", () => {
  const snap = loadFixture();
  assert.ok(snap.moduleGraph && typeof snap.moduleGraph === "object");
  assert.ok(Array.isArray(snap.moduleGraph.modules));
  assert.ok(Array.isArray(snap.moduleGraph.edges));
  assert.equal(typeof snap.moduleGraph.truncated, "boolean");
  const ids = new Set();
  for (const m of snap.moduleGraph.modules) {
    assert.equal(typeof m.id, "string");
    assert.ok(m.id.length > 0);
    assert.ok(!ids.has(m.id), `duplicate module id: ${m.id}`);
    ids.add(m.id);
    assert.equal(typeof m.files, "number");
    assert.equal(typeof m.symbols, "number");
  }
  let countSum = 0;
  for (const e of snap.moduleGraph.edges) {
    assert.ok(ids.has(e.source), `module edge source missing: ${e.source}`);
    assert.ok(ids.has(e.target), `module edge target missing: ${e.target}`);
    assert.notEqual(e.source, e.target);
    assert.equal(typeof e.count, "number");
    assert.ok(e.count >= 1);
    assert.ok(Array.isArray(e.kinds));
    if ("minConfidence" in e) assert.equal(typeof e.minConfidence, "number");
    countSum += e.count;
  }
  const expected = Number(execFileSync("python3", ["-c", `
import importlib.util, json, pathlib, sys
p = pathlib.Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("snapshot_gen", p)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
snap = json.load(sys.stdin)
lang = snap.get("summary", {}).get("language") or "rust"
node_mod = {n["id"]: mod.module_id_for_node(n, lang) for n in snap["graph"]["nodes"]}
n = 0
for e in snap["graph"]["edges"]:
    s, t = node_mod.get(e.get("source")), node_mod.get(e.get("target"))
    if s and t and s != t:
        n += 1
print(n)
`, path.join(WS, "scripts/codelattice-snapshot-gen.py")], {
    input: JSON.stringify(snap),
    encoding: "utf8",
  }).trim());
  assert.equal(countSum, expected, "module edge counts must equal aggregatable base edges");
});
