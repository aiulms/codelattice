#!/usr/bin/env python3
"""CodeLattice WebUI Runner — Phase E: Project Workbench + Profiles + Library.

Uses Python stdlib only. Binds 127.0.0.1.
Serves webui/snapshot-viewer/ + REST API with unified response structure.
"""
import http.server, json, os, subprocess, sys, time, urllib.parse, hashlib, shutil, re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
STATIC_D = REPO_ROOT / "webui" / "snapshot-viewer"
SNAP_SCRIPT = REPO_ROOT / "scripts" / "webui-snapshot.sh"
GEN_TIMEOUT = 120
SUPPORTED = ["rust","typescript","c","cpp","python","shell","arkts","cangjie","auto"]
LANG_MARKERS = {
    "cjpm.toml": "cangjie",
    "oh-package.json5": "arkts",
    "Cargo.toml": "rust",
    "tsconfig.json": "typescript",
    "pyproject.toml": "python",
    "setup.py": "python",
    "requirements.txt": "python",
    "build-profile.json5": "arkts",
    "CMakeLists.txt": "c/cpp",
    "compile_commands.json": "c/cpp",
    "Makefile": "c/cpp",
    "go.mod": "unsupported:go",
    ".sln": "unsupported:csharp",
    ".csproj": "unsupported:csharp",
    "pom.xml": "unsupported:java",
    "build.gradle": "unsupported:java",
    "build.gradle.kts": "unsupported:kotlin",
    "Package.swift": "unsupported:swift",
}
LANG_PRIORITY = {"cangjie": 0, "arkts": 1, "rust": 2, "typescript": 3, "python": 4, "c/cpp": 5, "c": 5, "cpp": 5, "shell": 6, "unsupported:csharp": 9, "unsupported:go": 9, "unsupported:java": 9, "unsupported:kotlin": 9, "unsupported:swift": 9}

# Workspace scan: file extensions for language detection without manifest files
WORKSPACE_EXT_LANG = {
    ".cj": "cangjie",
    ".ets": "arkts",
    ".rs": "rust",
    ".ts": "typescript", ".tsx": "typescript",
    ".py": "python",
    ".sh": "shell", ".bash": "shell", ".zsh": "shell", ".ksh": "shell", ".bats": "shell",
    ".c": "c", ".h": "c",
    ".cpp": "cpp", ".cc": "cpp", ".cxx": "cpp", ".hpp": "cpp", ".hh": "cpp", ".hxx": "cpp",
    ".cs": "unsupported:csharp",
    ".java": "unsupported:java",
    ".go": "unsupported:go",
    ".swift": "unsupported:swift",
    ".kt": "unsupported:kotlin", ".kts": "unsupported:kotlin",
}

# Workspace scan skip dirs
WORKSPACE_SKIP_DIRS = {".git", ".gitnexus", ".codelattice-webui", "target",
    "node_modules", "dist", "build", "__pycache__", ".venv", "venv"}

WORKSPACE_MAX_DEPTH = 5
WORKSPACE_MAX_ENTRIES = 5000
WORKSPACE_ANALYZE_MAX_PROJECTS = 20


def ok(data=None): return {"success": True, "data": data if data is not None else {}, "error": None, "hint": None}
def err(msg, code=400, hint=None):
    return {"success": False, "data": None, "error": msg, "hint": hint or "", "status": code}

def id_path_match(project, id_set):
    """Check if a project matches any ID in id_set (by path, relativePath, or name)."""
    if not id_set: return False
    for pid in id_set:
        if pid == project.get("path", "") or pid == project.get("relativePath", "") or pid == project.get("name", ""):
            return True
    return False


class Workbench(http.server.SimpleHTTPRequestHandler):
    sn_dir = str(REPO_ROOT / ".codelattice-webui" / "snapshots")
    pf_file = str(REPO_ROOT / ".codelattice-webui" / "profiles.json")
    ws_dir = str(REPO_ROOT / ".codelattice-webui" / "workspaces")

    def __init__(self, *a, **kw):
        self.dir = str(STATIC_D)
        super().__init__(*a, directory=self.dir, **kw)

    def log_message(self, f, *a):
        if "api/" in str(a[0] if a else ""):
            sys.stderr.write(f"[api] {a[0]}\n")

    def end_headers(self):
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def _resp(self, data, code=200):
        b = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(code); self.send_header("Content-Type","application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin","*"); self.send_header("Content-Length",len(b))
        self.end_headers(); self.wfile.write(b)

    def _rb(self):
        n = int(self.headers.get("Content-Length",0))
        if n == 0: return {}
        try: return json.loads(self.rfile.read(n))
        except: return {}

    def _safe_id(self, s):
        if not s or ".." in s or "/" in s or "\\" in s: return False
        return bool(re.match(r'^[a-f0-9]+$', s))

    def _ensure(self, d): os.makedirs(d, exist_ok=True)
    def _now(self): return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    def _nid(self): return hashlib.md5(str(time.time()).encode()).hexdigest()[:12]
    def _load_json(self, p):
        if not os.path.isfile(p): return None
        try:
            with open(p) as f: return json.load(f)
        except: return None
    def _save_json(self, p, d):
        self._ensure(os.path.dirname(p))
        with open(p,"w") as f: json.dump(d,f,ensure_ascii=False,indent=2)
    def _languages_for_files(self, files):
        langs = []
        for m, lang in LANG_MARKERS.items():
            if (m in files or any(name.endswith(m) for name in files)) and lang not in langs:
                langs.append(lang)
        if not langs:
            ext_langs = []
            for name in files[:120]:
                if name.endswith(".cj"): ext_langs.append("cangjie")
                elif name.endswith(".ets"): ext_langs.append("arkts")
                elif name.endswith(".rs"): ext_langs.append("rust")
                elif name.endswith((".ts",".tsx")): ext_langs.append("typescript")
                elif name.endswith(".py"): ext_langs.append("python")
                elif name.endswith((".sh",".bash",".zsh",".ksh",".bats")): ext_langs.append("shell")
                elif name.endswith((".c",".h")): ext_langs.append("c")
                elif name.endswith((".cpp",".cc",".cxx",".hpp",".hh",".hxx")): ext_langs.append("cpp")
                elif name.endswith(".cs"): ext_langs.append("unsupported:csharp")
            for lang in ext_langs:
                if lang not in langs:
                    langs.append(lang)
        return langs

    def _project_candidates(self, root, limit=8):
        skip = {".git",".gitnexus",".claude",".opencode","target","node_modules","dist","build","__pycache__",".venv","venv"}
        out = []
        root = os.path.abspath(root)
        for base, dirs, files in os.walk(root):
            rel = os.path.relpath(base, root)
            depth = 0 if rel == "." else rel.count(os.sep) + 1
            dirs[:] = [d for d in dirs if d not in skip and not d.startswith(".tmp")]
            if depth > 4:
                dirs[:] = []
                continue
            langs = self._languages_for_files(files)
            if langs and base != root:
                supported = [l for l in langs if not l.startswith("unsupported:")]
                unsupported = [l.split(":",1)[1] for l in langs if l.startswith("unsupported:")]
                analysis_lang = next((l for l in supported if l != "c/cpp"), None) or ("cpp" if "c/cpp" in supported else "")
                out.append({"path": base, "label": os.path.basename(base) or base, "languages": langs[:4], "supportedLanguages": supported, "unsupportedLanguages": unsupported, "analysisLanguage": analysis_lang, "depth": depth})
        out.sort(key=lambda c: (min(LANG_PRIORITY.get(l, 9) for l in c["languages"]), c.get("depth", 99), c["path"]))
        return out[:limit]

    def _project_inventory_data(self, root):
        root = os.path.abspath(root)
        if not os.path.exists(root):
            return {"root": root, "exists": False, "isDir": False, "status": "not_found", "message": "路径不存在。", "candidates": [], "supportedLanguages": [], "unsupportedLanguages": []}
        if not os.path.isdir(root):
            return {"root": root, "exists": True, "isDir": False, "status": "not_directory", "message": "请选择项目目录，不是文件。", "candidates": [], "supportedLanguages": [], "unsupportedLanguages": []}
        try:
            root_files = [p.name for p in Path(root).iterdir() if p.is_file()]
        except Exception:
            root_files = []
        root_langs = self._languages_for_files(root_files)
        root_supported = [l for l in root_langs if not l.startswith("unsupported:")]
        root_unsupported = [l.split(":",1)[1] for l in root_langs if l.startswith("unsupported:")]
        candidates = self._project_candidates(root, limit=16)
        supported_candidates = [c for c in candidates if c.get("supportedLanguages")]
        unsupported_candidates = [c for c in candidates if c.get("unsupportedLanguages") and not c.get("supportedLanguages")]
        if len(supported_candidates) > 1:
            status = "multi_project"
            message = "当前目录包含多个可分析子项目，已按工作区处理。"
        elif root_supported:
            status = "root_project"
            message = "当前目录看起来就是可分析项目。"
        elif len(supported_candidates) == 1 and not unsupported_candidates:
            status = "single_candidate"
            message = "当前目录不是项目根，但发现 1 个可分析子项目。"
        elif supported_candidates:
            status = "multi_project"
            message = "当前目录包含多个可分析子项目，请选择具体项目。"
        elif root_unsupported or unsupported_candidates:
            status = "unsupported_only"
            message = "发现暂不支持的语言模块，当前不会生成图谱。"
        else:
            status = "empty"
            message = "未发现 CodeLattice 当前支持的项目标记。"
        primary = supported_candidates[0] if supported_candidates else None
        analysis_lang = root_supported[0] if root_supported else (primary or {}).get("analysisLanguage", "")
        if analysis_lang == "c/cpp": analysis_lang = "cpp"
        return {
            "root": root,
            "rootLabel": os.path.basename(root) or root,
            "exists": True,
            "isDir": True,
            "status": status,
            "message": message,
            "supportedLanguages": root_supported,
            "unsupportedLanguages": root_unsupported,
            "candidates": candidates,
            "supportedCandidateCount": len(supported_candidates),
            "unsupportedCandidateCount": len(unsupported_candidates),
            "recommendedRoot": root if root_supported else ((primary or {}).get("path") or ""),
            "recommendedLanguage": analysis_lang if analysis_lang in SUPPORTED else "auto",
            "staticOnly": True
        }

    def _project_inventory(self, qs):
        root = (qs.get("root", [""])[0] or "").strip()
        if not root:
            return err("root is required", 400)
        return ok(self._project_inventory_data(root))

    def _inventory_hint(self, inv):
        lines = [inv.get("message") or "请选择可分析项目目录。"]
        candidates = inv.get("candidates") or []
        supported = [c for c in candidates if c.get("supportedLanguages")]
        unsupported = [c for c in candidates if c.get("unsupportedLanguages") and not c.get("supportedLanguages")]
        if supported:
            lines.append("可分析的候选子项目：")
            lines += [f"- {c['path']} ({', '.join(c.get('supportedLanguages') or c.get('languages') or [])})" for c in supported[:12]]
        if unsupported:
            lines.append("发现但暂不支持的模块：")
            lines += [f"- {c['path']} (unsupported:{', '.join(c.get('unsupportedLanguages') or [])})" for c in unsupported[:8]]
        return "\n".join(lines)[:1600]

    def _safe_root(self, root):
        """Prevent path traversal: root must be absolute and exist."""
        if not root or not root.strip(): return None, err("root is required", 400)
        root = os.path.abspath(root.strip())
        if ".." in os.path.relpath(root, "/"):
            return None, err("invalid path", 400, "path traversal detected")
        if not os.path.exists(root):
            return None, err("root not found", 400, f"path does not exist: {root}")
        if not os.path.isdir(root):
            return None, err("not a directory", 400, f"path is not a directory: {root}")
        return root, None

    def _workspace_scan(self, root):
        """Scan directory tree for supported and unsupported project markers.
        Returns (entries, truncated). Heuristic only — no file content read, no scripts executed.
        Skips directories that are inside already-detected project roots."""
        entries = []
        scanned = 0
        truncated = False
        # Track parent project directories to skip their children
        project_roots = set()
        for base, dirs, files in os.walk(root):
            rel = os.path.relpath(base, root)
            depth = 0 if rel == "." else rel.count(os.sep) + 1
            # 排除目录
            dirs[:] = [d for d in dirs if d not in WORKSPACE_SKIP_DIRS and not d.startswith(".tmp")]
            if depth > WORKSPACE_MAX_DEPTH:
                dirs[:] = []
                continue
            if scanned + len(dirs) > WORKSPACE_MAX_ENTRIES:
                truncated = True
                dirs[:] = dirs[:max(0, WORKSPACE_MAX_ENTRIES - scanned)]
            scanned += len(dirs)
            # 跳过已在父项目内部的子目录
            skip_children = False
            for pr in project_roots:
                if base.startswith(pr + os.sep):
                    skip_children = True
                    break
            if skip_children:
                continue
            # 根目录本身不算子项目
            if rel == ".": continue
            # 检测 marker 文件
            found_markers_raw = [m for m in LANG_MARKERS if m in files]
            found_markers_raw += [m for m in LANG_MARKERS if any(f.endswith(m) for f in files)]
            # 去重
            seen = set(); found_markers = [m for m in found_markers_raw if not (m in seen or seen.add(m))]
            ext_langs = set()
            # 决定是否将此目录识别为项目/模块
            if not found_markers:
                # 回退：检查文件扩展名
                for name in files[:200]:
                    _, ext = os.path.splitext(name)
                    lang = WORKSPACE_EXT_LANG.get(ext.lower(), "")
                    if lang: ext_langs.add(lang)
            has_langs = bool(found_markers) or bool(ext_langs)
            if not has_langs:
                continue
            if not found_markers:
                found_markers = []
            if has_langs:
                # 确定语言
                langs = []
                for m in found_markers:
                    lang = LANG_MARKERS.get(m, "")
                    if lang and lang not in langs:
                        langs.append(lang)
                # 也扫描扩展名补充
                ext_langs = set()
                for name in files[:120]:
                    _, ext = os.path.splitext(name)
                    elang = WORKSPACE_EXT_LANG.get(ext.lower(), "")
                    if elang and elang not in langs: ext_langs.add(elang)
                for el in ext_langs:
                    if el not in langs: langs.append(el)
                if not langs: continue
                # 区分 supported / unsupported
                supported = [l for l in langs if not l.startswith("unsupported:")]
                unsupported = [l.split(":", 1)[1] for l in langs if l.startswith("unsupported:")]
                # 确定主要语言（用于分析）
                langs_no_cc = [l for l in supported if l not in ("c/cpp", "c", "cpp")]
                analysis_lang = langs_no_cc[0] if langs_no_cc else ("cpp" if any(l in ("c/cpp", "c", "cpp") for l in supported) else "auto")
                # 如果当前目录被识别为项目根（有 marker 文件），标记以跳过其子目录
                if found_markers:
                    project_roots.add(base)
                confidence = "high" if found_markers else ("medium" if ext_langs else "low")
                # 确定 reason
                if found_markers:
                    reason = ", ".join(found_markers[:3]) + " detected"
                else:
                    reason = "file extensions: " + ", ".join(sorted(ext_langs)[:4])
                entry = {
                    "path": base,
                    "relativePath": rel,
                    "name": os.path.basename(base),
                    "languages": langs[:6],
                    "confidence": confidence,
                    "reason": reason,
                    "markers": found_markers[:5],
                    "supported": bool(supported),
                    "vendorLike": "vendor" in rel.lower() or "third_party" in rel.lower(),
                    "depth": depth,
                }
                if supported:
                    entry["recommended"] = bool(langs_no_cc) and depth <= 3 and bool(found_markers)
                entries.append(entry)
            if scanned >= WORKSPACE_MAX_ENTRIES:
                truncated = True
                break
        return entries, truncated

    def _prepare_analysis_target(self, root, lang):
        if lang != "auto":
            return root, lang, None
        inv = self._project_inventory_data(root)
        status = inv.get("status")
        if status == "single_candidate" and inv.get("recommendedRoot"):
            return inv["recommendedRoot"], inv.get("recommendedLanguage") or "auto", None
        if status in {"multi_project", "unsupported_only", "empty"}:
            return root, lang, err("project selection required", 400, self._inventory_hint(inv))
        return root, lang, None

    def _workspace_inventory_data(self, root):
        """Build workspace inventory response."""
        # 1. 根目录自身检测
        try:
            root_files = [p.name for p in Path(root).iterdir() if p.is_file()]
        except Exception:
            return err("cannot read directory", 400, f"permission denied or unreadable: {root}")
        root_markers = [m for m in LANG_MARKERS if m in root_files]
        root_markers += [m for m in LANG_MARKERS if any(f.endswith(m) for f in root_files)]
        root_langs = []
        for m in set(root_markers):
            lang = LANG_MARKERS.get(m, "")
            if lang and lang not in root_langs: root_langs.append(lang)
        # 扩展名补充
        root_ext_langs = set()
        for name in root_files[:120]:
            _, ext = os.path.splitext(name)
            elang = WORKSPACE_EXT_LANG.get(ext.lower(), "")
            if elang: root_ext_langs.add(elang)
        for el in root_ext_langs:
            if el not in root_langs: root_langs.append(el)
        root_supported = [l for l in root_langs if not l.startswith("unsupported:")]
        root_unsupported = [l.split(":", 1)[1] for l in root_langs if l.startswith("unsupported:")]

        # 2. 子目录扫描
        entries, truncated = self._workspace_scan(root)

        # 3. 分类
        supported_projects = [e for e in entries if e.get("supported")]
        unsupported_modules = [e for e in entries if not e.get("supported")]
        self_projects = [e for e in supported_projects if e.get("recommended", False)]
        if not self_projects: self_projects = supported_projects[:4]  # pick top

        # 4. 语言分布
        lang_breakdown = {}
        for e in entries:
            for l in e.get("languages", []):
                key = l.replace("c/cpp", "c/cpp")
                lang_breakdown[key] = lang_breakdown.get(key, 0) + 1

        # 5. 生成时间
        generated_at = self._now()

        return ok({
            "root": root,
            "rootLabel": os.path.basename(root) or root,
            "generatedAt": generated_at,
            "staticOnly": True,
            "truncated": truncated,
            "summary": {
                "supportedProjectCount": len(supported_projects),
                "unsupportedModuleCount": len(unsupported_modules),
                "languageCount": len(lang_breakdown),
                "recommendedProjectCount": len(self_projects),
                "totalCandidateCount": len(entries),
            },
            "languageBreakdown": lang_breakdown,
            "supportedProjects": supported_projects,
            "unsupportedModules": unsupported_modules,
            "warnings": [],
            "generatedFrom": {
                "staticAnalysis": True,
                "filesContentRead": False,
                "scriptsExecuted": False,
                "runtimeVerified": False,
            },
        })

    def _workspace_inventory(self, qs):
        root = (qs.get("root", [""])[0] or "").strip()
        safe_root, blocker = self._safe_root(root)
        if blocker: return blocker
        return self._workspace_inventory_data(safe_root)

    def _workspace_analyze(self, body):
        """Analyze a workspace: run snapshot generation on multiple sub-projects."""
        root = (body.get("root") or "").strip()
        safe_root, blocker = self._safe_root(root)
        if blocker: return blocker
        # 先获取 inventory
        inv_result = self._workspace_inventory_data(safe_root)
        if not inv_result.get("success"):
            return inv_result
        inv = inv_result["data"]
        all_projects = inv.get("supportedProjects", [])
        if not all_projects:
            return err("no supported projects found in workspace", 400,
                       "请选择更具体的子项目，或使用 GET /api/workspace/inventory 查看详情。")
        mode = body.get("mode", "recommended").strip()
        project_ids = body.get("projectIds") or []
        redact = body.get("redactRoot", True)
        # 筛选目标项目
        if mode == "recommended":
            targets = [p for p in all_projects if p.get("recommended")]
            if not targets: targets = all_projects[:4]
        elif mode == "selected":
            id_set = set(project_ids)
            targets = [p for p in all_projects if id_path_match(p, id_set)]
            if not targets: return err("no matching projects for selected IDs", 400,
                                       f"available IDs: {[p.get('relativePath','') for p in all_projects[:10]]}")
        elif mode == "all":
            if len(all_projects) > WORKSPACE_ANALYZE_MAX_PROJECTS:
                return err("too many projects", 400,
                           f"共 {len(all_projects)} 个子项目，超过单次上限 {WORKSPACE_ANALYZE_MAX_PROJECTS}。请选择部分项目分析。")
            targets = all_projects
        else:
            return err("invalid mode", 400, "mode must be: recommended, selected, or all")
        # 执行分析
        ws_id = self._nid()
        ws_entry = {
            "workspaceId": ws_id,
            "root": safe_root,
            "generatedAt": self._now(),
            "summary": {"requestedProjectCount": len(targets), "succeededProjectCount": 0,
                        "failedProjectCount": 0, "snapshotCount": 0,
                        "totalSourceFiles": 0, "totalSymbols": 0, "totalEdges": 0},
            "projects": [],
            "unsupportedModules": inv.get("unsupportedModules", []),
            "warnings": [],
            "generatedFrom": {"staticAnalysis": True, "scriptsExecuted": False, "runtimeVerified": False},
        }
        for proj in targets:
            proj_path = proj["path"]
            proj_lang = self._detect_project_language(proj_path, proj.get("languages", []))
            pj_entry = {
                "projectId": proj.get("relativePath", os.path.basename(proj_path)),
                "path": proj_path,
                "language": proj_lang,
                "status": "pending",
                "snapshotId": None,
                "error": None,
                "summary": None,
            }
            try:
                gen_body = {"root": proj_path, "language": proj_lang, "full": True, "redactRoot": redact, "label": proj.get("name", "")}
                gen_result = self._generate(gen_body)
                if gen_result.get("success"):
                    pj_entry["status"] = "succeeded"
                    pj_entry["snapshotId"] = gen_result["data"].get("id", "")
                    pj_entry["summary"] = gen_result["data"].get("summary", {})
                    ws_entry["summary"]["succeededProjectCount"] += 1
                    ws_entry["summary"]["snapshotCount"] += 1
                    ws_entry["summary"]["totalSourceFiles"] += pj_entry["summary"].get("sourceFileCount", 0)
                    ws_entry["summary"]["totalSymbols"] += pj_entry["summary"].get("symbolCount", 0)
                else:
                    pj_entry["status"] = "failed"
                    pj_entry["error"] = gen_result.get("error", "generation failed")
                    ws_entry["summary"]["failedProjectCount"] += 1
                    ws_entry["warnings"].append(f"{proj.get('name','')}: {gen_result.get('error','unknown error')}")
            except Exception as exc:
                pj_entry["status"] = "failed"
                pj_entry["error"] = str(exc)
                ws_entry["summary"]["failedProjectCount"] += 1
                ws_entry["warnings"].append(f"{proj.get('name','')}: {str(exc)}")
            ws_entry["projects"].append(pj_entry)
        # 保存 workspace run
        self._save_workspace_run(ws_entry)
        return ok(ws_entry)

    def _detect_project_language(self, proj_path, langs):
        """Determine analysis language from project path and detected languages."""
        supported = [l for l in langs if not l.startswith("unsupported:") and l != "c/cpp"]
        if supported: return supported[0]
        # fallback: check markers at path
        try:
            files = [p.name for p in Path(proj_path).iterdir() if p.is_file()]
        except Exception:
            files = []
        for m, lang in LANG_MARKERS.items():
            if (m in files or any(f.endswith(m) for f in files)) and not lang.startswith("unsupported:"):
                if lang == "c/cpp": return "cpp"
                return lang
        return "auto"

    def _save_workspace_run(self, entry):
        self._ensure(self.ws_dir)
        wid = entry["workspaceId"]
        fp = os.path.join(self.ws_dir, f"workspace-{wid}.json")
        self._save_json(fp, entry)

    def _load_workspace_runs(self):
        self._ensure(self.ws_dir)
        runs = []
        try:
            for fn in sorted(os.listdir(self.ws_dir), reverse=True):
                if not fn.endswith(".json"): continue
                d = self._load_json(os.path.join(self.ws_dir, fn))
                if d and d.get("workspaceId"): runs.append(d)
        except Exception:
            pass
        return runs

    def _workspace_runs(self):
        return ok(self._load_workspace_runs())

    def _workspace_run_get(self, wid):
        if not wid or "/" in wid or "\\" in wid or ".." in wid:
            return err("invalid workspace id", 400)
        fp = os.path.join(self.ws_dir, f"workspace-{wid}.json")
        d = self._load_json(fp)
        if not d: return err("workspace run not found", 404)
        return ok(d)

    def _workspace_inventory_hint(self, inv):
        """Generate human-readable hint from inventory data."""
        lines = [f"工作区扫描结果：{inv['rootLabel']}"]
        sp = inv.get("supportedProjectCount", 0)
        um = inv.get("unsupportedModuleCount", 0)
        if sp > 0:
            lines.append(f"发现 {sp} 个可分析子项目，建议先分析推荐项目。")
        if um > 0:
            lines.append(f"发现 {um} 个暂不支持的语言模块。")
        lines.append("只读取目录结构和文件名，不读取文件内容、不执行项目代码。")
        return "\n".join(lines)[:1000]

    # ── Workspace Insights ─────────────────────────────────────────────

    def _workspace_insights(self, qs_or_body):
        """Get workspace insights from a run ID or run object."""
        run = None
        # Mode 1: GET with runId query param
        if isinstance(qs_or_body, dict) and "runId" in qs_or_body:
            run_id = qs_or_body.get("runId", "")
            if not run_id:
                return err("runId is required", 400)
            fp = os.path.join(self.ws_dir, f"workspace-{run_id}.json")
            run = self._load_json(fp)
            if not run:
                return err("workspace run not found", 404,
                           f"runId={run_id} not found. Use GET /api/workspace/runs to list available runs.")
        # Mode 2: POST with run object or workspaceRunId
        elif isinstance(qs_or_body, dict) and "workspaceRunId" in qs_or_body:
            run_id = qs_or_body.get("workspaceRunId", "")
            fp = os.path.join(self.ws_dir, f"workspace-{run_id}.json")
            run = self._load_json(fp)
            if not run:
                return err("workspace run not found", 404)
        elif isinstance(qs_or_body, dict) and "root" in qs_or_body:
            # Direct run object (for testing / convenience)
            run = qs_or_body
        else:
            return err("provide runId, workspaceRunId, or a run object", 400)

        if not isinstance(run, dict):
            return err("invalid run data", 400)

        # Compute insights
        projects = run.get("projects", [])
        unsupported = run.get("unsupportedModules", [])
        run_summary = run.get("summary", {})
        # Try to load snapshot summaries for each succeeded project
        snap_meta = {}
        try:
            self._ensure(self.sn_dir)
            idx = self._load_index()
            idx_by_id = {e["id"]: e for e in idx if e.get("id")}
        except Exception:
            idx_by_id = {}

        for pj in projects:
            sid = pj.get("snapshotId", "")
            if sid and sid in idx_by_id:
                snap_meta[sid] = idx_by_id[sid]

        scores = self._compute_project_scores(projects, unsupported, snap_meta)
        overall = self._compute_overall_score(scores)
        read_first, review_first, cleanup_first = self._compute_recommendations(
            projects, scores, unsupported)
        cross = self._compute_cross_project_signals(projects)

        return ok({
            "workspaceRunId": run.get("workspaceId", ""),
            "generatedAt": self._now(),
            "staticOnly": True,
            "summary": {
                "projectCount": len(projects),
                "succeededProjectCount": run_summary.get("succeededProjectCount", 0),
                "failedProjectCount": run_summary.get("failedProjectCount", 0),
                "unsupportedModuleCount": len(unsupported),
                "totalSourceFiles": run_summary.get("totalSourceFiles", 0),
                "totalSymbols": run_summary.get("totalSymbols", 0),
                "totalEdges": run_summary.get("totalEdges", 0),
                "overallHealthScore": overall["score"],
                "overallRiskLevel": overall["riskLevel"],
            },
            "projectScores": scores,
            "readFirst": read_first,
            "reviewFirst": review_first,
            "cleanupFirst": cleanup_first,
            "crossProjectSignals": cross,
            "crossProjectGraphSummary": self._ws_insights_graph_summary(run),
            "crossProjectImpactHints": self._ws_insights_impact_hints(run),
            "cautions": [
                "workspace insights are heuristic and static-only",
                "no project code was executed",
                "failed projects may hide additional risk",
                "health scores are derived from snapshot metadata only, not compiler or runtime verified",
            ],
            "generatedFrom": {
                "staticAnalysis": True,
                "scriptsExecuted": False,
                "runtimeVerified": False,
                "coverageVerified": False,
            },
        })

    def _ws_insights_graph_summary(self, run):
        """Build lightweight graph summary for insights. Best-effort; 失败不影响 insights 返回。"""
        try:
            root = run.get("root", "")
            if not root or not os.path.isdir(root):
                return {"available": False, "reason": "root not accessible"}
            graph, error = self._workspace_graph_build(run, opts={"limit": 500})
            if error or not graph:
                return {"available": False, "reason": error or "build returned empty"}
            return {
                "available": True,
                "nodeCount": graph["summary"]["nodeCount"],
                "edgeCount": graph["summary"]["edgeCount"],
                "crossProjectEdgeCount": graph["summary"]["crossProjectEdgeCount"],
                "unsupportedBoundaryCount": graph["summary"]["unsupportedBoundaryCount"],
                "topConnectedProjects": graph.get("topConnectedProjects", []),
                "bridgeScripts": graph.get("bridgeScripts", []),
                "bridgeConfigs": graph.get("bridgeConfigs", []),
            }
        except Exception as e:
            return {"available": False, "reason": str(e)}

    def _compute_project_scores(self, projects, unsupported, snap_meta):
        """Compute health score and risk level per project from snapshot metadata."""
        results = []
        for pj in projects:
            status = pj.get("status", "unknown")
            sid = pj.get("snapshotId", "")
            sm = pj.get("summary", {}) or {}
            meta = snap_meta.get(sid, {})
            sec = meta.get("secondary", {}) or {}
            score = 100
            reasons = []
            # Status-based deductions
            if status == "failed":
                score -= 35
                reasons.append("snapshot generation failed")
            elif status != "succeeded":
                score -= 20
                reasons.append(f"status is {status}, not succeeded")
            # Quality gate deductions
            qf = sec.get("qualityFailedCount", 0)
            if qf > 0:
                deduct = min(qf * 8, 50)
                score -= deduct
                reasons.append(f"{qf} quality gate(s) failed")
            # Automation risk (from snapshot secondary)
            ar = sec.get("automationRiskCount", 0) if "automationRiskCount" in sec else 0
            if ar > 0:
                deduct = min(ar * 8, 40)
                score -= deduct
                reasons.append(f"{ar} automation risk(s) detected")
            # Edge density heuristic
            source_files = sm.get("sourceFileCount", 0)
            edges = sm.get("edgeCount", 0) or sec.get("graphEdgeCount", 0)
            if source_files > 20 and (not edges or edges == 0):
                score -= 15
                reasons.append("large project with no graph edges recorded")
            elif source_files > 0 and edges > 0:
                density = edges / max(source_files, 1)
                if density < 1.0:
                    score -= 10
                    reasons.append(f"low edge density ({density:.1f} edges per source file)")
            # Missing graph / quality data
            if not edges and source_files > 0:
                score -= 10
                reasons.append("graph data unavailable for analysis")
            # Unsupported adjacent modules
            proj_path = pj.get("path", "")
            proj_name = pj.get("projectId", "")
            adjacent_unsupported = 0
            for um in unsupported:
                um_path = um.get("path", "")
                if um_path and proj_path and (
                    os.path.dirname(um_path) == os.path.dirname(proj_path) or
                    proj_path.startswith(os.path.dirname(um_path)) or
                    um_path.startswith(os.path.dirname(proj_path))
                ):
                    adjacent_unsupported += 1
            if adjacent_unsupported > 0:
                deduct = min(adjacent_unsupported * 5, 15)
                score -= deduct
                reasons.append(f"{adjacent_unsupported} unsupported module(s) nearby")
            # Clamp
            score = max(score, 0)
            # Risk level
            if score >= 85:
                risk = "low"
            elif score >= 65:
                risk = "medium"
            elif score >= 1:
                risk = "high"
            else:
                risk = "high"
            if not reasons:
                reasons.append("no significant risks detected from snapshot metadata")
            results.append({
                "projectId": pj.get("projectId", ""),
                "name": pj.get("projectId", proj_name),
                "language": pj.get("language", "auto"),
                "status": status,
                "snapshotId": sid or None,
                "healthScore": score,
                "riskLevel": risk,
                "scoreReasons": reasons,
                "metrics": {
                    "sourceFileCount": source_files,
                    "symbolCount": sm.get("symbolCount", 0),
                    "edgeCount": edges,
                    "qualityFailedCount": qf,
                    "automationRiskCount": ar,
                    "unsupportedAdjacentCount": adjacent_unsupported,
                },
            })
        return results

    def _compute_overall_score(self, project_scores):
        """Aggregate project scores into overall workspace health."""
        if not project_scores:
            return {"score": 0, "riskLevel": "unknown"}
        weights = []
        total = 0
        for ps in project_scores:
            # weight by symbol count if available
            w = max(ps.get("metrics", {}).get("symbolCount", 1), 1)
            weights.append(w)
            total += w
        if total == 0:
            avg = sum(p["healthScore"] for p in project_scores) / len(project_scores)
        else:
            avg = sum(ps["healthScore"] * w for ps, w in zip(project_scores, weights)) / total
        score = int(round(avg))
        score = max(score, 0)
        if score >= 85:
            risk = "low"
        elif score >= 65:
            risk = "medium"
        elif score >= 1:
            risk = "high"
        else:
            risk = "high"
        return {"score": score, "riskLevel": risk}

    def _compute_recommendations(self, projects, scores, unsupported):
        """Generate read first / review first / cleanup first lists."""
        score_by_id = {s["projectId"]: s for s in scores}
        # Read first: recommended projects sorted by symbol count (high density)
        succeeded = [(s["projectId"], s) for s in scores if s["status"] == "succeeded"]
        succeeded.sort(key=lambda x: -(x[1].get("metrics", {}).get("symbolCount", 0)))
        read_first = []
        for pid, s in succeeded[:5]:
            sym = s.get("metrics", {}).get("symbolCount", 0)
            read_first.append({
                "projectId": pid,
                "reason": f"recommended project with {sym} symbols — highest symbol density in workspace",
                "priority": "P0" if sym > 50 else "P1",
            })
        # Review first: failed projects or low score projects
        review_first = []
        for s in sorted(scores, key=lambda x: x["healthScore"]):
            if s["healthScore"] < 65:
                review_first.append({
                    "projectId": s["projectId"],
                    "reason": f"low health score ({s['healthScore']}) — review quality gates and automation risks",
                    "priority": "P0" if s["healthScore"] < 50 else "P1",
                })
            elif s["status"] == "failed":
                review_first.append({
                    "projectId": s["projectId"],
                    "reason": "snapshot generation failed — inspect project boundary, manifest, or language selection",
                    "priority": "P0",
                })
        # Cleanup first: large projects with low edge density or adjacent unsupported modules
        cleanup_first = []
        for s in scores:
            m = s.get("metrics", {})
            src = m.get("sourceFileCount", 0)
            edges = m.get("edgeCount", 0)
            if src > 20 and (not edges or edges == 0):
                cleanup_first.append({
                    "projectId": s["projectId"],
                    "reason": f"large project ({src} source files) with no graph edges — verify analysis coverage or consider refactoring scope",
                    "priority": "P1",
                })
            elif m.get("unsupportedAdjacentCount", 0) > 0:
                cleanup_first.append({
                    "projectId": s["projectId"],
                    "reason": f"adjacent to unsupported modules — boundary risk may need manual review",
                    "priority": "P2",
                })
        return read_first, review_first, cleanup_first

    def _compute_cross_project_signals(self, projects):
        """Extract shared script/config names and unsupported language clusters."""
        # Collect project names/paths
        project_names = [pj.get("projectId", "") for pj in projects]
        # Simulate shared script/config names from common patterns
        known_script_names = {"build.sh", "test.sh", "setup.sh", "install.sh", "deploy.sh",
                              "Makefile", "Dockerfile", "docker-compose.yml",
                              "package.json", "tsconfig.json"}
        # For now, derive from project directory structure hints in snapshot metadata
        shared_scripts = []
        shared_configs = []
        seen_labels = {}
        for pj in projects:
            pid = pj.get("projectId", "")
            name = pid.split("/")[-1] if "/" in pid else pid
            seen_labels[name] = seen_labels.get(name, 0) + 1
        # Detect duplicate project labels
        duplicated = [{"name": k, "count": v} for k, v in seen_labels.items() if v > 1]
        # Unsupported language clusters
        unsup_clusters = {}
        for pj in projects:
            path = pj.get("path", "")
            lang = pj.get("language", "")
            if lang:
                unsup_clusters.setdefault(lang, {"language": lang, "count": 0, "paths": []})
                unsup_clusters[lang]["count"] += 1
                unsup_clusters[lang]["paths"].append(path[:80])
        # Also include the actual unsupported modules from the run
        return {
            "sharedScriptNames": shared_scripts,
            "sharedConfigNames": shared_configs,
            "unsupportedLanguageClusters": [v for v in unsup_clusters.values() if v["count"] >= 1],
            "duplicatedProjectLabels": duplicated,
        }

    # ── Workspace Cross-Project Graph ────────────────────────────────

    def _ws_node_id(self, kind, rel_path):
        """Generate deterministic node ID from kind + relative path."""
        h = hashlib.md5(rel_path.encode()).hexdigest()[:8]
        return f"{kind}:{h}"

    def _ws_edge_id(self, kind, src, tgt):
        """Generate deterministic edge ID."""
        h = hashlib.md5(f"{src}|{kind}|{tgt}".encode()).hexdigest()[:8]
        return f"e:{h}"

    def _ws_read_manifest(self, path, max_bytes=65536):
        """Read a manifest/config file with size limit. Returns text or None."""
        try:
            if not os.path.isfile(path): return None
            if os.path.getsize(path) > max_bytes: return None
            with open(path, "r", errors="replace") as f:
                return f.read()
        except Exception:
            return None

    def _ws_parse_toml_deps(self, text):
        """Lightweight TOML dependency path extraction.
        只提取 [dependencies] / [dev-dependencies] / [workspace] 段中的 path = "..." 值。
        不做完整 TOML 解析；不引入外部依赖。"""
        if not text: return []
        paths = []
        in_dep_section = False
        for line in text.splitlines():
            stripped = line.strip()
            # 检查是否进入依赖段
            if stripped.startswith("["):
                section = stripped.strip("[]").strip()
                # [dependencies], [dev-dependencies], [workspace.dependencies], [build-dependencies]
                in_dep_section = ("dependencies" in section or section == "workspace")
                continue
            if not in_dep_section: continue
            # 提取 path = "..." 或 path='...'
            for pat in [r'path\s*=\s*"([^"]+)"', r"path\s*=\s*'([^']+)'"]:
                m = re.search(pat, stripped)
                if m:
                    paths.append(m.group(1))
        return paths

    def _ws_parse_json_deps(self, text):
        """Extract local path references from package.json.
        提取 dependencies/devDependencies/workspaces 中的本地路径（以 . 或 / 或 file: 开头）。"""
        if not text: return {"paths": [], "workspaces": [], "scripts": {}}
        try:
            d = json.loads(text)
        except Exception:
            return {"paths": [], "workspaces": [], "scripts": {}}
        result = {"paths": [], "workspaces": [], "scripts": {}}
        # workspaces 字段
        ws = d.get("workspaces")
        if isinstance(ws, list):
            result["workspaces"] = [w for w in ws if isinstance(w, str)]
        elif isinstance(ws, dict):
            result["workspaces"] = ws.get("packages", [])
        # dependencies 中的本地路径
        for dep_key in ["dependencies", "devDependencies", "peerDependencies"]:
            dep_obj = d.get(dep_key, {})
            if not isinstance(dep_obj, dict): continue
            for name, ver in dep_obj.items():
                ver_str = str(ver)
                # file: 协议或以 . / / 开头
                if ver_str.startswith("file:") or ver_str.startswith(".") or ver_str.startswith("/"):
                    clean = ver_str.replace("file:", "").strip()
                    result["paths"].append({"name": name, "path": clean, "section": dep_key})
        # scripts 字段（用于 script_refs 边）
        scripts = d.get("scripts", {})
        if isinstance(scripts, dict):
            result["scripts"] = scripts
        return result

    def _ws_parse_pyproject_deps(self, text):
        """Extract local path references from pyproject.toml (Python)."""
        if not text: return []
        return self._ws_parse_toml_deps(text)

    def _ws_parse_cmake_refs(self, text):
        """Extract local directory references from CMakeLists.txt."""
        if not text: return []
        paths = []
        for line in text.splitlines():
            # add_subdirectory(dir) / include(dir...) 
            for pat in [r'add_subdirectory\s*\(\s*"?([^"\s)]+)"?', r'include\s*\(\s*"?([^"\s)]+)"?']:
                m = re.search(pat, line)
                if m and not m.group(1).startswith("/"):
                    paths.append(m.group(1))
        return paths

    def _ws_parse_makefile_refs(self, text):
        """Extract local script/path references from Makefile."""
        if not text: return []
        refs = []
        for line in text.splitlines():
            # source / include / bash / sh 引用本地脚本
            for pat in [r'(?:bash|sh|source|\.)\s+([^\s;&|]+\.sh)', r'include\s+([^\s]+)']:
                m = re.search(pat, line)
                if m and not m.group(1).startswith("/"):
                    refs.append(m.group(1))
        return refs

    def _ws_parse_ci_refs(self, text):
        """Extract local path references from CI YAML (GitHub Actions etc.)."""
        if not text: return []
        refs = []
        # 简单文本搜索：找 ./ 开头的路径引用
        for line in text.splitlines():
            for m in re.finditer(r'(?:"|\')(\./[^\s"\'&,;)]+)', line):
                refs.append(m.group(1))
            for m in re.finditer(r'(?:run:.*?)(\./[^\s"\'&,;)]+)', line):
                refs.append(m.group(1))
        return refs

    def _ws_parse_dockerfile_refs(self, text):
        """Extract COPY references from Dockerfile."""
        if not text: return []
        refs = []
        for line in text.splitlines():
            m = re.search(r'COPY\s+([^\s]+)', line)
            if m:
                src = m.group(1)
                if not src.startswith("/") and src != ".":
                    refs.append(src)
        return refs

    def _ws_parse_shell_refs(self, text, max_lines=50):
        """Extract source/. references from shell scripts. 只读前 max_lines 行。"""
        if not text: return []
        refs = []
        for line in text.splitlines()[:max_lines]:
            for pat in [r'(?:^|\s)(?:source|\.)\s+([^\s;&|]+)', r'(?:bash|sh)\s+([^\s;&|]+\.sh)']:
                m = re.search(pat, line)
                if m and not m.group(1).startswith("/"):
                    refs.append(m.group(1))
        return refs

    def _ws_resolve_rel(self, base_dir, ref_path, root):
        """Resolve a reference path relative to base_dir, return absolute or None."""
        try:
            resolved = os.path.normpath(os.path.join(base_dir, ref_path))
            # 只接受 root 内的路径
            if resolved.startswith(root):
                return resolved
        except Exception:
            pass
        return None

    def _workspace_graph_build(self, run, inv=None, opts=None):
        """Build CodeLatticeWorkspaceGraphV1 from a workspace run.
        
        静态提取 workspace 内子项目间关系图。
        只读取 manifest/config/script 文件；原则上不读源码。
        """
        if opts is None: opts = {}
        root = run.get("root", "")
        if not root or not os.path.isdir(root):
            return None, "workspace root not found or not a directory"
        
        include_config_refs = opts.get("includeConfigRefs", True)
        include_script_refs = opts.get("includeScriptRefs", True)
        include_unsupported = opts.get("includeUnsupported", True)
        limit = opts.get("limit", 1000)
        
        projects = run.get("projects", [])
        unsupported = run.get("unsupportedModules", [])
        
        nodes = []
        edges = []
        node_ids = set()
        
        def _add_node(kind, label, path, language="", supported=True, project_id="", metadata=None):
            """添加节点，返回 node id。去重：同 kind+path 只添加一次。"""
            rel = os.path.relpath(path, root) if path.startswith(root) else path
            nid = self._ws_node_id(kind, rel)
            if nid in node_ids: return nid
            node_ids.add(nid)
            nodes.append({
                "id": nid, "kind": kind, "label": label,
                "path": path, "relativePath": rel,
                "language": language, "supported": supported,
                "projectId": project_id,
                "metadata": metadata or {},
            })
            return nid
        
        def _add_edge(kind, src_id, tgt_id, confidence, reason, evidence=None):
            """添加边。source/target 必须已存在于 node_ids。"""
            if src_id not in node_ids or tgt_id not in node_ids: return
            eid = self._ws_edge_id(kind, src_id, tgt_id)
            # 去重
            if any(e["id"] == eid for e in edges): return
            if len(edges) >= limit: return
            edges.append({
                "id": eid, "kind": kind,
                "source": src_id, "target": tgt_id,
                "confidence": round(confidence, 2),
                "reason": reason,
                "evidence": evidence or {},
            })
        
        # 1. Workspace 根节点
        ws_id = _add_node("workspace", os.path.basename(root) or root, root)
        
        # 2. 项目节点 + contains 边
        proj_path_to_id = {}
        for pj in projects:
            pj_path = pj.get("path", "")
            pj_rel = pj.get("projectId", pj.get("relativePath", os.path.basename(pj_path)))
            pj_lang = pj.get("language", "")
            pj_id = _add_node("project", os.path.basename(pj_path) or pj_rel, pj_path,
                              language=pj_lang, supported=True,
                              project_id=pj_rel,
                              metadata={"status": pj.get("status"), "snapshotId": pj.get("snapshotId")})
            proj_path_to_id[pj_path] = pj_id
            _add_edge("contains", ws_id, pj_id, 1.0, "workspace-inventory",
                      {"file": "workspace-run", "field": "projects", "value": pj_rel})
        
        # 3. Unsupported 模块节点 + contains + unsupported_boundary 边
        unsup_path_to_id = {}
        if include_unsupported:
            for um in unsupported:
                um_path = um.get("path", "")
                um_langs = um.get("languages", [])
                um_label = os.path.basename(um_path) or um_path
                um_id = _add_node("unsupported", um_label, um_path,
                                  language=", ".join(um_langs), supported=False,
                                  metadata={"languages": um_langs, "markers": um.get("markers", [])})
                unsup_path_to_id[um_path] = um_id
                _add_edge("contains", ws_id, um_id, 1.0, "workspace-inventory-unsupported",
                          {"file": "workspace-run", "field": "unsupportedModules"})
        
        # 4. Manifest-based depends_on 边
        # 读取每个项目的 manifest，提取 path dependency / workspace member
        for pj in projects:
            pj_path = pj.get("path", "")
            pj_lang = pj.get("language", "")
            pj_id = proj_path_to_id.get(pj_path)
            if not pj_id: continue
            
            # 根据语言决定要读的 manifest 文件
            manifests_to_read = []
            if pj_lang in ("rust",):
                manifests_to_read.append(("Cargo.toml", "toml"))
            elif pj_lang in ("typescript", "arkts"):
                manifests_to_read.append(("package.json", "json"))
                # tsconfig 可选
                if os.path.isfile(os.path.join(pj_path, "tsconfig.json")):
                    manifests_to_read.append(("tsconfig.json", "tsconfig"))
            elif pj_lang in ("python",):
                if os.path.isfile(os.path.join(pj_path, "pyproject.toml")):
                    manifests_to_read.append(("pyproject.toml", "toml"))
                if os.path.isfile(os.path.join(pj_path, "requirements.txt")):
                    manifests_to_read.append(("requirements.txt", "requirements"))
            elif pj_lang in ("c", "cpp"):
                if os.path.isfile(os.path.join(pj_path, "CMakeLists.txt")):
                    manifests_to_read.append(("CMakeLists.txt", "cmake"))
                if os.path.isfile(os.path.join(pj_path, "Makefile")):
                    manifests_to_read.append(("Makefile", "makefile"))
            elif pj_lang in ("cangjie",):
                if os.path.isfile(os.path.join(pj_path, "cjpm.toml")):
                    manifests_to_read.append(("cjpm.toml", "toml"))
            elif pj_lang in ("shell",):
                pass  # shell 没有标准 manifest，依赖 source 引用
            
            for mfile, mtype in manifests_to_read:
                mp = os.path.join(pj_path, mfile)
                text = self._ws_read_manifest(mp)
                if not text: continue
                
                # 添加 config/script/workflow 节点
                if include_config_refs and mfile not in ("Cargo.toml", "package.json", "cjpm.toml", "pyproject.toml"):
                    cfg_id = _add_node("config", mfile, mp, language=pj_lang,
                                       project_id=pj.get("projectId", ""))
                    _add_edge("contains", pj_id, cfg_id, 1.0, "project-owned-file",
                              {"file": mfile, "field": "manifest", "value": "config"})
                
                dep_paths = []
                scripts_refs = {}
                if mtype == "toml":
                    dep_paths = self._ws_parse_toml_deps(text)
                elif mtype == "json":
                    parsed = self._ws_parse_json_deps(text)
                    for dp in parsed["paths"]:
                        dep_paths.append(dp["path"])
                    # workspaces 字段 → contains 边
                    for ws_pattern in parsed.get("workspaces", []):
                        # glob pattern 如 "packages/*" → 尝试匹配实际目录
                        if "*" in ws_pattern or "?" in ws_pattern:
                            import glob as globmod
                            matches = globmod.glob(os.path.join(pj_path, ws_pattern))
                            for match in matches:
                                if os.path.isdir(match):
                                    # 检查是否对应已知项目
                                    for pp, ppid in proj_path_to_id.items():
                                        if os.path.normpath(match) == os.path.normpath(pp):
                                            _add_edge("contains", pj_id, ppid, 0.9,
                                                      "workspace-pattern-match",
                                                      {"file": mfile, "field": "workspaces", "value": ws_pattern})
                        else:
                            resolved = self._ws_resolve_rel(pj_path, ws_pattern, root)
                            if resolved and resolved in proj_path_to_id:
                                _add_edge("contains", pj_id, proj_path_to_id[resolved], 0.9,
                                          "workspace-member",
                                          {"file": mfile, "field": "workspaces", "value": ws_pattern})
                    scripts_refs = parsed.get("scripts", {})
                elif mtype == "cmake":
                    dep_paths = self._ws_parse_cmake_refs(text)
                elif mtype == "makefile":
                    dep_paths = self._ws_parse_makefile_refs(text)
                elif mtype == "tsconfig":
                    # tsconfig paths 提取
                    try:
                        ts = json.loads(text)
                        compiler_opts = ts.get("compilerOptions", {})
                        paths_map = compiler_opts.get("paths", {})
                        for alias, targets in paths_map.items():
                            for t in (targets if isinstance(targets, list) else [targets]):
                                if isinstance(t, str) and (t.startswith(".") or t.startswith("./")):
                                    dep_paths.append(t.replace("/*", "").rstrip("*"))
                    except Exception:
                        pass
                
                # 解析 dep_paths → depends_on 边
                for dp in dep_paths:
                    resolved = self._ws_resolve_rel(pj_path, dp, root)
                    if not resolved: continue
                    # 匹配已知项目
                    for pp, ppid in proj_path_to_id.items():
                        if os.path.normpath(resolved).startswith(os.path.normpath(pp)) or \
                           os.path.normpath(pp).startswith(os.path.normpath(resolved)):
                            conf = 0.85 if dp.startswith(".") or dp.startswith("/") else 0.45
                            _add_edge("depends_on", pj_id, ppid, conf,
                                      "manifest-path-dependency",
                                      {"file": mfile, "field": "path", "value": dp})
                            break
                
                # scripts → script_refs 边
                if include_script_refs and scripts_refs:
                    for script_name, script_cmd in scripts_refs.items():
                        if not isinstance(script_cmd, str): continue
                        # 检查是否引用本地文件
                        for local_ref in re.findall(r'(?:"|\')(\./[^\s"\'&,;)]+)', script_cmd):
                            resolved = self._ws_resolve_rel(pj_path, local_ref, root)
                            if resolved:
                                scr_id = _add_node("script", os.path.basename(resolved), resolved,
                                                   language="shell", project_id=pj.get("projectId", ""))
                                _add_edge("contains", pj_id, scr_id, 1.0, "project-owned-script",
                                          {"file": mfile, "field": f"scripts.{script_name}"})
                                _add_edge("script_refs", pj_id, scr_id, 0.75,
                                          "package-json-script",
                                          {"file": mfile, "field": f"scripts.{script_name}", "value": local_ref})
                        # bash/sh 引用
                        for local_ref in re.findall(r'(?:bash|sh)\s+([^\s;&|]+\.[\w]+)', script_cmd):
                            if local_ref.startswith("/"): continue
                            resolved = self._ws_resolve_rel(pj_path, local_ref, root)
                            if resolved:
                                scr_id = _add_node("script", os.path.basename(resolved), resolved,
                                                   language="shell", project_id=pj.get("projectId", ""))
                                _add_edge("script_refs", pj_id, scr_id, 0.75,
                                          "package-json-script-cmd",
                                          {"file": mfile, "field": f"scripts.{script_name}", "value": local_ref})
        
        # 5. CI/自动化工作流节点 + config_refs 边
        if include_config_refs:
            ci_dirs = [os.path.join(root, ".github", "workflows")]
            for ci_dir in ci_dirs:
                if not os.path.isdir(ci_dir): continue
                for fn in os.listdir(ci_dir)[:30]:
                    fp = os.path.join(ci_dir, fn)
                    if not os.path.isfile(fp): continue
                    text = self._ws_read_manifest(fp)
                    if not text: continue
                    wf_id = _add_node("workflow", fn, fp, language="yaml")
                    _add_edge("contains", ws_id, wf_id, 1.0, "workspace-ci-workflow",
                              {"file": fn, "field": "path"})
                    # 提取引用
                    refs = self._ws_parse_ci_refs(text)
                    for ref in refs:
                        resolved = self._ws_resolve_rel(root, ref, root)
                        if resolved:
                            # 匹配已知项目
                            for pp, ppid in proj_path_to_id.items():
                                if resolved.startswith(pp) or pp.startswith(resolved):
                                    _add_edge("config_refs", wf_id, ppid, 0.75,
                                              "ci-path-reference",
                                              {"file": fn, "field": "path", "value": ref})
                                    break
        
        # 6. Dockerfile + config_refs
        if include_config_refs:
            for df_name in ["Dockerfile", "dockerfile", "Dockerfile.dev", "Dockerfile.prod"]:
                df_path = os.path.join(root, df_name)
                if not os.path.isfile(df_path): continue
                text = self._ws_read_manifest(df_path)
                if not text: continue
                df_id = _add_node("config", df_name, df_path)
                _add_edge("contains", ws_id, df_id, 1.0, "workspace-dockerfile")
                refs = self._ws_parse_dockerfile_refs(text)
                for ref in refs:
                    resolved = self._ws_resolve_rel(root, ref, root)
                    if resolved:
                        for pp, ppid in proj_path_to_id.items():
                            if resolved.startswith(pp):
                                _add_edge("config_refs", df_id, ppid, 0.75,
                                          "dockerfile-copy",
                                          {"file": df_name, "field": "COPY", "value": ref})
                                break
        
        # 7. Makefile (workspace root level) + script_refs
        if include_script_refs:
            for mf_name in ["Makefile", "makefile", "GNUmakefile"]:
                mf_path = os.path.join(root, mf_name)
                if not os.path.isfile(mf_path): continue
                text = self._ws_read_manifest(mf_path)
                if not text: continue
                mf_id = _add_node("config", mf_name, mf_path)
                _add_edge("contains", ws_id, mf_id, 1.0, "workspace-makefile")
                refs = self._ws_parse_makefile_refs(text)
                for ref in refs:
                    resolved = self._ws_resolve_rel(root, ref, root)
                    if resolved:
                        scr_id = _add_node("script", os.path.basename(resolved), resolved, language="shell")
                        _add_edge("script_refs", mf_id, scr_id, 0.75,
                                  "makefile-script-ref",
                                  {"file": mf_name, "field": "source", "value": ref})
        
        # 8. Shell 脚本节点 + script_refs 边（workspace root level scripts）
        if include_script_refs:
            for fn in os.listdir(root)[:100]:
                fp = os.path.join(root, fn)
                if not os.path.isfile(fp): continue
                _, ext = os.path.splitext(fn)
                if ext.lower() not in (".sh", ".bash", ".zsh", ".ksh"): continue
                text = self._ws_read_manifest(fp)
                if not text: continue
                scr_id = _add_node("script", fn, fp, language="shell")
                _add_edge("contains", ws_id, scr_id, 1.0, "workspace-script")
                refs = self._ws_parse_shell_refs(text)
                for ref in refs:
                    resolved = self._ws_resolve_rel(root, ref, root)
                    if resolved:
                        ref_scr_id = _add_node("script", os.path.basename(resolved), resolved, language="shell")
                        _add_edge("script_refs", scr_id, ref_scr_id, 0.75,
                                  "shell-source",
                                  {"file": fn, "field": "source", "value": ref})
        
        # 9. adjacent_to 边（同父目录下的兄弟模块）
        if include_unsupported and unsupported:
            for pj in projects:
                pj_path = pj.get("path", "")
                pj_id = proj_path_to_id.get(pj_path)
                if not pj_id: continue
                pj_parent = os.path.dirname(pj_path)
                for um in unsupported:
                    um_path = um.get("path", "")
                    um_parent = os.path.dirname(um_path)
                    if pj_parent == um_parent and pj_path != um_path:
                        um_id = unsup_path_to_id.get(um_path)
                        if um_id:
                            _add_edge("adjacent_to", pj_id, um_id, 0.4,
                                      "sibling-module-boundary",
                                      {"field": "parent", "value": os.path.basename(pj_parent)})
                            _add_edge("unsupported_boundary", pj_id, um_id, 0.5,
                                      "unsupported-adjacent-module",
                                      {"field": "unsupported-language", "value": ", ".join(um.get("languages", []))})
        
        # 10. supported 项目之间的 adjacent_to（同父目录）
        proj_list = [(pj.get("path",""), proj_path_to_id.get(pj.get("path",""))) for pj in projects]
        for i, (p1, id1) in enumerate(proj_list):
            if not id1: continue
            for j, (p2, id2) in enumerate(proj_list):
                if j <= i or not id2: continue
                if os.path.dirname(p1) == os.path.dirname(p2) and p1 != p2:
                    _add_edge("adjacent_to", id1, id2, 0.35,
                              "sibling-projects",
                              {"field": "parent", "value": os.path.basename(os.path.dirname(p1))})
        
        # 构建 graph summary
        cross_project_edges = [e for e in edges if e["kind"] in ("depends_on", "imports", "script_refs", "config_refs")]
        auto_edges = [e for e in edges if e["kind"] in ("script_refs", "config_refs")]
        unsup_boundary_edges = [e for e in edges if e["kind"] == "unsupported_boundary"]
        langs = set()
        for n in nodes:
            if n.get("language") and n["kind"] == "project":
                langs.add(n["language"])
        
        # Top connected projects（按连接度排序）
        proj_connectivity = {}
        for e in edges:
            if e["kind"] == "contains": continue
            for nid in (e["source"], e["target"]):
                for n in nodes:
                    if n["id"] == nid and n["kind"] == "project":
                        proj_connectivity[n["id"]] = proj_connectivity.get(n["id"], 0) + 1
        top_connected = sorted(proj_connectivity.items(), key=lambda x: -x[1])[:5]
        
        # Bridge scripts/configs（连接多个项目的脚本/配置）
        bridge_items = {}
        for e in edges:
            if e["kind"] in ("script_refs", "config_refs"):
                tgt_id = e["target"]
                if tgt_id not in bridge_items:
                    bridge_items[tgt_id] = {"id": tgt_id, "refCount": 0, "kind": ""}
                bridge_items[tgt_id]["refCount"] += 1
                for n in nodes:
                    if n["id"] == tgt_id:
                        bridge_items[tgt_id]["kind"] = n["kind"]
                        bridge_items[tgt_id]["label"] = n.get("label", "")
        bridge_scripts = [b for b in bridge_items.values() if b.get("kind") == "script" and b["refCount"] > 1]
        bridge_configs = [b for b in bridge_items.values() if b.get("kind") in ("config", "workflow") and b["refCount"] > 1]
        
        graph = {
            "schemaVersion": "workspace.graph.v1",
            "workspaceId": run.get("workspaceId", ""),
            "root": root,
            "generatedAt": self._now(),
            "summary": {
                "projectCount": len([n for n in nodes if n["kind"] == "project"]),
                "supportedProjectCount": len([n for n in nodes if n["kind"] == "project" and n["supported"]]),
                "unsupportedModuleCount": len([n for n in nodes if n["kind"] == "unsupported"]),
                "nodeCount": len(nodes),
                "edgeCount": len(edges),
                "crossProjectEdgeCount": len(cross_project_edges),
                "automationEdgeCount": len(auto_edges),
                "unsupportedBoundaryCount": len(unsup_boundary_edges),
                "languageCount": len(langs),
                "truncated": len(edges) >= limit,
            },
            "nodes": nodes,
            "edges": edges,
            "clusters": [],
            "topConnectedProjects": [{"id": tid, "label": next((n["label"] for n in nodes if n["id"] == tid), ""), "connections": cnt} for tid, cnt in top_connected],
            "bridgeScripts": bridge_scripts,
            "bridgeConfigs": bridge_configs,
            "readFirst": [],
            "reviewFirst": [],
            "cautions": [
                "workspace graph is static-only and heuristic",
                "no project code was executed to discover relationships",
                "depends_on edges are based on manifest path references only",
                "adjacent_to edges are proximity hints, not dependency proof",
            ],
            "generatedFrom": {
                "staticAnalysis": True,
                "scriptsExecuted": False,
                "buildExecuted": False,
                "runtimeVerified": False,
                "coverageVerified": False,
                "heuristic": True,
            },
        }
        
        # Read First / Review First from graph
        for tid, cnt in top_connected:
            for n in nodes:
                if n["id"] == tid and n["kind"] == "project" and cnt >= 2:
                    graph["readFirst"].append({"id": n["id"], "label": n["label"], "reason": f"high connectivity ({cnt} connections)"})
        # failed projects with connections → reviewFirst
        for pj in projects:
            if pj.get("status") == "failed":
                pj_id = proj_path_to_id.get(pj.get("path", ""))
                if pj_id and proj_connectivity.get(pj_id, 0) > 0:
                    graph["reviewFirst"].append({"id": pj_id, "label": os.path.basename(pj.get("path", "")), "reason": "failed with cross-project connections"})
        # unsupported boundaries → reviewFirst
        for e in unsup_boundary_edges[:3]:
            src_label = next((n["label"] for n in nodes if n["id"] == e["source"]), "")
            tgt_label = next((n["label"] for n in nodes if n["id"] == e["target"]), "")
            graph["reviewFirst"].append({"id": e["source"], "label": src_label, "reason": f"adjacent to unsupported module: {tgt_label}"})
        
        return graph, None

    def _workspace_graph_get(self, qs):
        """GET /api/workspace/graph?runId=..."""
        params = {k: (v[0] if isinstance(v, list) and len(v) > 0 else v) for k, v in qs.items()}
        return self._workspace_graph_dispatch(params)

    def _workspace_graph_post(self, body):
        """POST /api/workspace/graph"""
        return self._workspace_graph_dispatch(body)

    def _workspace_graph_dispatch(self, params):
        """Shared graph dispatch for GET/POST."""
        run_id = params.get("runId") or params.get("workspaceRunId") or ""
        if not run_id:
            return err("runId is required", 400, "Provide runId from a workspace analyze run.")
        fp = os.path.join(self.ws_dir, f"workspace-{run_id}.json")
        run = self._load_json(fp)
        if not run:
            return err("workspace run not found", 404,
                       f"runId={run_id} not found. Use GET /api/workspace/runs to list available runs.")
        # Build graph
        opts = {
            "includeConfigRefs": params.get("includeConfigRefs", True),
            "includeScriptRefs": params.get("includeScriptRefs", True),
            "includeUnsupported": params.get("includeUnsupported", True),
            "limit": int(params.get("limit", 1000)),
        }
        graph, error = self._workspace_graph_build(run, opts=opts)
        if error:
            return err("graph build failed", 500, error)
        return ok(graph)

    # ── Cross-Project Impact Analysis ────────────────────────────────

    def _workspace_impact_get(self, qs):
        """GET /api/workspace/impact?runId=...&projectId=...&direction=both"""
        params = {k: (v[0] if isinstance(v, list) and len(v) > 0 else v) for k, v in qs.items()}
        return self._workspace_impact_dispatch(params)

    def _workspace_impact_post(self, body):
        """POST /api/workspace/impact"""
        return self._workspace_impact_dispatch(body)

    def _workspace_impact_dispatch(self, params):
        """共享 impact dispatch：验证输入、加载 run、构建 graph、resolve target、遍历、评分。"""
        run_id = params.get("runId") or params.get("workspaceRunId") or ""
        if not run_id:
            return err("runId (or workspaceRunId) is required", 400,
                       "Provide runId from a workspace analyze run.")
        fp = os.path.join(self.ws_dir, f"workspace-{run_id}.json")
        run = self._load_json(fp)
        if not run:
            return err("workspace run not found", 404,
                       f"runId={run_id} not found. Use GET /api/workspace/runs to list available runs.")

        # 方向验证
        direction = params.get("direction", "both")
        if direction not in ("downstream", "upstream", "both"):
            return err("invalid direction; must be downstream, upstream, or both", 400)

        # 构建 graph
        include_unsup = params.get("includeUnsupported", True)
        opts = {"includeConfigRefs": True, "includeScriptRefs": True,
                "includeUnsupported": include_unsup, "limit": 1000}
        graph, graph_err = self._workspace_graph_build(run, opts=opts)
        if graph_err or not graph:
            return err("graph build failed — cannot run impact analysis", 500,
                       graph_err or "empty graph")

        nodes = graph.get("nodes", [])
        edges = graph.get("edges", [])

        # Target resolution
        target_input = params.get("target") if isinstance(params.get("target"), dict) else {}
        # 扁平参数也接受
        if not target_input:
            target_input = {}
        for flat_key in ("targetNodeId", "projectId", "path", "snapshotId", "query"):
            if flat_key in params and flat_key not in target_input:
                target_input[flat_key] = params[flat_key]

        resolved = self._ws_impact_resolve_target(nodes, target_input, run)
        if not resolved.get("resolvedNodeId"):
            # 返回 structured unknown target result 而不是 error
            return ok(self._ws_impact_unknown_result(run_id, target_input, resolved, direction, graph))

        max_depth = min(int(params.get("maxDepth", 3)), 10)
        limit = min(int(params.get("limit", 100)), 500)
        result = self._ws_impact_analyze(run_id, resolved, nodes, edges,
                                          direction, max_depth, limit, include_unsup, graph)
        return ok(result)

    def _ws_impact_resolve_target(self, nodes, target_input, run):
        """Target resolution：在 graph nodes 中查找目标节点。
        优先级：exact nodeId > projectId > snapshotId > exact path > suffix path > label > fuzzy > unknown"""
        candidates = []

        # 1. Exact node ID
        nid = target_input.get("targetNodeId", "")
        if nid:
            for n in nodes:
                if n["id"] == nid:
                    return {"resolvedNodeId": n["id"], "resolvedKind": n["kind"],
                            "label": n.get("label", ""), "path": n.get("path", ""),
                            "language": n.get("language", ""),
                            "resolutionConfidence": 1.0,
                            "resolutionReason": "exact-node-id"}

        # 2. Exact projectId
        pid = target_input.get("projectId", "")
        if pid:
            for n in nodes:
                if n.get("projectId") == pid or n.get("projectId") == pid:
                    candidates.append((n, 1.0, "exact-project-id"))
                elif n.get("projectId", "").endswith("/" + pid):
                    candidates.append((n, 0.95, "project-id-suffix"))

        # 3. Exact snapshotId → project node
        sid = target_input.get("snapshotId", "")
        if sid:
            for n in nodes:
                if n.get("metadata", {}).get("snapshotId") == sid:
                    candidates.append((n, 0.95, "exact-snapshot-id"))

        # 4. Path match
        path_query = target_input.get("path", "")
        if path_query:
            for n in nodes:
                np = n.get("path", "") or n.get("relativePath", "")
                if np == path_query:
                    candidates.append((n, 0.90, "exact-path"))
                elif np.endswith(path_query) or path_query.endswith(np.split("/")[-1] if "/" in np else np):
                    candidates.append((n, 0.75, "suffix-path"))

        # 5. Label match
        label_query = target_input.get("query", "") or target_input.get("label", "") or pid
        if label_query and not candidates:
            for n in nodes:
                nl = n.get("label", "").lower()
                nq = label_query.lower()
                if nl == nq:
                    candidates.append((n, 0.65, "exact-label"))
                elif nq in nl or nl in nq:
                    candidates.append((n, 0.45, "fuzzy-contains"))

        # 排序取最高 confidence
        if candidates:
            candidates.sort(key=lambda x: -x[1])
            # 如果并列，降级
            top_conf = candidates[0][1]
            top_ties = [c for c in candidates if c[1] == top_conf]
            n, conf, reason = top_ties[0]
            caution = ""
            if len(top_ties) > 1:
                conf = max(conf - 0.1, 0.1)
                caution = f"ambiguous target: {len(top_ties)} candidates with confidence {top_conf}"
            return {"resolvedNodeId": n["id"], "resolvedKind": n["kind"],
                    "label": n.get("label", ""), "path": n.get("path", ""),
                    "language": n.get("language", ""),
                    "resolutionConfidence": round(conf, 2),
                    "resolutionReason": reason,
                    "caution": caution,
                    "resolutionCandidates": [{"id": c[0]["id"], "label": c[0].get("label", ""),
                                              "confidence": c[1], "reason": c[2]} for c in candidates[:5]]}

        # 没有找到
        return {"resolvedNodeId": None, "resolvedKind": "unknown",
                "label": label_query or path_query or nid or "",
                "path": "", "language": "",
                "resolutionConfidence": 0.0,
                "resolutionReason": "no-match"}

    def _ws_impact_unknown_result(self, run_id, target_input, resolved, direction, graph):
        """target resolution 失败时返回的 structured unknown result。"""
        return {
            "schemaVersion": "workspace.impact.v1",
            "workspaceRunId": run_id,
            "target": {
                "input": json.dumps(target_input) if isinstance(target_input, dict) else str(target_input),
                "resolvedNodeId": None, "resolvedKind": "unknown",
                "label": resolved.get("label", ""), "path": "",
                "language": "", "resolutionConfidence": 0.0,
                "resolutionReason": resolved.get("resolutionReason", "no-match"),
                "resolutionCandidates": resolved.get("resolutionCandidates", []),
            },
            "summary": {"direction": direction, "affectedProjectCount": 0,
                        "affectedConfigCount": 0, "affectedScriptCount": 0,
                        "affectedWorkflowCount": 0, "unsupportedBoundaryCount": 0,
                        "maxDepth": 0, "edgeCountConsidered": 0,
                        "riskLevel": "unknown", "confidence": "unknown"},
            "affectedProjects": [], "affectedConfigs": [],
            "affectedScripts": [], "affectedWorkflows": [],
            "unsupportedBoundaries": [], "paths": [],
            "riskReasons": ["target could not be resolved — no impact analysis possible"],
            "reviewChecklist": ["verify target identifier is correct",
                                "check if target exists in workspace graph"],
            "cautions": ["target resolution failed", "impact analysis could not be performed",
                         "all results are static-only and heuristic"],
            "generatedFrom": {"workspaceGraph": True, "staticAnalysis": True,
                              "scriptsExecuted": False, "buildExecuted": False,
                              "runtimeVerified": False, "coverageVerified": False,
                              "heuristic": True},
        }

    def _ws_impact_analyze(self, run_id, resolved, nodes, edges,
                            direction, max_depth, limit, include_unsup, graph):
        """核心影响分析：BFS 遍历 graph，计算受影响节点、路径、风险。"""
        target_id = resolved["resolvedNodeId"]
        node_by_id = {n["id"]: n for n in nodes}

        # 构建邻接表
        # downstream: target → outgoing edges → targets affected by target change
        # upstream: incoming edges → target → sources that depend on target
        outgoing = {}  # node_id -> [(edge, target_id)]
        incoming = {}  # node_id -> [(edge, source_id)]
        for e in edges:
            s, t = e["source"], e["target"]
            outgoing.setdefault(s, []).append((e, t))
            incoming.setdefault(t, []).append((e, s))

        affected_nodes = {}  # node_id -> {distance, paths, confidences, directions}
        all_paths = []
        cautions = list(graph.get("cautions", []))
        edge_count = 0

        def _bfs(start_id, edge_map, dir_label, follow_contains=True):
            """BFS 遍历。follow_controls 控制 contains 边是否扩散。"""
            nonlocal edge_count
            visited = {start_id}
            queue = [(start_id, 0, [])]  # (node_id, distance, path_edges)
            while queue and edge_count < limit:
                nid, dist, path_e = queue.pop(0)
                if dist >= max_depth:
                    continue
                for e, neighbor_id in edge_map.get(nid, []):
                    if neighbor_id in visited:
                        continue
                    if neighbor_id not in node_by_id:
                        # dangling edge — 记录 caution
                        cautions.append(f"dangling edge detected: {e.get('id', '?')}")
                        continue
                    edge_count += 1
                    if edge_count > limit:
                        break

                    kind = e.get("kind", "")
                    conf = e.get("confidence", 0.5)

                    # contains 边策略：workspace→project 可以扩散，project→config/script 不太扩散
                    if kind == "contains" and not follow_contains:
                        nn = node_by_id.get(neighbor_id, {})
                        # 只允许 workspace→project 的 contains 扩散
                        if node_by_id.get(nid, {}).get("kind") != "workspace":
                            continue

                    # adjacent_to 和 unsupported_boundary 只记录，不作为强影响
                    is_weak = kind in ("adjacent_to", "unsupported_boundary")
                    path_conf = min([c for _, c in path_e] + [conf]) if path_e else conf
                    if is_weak:
                        path_conf = min(path_conf, 0.4)  # 降级

                    new_path = path_e + [(e.get("id", ""), conf)]
                    visited.add(neighbor_id)

                    if neighbor_id not in affected_nodes:
                        affected_nodes[neighbor_id] = {
                            "distance": dist + 1, "confidences": [path_conf],
                            "directions": [dir_label], "via": [e.get("id", "")]
                        }
                    else:
                        an = affected_nodes[neighbor_id]
                        an["distance"] = min(an["distance"], dist + 1)
                        an["confidences"].append(path_conf)
                        if dir_label not in an["directions"]:
                            an["directions"].append(dir_label)
                        an["via"].append(e.get("id", ""))

                    # 记录 path（限制总数）
                    if len(all_paths) < limit:
                        nn = node_by_id[neighbor_id]
                        all_paths.append({
                            "from": target_id, "to": neighbor_id,
                            "direction": dir_label, "distance": dist + 1,
                            "edges": [{"kind": kind, "source": e["source"],
                                       "target": e["target"],
                                       "confidence": round(conf, 2),
                                       "reason": e.get("reason", "")}
                                      for e_item in [e]]
                        })

                    # 弱边不继续扩散
                    if not is_weak:
                        queue.append((neighbor_id, dist + 1, new_path))

        # 根据方向运行 BFS
        if direction in ("downstream", "both"):
            _bfs(target_id, outgoing, "downstream", follow_contains=True)
        if direction in ("upstream", "both"):
            _bfs(target_id, incoming, "upstream", follow_contains=True)

        # 分类受影响节点
        affected_projects = []
        affected_configs = []
        affected_scripts = []
        affected_workflows = []
        unsupported_boundaries = []

        for nid, info in affected_nodes.items():
            nn = node_by_id.get(nid)
            if not nn:
                continue
            best_conf = round(max(info["confidences"]), 2)
            entry = {
                "id": nid, "label": nn.get("label", ""), "kind": nn.get("kind", ""),
                "path": nn.get("path", ""), "relativePath": nn.get("relativePath", ""),
                "language": nn.get("language", ""),
                "distance": info["distance"],
                "directions": info["directions"],
                "via": info["via"][:5],
                "confidence": best_conf,
                "reasons": self._ws_edge_reasons(nid, edges, node_by_id),
            }
            kind = nn.get("kind", "")
            if kind == "project":
                entry["projectId"] = nn.get("projectId", "")
                affected_projects.append(entry)
            elif kind == "config":
                affected_configs.append(entry)
            elif kind == "script":
                affected_scripts.append(entry)
            elif kind == "workflow":
                affected_workflows.append(entry)
            elif kind == "unsupported":
                unsupported_boundaries.append(entry)

        # unsupported_boundary edge 相关的 supported 项目也加入
        if include_unsup:
            for e in edges:
                if e["kind"] == "unsupported_boundary" and e["source"] in node_by_id:
                    src = node_by_id[e["source"]]
                    # 如果 source 是受影响项目，记录 unsupported boundary
                    if src["id"] in affected_nodes and src["kind"] == "project":
                        if not any(b["id"] == src["id"] for b in unsupported_boundaries):
                            unsupported_boundaries.append({
                                "id": src["id"], "label": src.get("label", ""),
                                "kind": "project-boundary",
                                "path": src.get("path", ""),
                                "distance": affected_nodes[src["id"]]["distance"],
                                "directions": affected_nodes[src["id"]]["directions"],
                                "confidence": round(max(affected_nodes[src["id"]]["confidences"]), 2),
                                "reasons": ["adjacent to unsupported module"],
                            })

        # 排序：distance 升序，confidence 降序
        affected_projects.sort(key=lambda x: (x["distance"], -x["confidence"]))
        affected_configs.sort(key=lambda x: (x["distance"], -x["confidence"]))
        affected_scripts.sort(key=lambda x: (x["distance"], -x["confidence"]))
        affected_workflows.sort(key=lambda x: (x["distance"], -x["confidence"]))

        # Risk scoring
        proj_count = len(affected_projects)
        cfg_count = len(affected_configs)
        scr_count = len(affected_scripts)
        wf_count = len(affected_workflows)
        unsup_count = len(unsupported_boundaries)

        risk_level = self._ws_impact_risk_level(proj_count, cfg_count, scr_count,
                                                  wf_count, unsup_count, affected_nodes)
        risk_reasons = self._ws_impact_risk_reasons(proj_count, cfg_count, scr_count,
                                                      wf_count, unsup_count,
                                                      affected_projects, resolved)
        review_checklist = self._ws_impact_review_checklist(affected_projects, affected_configs,
                                                              affected_scripts, affected_workflows,
                                                              unsupported_boundaries)

        # Confidence summary
        all_confs = []
        for info in affected_nodes.values():
            all_confs.extend(info["confidences"])
        if all_confs:
            avg_conf = sum(all_confs) / len(all_confs)
            if avg_conf >= 0.75:
                conf_summary = "high"
            elif avg_conf >= 0.5:
                conf_summary = "medium"
            else:
                conf_summary = "low"
        else:
            conf_summary = "unknown"

        # Ambiguous target caution
        if resolved.get("caution"):
            cautions.append(resolved["caution"])

        max_dist = max((info["distance"] for info in affected_nodes.values()), default=0)

        return {
            "schemaVersion": "workspace.impact.v1",
            "workspaceRunId": run_id,
            "target": {
                "input": resolved.get("resolutionReason", ""),
                "resolvedNodeId": target_id,
                "resolvedKind": resolved.get("resolvedKind", ""),
                "label": resolved.get("label", ""),
                "path": resolved.get("path", ""),
                "language": resolved.get("language", ""),
                "resolutionConfidence": resolved.get("resolutionConfidence", 0.0),
                "resolutionReason": resolved.get("resolutionReason", ""),
            },
            "summary": {
                "direction": direction,
                "affectedProjectCount": proj_count,
                "affectedConfigCount": cfg_count,
                "affectedScriptCount": scr_count,
                "affectedWorkflowCount": wf_count,
                "unsupportedBoundaryCount": unsup_count,
                "maxDepth": max_dist,
                "edgeCountConsidered": edge_count,
                "riskLevel": risk_level,
                "confidence": conf_summary,
            },
            "affectedProjects": affected_projects[:limit],
            "affectedConfigs": affected_configs[:limit],
            "affectedScripts": affected_scripts[:limit],
            "affectedWorkflows": affected_workflows[:limit],
            "unsupportedBoundaries": unsupported_boundaries[:limit],
            "paths": all_paths[:limit],
            "riskReasons": risk_reasons,
            "reviewChecklist": review_checklist,
            "cautions": cautions + [
                "cross-project impact is static-only and heuristic",
                "no project code was executed to verify impact",
                "impact paths are based on graph traversal, not runtime analysis",
            ],
            "generatedFrom": {
                "workspaceGraph": True,
                "staticAnalysis": True,
                "scriptsExecuted": False,
                "buildExecuted": False,
                "runtimeVerified": False,
                "coverageVerified": False,
                "heuristic": True,
            },
        }

    def _ws_edge_reasons(self, nid, edges, node_by_id):
        """收集指向/来自某节点的边的原因描述。"""
        reasons = []
        for e in edges:
            if e["source"] == nid or e["target"] == nid:
                kind = e.get("kind", "")
                reason = e.get("reason", "")
                if kind != "contains":
                    other = e["target"] if e["source"] == nid else e["source"]
                    other_n = node_by_id.get(other, {})
                    other_label = other_n.get("label", other)
                    reasons.append(f"{kind} → {other_label} ({reason})")
        return reasons[:5]

    def _ws_impact_risk_level(self, proj_count, cfg_count, scr_count,
                               wf_count, unsup_count, affected_nodes):
        """Risk level 评分。"""
        if proj_count >= 8:
            return "critical"
        if wf_count >= 3 and (scr_count > 0 or cfg_count > 0):
            return "critical"
        if proj_count >= 4:
            return "high"
        if wf_count > 0:
            return "high"
        if unsup_count >= 3:
            return "high"
        if proj_count >= 2:
            return "medium"
        if cfg_count > 0 or scr_count > 0:
            return "medium"
        if unsup_count > 0:
            return "medium"
        if proj_count <= 1:
            return "low"
        return "medium"

    def _ws_impact_risk_reasons(self, proj_count, cfg_count, scr_count,
                                 wf_count, unsup_count, affected_projects, resolved):
        """生成可读的 risk reasons。"""
        reasons = []
        target_label = resolved.get("label", "target")
        if proj_count > 0:
            reasons.append(f"{target_label} may affect {proj_count} project(s)")
        if wf_count > 0:
            reasons.append(f"{wf_count} workflow(s) may be impacted — manual CI review recommended")
        if cfg_count > 0 or scr_count > 0:
            reasons.append(f"{cfg_count} config(s) and {scr_count} script(s) may require review")
        if unsup_count > 0:
            reasons.append(f"impact crosses {unsup_count} unsupported boundary/boundaries")
        # 高连接度项目
        for p in affected_projects[:3]:
            if p.get("confidence", 0) >= 0.75:
                reasons.append(f"high-confidence path to {p.get('label', '?')}")
        if not reasons:
            reasons.append("limited cross-project impact detected")
        return reasons

    def _ws_impact_review_checklist(self, projects, configs, scripts,
                                     workflows, boundaries):
        """生成 review checklist。"""
        items = []
        for p in projects[:3]:
            items.append(f"review changes in {p.get('label', '?')} (distance: {p.get('distance', '?')})")
        for c in configs[:2]:
            items.append(f"check config {c.get('label', '?')} for compatibility")
        for s in scripts[:2]:
            items.append(f"verify script {s.get('label', '?')} still works")
        for w in workflows[:2]:
            items.append(f"update workflow {w.get('label', '?')} if paths changed")
        for b in boundaries[:2]:
            items.append(f"verify unsupported module boundary for {b.get('label', '?')}")
        items.append("run project tests/builds outside CodeLattice")
        items.append("verify no runtime regressions via manual or CI testing")
        return items

    # ── Insights Impact Hints ────────────────────────────────────────

    def _ws_insights_impact_hints(self, run, graph_summary=None):
        """从 graph 提取 impact hints，加入 insights。Best-effort。"""
        try:
            # 使用已有的 graph 或轻量构建
            if graph_summary and graph_summary.get("available"):
                pass  # graph summary 已存在
            root = run.get("root", "")
            if not root or not os.path.isdir(root):
                return {"available": False, "reason": "root not accessible"}

            graph, error = self._workspace_graph_build(run, opts={"limit": 500})
            if error or not graph:
                return {"available": False, "reason": error or "empty graph"}

            nodes = graph.get("nodes", [])
            edges = graph.get("edges", [])

            # 高 fanout 项目
            proj_fanout = {}
            for n in nodes:
                if n["kind"] == "project":
                    out_count = sum(1 for e in edges if e["source"] == n["id"] and e["kind"] != "contains")
                    in_count = sum(1 for e in edges if e["target"] == n["id"] and e["kind"] != "contains")
                    proj_fanout[n["id"]] = {"label": n.get("label", ""),
                                             "language": n.get("language", ""),
                                             "projectId": n.get("projectId", ""),
                                             "outgoing": out_count, "incoming": in_count,
                                             "total": out_count + in_count}

            high_fanout = sorted(proj_fanout.values(), key=lambda x: -x["total"])[:5]
            high_fanout = [p for p in high_fanout if p["total"] >= 2]

            # Shared scripts（被多个项目引用的脚本）
            shared_scripts = []
            for n in nodes:
                if n["kind"] == "script":
                    refs = [e for e in edges if e["target"] == n["id"] and e["kind"] == "script_refs"]
                    if len(refs) >= 1:
                        shared_scripts.append({"id": n["id"], "label": n.get("label", ""),
                                               "refCount": len(refs)})
            shared_scripts.sort(key=lambda x: -x["refCount"])
            shared_scripts = shared_scripts[:5]

            # Shared configs
            shared_configs = []
            for n in nodes:
                if n["kind"] in ("config", "workflow"):
                    refs = [e for e in edges if e["target"] == n["id"] and e["kind"] in ("config_refs", "script_refs")]
                    if len(refs) >= 1:
                        shared_configs.append({"id": n["id"], "label": n.get("label", ""),
                                               "kind": n["kind"], "refCount": len(refs)})
            shared_configs.sort(key=lambda x: -x["refCount"])
            shared_configs = shared_configs[:5]

            # Unsupported boundary projects
            unsup_boundary_projects = []
            for e in edges:
                if e["kind"] == "unsupported_boundary":
                    src = next((n for n in nodes if n["id"] == e["source"]), None)
                    tgt = next((n for n in nodes if n["id"] == e["target"]), None)
                    if src:
                        unsup_boundary_projects.append({"id": src["id"], "label": src.get("label", ""),
                                                         "language": src.get("language", "")})
            unsup_boundary_projects = unsup_boundary_projects[:5]

            # Suggested impact targets（高 fanout + shared）
            suggested = []
            for p in high_fanout[:3]:
                if not any(s["id"] == p["id"] for s in suggested):
                    suggested.append({"id": p["id"], "label": p["label"],
                                      "reason": f"high connectivity ({p['total']} connections)"})
            for s in shared_scripts[:2]:
                if not any(x["id"] == s["id"] for x in suggested):
                    suggested.append({"id": s["id"], "label": s["label"],
                                      "reason": f"shared script ({s['refCount']} references)"})

            return {
                "available": True,
                "highFanoutProjects": high_fanout,
                "sharedScripts": shared_scripts,
                "sharedConfigs": shared_configs,
                "unsupportedBoundaryProjects": unsup_boundary_projects,
                "suggestedImpactTargets": suggested[:5],
            }
        except Exception as e:
            return {"available": False, "reason": str(e)}

    def _workspace_insights_get(self, qs):
        """GET /api/workspace/insights?runId=..."""
        # parse_qs returns lists; normalize to single values
        params = {k: (v[0] if isinstance(v, list) and len(v) > 0 else v) for k, v in qs.items()}
        return self._workspace_insights(params)

    def _workspace_insights_post(self, body):
        """POST /api/workspace/insights"""
        return self._workspace_insights(body)
        if lang != "auto":
            return root, lang, None
        inv = self._project_inventory_data(root)
        status = inv.get("status")
        if status == "single_candidate" and inv.get("recommendedRoot"):
            return inv["recommendedRoot"], inv.get("recommendedLanguage") or "auto", None
        if status in {"multi_project", "unsupported_only", "empty"}:
            return root, lang, err("project selection required", 400, self._inventory_hint(inv))
        return root, lang, None
    def _generation_error_hint(self, root, detail):
        candidates = self._project_candidates(root)
        hint = (detail or "").strip()
        if candidates:
            lines = ["请选择具体项目目录，或显式选择语言。候选子项目："]
            lines += [f"- {c['path']} ({', '.join(c['languages'])})" for c in candidates]
            hint = (hint + "\n\n" if hint else "") + "\n".join(lines)
        return hint[:1200]

    def do_OPTIONS(self):
        self.send_response(204); self.send_header("Access-Control-Allow-Origin","*")
        self.send_header("Access-Control-Allow-Methods","GET,POST,PUT,DELETE,OPTIONS")
        self.send_header("Access-Control-Allow-Headers","Content-Type"); self.end_headers()

    # ── Route dispatch ────────────────────────────────────────────

    def do_GET(self):
        p = urllib.parse.urlparse(self.path)
        if p.path=="/api/health": return self._r(self._health)
        if p.path=="/api/profiles": return self._r(self._list_profiles)
        if p.path=="/api/snapshots": return self._r(lambda:self._list_snaps(urllib.parse.parse_qs(p.query)))
        if p.path.startswith("/api/profile/"):
            pid=p.path.split("/api/profile/",1)[1]
            return self._r(lambda:self._get_profile(pid))
        if p.path.startswith("/api/snapshot/"):
            sid=p.path.split("/api/snapshot/",1)[1]
            return self._r(lambda:self._get_snap(sid))
        if p.path=="/api/rebuild-index": return self._r(self._rebuild_index)
        if p.path=="/api/project/inventory": return self._r(lambda:self._project_inventory(urllib.parse.parse_qs(p.query)))
        if p.path=="/api/mcp/status": return self._r(self._mcp_status)
        if p.path=="/api/mcp/tools": return self._r(self._mcp_tools_api)
        if p.path=="/api/mcp/jobs": return self._r(lambda:self._list_jobs(urllib.parse.parse_qs(p.query)))
        if p.path.startswith("/api/mcp/job/"):
            jid=p.path.split("/api/mcp/job/",1)[1]
            return self._r(lambda:self._get_job(jid))
        if p.path=="/api/fs/roots": return self._r(self._fs_roots)
        if p.path=="/api/fs/list": return self._r(lambda:self._fs_list(urllib.parse.parse_qs(p.query)))
        if p.path=="/api/fs/validate-root": return self._r(lambda:self._fs_validate(urllib.parse.parse_qs(p.query)))
        if p.path=="/api/workspace/inventory": return self._r(lambda:self._workspace_inventory(urllib.parse.parse_qs(p.query)))
        if p.path=="/api/workspace/runs": return self._r(self._workspace_runs)
        if p.path.startswith("/api/workspace/run/"):
            wid = p.path.split("/api/workspace/run/", 1)[1]
            return self._r(lambda: self._workspace_run_get(wid))
        if p.path == "/api/workspace/graph":
            return self._r(lambda: self._workspace_graph_get(urllib.parse.parse_qs(p.query)))
        if p.path == "/api/workspace/impact":
            return self._r(lambda: self._workspace_impact_get(urllib.parse.parse_qs(p.query)))
        if p.path == "/api/workspace/insights":
            return self._r(lambda: self._workspace_insights_get(urllib.parse.parse_qs(p.query)))
        return super().do_GET()

    def do_POST(self):
        p = urllib.parse.urlparse(self.path)
        if p.path=="/api/profiles": return self._r(lambda:self._create_profile(self._rb()))
        if p.path=="/api/generate-snapshot": return self._r(lambda:self._generate(self._rb()))
        if p.path=="/api/rebuild-index": return self._r(self._rebuild_index)
        if p.path=="/api/mcp/jobs": return self._r(lambda:self._create_job(self._rb()))
        if p.path=="/api/quick-analyze": return self._r(self._quick_analyze)
        if p.path=="/api/fs/pick-directory": return self._r(self._fs_pick_directory)
        if p.path=="/api/workspace/analyze": return self._r(lambda:self._workspace_analyze(self._rb()))
        if p.path=="/api/workspace/graph": return self._r(lambda:self._workspace_graph_post(self._rb()))
        if p.path=="/api/workspace/impact": return self._r(lambda:self._workspace_impact_post(self._rb()))
        if p.path=="/api/workspace/insights": return self._r(lambda:self._workspace_insights_post(self._rb()))
        if p.path.startswith("/api/mcp/job/") and p.path.endswith("/cancel"):
            jid=p.path.split("/api/mcp/job/",1)[1].split("/",1)[0]
            return self._r(lambda:self._cancel_job(jid))
        if p.path.startswith("/api/profile/") and p.path.endswith("/generate-snapshot"):
            pid=p.path.split("/api/profile/",1)[1].split("/",1)[0]
            return self._r(lambda:self._gen_for_profile(pid))
        return self._resp(err("not found",404),404)

    def do_PUT(self):
        p = urllib.parse.urlparse(self.path)
        if p.path.startswith("/api/profile/"):
            pid=p.path.split("/api/profile/",1)[1]
            return self._r(lambda:self._update_profile(pid,self._rb()))
        return self._resp(err("not found",404),404)

    def do_DELETE(self):
        p = urllib.parse.urlparse(self.path)
        if p.path.startswith("/api/snapshot/"):
            sid=p.path.split("/api/snapshot/",1)[1]
            return self._r(lambda:self._delete_snap(sid))
        if p.path.startswith("/api/profile/"):
            pid=p.path.split("/api/profile/",1)[1]
            return self._r(lambda:self._delete_profile(pid))
        if p.path.startswith("/api/mcp/job/"):
            jid=p.path.split("/api/mcp/job/",1)[1]
            return self._r(lambda:self._delete_job(jid))
        return self._resp(err("not found",404),404)

    def _r(self, fn):
        try: d=fn(); self._resp(d, d.get("status",200) if isinstance(d,dict) else 200)
        except Exception as e: self._resp(err(str(e),500),500)

    # ── Health ────────────────────────────────────────────────────

    def _health(self):
        data = {"status":"ok","mode":"runner","version":"phase-e","repoRoot":str(REPO_ROOT),
                "snapshotDir":self.sn_dir,"supportedLanguages":SUPPORTED,"staticOnly":True}
        return ok(data)

    # ── Profiles ──────────────────────────────────────────────────

    def _load_profiles(self):
        d = self._load_json(self.pf_file)
        return d if isinstance(d,list) else []

    def _save_profiles(self, lst):
        self._save_json(self.pf_file, lst)

    def _list_profiles(self):
        return ok(self._load_profiles())

    def _create_profile(self, body):
        name = (body.get("name") or "").strip()
        root = (body.get("root") or "").strip()
        if not name: return err("name is required",400)
        if not root: return err("root is required",400)
        if not os.path.isdir(root): return err("root not found or not a directory",400,f"path: {root}")
        lang = body.get("language","auto").strip()
        if lang not in SUPPORTED: return err(f"unsupported language: {lang}",400,f"supported: {','.join(SUPPORTED)}")
        pf = {"id":self._nid(),"name":name,"root":root,"rootLabel":os.path.basename(root) or root,
              "language":lang,"createdAt":self._now(),"updatedAt":self._now(),
              "lastSnapshotId":None,"snapshotCount":0,"notes":(body.get("notes") or "").strip()}
        lst = self._load_profiles(); lst.append(pf); self._save_profiles(lst)
        return ok(pf)

    def _get_profile(self, pid):
        lst=self._load_profiles(); pf=next((p for p in lst if p["id"]==pid),None)
        if not pf: return err("profile not found",404)
        return ok(pf)

    def _update_profile(self, pid, body):
        lst=self._load_profiles(); pf=next((p for p in lst if p["id"]==pid),None)
        if not pf: return err("profile not found",404)
        for k in ["name","language","notes"]:
            if k in body and body[k] is not None: pf[k] = str(body[k]).strip()
        if "root" in body and body["root"]:
            r=str(body["root"]).strip()
            if not os.path.isdir(r): return err("root not found",400)
            pf["root"]=r; pf["rootLabel"]=os.path.basename(r) or r
        if pf.get("language","") not in SUPPORTED: pf["language"]="auto"
        pf["updatedAt"]=self._now(); self._save_profiles(lst)
        return ok(pf)

    def _delete_profile(self, pid):
        lst=self._load_profiles(); lst=[p for p in lst if p["id"]!=pid]; self._save_profiles(lst)
        return ok({"deleted":pid})

    # ── Snapshots ─────────────────────────────────────────────────

    def _load_index(self):
        ip = os.path.join(self.sn_dir,"index.json")
        if not os.path.isfile(ip): return []
        try:
            with open(ip) as f: d=json.load(f)
            return d if isinstance(d,list) else []
        except: return []

    def _save_index(self, idx):
        self._ensure(self.sn_dir); self._save_json(os.path.join(self.sn_dir,"index.json"),idx)

    def _snap_meta(self, entry):
        fp = os.path.join(self.sn_dir,entry.get("filename",""))
        sm = {}; sec = {}
        if os.path.isfile(fp):
            try:
                with open(fp) as f: d=json.load(f)
                s=d.get("summary",{}); g=d.get("graph",{}).get("summary",{}) if d.get("graph") else {}
                q=d.get("quality",{})
                sm={"sourceFileCount":s.get("sourceFileCount",0),"symbolCount":s.get("symbolCount",0),
                    "edgeCount":s.get("edgeCount",0),"nodeCount":s.get("nodeCount",0)}
                sec={"graphNodeCount":g.get("nodeCount",0),"graphEdgeCount":g.get("edgeCount",0),
                     "qualityFailedCount":q.get("failedGateCount",0),
                     "limitationsCount":len(d.get("limitations",{}).get("notes",[])) if isinstance(d.get("limitations"),dict) else len(d.get("limitations",[]))}
            except: pass
        return {**entry,"summary":sm,"secondary":sec}

    def _list_snaps(self, qp):
        idx = self._load_index()
        entries = [self._snap_meta(e) for e in idx]
        # Filters
        lang = qp.get("language",[None])[0]
        pid = qp.get("profileId",[None])[0]
        q = (qp.get("q",[""])[0] or "").lower()
        if lang: entries=[e for e in entries if e.get("language","")==lang]
        if pid: entries=[e for e in entries if e.get("profileId","")==pid]
        if q: entries=[e for e in entries if q in str(e.get("rootLabel","")).lower() or q in str(e.get("id","")).lower() or q in str(e.get("profileName","")).lower()]
        # Sort
        sk = qp.get("sort",["createdAt"])[0]
        rev = qp.get("order",["desc"])[0]=="asc"
        key_map = {"createdAt":lambda e:e.get("createdAt",""),"language":lambda e:e.get("language",""),
                   "symbolCount":lambda e:e.get("summary",{}).get("symbolCount",0),
                   "sourceFileCount":lambda e:e.get("summary",{}).get("sourceFileCount",0)}
        entries.sort(key=key_map.get(sk,lambda e:e.get("createdAt","")),reverse=not rev)
        return ok(entries)

    def _get_snap(self, sid):
        self._ensure(self.sn_dir)
        if not self._safe_id(sid): return err("invalid snapshot id",400)
        idx=self._load_index(); e=next((x for x in idx if x["id"]==sid),None)
        if not e: return err("snapshot not found",404)
        fp=os.path.join(self.sn_dir,e["filename"]); d=self._load_json(fp)
        if not d: return err("snapshot file corrupt",500)
        return ok(d)

    def _attach_automation_graph(self, snapshot, root, lang, redact_root=True):
        """Best-effort enrichment: snapshot generation must not fail if Live MCP is unavailable."""
        try:
            if lang == "auto":
                lang = snapshot.get("summary",{}).get("language","auto") or "auto"
            if not self._probe_mcp():
                snapshot["automationGraph"] = {"status":"not_collected","reason":"mcp_unavailable","staticOnly":True}
                return snapshot
            ag = self._call_mcp_tool("codelattice_automation_graph",
                {"root":root,"language":lang,"compact":False,"limit":80}, timeout=60)
            if redact_root and isinstance(ag, dict):
                raw = json.dumps(ag, ensure_ascii=False)
                raw = raw.replace(root, "<project-root>")
                raw = raw.replace(os.path.dirname(root), "<parent-dir>")
                ag = json.loads(raw)
            snapshot["automationGraph"] = ag if isinstance(ag, dict) else {"status":"not_collected","text":str(ag),"staticOnly":True}
        except Exception as exc:
            snapshot["automationGraph"] = {"status":"not_collected","reason":"automation_graph_failed","hint":str(exc),"staticOnly":True}
        return snapshot

    def _generate(self, body):
        root=(body.get("root") or "").strip()
        if not root: return err("root is required",400)
        if not os.path.isdir(root): return err("root directory not found",400,f"path: {root}")
        lang=body.get("language","auto").strip()
        if lang not in SUPPORTED: return err(f"unsupported language: {lang}",400)
        root, lang, blocker = self._prepare_analysis_target(root, lang)
        if blocker: return blocker
        df=body.get("full",True); rd=body.get("redactRoot",True)
        sid=self._nid(); fn=f"snapshot-{sid}.json"; op=os.path.join(self.sn_dir,fn)
        self._ensure(self.sn_dir)
        cmd=["bash",str(SNAP_SCRIPT),"--root",root,"--language",lang,"--output",op]
        if df: cmd.append("--full")
        if rd: cmd.append("--redact-root")
        try:
            r=subprocess.run(cmd,capture_output=True,text=True,timeout=GEN_TIMEOUT,cwd=str(REPO_ROOT))
            if r.returncode!=0:
                detail = (r.stderr or r.stdout or "").strip()
                return err("snapshot generation failed",500,self._generation_error_hint(root, detail or f"exit code {r.returncode}"))
        except subprocess.TimeoutExpired:
            return err("timeout",504,f"generation exceeded {GEN_TIMEOUT}s")
        except OSError as e: return err("command error",500,str(e))
        if not os.path.isfile(op) or os.path.getsize(op)==0:
            return err("generated snapshot empty",500)
        d=self._load_json(op)
        if not d: return err("invalid snapshot JSON",500)
        self._attach_automation_graph(d, root, lang, rd)
        self._save_json(op, d)
        pid=body.get("profileId",""); pn=""
        if pid:
            pl=self._load_profiles(); pi=next((p for p in pl if p["id"]==pid),None)
            if pi: pn=pi["name"]; pi["lastSnapshotId"]=sid; pi["snapshotCount"]=(pi.get("snapshotCount",0)+1)
            pi["updatedAt"]=self._now(); self._save_profiles(pl)
        entry={"id":sid,"filename":fn,"createdAt":self._now(),"rootLabel":os.path.basename(root) or root,
               "language":lang,"profileId":pid,"profileName":pn,"schemaVersion":d.get("schemaVersion",""),
               "label":body.get("label","")}
        idx=self._load_index(); idx.append(entry); self._save_index(idx)
        sm={"sourceFileCount":d.get("summary",{}).get("sourceFileCount",0),
            "symbolCount":d.get("summary",{}).get("symbolCount",0)}
        return ok({"id":sid,"filename":fn,"summary":sm,"profileId":pid,"profileName":pn})

    def _gen_for_profile(self, pid):
        pl=self._load_profiles(); pf=next((p for p in pl if p["id"]==pid),None)
        if not pf: return err("profile not found",404)
        return self._generate({"root":pf["root"],"language":pf.get("language","auto"),
                               "full":True,"redactRoot":True,"profileId":pid})

    def _delete_snap(self, sid):
        if not self._safe_id(sid): return err("invalid snapshot id",400)
        idx=self._load_index(); e=next((x for x in idx if x["id"]==sid),None)
        if not e: return err("snapshot not found",404)
        fp=os.path.join(self.sn_dir,e.get("filename",""))
        if os.path.isfile(fp):
            try: os.unlink(fp)
            except OSError as ex: return err("delete failed",500,str(ex))
        idx=[x for x in idx if x["id"]!=sid]; self._save_index(idx)
        return ok({"deleted":sid})

    def _rebuild_index(self):
        self._ensure(self.sn_dir)
        files=sorted([f for f in os.listdir(self.sn_dir) if f.endswith(".json") and f!="index.json"])
        idx=[]
        for fn in files:
            d=self._load_json(os.path.join(self.sn_dir,fn))
            if not d: continue
            sid=fn.replace("snapshot-","").replace(".json","")
            idx.append({"id":sid,"filename":fn,"createdAt":d.get("generatedAt",self._now()),
                        "rootLabel":"","language":d.get("language",d.get("summary",{}).get("language","")),
                        "profileId":"","profileName":"","schemaVersion":d.get("schemaVersion",""),"label":""})
        self._save_index(idx)
        return ok({"rebuilt":len(idx)})

    # ── Live MCP Jobs (Phase H: Stabilized) ─────────────────────────
    _mcp_state = {"available":None,"toolCount":0,"serverVersion":"","lastError":"","initializedAt":None}

    def _find_mcp_bin(self):
        for p in [str(REPO_ROOT/"target"/"release"/"codelattice"),
                  str(REPO_ROOT/"target"/"debug"/"codelattice")]:
            if os.path.isfile(p): return p
        return None

    def _probe_mcp(self):
        """One-shot probe: does MCP binary respond to initialize?"""
        if self._mcp_state["available"] is not None and self._mcp_state["initializedAt"]:
            return self._mcp_state["available"]
        bin = self._find_mcp_bin()
        if not bin:
            self._mcp_state["available"]=False; self._mcp_state["lastError"]="binary not found"
            return False
        try:
            r = subprocess.run([bin,"mcp"], input='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n',
                capture_output=True,text=True,timeout=8)
            for line in r.stdout.split("\n"):
                try: d=json.loads(line.strip())
                except: continue
                if d.get("id")==1 and "result" in d:
                    sv=d["result"].get("serverInfo",{})
                    self._mcp_state["available"]=True
                    self._mcp_state["serverVersion"]=sv.get("version","")
                    self._mcp_state["toolCount"]=sv.get("toolCount",0)
                    self._mcp_state["initializedAt"]=self._now()
                    self._mcp_state["lastError"]=""
                    return True
            self._mcp_state["available"]=False; self._mcp_state["lastError"]="no init response"
        except Exception as e:
            self._mcp_state["available"]=False; self._mcp_state["lastError"]=str(e)
        return self._mcp_state["available"]

    def _list_mcp_tools(self, force=False):
        if not force and self._mcp_state.get("_tools_cached"): return self._mcp_state.get("_tools",[])
        self._probe_mcp()
        if not self._mcp_state["available"]: return []
        bin = self._find_mcp_bin()
        if not bin: return []
        try:
            p = subprocess.Popen([bin,"mcp"], stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            p.stdin.write('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n')
            p.stdin.flush()
            _t0=time.time()
            while time.time()-_t0<8:
                l=p.stdout.readline()
                try:
                    if json.loads(l).get("id")==1: break
                except: pass
            p.stdin.write('{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n'); p.stdin.flush()
            _t1=time.time(); resp=""
            while time.time()-_t1<8:
                l=p.stdout.readline(); resp+=l
                try:
                    d=json.loads(l); p.terminate()
                    tools=d.get("result",{}).get("tools",[])
                    self._mcp_state["_tools"]=tools; self._mcp_state["_tools_cached"]=True
                    self._mcp_state["toolCount"]=len(tools)
                    return tools
                except: pass
            p.terminate()
        except: pass
        return []

    def _call_mcp_tool(self, tool, params, timeout=120):
        bin = self._find_mcp_bin()
        if not bin: raise Exception("MCP binary not found")
        p = subprocess.Popen([bin,"mcp"], stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        p.stdin.write('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n')
        p.stdin.flush()
        _t0=time.time()
        while time.time()-_t0<min(timeout,15):
            l=p.stdout.readline()
            try:
                if json.loads(l).get("id")==1: break
            except: pass
        req = {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":tool,"arguments":params}}
        p.stdin.write(json.dumps(req)+"\n"); p.stdin.flush()
        _t1=time.time(); resp=""
        while time.time()-_t1<timeout:
            l=p.stdout.readline(); resp+=l
            try:
                d=json.loads(l); p.terminate()
                r=d.get("result",{})
                text=r.get("content",[{}])[0].get("text","")
                if text:
                    try: return json.loads(text)
                    except: return {"text":text}
                return r
            except: pass
        p.terminate()
        raise TimeoutError(f"timed out after {timeout}s")

    # ── MCP API endpoints ──────────────────────────────────────────

    def _mcp_status(self):
        self._probe_mcp()
        return ok({"available":self._mcp_state["available"],"toolCount":self._mcp_state["toolCount"],
            "serverVersion":self._mcp_state["serverVersion"],"initializedAt":self._mcp_state["initializedAt"],
            "lastError":self._mcp_state["lastError"],"staticOnly":True})

    def _mcp_tools_api(self):
        tools=self._list_mcp_tools(force=True)
        if not tools: return err("failed to load tools",503,"Run 'cargo build --release --bins' first")
        return ok(tools)

# ── Job management ─────────────────────────────────────────────

    # ── Job management ────────────────────────────────────────────
    def _job_dir(self): return os.path.join(os.path.dirname(self.sn_dir),"jobs")
    def _job_index_path(self): return os.path.join(self._job_dir(),"jobs.json")
    def _load_jobs(self): return self._load_json(self._job_index_path()) or []
    def _save_jobs(self, lst): self._save_json(self._job_index_path(),lst)

    def _run_job(self, jid, workflow, tool, params, root, lang, rd):
        jdir=self._job_dir()
        # update status
        lst=self._load_jobs(); job=next((j for j in lst if j["id"]==jid),None)
        if not job: return
        job["status"]="running"; job["startedAt"]=self._now(); self._save_jobs(lst)
        try:
            result=self._call_mcp_tool(tool,params)
            job["status"]="succeeded"; job["finishedAt"]=self._now()
            # redact if needed
            if rd and isinstance(result,dict):
                raw=json.dumps(result)
                raw=raw.replace(str(REPO_ROOT).replace("/codelattice",""),"<project>") if "/codelattice" in str(REPO_ROOT) else raw
                result=json.loads(raw)
            if isinstance(result,str): result={"text":result}
            job["result"]=result
            fp=os.path.join(jdir,f"job-{jid}.json"); self._save_json(fp,result)
        except Exception as e:
            job["status"]="failed"; job["finishedAt"]=self._now(); job["error"]=str(e)
        self._save_jobs(lst)

    def _list_jobs(self, qp=None):
        if qp is None: qp={}
        lst=self._load_jobs(); ws=qp.get("workflow",[""])[0]; st=qp.get("status",[""])[0]; pid=qp.get("profileId",[""])[0]
        if ws: lst=[j for j in lst if j.get("workflow")==ws]
        if st: lst=[j for j in lst if j.get("status")==st]
        if pid: lst=[j for j in lst if j.get("profileId")==pid]
        return ok(lst)

    def _get_job(self, jid):
        if not self._safe_id(jid): return err("invalid id",400)
        lst=self._load_jobs(); job=next((j for j in lst if j["id"]==jid),None)
        if not job: return err("job not found",404)
        return ok(job)

    def _create_job(self, body):
        root=(body.get("root") or "").strip()
        lang=(body.get("language") or "auto").strip()
        wf=(body.get("workflow") or "").strip()
        tool=(body.get("tool") or "").strip()
        params=body.get("params",{}) or {}
        pid=body.get("profileId","")
        timeout=body.get("timeoutSeconds",120)
        rd=body.get("redactRoot",True)

        # Validate root/profile
        if pid:
            pl=self._load_profiles(); pf=next((p for p in pl if p["id"]==pid),None)
            if pf: root=pf["root"]; lang=pf.get("language","auto")
        if not root: return err("root is required",400)
        if not os.path.isdir(root): return err("root not found",400)
        if lang not in SUPPORTED: return err(f"unsupported language: {lang}",400)

        # Map workflow to tool + params
        WF_MAP={
            "project_overview":("codelattice_project_overview",{"root":root,"language":lang,"compact":False}),
            "symbol_search":("codelattice_symbol_search",{"root":root,"language":lang,"query":params.get("query",""),"limit":params.get("limit",20)}),
            "impact_preview":("codelattice_impact_preview",{"root":root,"language":lang,"symbol":params.get("symbol",""),"compact":False}),
            "project_insights":("codelattice_project_insights",{"root":root,"language":lang}),
            "dead_code_candidates":("codelattice_dead_code_candidates",{"root":root,"language":lang,"compact":False}),
            "automation_graph":("codelattice_automation_graph",{"root":root,"language":lang,"compact":False,"limit":80}),
            "release_check":("codelattice_production_assist",{"root":root,"language":lang}),
        }
        if wf in WF_MAP:
            tool=WF_MAP[wf][0]; params=WF_MAP[wf][1]
        elif wf=="custom_tool":
            if not tool or not tool.startswith("codelattice_"): return err("custom tool must start with codelattice_",400)
            params=params if isinstance(params,dict) else {}
        else:
            return err(f"unsupported workflow: {wf}",400,f"supported: {','.join(WF_MAP.keys())},custom_tool")

        if not self._probe_mcp(): return err("MCP server not available",503,"Run 'cargo build --release --bins' first")

        jid=self._nid(); now=self._now()
        job={"id":jid,"workflow":wf,"tool":tool,"status":"queued","createdAt":now,
             "rootLabel":os.path.basename(root) or root,"language":lang,"profileId":pid,
             "params":params,"logs":[],"result":None,"error":None,"hint":None,"staticOnly":True}
        lst=self._load_jobs(); lst.append(job); self._save_jobs(lst)
        # Execute in thread
        import threading
        t=threading.Thread(target=self._run_job,args=(jid,wf,tool,params,root,lang,rd),daemon=True)
        t.start()
        return ok(job)

    def _cancel_job(self, jid):
        if not self._safe_id(jid): return err("invalid id",400)
        lst=self._load_jobs(); job=next((j for j in lst if j["id"]==jid),None)
        if not job: return err("job not found",404)
        if job["status"] not in ("queued","running"): return err("job already finished",400)
        job["status"]="cancelled"; job["finishedAt"]=self._now()
        self._save_jobs(lst); return ok(job)

    def _delete_job(self, jid):
        if not self._safe_id(jid): return err("invalid id",400)
        lst=self._load_jobs(); job=next((j for j in lst if j["id"]==jid),None)
        if not job: return err("job not found",404)
        lst=[j for j in lst if j["id"]!=jid]; self._save_jobs(lst)
        return ok({"deleted":jid})

    # ── File System API (Phase I: safe directory browsing) ─────────

    def _fs_roots(self):
        roots=[{"path":os.path.expanduser("~"),"label":"Home","icon":"🏠"},
               {"path":os.path.expanduser("~/Desktop"),"label":"Desktop","icon":"🖥️"},
               {"path":str(REPO_ROOT),"label":"CodeLattice Repo","icon":"📦"}]
        return ok(roots)

    def _fs_list(self, qp):
        path=(qp.get("path",[""])[0] or "").strip() or os.path.expanduser("~")
        path=os.path.expanduser(path) if path.startswith("~") else path
        if ".." in path.split("/") or not path.startswith("/"): return err("invalid path",400)
        if not os.path.isdir(path): return err("not a directory",400)
        try:
            entries=[]
            for name in sorted(os.listdir(path)):
                fp=os.path.join(path,name)
                if name.startswith(".") and not qp.get("showHidden"): continue
                entries.append({"name":name,"path":fp,"isDir":os.path.isdir(fp)})
            return ok({"path":path,"entries":entries,"parentPath":os.path.dirname(path)})
        except PermissionError: return err("permission denied",403)
        except OSError as e: return err(str(e),500)

    def _fs_validate(self, qp):
        path=(qp.get("path",[""])[0] or "").strip()
        path=os.path.expanduser(path) if path.startswith("~") else path
        if ".." in path.split("/") or not path.startswith("/"): return ok({"valid":False,"reason":"invalid path"})
        if not os.path.exists(path): return ok({"valid":False,"reason":"path not found"})
        if not os.path.isdir(path): return ok({"valid":False,"reason":"not a directory"})
        return ok({"valid":True,"language":"auto","name":os.path.basename(path) or path})

    def _fs_pick_directory(self):
        body = self._rb()
        if body.get("dryRun"):
            return ok({"supported": sys.platform == "darwin", "method": "osascript" if sys.platform == "darwin" else "in-page-browser"})
        if sys.platform != "darwin":
            return err("native folder picker unavailable", 501, "Use the in-page directory browser or paste an absolute path.")
        prompt = str(body.get("prompt") or "选择 CodeLattice 项目文件夹")
        prompt = prompt.replace("\\", "\\\\").replace('"', '\\"')
        script = f'POSIX path of (choose folder with prompt "{prompt}")'
        try:
            result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=120)
        except subprocess.TimeoutExpired:
            return err("folder picker timed out", 504, "Try pasting the project path or use the in-page browser.")
        except OSError as exc:
            return err("folder picker failed", 500, str(exc))
        if result.returncode != 0:
            return err("folder selection cancelled", 400, "Use the in-page directory browser or paste a project path.")
        path = result.stdout.strip()
        path = os.path.expanduser(path) if path.startswith("~") else path
        if ".." in path.split("/") or not path.startswith("/"):
            return err("invalid selected path", 400)
        if not os.path.isdir(path):
            return err("selected path is not a directory", 400)
        return ok({"path": path, "name": os.path.basename(path) or path, "method": "native"})

    # ── One-Click Analyze (Phase I: profile+generate+return snapshot) ──
    def _quick_analyze(self):
        body=self._rb()
        root=(body.get("root") or "").strip()
        lang=(body.get("language") or "auto").strip()
        if not root or not os.path.isdir(root): return err("invalid root",400)
        if lang not in SUPPORTED: return err(f"unsupported language: {lang}",400)
        if lang == "auto":
            inv = self._project_inventory_data(root)
            if inv.get("status") == "multi_project":
                ws_result = self._workspace_analyze({"root": root, "mode": "recommended", "redactRoot": True})
                if not ws_result.get("success"):
                    return ws_result
                return ok({
                    "kind": "workspace",
                    "workspaceId": ws_result["data"].get("workspaceId"),
                    "workspace": ws_result["data"],
                    "inventory": inv,
                    "summary": ws_result["data"].get("summary", {}),
                    "generatedFrom": {
                        "staticAnalysis": True,
                        "workspaceAutoEntry": True,
                        "scriptsExecuted": False,
                        "runtimeVerified": False,
                    },
                })
        root, lang, blocker = self._prepare_analysis_target(root, lang)
        if blocker: return blocker
        # Create/touch profile
        pl=self._load_profiles()
        pf=next((p for p in pl if p["root"]==root),None)
        if not pf:
            pf={"id":self._nid(),"name":os.path.basename(root),"root":root,"rootLabel":os.path.basename(root),
                "language":lang,"createdAt":self._now(),"updatedAt":self._now(),
                "lastSnapshotId":None,"snapshotCount":0,"notes":""}
            pl.append(pf); self._save_profiles(pl)
        # Generate snapshot
        sid=self._nid(); fn=f"snapshot-{sid}.json"; op=os.path.join(self.sn_dir,fn)
        self._ensure(self.sn_dir)
        cmd=["bash",str(SNAP_SCRIPT),"--root",root,"--language",lang,"--output",op,"--full","--redact-root"]
        r=subprocess.run(cmd,capture_output=True,text=True,timeout=GEN_TIMEOUT,cwd=str(REPO_ROOT))
        if r.returncode!=0:
            detail = (r.stderr or r.stdout or "").strip()
            return err("generation failed",500,self._generation_error_hint(root, detail or f"exit code {r.returncode}"))
        d=self._load_json(op)
        if not d: return err("invalid snapshot json",500)
        self._attach_automation_graph(d, root, lang, True)
        self._save_json(op, d)
        # Update profile + index
        pf["lastSnapshotId"]=sid; pf["snapshotCount"]=pf.get("snapshotCount",0)+1; pf["updatedAt"]=self._now(); self._save_profiles(pl)
        entry={"id":sid,"filename":fn,"createdAt":self._now(),"rootLabel":os.path.basename(root),"language":lang,
               "profileId":pf["id"],"profileName":pf["name"],"schemaVersion":d.get("schemaVersion",""),"label":""}
        idx=self._load_index(); idx.append(entry); self._save_index(idx)
        return ok({"snapshotId":sid,"snapshot":d,"profileId":pf["id"],"summary":{"sourceFileCount":d.get("summary",{}).get("sourceFileCount",0),"symbolCount":d.get("summary",{}).get("symbolCount",0)}})


def main():
    port=8765
    for i,a in enumerate(sys.argv):
        if a=="--port" and i+1<len(sys.argv): port=int(sys.argv[i+1])
        if a=="--snapshot-dir" and i+1<len(sys.argv): Workbench.sn_dir=sys.argv[i+1]
        if a=="--timeout" and i+1<len(sys.argv): global GEN_TIMEOUT; GEN_TIMEOUT=int(sys.argv[i+1])
    server=http.server.HTTPServer(("127.0.0.1",port),Workbench)
    url=f"http://127.0.0.1:{port}"
    print(f"CodeLattice WebUI Workbench (Phase E)")
    print(f"  URL: {url}  |  API: {url}/api/health")
    print(f"  Snapshots: {Workbench.sn_dir}")
    sys.stdout.flush()
    try: server.serve_forever()
    except KeyboardInterrupt: print("\nShutdown."); server.server_close()

if __name__=="__main__": main()
