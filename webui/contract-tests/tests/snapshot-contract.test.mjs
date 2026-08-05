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
