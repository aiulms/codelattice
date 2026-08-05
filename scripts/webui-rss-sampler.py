#!/usr/bin/env python3
"""webui-rss-sampler.py — psutil 进程内存采样（F1 基线用；ps/top 在沙箱被禁）。

输出：core 匹配进程及其后代（WebKit 等）的聚合 RSS（MB）。
用法：
  python3 scripts/webui-rss-sampler.py --core codelattice-workbench [--once]
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.expanduser("~/.workbuddy/binaries/python/envs/default/lib/python3.13/site-packages"))

import psutil  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--core", default="codelattice-workbench")
    ap.add_argument("--once", action="store_true", help="采样一次后退出（供 node 调用）")
    args = ap.parse_args()

    core_pids = [p.info["pid"] for p in psutil.process_iter(["pid", "cmdline"])
                 if p.info.get("cmdline") and any(args.core in " ".join(p.info["cmdline"]) for p in [p])]
    if not core_pids:
        print(json.dumps({"coreMb": 0, "webviewMb": 0, "aggregateMb": 0, "corePids": 0, "webviewPids": 0}))
        return

    # 进程树：BFS 收集 core 后代
    seen = set(core_pids)
    queue = list(core_pids)
    descendants = []
    while queue:
        pid = queue.pop(0)
        for child in psutil.process_iter(["pid", "ppid"]):
            if child.info["ppid"] == pid and child.info["pid"] not in seen:
                seen.add(child.info["pid"])
                descendants.append(child.info["pid"])
                queue.append(child.info["pid"])

    core_mb = 0
    for pid in core_pids:
        try:
            core_mb += psutil.Process(pid).memory_info().rss / 1e6
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass

    # WKWebView 内容进程是独立 LaunchAgent 子进程（ppid 不挂在 app 下）：
    # 按 `com.apple.WebKit.WebContent ... -bundleIdentifier com.codelattice.workbench`
    # 过滤，避免混入其他 app 的 WebKit 进程。
    webview_mb = 0
    webview_pids = []
    for p in psutil.process_iter(["pid", "cmdline", "name"]):
        try:
            cmd = " ".join(p.info.get("cmdline") or [])
            if "WebContent" not in cmd:
                continue
            if "bundleIdentifier" in cmd and "codelattice" not in cmd:
                continue
            webview_mb += p.memory_info().rss / 1e6
            webview_pids.append(p.info["pid"])
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass

    print(json.dumps({
        "coreMb": round(core_mb),
        "webviewMb": round(webview_mb),
        "aggregateMb": round(core_mb + webview_mb),
        "corePids": len(core_pids),
        "webviewPids": len(webview_pids),
    }))


if __name__ == "__main__":
    main()
