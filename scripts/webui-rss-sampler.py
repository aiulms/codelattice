#!/usr/bin/env python3
"""Sample RSS for one explicitly launched Workbench process tree.

macOS reparents WebKit XPC helpers to launchd, so PPID alone cannot associate
them with the app. The benchmark therefore records a pre-launch WebKit PID
baseline and accepts the post-launch delta only when at least one helper has a
bundle-specific WebKit data/cache open-file marker.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ProcessRow:
    pid: int
    ppid: int
    rss_kb: int
    command: str


def process_table() -> dict[int, ProcessRow]:
    completed = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,rss=,command="],
        capture_output=True,
        text=True,
        check=True,
    )
    rows: dict[int, ProcessRow] = {}
    for line in completed.stdout.splitlines():
        parts = line.strip().split(maxsplit=3)
        if len(parts) < 3:
            continue
        try:
            pid, ppid, rss_kb = map(int, parts[:3])
        except ValueError:
            continue
        rows[pid] = ProcessRow(
            pid=pid,
            ppid=ppid,
            rss_kb=rss_kb,
            command=parts[3] if len(parts) == 4 else "",
        )
    return rows


def is_webkit_helper(row: ProcessRow) -> bool:
    return (
        "/XPCServices/com.apple.WebKit." in row.command
        and "/Contents/MacOS/com.apple.WebKit." in row.command
    )


def webkit_pids(rows: dict[int, ProcessRow]) -> set[int]:
    return {row.pid for row in rows.values() if is_webkit_helper(row)}


def descendants(rows: dict[int, ProcessRow], root_pid: int) -> set[int]:
    owned = {root_pid}
    changed = True
    while changed:
        changed = False
        for row in rows.values():
            if row.ppid in owned and row.pid not in owned:
                owned.add(row.pid)
                changed = True
    return owned


def has_bundle_open_file(pid: int, data_name: str) -> bool:
    try:
        completed = subprocess.run(
            ["lsof", "-p", str(pid)],
            capture_output=True,
            text=True,
            check=False,
            timeout=3,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    output = completed.stdout.lower()
    marker = data_name.lower()
    return (
        f"/library/webkit/{marker}/" in output
        or f"/library/caches/{marker}/webkit/" in output
    )


def load_baseline(path: str | None) -> set[int] | None:
    if path is None:
        return None
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    return {int(pid) for pid in payload.get("pids", [])}


def sample(
    root_pid: int,
    bundle_id: str,
    data_name: str,
    baseline_pids: set[int] | None,
) -> dict[str, object]:
    rows = process_table()
    if root_pid not in rows:
        raise ProcessLookupError(root_pid)

    core_pids = descendants(rows, root_pid)
    current_webkit = webkit_pids(rows)
    candidates = current_webkit - baseline_pids if baseline_pids is not None else set()
    command_marker = f"bundleIdentifier {bundle_id}"
    anchor_pids = {
        pid
        for pid in candidates
        if command_marker in rows[pid].command or has_bundle_open_file(pid, data_name)
    }
    ownership_verified = bool(anchor_pids)
    owned_webview_pids = candidates if ownership_verified else set()
    core_kb = sum(rows[pid].rss_kb for pid in core_pids if pid in rows)
    webview_kb_by_pid = {
        pid: rows[pid].rss_kb for pid in owned_webview_pids if pid in rows
    }
    webview_kb = sum(webview_kb_by_pid.values())
    return {
        "rootPid": root_pid,
        "bundleId": bundle_id,
        "webkitDataName": data_name,
        "ownershipMethod": "controlled-launch-delta-with-bundle-anchor",
        "ownershipVerified": ownership_verified,
        "bundleAnchorPidList": sorted(anchor_pids),
        "candidateWebviewPidList": sorted(candidates),
        "coreMb": round(core_kb / 1024, 1),
        "webviewMb": round(webview_kb / 1024, 1),
        "aggregateMb": round((core_kb + webview_kb) / 1024, 1),
        "corePids": len(core_pids),
        "webviewPids": len(owned_webview_pids),
        "corePidList": sorted(core_pids),
        "webviewPidList": sorted(owned_webview_pids),
        "webviewRssKbByPid": {
            str(pid): rss for pid, rss in sorted(webview_kb_by_pid.items())
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pid", type=int, help="PID returned by the controlled app launch")
    parser.add_argument("--bundle-id", default="com.codelattice.workbench")
    parser.add_argument("--webkit-data-name", default="codelattice-workbench")
    parser.add_argument("--webkit-baseline-file")
    parser.add_argument("--list-webkit-pids", action="store_true")
    parser.add_argument("--once", action="store_true", help="compatibility flag; sampling is always one-shot")
    args = parser.parse_args()

    try:
        rows = process_table()
        if args.list_webkit_pids:
            print(json.dumps({
                "schemaVersion": "codelattice.webkit-baseline.v1",
                "pids": sorted(webkit_pids(rows)),
            }))
            return 0
        if args.pid is None:
            parser.error("--pid is required unless --list-webkit-pids is used")
        baseline = load_baseline(args.webkit_baseline_file)
        payload = sample(args.pid, args.bundle_id, args.webkit_data_name, baseline)
    except ProcessLookupError:
        print(json.dumps({
            "error": "root_process_not_found",
            "rootPid": args.pid,
            "bundleId": args.bundle_id,
        }))
        return 3
    except (OSError, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(json.dumps({
            "error": "process_table_or_baseline_unavailable",
            "detail": str(error),
            "rootPid": args.pid,
        }))
        return 4

    print(json.dumps(payload, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
