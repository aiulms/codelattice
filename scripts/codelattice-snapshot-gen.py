#!/usr/bin/env python3
"""codelattice-snapshot-gen.py — Generate CodeLatticeWebSnapshotV1 JSON (Phase A Enriched).

Reads CLI analyze JSON + quality JSON from stdin/files, produces an enriched
snapshot conforming to docs/webui/webui-snapshot-contract.md.

Usage (invoked by webui-snapshot.sh):
  python3 codelattice-snapshot-gen.py \\
    --analyze <analyze.json> --quality <quality.json> \\
    --timestamp <iso8601> --version <ver> \\
    --root <path> --language <lang> \\
    [--explore] [--review] [--workflows] [--redact-root] [--compact]

Output: JSON to stdout.
"""

import json
import os
import re
import sys
from collections import Counter
from datetime import datetime, timezone

# ── Constants ────────────────────────────────────────────────────────────────

SCHEMA_VERSION = "webui.snapshot.v1"
MAX_SYMBOLS_DEFAULT = 500
MAX_SOURCE_FILES_DEFAULT = 200

WORKFLOW_PRESETS = [
    {
        "id": "onboarding",
        "name": "项目接入 / Onboarding",
        "description": "首次接入 CodeLattice：理解项目结构、符号分布、入口点",
        "tools": ["analyze", "summary", "explore"],
        "stopLines": ["不执行目标项目代码", "不修改源码", "静态分析结果仅供参考"]
    },
    {
        "id": "before_edit",
        "name": "编辑前检查 / Before Edit",
        "description": "修改代码前了解影响范围、调用链、风险点",
        "tools": ["impact_preview", "context", "analyze"],
        "stopLines": ["不替代 code review", "运行时行为需实测确认", "trait 解析为启发式"]
    },
    {
        "id": "after_edit",
        "name": "编辑后验证 / After Edit",
        "description": "修改后快速检查格式、符号完整性、基本质量门禁",
        "tools": ["quality", "analyze", "detect-changes"],
        "stopLines": ["不运行测试套件", "不执行 package manager", "不保证无回归"]
    },
    {
        "id": "delete_code",
        "name": "删除代码前评估 / Delete Code Assessment",
        "description": "删除代码/模块前识别引用关系、死代码候选、外部使用风险",
        "tools": ["impact_preview", "context", "analyze --include calls"],
        "stopLines": ["dead-code candidate ≠ 可安全删除", "外部 API heuristic 不等于真实使用者", "必须人工复核"]
    },
    {
        "id": "release_check",
        "name": "发布前检查 / Release Check",
        "description": "版本发布前的静态审查：breaking change 风险、文档一致性、配置示例",
        "tools": ["quality", "analyze", "release_review"],
        "stopLines": ["不是 GA 质量证明", "不覆盖运行时测试", "不验证外部依赖兼容性"]
    },
    {
        "id": "legacy_cleanup",
        "name": "遗留代码清理 / Legacy Cleanup",
        "description": "识别未使用的符号、过时的模块、可简化的调用链",
        "tools": ["analyze --include calls", "quality", "cleanup_summary"],
        "stopLines": ["低置信度标记需逐一核实", "不自动删除任何代码", "framework entry 点不可轻移"]
    },
    {
        "id": "public_api_change",
        "name": "公共 API 变更评估 / Public API Change",
        "description": "变更 public/exported 符号前评估下游影响、ABI 兼容性",
        "tools": ["impact_preview", "context", "analyze --include graph"],
        "stopLines": ["external usage 为启发式推断", "文档同步需人工处理", "semantic versioning 需人工判断"]
    },
    {
        "id": "framework_route_change",
        "name": "框架路由变更 / Framework Route Change",
        "description": "修改框架入口/路由/控制器时的影响分析",
        "tools": ["analyze", "impact_preview", "entry_points"],
        "stopLines": ["路由解析为模式匹配", "动态路由不可完全覆盖", "需结合框架文档"]
    },
    {
        "id": "docs_tests_sync",
        "name": "文档-测试同步检查 / Docs-Tests Sync",
        "description": "发现文档与测试覆盖不一致的区域、缺失的 API 文档候选",
        "tools": ["analyze", "quality", "release_review"],
        "stopLines": ["基于文件名/符号名的启发式", "不解析文档内容语义", "不判断测试充分性"]
    },
    {
        "id": "config_examples_sync",
        "name": "配置-示例同步检查 / Config-Examples Sync",
        "description": "发现配置项与示例/文档不同步的问题",
        "tools": ["analyze", "quality", "release_review"],
        "stopLines": ["不验证配置值正确性", "不执行配置加载", "模板/占位符可能误报"]
    }
]

# ── Helpers ──────────────────────────────────────────────────────────────────

def safe_parse(text: str) -> dict:
    """Parse JSON text, return dict (empty on failure)."""
    if not text or not text.strip():
        return {}
    try:
        obj = json.loads(text)
        return obj if isinstance(obj, dict) else {}
    except (json.JSONDecodeError, TypeError):
        return {}

def redact_path(path: str, root: str) -> str:
    """Replace absolute root and common user paths with <redacted-root>."""
    if not path:
        return path

    # Strip URI-like prefixes (file:/, repo:/, py:repo:) before processing
    uri_prefix = ""
    check = path
    for prefix in ("file:", "repo:", "py:repo:", "c:repo:"):
        if path.startswith(prefix):
            uri_prefix = prefix
            check = path[len(prefix):]
            break

    # 1. Replace the project root (in the stripped path)
    if root and check.startswith(root):
        rest = check[len(root):]
        if rest.startswith("/"):
            rest = rest[1:]
        return f"{uri_prefix}<redacted-root>/{rest}" if rest else f"{uri_prefix}<redacted-root>"

    # 2. Common user home patterns
    for prefix in ["/Users/", "/home/", "/tmp/"]:
        if check.startswith(prefix):
            parts = check.split("/")
            if len(parts) > 2:
                return f"{uri_prefix}<redacted-user>/{'/'.join(parts[3:])}"
            return f"{uri_prefix}<redacted-path>"

    # 3. Absolute paths that look like workspace roots
    if check.startswith("/") and "/" in check[1:]:
        parts = check.split("/")
        if len(parts) >= 3:
            return f"{uri_prefix}<redacted-abs>/{'/'.join(parts[-(min(3, len(parts))):])}"

    # If no redaction needed but had a prefix, still return as-is
    return path

def symbol_kind_label(kind: str) -> str:
    """Normalize symbol kind for display."""
    mapping = {
        "function": "Function",
        "method": "Method",
        "struct": "Struct",
        "enum": "Enum",
        "trait": "Trait",
        "impl": "Impl",
        "mod": "Module",
        "const": "Constant",
        "static": "Static",
        "type": "Type Alias",
        "macro": "Macro",
        "interface": "Interface",
        "class": "Class",
        "variable": "Variable",
        "parameter": "Parameter",
        "fn": "Function",
        "unknown": "Unknown",
    }
    return mapping.get(kind, kind)

def _extract_path_from_id(node_id: str) -> str:
    """Extract a file path from node IDs like 'py:src:src/main.py' or 'c:src:main.c'."""
    if not node_id or ":" not in node_id:
        return ""
    # Patterns like rust:symbol::crate::path, py:src:path, c:src:path
    parts = node_id.split(":")
    # Look for path-like segments (contain .py/.rs/.c/.cpp/.ts/ etc.)
    for p in parts:
        if any(p.endswith(ext) for ext in (".py", ".rs", ".c", ".cpp", ".h", ".ts", ".tsx", ".ets")):
            return p
    # If no extension match, return the last segment if it looks like a path
    if len(parts) >= 3 and "/" in parts[-1]:
        return parts[-1]
    return ""

def _looks_like_absolute_path(s: str) -> bool:
    """Check if a string looks like an absolute file path that needs redaction."""
    if not s or len(s) < 5:
        return False
    # Check both absolute (/path) and prefixed (file:/path, repo:/path) patterns FIRST
    is_prefixed = s.startswith("file:/") or s.startswith("repo:/") or s.startswith("py:repo:/")
    is_abs = s.startswith("/")
    if not is_prefixed and not is_abs:
        return False
    if "/" not in (s if is_prefixed else s[1:]):
        return False
    # For prefixed paths, extract the part after the prefix for further checks
    check_str = s
    for prefix in ("file:", "repo:", "py:repo:"):
        if s.startswith(prefix):
            check_str = s[len(prefix):]
            break
    indicators = ["/Users/", "/home/", "/tmp/", "/Desktop/",
                  "/opt/", "/usr/local/", "/var/",
                  "fixtures/"]
    for ind in indicators:
        if ind in s:
            return True
    for ext in ('.py"', '.rs"', '.ts"', '.c"', '.cpp"',
                '.py,', '.rs,', '.ts,', '.c,', '.cpp,'):
        if ext in s:
            return True
    return False

def _redact_all_paths(obj, root: str) -> None:
      # dbg removed open("/tmp/was_called.txt","w").write("yes")
    """Recursively redact absolute paths in all string values of a JSON-like object."""
    if isinstance(obj, dict):
        for key, value in list(obj.items()):
            if isinstance(value, str) and _looks_like_absolute_path(value):
                obj[key] = redact_path(value, root)
            else:
                _redact_all_paths(value, root)
    elif isinstance(obj, list):
        for i, item in enumerate(obj):
            if isinstance(item, str) and _looks_like_absolute_path(item):
                obj[i] = redact_path(item, root)
            else:
                _redact_all_paths(item, root)

# ── Section Builders ─────────────────────────────────────────────────────────

def build_summary(analyze: dict, quality: dict) -> dict:
    """Build summary section from analyze output."""
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])
    edges = graph.get("edges", {})

    # Count by node label
    label_counts = Counter(n.get("label", "?") for n in nodes)
    symbol_count = label_counts.get("symbol", 0)
    source_file_count = label_counts.get("source-file", 0)
    module_count = label_counts.get("module", 0)
    package_count = label_counts.get("package", 0)

    # Edge counts
    edge_count = 0
    if isinstance(edges, list):
        edge_count = len(edges)
    elif isinstance(edges, dict):
        for key, val in edges.items():
            if isinstance(val, list):
                edge_count += len(val)

    # Language from metadata
    language = analyze.get("metadata", {}).get("language", "unknown")

    return {
        "schemaVersion": SCHEMA_VERSION,
        "nodeCount": len(nodes),
        "edgeCount": edge_count,
        "symbolCount": symbol_count,
        "sourceFileCount": source_file_count,
        "moduleCount": module_count,
        "packageCount": package_count,
        "generatedAt": "",  # filled by caller
        "language": language,
        "toolVersion": "",  # filled by caller
    }

def build_quality_section(analyze: dict, quality: dict) -> dict:
    """Build quality section from quality gate output."""
    gates = []
    if isinstance(quality, dict):
        raw_gates = quality.get("gates", [])
        if isinstance(raw_gates, list):
            gates = raw_gates
        elif isinstance(raw_gates, dict):
            # Some formats use dict of gates
            gates = [
                {"name": k, **(v if isinstance(v, dict) else {"status": str(v)})}
                for k, v in raw_gates.items()
            ]

    passed = sum(1 for g in gates if str(g.get("status", "")).lower() in ("pass", "passed", "ok", "true"))
    failed = sum(1 for g in gates if str(g.get("status", "")).lower() in ("fail", "failed", "error"))

    overall = "unknown"
    if isinstance(quality, dict) and "overall" in quality:
        overall = str(quality["overall"])
    elif failed == 0 and passed > 0:
        overall = "pass"
    elif failed > 0:
        overall = "fail"

    # Diagnostics summary from analyze
    diagnostics = analyze.get("diagnostics", [])
    diag_by_level = Counter(d.get("level", "unknown") for d in diagnostics if isinstance(d, dict))

    return {
        "overall": overall,
        "gates": gates,
        "passedGateCount": passed,
        "failedGateCount": failed,
        "diagnosticsSummary": {
            "total": len(diagnostics),
            "error": diag_by_level.get("error", 0),
            "warning": diag_by_level.get("warning", 0),
            "info": diag_by_level.get("info", 0),
        } if diagnostics else None,
        "cautions": [
            "Quality gates are based on static analysis only.",
            "Pass/fail status does not guarantee runtime correctness.",
            "External crate resolution is bounded; stdlib-only.",
        ]
    }

def build_explore_section(analyze: dict, max_symbols: int, max_files: int,
                           redact_root: bool, root: str) -> dict:
    """Build explore section with source files and symbols."""
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])

    # Separate nodes by type — handle both label-based (Rust) and kind-based (C/C++/Python/TS)
    def is_symbol_node(n):
        return n.get("label") == "symbol" or n.get("kind") == "symbol"

    def is_source_file_node(n):
        return n.get("label") == "source-file" or n.get("kind") == "source-file"

    def is_repo_node(n):
        return n.get("kind") in ("repository", "repo") or n.get("label") in ("repository", "repo")

    sym_nodes = [n for n in nodes if is_symbol_node(n) and not is_repo_node(n)]
    sf_nodes = [n for n in nodes if is_source_file_node(n)]

    # Source files
    source_files = []
    sf_symbol_counts = Counter()

    # Build symbols list
    symbols = []
    for n in sym_nodes[:max_symbols]:
        props = n.get("properties", {})
        node_id = n.get("id", "")

        # Extract source path from multiple possible locations
        src_path = (
            props.get("sourcePath")
            or props.get("file")
            or props.get("path")
            or _extract_path_from_id(node_id)
            or "?"
        )
        if redact_root:
            src_path = redact_path(src_path, root)

        kind = props.get("symbolKind", props.get("kind", "unknown"))
        visibility = props.get("visibility", None)

        sym_entry = {
            "id": n.get("id", ""),
            "name": props.get("name", n.get("id", "")),
            "kind": kind,
            "kindLabel": symbol_kind_label(kind),
            "file": src_path,
            "line": props.get("lineStart", props.get("line", None)),
            "endLine": props.get("lineEnd", None),
        }
        if visibility:
            sym_entry["visibility"] = visibility
        if visibility in ("pub", "public", "exported", "export"):
            sym_entry["exported"] = True

        symbols.append(sym_entry)
        sf_symbol_counts[src_path] += 1

    # Source file entries
    for n in sf_nodes[:max_files]:
        props = n.get("properties", {})
        path = (
            props.get("path")
            or props.get("sourcePath")
            or n.get("label")
            or _extract_path_from_id(n.get("id", ""))
            or "?"
        )
        if redact_root:
            path = redact_path(path, root)

        sf_entry = {
            "path": path,
            "language": props.get("language", ""),
            "symbolCount": sf_symbol_counts.get(path, 0),
        }
        source_files.append(sf_entry)

    # Also add source files that have symbols but no source-file node
    seen_paths = {sf["path"] for sf in source_files}
    for path, count in sf_symbol_counts.most_common(max_files):
        if path and path not in seen_paths and path != "?":
            source_files.append({
                "path": path,
                "language": "",
                "symbolCount": count,
            })
            seen_paths.add(path)

    # Top files by symbol count
    top_files = [
        {"path": p, "symbolCount": c, "reason": "highest-symbol-count"}
        for p, c in sf_symbol_counts.most_common(10)
        if p and p != "?"
    ]

    status = "collected" if (symbols or source_files) else "empty"
    return {
        "status": status,
        "sourceFiles": source_files[:max_files],
        "symbols": symbols[:max_symbols],
        "topFiles": top_files,
        "totalSymbols": len(sym_nodes),
        "totalSourceFiles": len(sf_nodes),
        "truncated": len(sym_nodes) > max_symbols or len(sf_nodes) > max_files,
    }

def build_cleanup_section(analyze: dict) -> dict:
    """Build cleanup summary using heuristics from graph data."""
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])
    edges = graph.get("edges", [])

    # Flatten edges if it's a dict
    edge_list = []
    if isinstance(edges, list):
        edge_list = edges
    elif isinstance(edges, dict):
        for v in edges.values():
            if isinstance(v, list):
                edge_list.extend(v)

    sym_nodes = [n for n in nodes if n.get("label") == "symbol"]
    sym_ids = set(n.get("id", "") for n in sym_nodes)

    # Find symbols with no incoming CALLS/CONTAINS edges (potential dead code)
    target_ids = set()
    source_ids = set()
    for e in edge_list:
        if isinstance(e, dict):
            t = e.get("target", e.get("to", ""))
            s = e.get("source", e.get("from", ""))
            if t:
                target_ids.add(t)
            if s:
                source_ids.add(s)

    # Symbols that are sources but never targets of certain edge types
    call_targets = set()
    call_sources = set()
    for e in edge_list:
        if isinstance(e, dict):
            etype = e.get("type", e.get("label", ""))
            if "call" in etype.lower():
                target_ids.add(e.get("target", e.get("to", "")))
                source_ids.add(e.get("source", e.get("from", "")))

    # Heuristic: symbols not called by anything else (excluding entry points)
    uncalled = [s for s in sym_nodes if s.get("id", "") not in call_targets]
    exported_uncalled = [
        s for s in uncalled
        if s.get("properties", {}).get("visibility") in ("pub", "public", "exported")
    ]

    candidates = min(len(uncalled), len(sym_nodes))
    external_api = len([n for n in sym_nodes
                        if n.get("properties", {}).get("visibility") in ("pub", "public", "exported")])

    return {
        "status": "partial" if sym_nodes else "not_collected",
        "deadCodeCandidateCount": candidates if candidates > 0 else None,
        "unreachableCandidateCount": len(uncalled) if uncalled else None,
        "externalApiSurfaceCount": external_api if external_api > 0 else None,
        "frameworkEntryHintCount": None,  # Would need deeper analysis
        "cautions": [
            "Dead-code detection is heuristic-based on call-graph shape.",
            "Candidates are NOT proven unused — they may be called via reflection/dynamic dispatch/tests.",
            "Public/exported symbols may be used by external crates not analyzed here.",
            "Framework entry points (main/test/bin) should NEVER be removed based on this analysis.",
            "Auto-deletion is explicitly forbidden without human review + test regression check.",
        ]
    }

def build_release_review_section(analyze: dict) -> dict:
    """Build release review summary from static analysis data."""
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])

    sym_nodes = [n for n in nodes if n.get("label") == "symbol"]

    # Count public/exported symbols as breaking-change surface
    pub_symbols = [n for n in sym_nodes
                   if n.get("properties", {}).get("visibility") in ("pub", "public", "exported")]

    # Heuristic: look for doc-related patterns in file paths
    all_paths = set()
    for n in nodes:
        props = n.get("properties", {})
        p = props.get("sourcePath", props.get("path", props.get("file", "")))
        if p:
            all_paths.add(p)

    doc_files = [p for p in all_paths if any(ext in p.lower() for ext in
                  [".md", ".rst", ".txt", "doc/", "docs/", "readme"])]
    test_files = [p for p in all_paths if any(pattern in p.lower() for pattern in
                  ["test_", "_test.", "tests/", "spec.", "mock.", "fixture"])]

    return {
        "status": "partial" if sym_nodes else "not_collected",
        "breakingChangeRisk": "medium" if len(pub_symbols) > 20 else "low",
        "breakingChangeSurface": len(pub_symbols),
        "staleDocCandidateCount": len(doc_files) if doc_files else None,
        "missingTestCandidateCount": None,  # Would need coverage data
        "configExampleIssueCount": None,  # Would need config file parsing
        "cautions": [
            "Release review is based on static analysis only — does not run tests or verify docs accuracy.",
            "Breaking-change risk assessment is heuristic; actual impact depends on downstream usage.",
            "Documentation staleness requires manual review — this only lists doc files found.",
            "Test coverage gaps require test runner integration — not available in static mode.",
            "Config example drift needs template comparison — not performed in this snapshot.",
        ]
    }

def build_workflow_presets_section(include: bool) -> dict:
    """Build workflow presets section."""
    if not include:
        return {"status": "not_collected", "presets": []}
    return {
        "status": "collected",
        "presets": WORKFLOW_PRESETS,
    }

def build_limitations() -> dict:
    """Build standard limitations section."""
    return {
        "runtimeVerified": False,
        "externalUsageVerified": False,
        "coverageVerified": False,
        "deletionSafetyVerified": False,
        "projectCodeExecuted": False,
        "notes": [
            "All results derived from static source analysis via CodeLattice CLI.",
            "No project code was executed during snapshot generation.",
            "Call graph is built from syntactic patterns; method dispatch is heuristic.",
            "External crate symbols are resolved only for std/core/alloc direct imports.",
            "Trait resolution and type inference are NOT performed.",
            "Macro expansion is NOT performed.",
            "Results are informative, not prescriptive. Always verify before acting.",
        ]
    }

def build_generated_from(version: str) -> dict:
    """Build generatedFrom metadata."""
    return {
        "tool": "CodeLattice",
        "toolVersion": version,
        "snapshotSchema": SCHEMA_VERSION,
        "staticAnalysis": True,
        "runtimeVerified": False,
        "generationMethod": "cli-aggregate-phase-a",
    }

# ── Insights (optional enrichment) ───────────────────────────────────────────

def build_insights(analyze: dict) -> dict:
    """Build insights section with hotspots and entry points."""
    graph = analyze.get("graph", {})
    nodes = graph.get("nodes", [])
    edges = graph.get("edges", [])

    edge_list = []
    if isinstance(edges, list):
        edge_list = edges
    elif isinstance(edges, dict):
        for v in edges.values():
            if isinstance(v, list):
                edge_list.extend(v)

    # Fan-out hotspot: symbols that call many others
    fan_out = Counter()
    fan_in = Counter()
    for e in edge_list:
        if isinstance(e, dict):
            s = e.get("source", e.get("from", ""))
            t = e.get("target", e.get("to", ""))
            if s:
                fan_out[s] += 1
            if t:
                fan_in[t] += 1

    # Build hotspot list with file info
    sym_map = {}
    for n in nodes:
        if n.get("label") == "symbol":
            sym_map[n.get("id", "")] = n

    hotspots = []
    for sym_id, count in fan_out.most_common(5):
        if count >= 3:  # Only high-fan-out
            n = sym_map.get(sym_id, {})
            props = n.get("properties", {})
            hotspots.append({
                "id": sym_id,
                "name": props.get("name", sym_id),
                "fanOut": count,
                "fanIn": fan_in.get(sym_id, 0),
                "file": props.get("sourcePath", props.get("file", "")),
                "kind": props.get("symbolKind", props.get("kind", "")),
            })

    # Entry point hints: main, test functions, lib entry
    entry_points = []
    for n in nodes:
        if n.get("label") != "symbol":
            continue
        props = n.get("properties", {})
        name = props.get("name", "")
        kind = props.get("symbolKind", props.get("kind", ""))
        vis = props.get("visibility", "")
        # Heuristic entry points
        if name in ("main", "test_main", "_start", "init", "Main"):
            entry_points.append({
                "id": n.get("id", ""),
                "name": name,
                "kind": kind,
                "file": props.get("sourcePath", props.get("file", "")),
                "reason": "well-known-entry-point",
            })
        elif vis in ("pub", "public") and kind in ("function", "fn") and fan_in.get(n.get("id", ""), 0) == 0:
            # Public function with no callers — possible API entry
            entry_points.append({
                "id": n.get("id", ""),
                "name": name,
                "kind": kind,
                "file": props.get("sourcePath", props.get("file", "")),
                "reason": "public-no-callers-api-candidate",
            })

    has_data = hotspots or entry_points
    return {
        "status": "partial" if has_data else "not_collected",
        "hotspots": hotspots[:5],
        "entryPoints": entry_points[:10],
        "reviewFirst": [dict(file=h.get("file", "?"), reason="high-fan-out-hotspot")
                        for h in hotspots[:3]],
        "cautions": [
            "Fan-in/out counts are based on resolved call edges only; unresolved calls are excluded.",
            "Entry-point detection is heuristic; may miss dynamic/conditional entries or include false positives.",
        ] if has_data else []
    }

# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    args = sys.argv[1:]

    # Parse arguments
    params = {"--explore": False, "--review": False, "--workflows": False,
              "--redact-root": False, "--compact": False}
    positional = []
    i = 0
    while i < len(args):
        if args[i] in params:
            params[args[i]] = True
        else:
            positional.append(args[i])
        i += 1

    # Expected positional: analyze_json_file quality_json timestamp version root lang [extra_flags...]
    if len(positional) < 6:
        print(json.dumps({"_error": f"insufficient_args: got {len(positional)} positional, need >=6"}))
        sys.exit(1)

    analyze_file = positional[0]
    quality_file = positional[1]
    timestamp = positional[2]
    version_str = positional[3]
    root = positional[4]
    language = positional[5]
    extra_flags = positional[6:] if len(positional) > 6 else []

    # Parse extra flags from bash
    for flag in extra_flags:
        if flag == "EXPLORE":
            params["--explore"] = True
        elif flag == "REVIEW":
            params["--review"] = True
        elif flag == "WORKFLOWS":
            params["--workflows"] = True
            params["--redact-root"] = True
        elif flag == "COMPACT":
            params["--compact"] = True

    # Read input files
    try:
        with open(analyze_file, "r") as f:
            analyze_text = f.read()
    except (FileNotFoundError, OSError):
        analyze_text = "{}"

    try:
        with open(quality_file, "r") as f:
            quality_text = f.read()
    except (FileNotFoundError, OSError):
        quality_text = "{}"

    analyze = safe_parse(analyze_text)
    quality = safe_parse(quality_text)

    # Determine what sections to include
    do_explore = params["--explore"]
    do_review = params["--review"]
    do_workflows = params["--workflows"]
    redact_root = params["--redact-root"]
    compact = params["--compact"]

    # ── Build the snapshot ───────────────────────────────────────────

    summary = build_summary(analyze, quality)
    summary["generatedAt"] = timestamp
    summary["toolVersion"] = version_str

    snapshot = {
        "schemaVersion": SCHEMA_VERSION,
        "generatedAt": timestamp,
        "generatedFrom": build_generated_from(version_str),
        "summary": summary,
        "quality": build_quality_section(analyze, quality),
        "limitations": build_limitations(),
    }

    # Optional enriched sections
    if do_explore:
        snapshot["explore"] = build_explore_section(
            analyze, MAX_SYMBOLS_DEFAULT, MAX_SOURCE_FILES_DEFAULT, redact_root, root
        )

    if do_review:
        snapshot["cleanup"] = build_cleanup_section(analyze)
        snapshot["releaseReview"] = build_release_review_section(analyze)
        snapshot["insights"] = build_insights(analyze)

    if do_workflows:
        snapshot["workflowPresets"] = build_workflow_presets_section(True)

    # Global path redaction: scan all string values in the final snapshot
    # This catches paths embedded in id/name fields from CLI raw output
    if redact_root:
        _redact_all_paths(snapshot, root)

    # Output
    indent = None if compact else 2
    output = json.dumps(snapshot, indent=indent, ensure_ascii=False)
    
    # Clean up temp files
    for f in [analyze_file, quality_file]:
        if f.startswith("/tmp/codelattice-snap-"):
            try:
                os.unlink(f)
            except OSError:
                pass

    print(output)
    sys.exit(0)

if __name__ == "__main__":
    main()
