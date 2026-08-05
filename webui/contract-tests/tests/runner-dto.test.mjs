// P0-F0 characterization: webui-runner.py HTTP DTO contract.
// Freezes request/response shapes of the legacy Web Runner REST API that the
// new Workbench transport adapter must either reuse or explicitly diverge from.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { mkdtempSync, copyFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileP = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "../../..");
const RUNNER = path.join(WS, "scripts/webui-runner.py");
const FIXTURE_SNAP = path.join(WS, "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");

let base = "";
let child = null;
let snapshotId = "deadbeef0001";

function apiGet(p) {
  return fetch(`${base}${p}`).then(async (r) => ({
    status: r.status,
    contentType: r.headers.get("content-type") || "",
    body: await r.text().then((t) => {
      try { return JSON.parse(t); } catch { return t; }
    }),
  }));
}

before(async () => {
  assert.ok(existsSync(RUNNER), "webui-runner.py missing");
  // pick a free port
  const port = await execFileP("python3", ["-c",
    "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()"]);
  const snapDir = mkdtempSync(path.join(tmpdir(), "cls-dto-"));
  // the runner indexes snapshots through snapshotDir/index.json; entries are
  // {id, filename, createdAt, rootLabel, language, profileId, profileName}
  const fn = `snapshot-${snapshotId}.json`;
  copyFileSync(FIXTURE_SNAP, path.join(snapDir, fn));
  writeFileSync(path.join(snapDir, "index.json"), JSON.stringify([{
    id: snapshotId, filename: fn, createdAt: "2026-08-05T00:00:00Z",
    rootLabel: "rust-portable-smoke", language: "rust",
    profileId: "", profileName: "", schemaVersion: "webui.snapshot.v1",
  }]));
  const portNum = Number(port.stdout.trim());
  base = `http://127.0.0.1:${portNum}`;
  child = spawn("python3", [RUNNER, "--port", String(portNum), "--snapshot-dir", snapDir],
    { stdio: ["ignore", "pipe", "pipe"] });
  // wait for health
  for (let i = 0; i < 40; i++) {
    try {
      const r = await fetch(`${base}/api/health`);
      if (r.ok) break;
    } catch { /* not up yet */ }
    await new Promise((res) => setTimeout(res, 250));
  }
  const health = await apiGet("/api/health");
  assert.equal(health.body.success, true, "runner must be reachable");
});

after(() => {
  if (child) child.kill("SIGTERM");
});

test("ok DTO: { success, data, error:null, hint:null }", async () => {
  const { status, body } = await apiGet("/api/health");
  assert.equal(status, 200);
  assert.equal(body.success, true);
  assert.equal(body.error, null);
  assert.equal(body.hint, null);
  assert.equal(typeof body.data, "object");
  assert.equal(body.data.staticOnly, true, "runner is static-only");
});

test("error DTO: { success:false, data:null, error, hint, status }", async () => {
  // hex-shaped id that does not exist in the index -> 404 JSON error DTO
  const { status, body } = await apiGet("/api/snapshot/abcdef012345");
  assert.equal(status, 404);
  assert.equal(body.success, false);
  assert.equal(body.data, null);
  assert.equal(typeof body.error, "string");
  assert.equal(body.status, 404);
});

test("snapshot list DTO and snapshot get DTO round-trip the fixture", async () => {
  const list = await apiGet("/api/snapshots");
  assert.equal(list.body.success, true);
  assert.ok(Array.isArray(list.body.data));
  assert.equal(list.body.data.length, 1, "index.json entry must be listed");
  const entry = list.body.data[0];
  assert.equal(entry.id, snapshotId);
  assert.equal(typeof entry.rootLabel, "string");
  assert.equal(typeof entry.language, "string");
  assert.equal(typeof entry.createdAt, "string");
  // _snap_meta enriches the entry with summary derived from the snapshot file
  assert.equal(typeof entry.summary.symbolCount, "number");

  const got = await apiGet(`/api/snapshot/${snapshotId}`);
  assert.equal(got.body.success, true);
  assert.equal(got.body.data.schemaVersion, "webui.snapshot.v1");
  assert.equal(got.body.data.summary.symbolCount, entry.summary.symbolCount);
});

test("invalid snapshot id returns 400 error DTO (not crash)", async () => {
  const { status, body } = await apiGet("/api/snapshot/__invalid__");
  assert.equal(status, 400);
  assert.equal(body.success, false);
  assert.equal(body.status, 400);
});

test("unknown API route: HTML 404 fallback (documented gap: no JSON envelope)", async () => {
  const { status, contentType } = await apiGet("/api/no-such-route");
  assert.equal(status, 404);
  assert.match(contentType, /text\/html/);
});

test("P0-F0 gap: no graph query endpoints exist yet (on-demand evidence is P0-A)", async () => {
  const candidates = [
    `/api/graph/snapshots/${snapshotId}/relations/rel%3Aabc`,
    `/api/graph/snapshots/${snapshotId}/nodes/n%3Aa/neighbors?direction=both&depth=1`,
  ];
  for (const p of candidates) {
    const { status } = await apiGet(p);
    assert.equal(status, 404, `endpoint ${p} must not exist before P0-A Track B decision`);
  }
});
