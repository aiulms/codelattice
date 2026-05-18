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
    "build.sh": "shell",
    "test.sh": "shell",
    "release.sh": "shell",
    "install.sh": "shell",
    "CMakeLists.txt": "c/cpp",
    "compile_commands.json": "c/cpp",
    ".sln": "unsupported:csharp",
    ".csproj": "unsupported:csharp",
}
LANG_PRIORITY = {"cangjie": 0, "arkts": 1, "rust": 2, "typescript": 3, "python": 4, "c/cpp": 5, "c": 5, "cpp": 5, "shell": 6, "unsupported:csharp": 9}


def ok(data=None): return {"success": True, "data": data if data is not None else {}, "error": None, "hint": None}
def err(msg, code=400, hint=None):
    return {"success": False, "data": None, "error": msg, "hint": hint or "", "status": code}


class Workbench(http.server.SimpleHTTPRequestHandler):
    sn_dir = str(REPO_ROOT / ".codelattice-webui" / "snapshots")
    pf_file = str(REPO_ROOT / ".codelattice-webui" / "profiles.json")

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
        if root_supported:
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
        return super().do_GET()

    def do_POST(self):
        p = urllib.parse.urlparse(self.path)
        if p.path=="/api/profiles": return self._r(lambda:self._create_profile(self._rb()))
        if p.path=="/api/generate-snapshot": return self._r(lambda:self._generate(self._rb()))
        if p.path=="/api/rebuild-index": return self._r(self._rebuild_index)
        if p.path=="/api/mcp/jobs": return self._r(lambda:self._create_job(self._rb()))
        if p.path=="/api/quick-analyze": return self._r(self._quick_analyze)
        if p.path=="/api/fs/pick-directory": return self._r(self._fs_pick_directory)
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
