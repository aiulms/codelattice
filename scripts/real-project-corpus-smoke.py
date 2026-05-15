#!/usr/bin/env python3
"""Run CodeLattice smoke checks against a GitCode real-project corpus.

The script clones configured targets on demand, runs read-only MCP tools, and
prints stable metrics that can be compared across releases. It never vendors
the target repositories into CodeLattice and never runs target build scripts.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
DEFAULT_CONFIG = REPO_ROOT / "docs" / "real-project-corpus.json"
ALL_LANGUAGE_FEATURES = (
    "tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,"
    "tree-sitter-c,tree-sitter-cpp,tree-sitter-python"
)


def run_cmd(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    timeout: int | None = None,
    check: bool = False,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        timeout=timeout,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def load_config(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as fh:
        data = json.load(fh)
    if not isinstance(data.get("targets"), list):
        raise SystemExit(f"invalid corpus config, missing targets array: {path}")
    return data


def default_cache_dir() -> Path:
    root = os.environ.get("CODELATTICE_CORPUS_DIR")
    if root:
        return Path(root).expanduser()
    tmp = os.environ.get("TMPDIR") or "/tmp"
    return Path(tmp).expanduser() / "codelattice-real-project-corpus"


def find_binary(user_bin: str | None, build: bool) -> Path:
    if user_bin:
        candidate = Path(user_bin).expanduser()
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate.resolve()
        raise SystemExit(f"binary is not executable: {candidate}")

    candidates = [
        REPO_ROOT / "target" / "release" / "codelattice",
        REPO_ROOT / "target" / "debug" / "codelattice",
    ]
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate

    if not build:
        raise SystemExit(
            "no codelattice binary found. Run scripts/install-mcp.sh --build "
            "or pass --build / --bin <path>."
        )

    cmd = [
        "cargo",
        "build",
        "--release",
        "-p",
        "gitnexus-rust-core-cli",
        "--features",
        ALL_LANGUAGE_FEATURES,
        "--bins",
        "--manifest-path",
        str(REPO_ROOT / "Cargo.toml"),
    ]
    print("$ " + " ".join(cmd))
    run_cmd(cmd, check=True)
    binary = REPO_ROOT / "target" / "release" / "codelattice"
    if not binary.is_file():
        raise SystemExit(f"build completed but binary is missing: {binary}")
    return binary


def selected_targets(config: dict[str, Any], args: argparse.Namespace) -> list[dict[str, Any]]:
    targets = list(config["targets"])
    if args.all:
        selected = targets
    elif args.target or args.language or args.tier:
        selected = targets
    else:
        selected = [t for t in targets if t.get("enabledByDefault") is True]

    if args.target:
        wanted = set(args.target)
        selected = [t for t in selected if t["id"] in wanted]
    if args.language:
        languages = set(args.language)
        selected = [t for t in selected if t.get("language") in languages]
    if args.tier:
        tiers = set(args.tier)
        selected = [t for t in selected if t.get("tier") in tiers]
    if args.max_targets is not None:
        selected = selected[: args.max_targets]

    if not selected:
        raise SystemExit("no corpus targets selected")
    return selected


def print_targets(targets: list[dict[str, Any]]) -> None:
    for target in targets:
        default_mark = "default" if target.get("enabledByDefault") else "optional"
        print(
            "{id:32} {language:10} {tier:6} {default_mark:8} {repo}".format(
                id=target["id"],
                language=target.get("language", "?"),
                tier=target.get("tier", "?"),
                default_mark=default_mark,
                repo=target.get("repo", ""),
            )
        )


def checkout_dir(cache_dir: Path, target: dict[str, Any]) -> Path:
    return cache_dir / target["id"]


def ensure_checkout(
    target: dict[str, Any],
    target_dir: Path,
    *,
    update: bool,
    offline: bool,
    dry_run: bool,
    timeout: int,
) -> str:
    repo = target["repo"]
    ref = target.get("ref")

    if target_dir.exists():
        if update and not offline:
            cmd = ["git", "-C", str(target_dir), "pull", "--ff-only"]
            if dry_run:
                return "$ " + " ".join(cmd)
            result = run_cmd(cmd, timeout=timeout)
            if result.returncode != 0:
                raise RuntimeError(result.stderr.strip() or result.stdout.strip())
            return "updated"
        return "cached"

    if offline:
        raise RuntimeError(f"offline and target is not cached: {target_dir}")

    target_dir.parent.mkdir(parents=True, exist_ok=True)
    cmd = ["git", "clone", "--depth", "1"]
    if ref:
        cmd.extend(["--branch", ref])
    cmd.extend([repo, str(target_dir)])
    if dry_run:
        return "$ " + " ".join(cmd)
    result = run_cmd(cmd, timeout=timeout)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return "cloned"


def project_root(target: dict[str, Any], target_dir: Path) -> Path:
    project_path = target.get("projectPath") or "."
    root = (target_dir / project_path).resolve()
    if not root.is_dir():
        raise RuntimeError(f"projectPath does not exist: {root}")
    return root


def mcp_call(binary: Path, tool: str, arguments: dict[str, Any], timeout: int) -> dict[str, Any]:
    payload = "\n".join(
        [
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": {
                            "name": "real-project-corpus-smoke",
                            "version": "1.0",
                        },
                    },
                },
                separators=(",", ":"),
            ),
            '{"jsonrpc":"2.0","method":"notifications/initialized"}',
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {"name": tool, "arguments": arguments},
                },
                separators=(",", ":"),
            ),
        ]
    )
    proc = subprocess.run(
        [str(binary), "mcp"],
        input=payload + "\n",
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip())
    for line in proc.stdout.splitlines():
        if not line.strip():
            continue
        doc = json.loads(line)
        if doc.get("id") != 2:
            continue
        if "error" in doc:
            raise RuntimeError(json.dumps(doc["error"], ensure_ascii=False))
        result = doc.get("result", {})
        if result.get("isError"):
            raise RuntimeError(json.dumps(result, ensure_ascii=False))
        content = result.get("content") or []
        text = content[0].get("text", "{}") if content else "{}"
        return json.loads(text)
    raise RuntimeError(f"missing MCP response for tool {tool}")


def check_thresholds(metrics: dict[str, Any], thresholds: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    mapping = {
        "nodeCount": "nodeCount",
        "edgeCount": "edgeCount",
        "symbolCount": "symbolCount",
        "sourceFileCount": "sourceFileCount",
    }
    for key, metric_key in mapping.items():
        if key not in thresholds:
            continue
        actual = int(metrics.get(metric_key, 0) or 0)
        expected = int(thresholds[key])
        if actual < expected:
            failures.append(f"{metric_key} {actual} < {expected}")
    return failures


def analyze_target(
    binary: Path,
    target: dict[str, Any],
    root: Path,
    *,
    timeout: int,
    insight_limit: int,
    skip_insights: bool,
) -> dict[str, Any]:
    language = target["language"]
    overview_args = {
        "root": str(root),
        "language": language,
        "compact": True,
    }
    overview = mcp_call(binary, "codelattice_project_overview", overview_args, timeout)
    metrics = {
        "nodeCount": int(overview.get("nodeCount", 0) or 0),
        "edgeCount": int(overview.get("edgeCount", 0) or 0),
        "symbolCount": int(overview.get("symbolCount", 0) or 0),
        "sourceFileCount": int(overview.get("sourceFileCount", 0) or 0),
    }

    insights_summary: dict[str, Any] = {}
    if not skip_insights:
        insights_args = {
            "root": str(root),
            "language": language,
            "compact": True,
            "limit": insight_limit,
            "includeDocs": False,
            "includeDiagnostics": True,
        }
        insights = mcp_call(binary, "codelattice_project_insights", insights_args, timeout)
        insights_summary = insights.get("summary", {})

    threshold_failures = check_thresholds(metrics, target.get("min", {}))
    status = "pass" if not threshold_failures else "fail"
    return {
        "id": target["id"],
        "name": target.get("name", target["id"]),
        "language": language,
        "root": str(root),
        "status": status,
        "metrics": metrics,
        "insightsSummary": insights_summary,
        "thresholdFailures": threshold_failures,
    }


def run_target(binary: Path, target: dict[str, Any], args: argparse.Namespace) -> dict[str, Any]:
    target_dir = checkout_dir(args.cache_dir, target)
    result: dict[str, Any] = {
        "id": target["id"],
        "name": target.get("name", target["id"]),
        "language": target.get("language"),
        "repo": target.get("repo"),
        "status": "unknown",
    }
    started = time.monotonic()
    try:
        checkout_status = ensure_checkout(
            target,
            target_dir,
            update=args.update,
            offline=args.offline,
            dry_run=args.dry_run,
            timeout=args.clone_timeout,
        )
        result["checkout"] = checkout_status
        if args.dry_run:
            result["status"] = "dry-run"
            result["root"] = str((target_dir / (target.get("projectPath") or ".")).resolve())
            return result
        root = project_root(target, target_dir)
        result.update(
            analyze_target(
                binary,
                target,
                root,
                timeout=args.timeout,
                insight_limit=args.insight_limit,
                skip_insights=args.skip_insights,
            )
        )
    except Exception as exc:  # noqa: BLE001 - script reports and continues by default.
        result["status"] = "fail"
        result["error"] = str(exc)
    result["elapsedSeconds"] = round(time.monotonic() - started, 2)
    return result


def print_result(result: dict[str, Any]) -> None:
    status = result["status"].upper()
    prefix = "PASS" if result["status"] == "pass" else status
    print(f"{prefix}: {result['id']} ({result.get('language')})")
    if "checkout" in result:
        print(f"  checkout: {result['checkout']}")
    if "metrics" in result:
        m = result["metrics"]
        print(
            "  overview: nodes={nodeCount} edges={edgeCount} symbols={symbolCount} files={sourceFileCount}".format(
                **m
            )
        )
    insights = result.get("insightsSummary") or {}
    if insights:
        print(
            "  insights: hotspots(files={hf}, symbols={hs}) lowConfidenceZones={lc}".format(
                hf=insights.get("hotspotFileCount", 0),
                hs=insights.get("hotspotSymbolCount", 0),
                lc=insights.get("lowConfidenceZoneCount", 0),
            )
        )
    if result.get("thresholdFailures"):
        print("  threshold failures: " + "; ".join(result["thresholdFailures"]))
    if result.get("error"):
        print("  error: " + result["error"])
    if "elapsedSeconds" in result:
        print(f"  elapsed: {result['elapsedSeconds']}s")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run CodeLattice against GitCode real-project smoke corpus."
    )
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--cache-dir", type=Path, default=default_cache_dir())
    parser.add_argument("--bin", help="Path to codelattice binary")
    parser.add_argument("--build", action="store_true", help="Build release binary if missing")
    parser.add_argument("--target", action="append", help="Target id to run; repeatable")
    parser.add_argument("--language", action="append", help="Language to run; repeatable")
    parser.add_argument("--tier", action="append", help="Tier to run; repeatable")
    parser.add_argument("--all", action="store_true", help="Include optional corpus targets")
    parser.add_argument("--list", action="store_true", help="List selected targets and exit")
    parser.add_argument("--dry-run", action="store_true", help="Print clone/analyze plan only")
    parser.add_argument("--offline", action="store_true", help="Use cached checkouts only")
    parser.add_argument("--update", action="store_true", help="Update existing cached checkouts")
    parser.add_argument("--skip-insights", action="store_true", help="Skip project_insights")
    parser.add_argument("--insight-limit", type=int, default=5)
    parser.add_argument("--timeout", type=int, default=240, help="Per MCP call timeout seconds")
    parser.add_argument("--clone-timeout", type=int, default=300)
    parser.add_argument("--max-targets", type=int)
    parser.add_argument("--json-out", type=Path, help="Write full result JSON to this path")
    parser.add_argument("--fail-fast", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    args.config = args.config.resolve()
    args.cache_dir = args.cache_dir.expanduser().resolve()
    config = load_config(args.config)
    targets = selected_targets(config, args)

    if args.list:
        print_targets(targets)
        return 0

    binary = find_binary(args.bin, args.build)
    print("=== CodeLattice Real Project Corpus Smoke ===")
    print(f"config:    {args.config}")
    print(f"cacheDir:  {args.cache_dir}")
    print(f"binary:    {binary}")
    print(f"targets:   {len(targets)}")
    print("")

    results: list[dict[str, Any]] = []
    for target in targets:
        print(f"--- {target['id']} ---")
        result = run_target(binary, target, args)
        results.append(result)
        print_result(result)
        print("")
        if args.fail_fast and result["status"] == "fail":
            break

    summary = {
        "total": len(results),
        "passed": sum(1 for r in results if r["status"] == "pass"),
        "failed": sum(1 for r in results if r["status"] == "fail"),
        "dryRun": sum(1 for r in results if r["status"] == "dry-run"),
    }
    output = {"summary": summary, "results": results}
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n")
        print(f"wrote: {args.json_out}")

    print(
        "Summary: total={total} passed={passed} failed={failed} dryRun={dryRun}".format(
            **summary
        )
    )
    return 1 if summary["failed"] else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
