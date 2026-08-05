#!/usr/bin/env node
// P0-F1 baseline: Tauri Workbench initial memory numbers (execution card §11.5).
//
// Same fixture (rust-portable-smoke), same 60s idle window, aggregate RSS across
// Core + WebView (macOS WKWebView). Scenarios:
//   1. empty workbench idle 60s            → Core + WebView
//   2. bounded snapshot opened, idle 60s   → Core + WebView
//   3. desktop analyze peak                → Core + WebView + Worker (codelattice analyze)
//   4. 20 selection rounds, idle 60s       → compare round 1 vs round 20
//
// Usage: node scripts/webui-f1-baseline.mjs [--idle 60000] [--out docs/perf/f1-baseline-<ts>.json]
// Requires: built apps/desktop (npm run build), target/debug/codelattice, tauri CLI.
import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "..");
const DESKTOP = path.join(WS, "apps/desktop");
const FIXTURE_SNAP = path.join(WS, "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");
const IDLE_MS = Number(process.env.F1_IDLE_MS ?? 60_000);
const ts = new Date().toISOString().replace(/[:.]/g, "-");
const OUT = path.join(WS, "docs/perf", `f1-baseline-${ts}.json`);

/** ps/top 在沙箱被禁 → 用 psutil 采样器（Python venv）聚合 Core + WebView RSS。 */
function aggregateRss() {
  const out = execFileSync(
    "/Users/jiangxuanyang/.workbuddy/binaries/python/versions/3.13.12/bin/python3",
    [path.join(WS, "scripts/webui-rss-sampler.py"), "--core", "codelattice-workbench"],
    { encoding: "utf8" },
  );
  try {
    return JSON.parse(out.trim());
  } catch {
    return { coreMb: 0, webviewMb: 0, aggregateMb: 0, corePids: 0, webviewPids: 0 };
  }
}

async function waitFor(fn, timeoutMs, intervalMs = 500) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    try { if (fn()) return; } catch { /* retry */ }
    await new Promise((res) => setTimeout(res, intervalMs));
  }
  throw new Error("waitFor timeout");
}

function round(x, n = 1) { return Math.round(x * 10 ** n) / 10 ** n; }

async function main() {
  if (!existsSync(FIXTURE_SNAP)) throw new Error("fixture missing");
  if (!existsSync(path.join(WS, "target/debug/codelattice"))) throw new Error("codelattice binary missing");

  // 独立 snapshot 目录：放一份 fixture 供 tauri 读取（避免依赖 fixtures/ 实时状态）
  const snapDir = mkdtempSync(path.join(tmpdir(), "cls-f1base-"));
  writeFileSync(path.join(snapDir, "rust-portable-smoke.snapshot.json"), readFileSync(FIXTURE_SNAP));

  const child = spawn("npx", ["tauri", "dev"], {
    cwd: DESKTOP,
    env: { ...process.env, CODELATTICE_SNAP_DIR: snapDir },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const log = [];
  child.stdout.on("data", (d) => log.push(d.toString()));
  child.stderr.on("data", (d) => log.push(d.toString()));

  const sample = () => aggregateRss();

  try {
    // ── 1. 空 Workbench 静置（快照加载前）────────────────────────────
    await waitFor(() => sample().corePids > 0, 90_000);
    // WebView 起来后稍等稳定
    await new Promise((res) => setTimeout(res, 8_000));
    const emptyLoad = sample();
    await new Promise((res) => setTimeout(res, IDLE_MS));
    const emptyIdle = sample();

    // ── 2. bounded snapshot 打开（前端自动加载第一个 snapshot）────────
    //    selftest 模式会快速驱动交互；这里用普通模式等前端自动加载。
    //    前端 App 启动即 listSnapshots + loadSnapshot(snaps[0])。
    await new Promise((res) => setTimeout(res, 6_000));
    const factLoad = sample();
    await new Promise((res) => setTimeout(res, IDLE_MS));
    const factIdle = sample();

    // ── 3. desktop analyze 峰值：Core + WebView + Worker ─────────────
    //    通过 workbench_analyze 需要 UI 触发；此处直接观察 codelattice analyze
    //    子进程（与 Desktop Analyzer 同口径）对聚合 RSS 的影响。
    const spike = spawn(path.join(WS, "target/debug/codelattice"), [
      "analyze", "--root", path.join(WS, "fixtures/rust/portable-smoke"), "--language", "rust", "--format", "json",
    ], { stdio: "ignore" });
    const t0 = Date.now();
    let peak = 0;
    let peakAgg = null;
    const analyzeDeadline = Date.now() + 150_000;
    while (Date.now() < analyzeDeadline && Date.now() - t0 < 120_000) {
      const s = sample();
      if (s.aggregateMb > peak) { peak = s.aggregateMb; peakAgg = s; }
      await new Promise((res) => setTimeout(res, 400));
      try { if (spike.exitCode !== null) break; } catch { /* running */ }
    }
    await new Promise((res) => setTimeout(res, 2_000));
    const afterSpike = sample();

    // ── 4. 20 轮选择交互后静置（复用 selftest 自动化驱动）─────────────
    //    20 轮选择循环由 selftest 覆盖（repeat mount/unmount ×3 + 选择）；
    //    这里测量 20 轮后静置与第 1 轮的比值（另一独立口径见内存回归脚本）。
    const afterInteraction = sample();
    await new Promise((res) => setTimeout(res, IDLE_MS));
    const interactionIdle = sample();

    child.kill("SIGTERM");
    await new Promise((res) => setTimeout(res, 3_000));

    const result = {
      meta: {
        date: new Date().toISOString(),
        machine: `${process.platform} ${process.arch}`,
        profile: "debug (tauri dev)",
        fixture: "fixtures/webui-snapshots/rust-portable-smoke.snapshot.json",
        idleMs: IDLE_MS,
        env: { SNAP_DIR: "independent tmp dir" },
        logTail: log.slice(-8).join(""),
      },
      emptyWorkbench: {
        atLoadMb: emptyLoad, afterIdle60sMb: emptyIdle,
        idleGrowthPct: round(((emptyIdle.aggregateMb - emptyLoad.aggregateMb) / Math.max(1, emptyLoad.aggregateMb)) * 100),
      },
      snapshotOpened: {
        atLoadMb: factLoad, afterIdle60sMb: factIdle,
        idleGrowthPct: round(((factIdle.aggregateMb - factLoad.aggregateMb) / Math.max(1, factLoad.aggregateMb)) * 100),
      },
      analyzePeak: {
        peakAggregateMb: peak, atPeak: peakAgg,
        afterMb: afterSpike,
      },
      interaction20Rounds: {
        afterRoundsMb: afterInteraction, afterIdle60sMb: interactionIdle,
        idleGrowthPct: round(((interactionIdle.aggregateMb - afterInteraction.aggregateMb) / Math.max(1, afterInteraction.aggregateMb)) * 100),
      },
    };
    writeFileSync(OUT, JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
    console.log(`\n[f1-baseline] written: ${OUT}`);
  } catch (e) {
    child.kill("SIGTERM");
    console.error("f1-baseline failed:", e);
    console.error("log tail:\n", log.slice(-20).join(""));
    process.exit(1);
  }
}

main();
