// P0-F0 characterization: legacy snapshot-viewer browser smoke.
// Loads the real page in headless Chrome, injects a real fixture snapshot,
// switches the main views and asserts: no uncaught error, dashboard renders,
// graph view mounts G6 without crashing.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { mkdtempSync, copyFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { chromium } from "playwright-core";

const execFileP = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "../../..");
const RUNNER = path.join(WS, "scripts/webui-runner.py");
const FIXTURE_SNAP = path.join(WS, "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

let base = "";
let child = null;
let browser = null;
let page = null;
let pageErrors = [];
let consoleErrors = [];

before(async () => {
  assert.ok(existsSync(CHROME), "Google Chrome not found (browser smoke requires macOS Chrome)");
  const port = await execFileP("python3", ["-c",
    "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()"]);
  const snapDir = mkdtempSync(path.join(tmpdir(), "cls-smoke-"));
  copyFileSync(FIXTURE_SNAP, path.join(snapDir, "fixture.json"));
  const portNum = Number(port.stdout.trim());
  base = `http://127.0.0.1:${portNum}`;
  child = spawn("python3", [RUNNER, "--port", String(portNum), "--snapshot-dir", snapDir],
    { stdio: ["ignore", "pipe", "pipe"] });
  for (let i = 0; i < 40; i++) {
    try { const r = await fetch(`${base}/api/health`); if (r.ok) break; } catch { /* wait */ }
    await new Promise((res) => setTimeout(res, 250));
  }
  browser = await chromium.launch({ executablePath: CHROME, headless: true });
  page = await browser.newPage();
  page.on("pageerror", (e) => pageErrors.push(String(e)));
  page.on("console", (m) => { if (m.type() === "error") consoleErrors.push(m.text()); });
});

after(async () => {
  if (browser) await browser.close();
  if (child) child.kill("SIGTERM");
});

test("page loads without uncaught errors", async () => {
  const resp = await page.goto(base, { waitUntil: "domcontentloaded", timeout: 30000 });
  assert.ok(resp.ok());
  await page.waitForTimeout(800);
  assert.deepEqual(pageErrors, [], `uncaught page errors on load: ${pageErrors.join("; ")}`);
  const title = await page.title();
  assert.match(title, /CodeLattice|Codelattice|WebUI|Viewer/i);
});

test("injecting a real fixture snapshot renders the dashboard without errors", async () => {
  const snapJson = readFileSync(FIXTURE_SNAP, "utf8");
  await page.evaluate((json) => {
    if (typeof window.loadSnapshot === "function") window.loadSnapshot(json);
    else window.CodeLatticeApp && window.CodeLatticeApp.loadSnapshot(json);
  }, snapJson);
  await page.waitForTimeout(1500);
  assert.deepEqual(pageErrors, [], `uncaught errors after loadSnapshot: ${pageErrors.join("; ")}`);
  const dashVisible = await page.locator("#view-dashboard").isVisible().catch(() => false);
  const graphHost = await page.locator(".graph-visual, .graph-g6-host").count();
  assert.ok(dashVisible || graphHost > 0, "dashboard or graph must be visible after snapshot load");
});

test("main views can be switched without uncaught errors", async () => {
  const views = ["dashboard", "explore", "graph", "cleanup", "release", "workflows", "diff"];
  let switched = 0;
  for (const v of views) {
    const clicked = await page.locator(`[data-view="${v}"], #tab-${v}, [data-tab="${v}"]`)
      .first().click({ timeout: 3000 }).then(() => true).catch(() => false);
    await page.waitForTimeout(250);
    if (clicked) switched += 1;
    assert.deepEqual(pageErrors, [], `uncaught error after switching to view=${v}`);
  }
  assert.ok(switched >= 5, `expected >=5 views clickable, got ${switched}`);
});

test("graph view mounts the G6 engine", async () => {
  const hasG6 = await page.evaluate(() => !!(window.G6 || window.g6) && !!window.CodeLatticeG6Graph);
  assert.ok(hasG6, "G6 adapter must be present on page");
  const engineState = await page.evaluate(() => {
    const api = window.CodeLatticeG6Graph;
    if (!api) return "no-api";
    return { available: api.available(), lastRendered: api.lastRendered() };
  });
  assert.equal(engineState.available, true, "G6 library must be loaded");
});
