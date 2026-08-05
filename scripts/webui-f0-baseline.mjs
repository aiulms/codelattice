#!/usr/bin/env node
// P0-F0 baseline: legacy snapshot-viewer + Web Runner reference numbers.
//
// Measures, on the same fixture (rust-portable-smoke):
//   - snapshot API response time (P50/P95 over N samples)
//   - runner RSS, Chrome RSS and aggregate (Runner + Browser) RSS
//     at load and after a 60s idle window
//
// Writes JSON to docs/perf/f0-baseline-<timestamp>.json and prints a summary.
// Numbers are F0 reference values only; P0 budgets live in the execution card.
import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, copyFileSync, writeFileSync, existsSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "../webui/contract-tests/node_modules/playwright-core/index.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "..");
const RUNNER = path.join(WS, "scripts/webui-runner.py");
const FIXTURE_SNAP = path.join(WS, "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");
const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const IDLE_MS = 60_000;
const SAMPLES = 11;

const ts = new Date().toISOString().replace(/[:.]/g, "-");
const OUT = path.join(WS, "docs/perf", `f0-baseline-${ts}.json`);

function rssOf(pattern) {
  // ps rss in KB; sum matching processes
  const out = execFileSync("ps", ["-Ao", "pid=,rss=,command="], { encoding: "utf8" });
  let totalKb = 0;
  const pids = [];
  for (const line of out.split("\n")) {
    const m = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
    if (!m) continue;
    const [, pid, rss, cmd] = m;
    if (cmd.includes(pattern)) { totalKb += Number(rss); pids.push(pid); }
  }
  return { rssKb: totalKb, pids };
}

function rssMb(x) { return Math.round(x / 1024); }

async function main() {
  if (!existsSync(CHROME)) throw new Error("Google Chrome not found");
  if (!existsSync(RUNNER)) throw new Error("webui-runner.py missing");

  // ── start runner with fixture snapshot ─────────────────────────────
  const port = Number(execFileSync("python3", ["-c",
    "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()"], { encoding: "utf8" }).trim());
  const snapDir = mkdtempSync(path.join(tmpdir(), "cls-f0base-"));
  const sid = "f0baseline0001";
  copyFileSync(FIXTURE_SNAP, path.join(snapDir, `snapshot-${sid}.json`));
  writeFileSync(path.join(snapDir, "index.json"), JSON.stringify([{
    id: sid, filename: `snapshot-${sid}.json`, createdAt: new Date().toISOString(),
    rootLabel: "rust-portable-smoke", language: "rust", profileId: "", profileName: "",
  }]));
  const runner = spawn("python3", [RUNNER, "--port", String(port), "--snapshot-dir", snapDir],
    { stdio: ["ignore", "pipe", "pipe"] });
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 40; i++) {
    try { const r = await fetch(`${base}/api/health`); if (r.ok) break; } catch { /* wait */ }
    await new Promise((res) => setTimeout(res, 250));
  }

  // ── API response time samples ──────────────────────────────────────
  const lat = { list: [], get: [] };
  for (let i = 0; i < SAMPLES; i++) {
    let t0 = performance.now();
    await (await fetch(`${base}/api/snapshots`)).json();
    lat.list.push(performance.now() - t0);
    t0 = performance.now();
    await (await fetch(`${base}/api/snapshot/${sid}`)).json();
    lat.get.push(performance.now() - t0);
  }
  const pct = (arr, p) => {
    const s = [...arr].sort((a, b) => a - b);
    return s[Math.min(s.length - 1, Math.floor(s.length * p))];
  };

  // ── browser: open viewer + inject snapshot ─────────────────────────
  const browser = await chromium.launch({ executablePath: CHROME, headless: true });
  const page = await browser.newPage();
  const pageErrors = [];
  page.on("pageerror", (e) => pageErrors.push(String(e)));
  await page.goto(base, { waitUntil: "domcontentloaded", timeout: 30000 });
  await page.evaluate((json) => window.loadSnapshot(json), readFileSync(FIXTURE_SNAP, "utf8"));
  await page.waitForTimeout(2500);

  const sampleRss = () => {
    const runnerRss = rssOf("webui-runner.py");
    const chromeRss = rssOf("Google Chrome");
    return {
      runnerKb: runnerRss.rssKb, chromeKb: chromeRss.rssKb,
      aggregateKb: runnerRss.rssKb + chromeRss.rssKb,
      runnerPids: runnerRss.pids.length, chromePids: chromeRss.pids.length,
    };
  };

  const load = sampleRss();
  await new Promise((res) => setTimeout(res, IDLE_MS));
  const idle = sampleRss();
  await browser.close();
  runner.kill("SIGTERM");

  const result = {
    meta: {
      date: new Date().toISOString(),
      machine: `${process.platform} ${process.arch}`,
      fixture: "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json",
      idleMs: IDLE_MS, samples: SAMPLES,
      runner: "scripts/webui-runner.py", browser: "Google Chrome (headless)",
      chromePidsAtLoad: load.chromePids, runnerPidsAtLoad: load.runnerPids,
      pageErrors: pageErrors,
    },
    apiLatencyMs: {
      listSnapshots: { p50: pct(lat.list, 0.5), p95: pct(lat.list, 0.95) },
      getSnapshot: { p50: pct(lat.get, 0.5), p95: pct(lat.get, 0.95) },
    },
    rssAtLoadMb: {
      runner: rssMb(load.runnerKb), chrome: rssMb(load.chromeKb), aggregate: rssMb(load.aggregateKb),
    },
    rssAfter60sIdleMb: {
      runner: rssMb(idle.runnerKb), chrome: rssMb(idle.chromeKb), aggregate: rssMb(idle.aggregateKb),
    },
    idleGrowthPct: Math.round(((idle.aggregateKb - load.aggregateKb) / Math.max(1, load.aggregateKb)) * 1000) / 10,
  };
  writeFileSync(OUT, JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
  console.log(`\n[baseline] written: ${OUT}`);
}

main().catch((e) => { console.error("baseline failed:", e); process.exit(1); });
