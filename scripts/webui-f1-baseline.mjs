#!/usr/bin/env node
// 兼容入口：旧 Node 基准已废弃，统一委托给精确 PID 的真实 RSS 基准。
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync("bash", [path.join(root, "scripts/f1-memory-benchmark.sh")], {
  cwd: root,
  env: process.env,
  stdio: "inherit",
});
process.exit(result.status ?? 1);
