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
SUPPORTED = ["rust","typescript","c","cpp","python","arkts","cangjie","auto"]


def ok(data=None): return {"success": True, "data": data or {}, "error": None, "hint": None}
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
        if p.path=="/api/mcp/status": return self._r(self._mcp_status)
        if p.path=="/api/mcp/tools": return self._r(lambda:ok(self._mcp_list_tools()))
        if p.path=="/api/mcp/jobs": return self._r(lambda:self._list_jobs(urllib.parse.parse_qs(p.query)))
        if p.path.startswith("/api/mcp/job/"):
            jid=p.path.split("/api/mcp/job/",1)[1]
            return self._r(lambda:self._get_job(jid))
        return super().do_GET()

    def do_POST(self):
        p = urllib.parse.urlparse(self.path)
        if p.path=="/api/profiles": return self._r(lambda:self._create_profile(self._rb()))
        if p.path=="/api/generate-snapshot": return self._r(lambda:self._generate(self._rb()))
        if p.path=="/api/rebuild-index": return self._r(self._rebuild_index)
        if p.path=="/api/mcp/jobs": return self._r(lambda:self._create_job(self._rb()))
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

    def _generate(self, body):
        root=(body.get("root") or "").strip()
        if not root: return err("root is required",400)
        if not os.path.isdir(root): return err("root directory not found",400,f"path: {root}")
        lang=body.get("language","auto").strip()
        if lang not in SUPPORTED: return err(f"unsupported language: {lang}",400)
        df=body.get("full",True); rd=body.get("redactRoot",True)
        sid=self._nid(); fn=f"snapshot-{sid}.json"; op=os.path.join(self.sn_dir,fn)
        self._ensure(self.sn_dir)
        cmd=["bash",str(SNAP_SCRIPT),"--root",root,"--language",lang,"--output",op]
        if df: cmd.append("--full")
        if rd: cmd.append("--redact-root")
        try:
            r=subprocess.run(cmd,capture_output=True,text=True,timeout=GEN_TIMEOUT,cwd=str(REPO_ROOT))
            if r.returncode!=0:
                return err("snapshot generation failed",500,r.stderr[:300] or "unknown error")
        except subprocess.TimeoutExpired:
            return err("timeout",504,f"generation exceeded {GEN_TIMEOUT}s")
        except OSError as e: return err("command error",500,str(e))
        if not os.path.isfile(op) or os.path.getsize(op)==0:
            return err("generated snapshot empty",500)
        d=self._load_json(op)
        if not d: return err("invalid snapshot JSON",500)
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

    # ── Live MCP Jobs (Phase G) ────────────────────────────────────
    mcp_bin = None; _mcp_ok = None; _mcp_tools = None

    def _find_mcp(self):
        if self.mcp_bin and os.path.isfile(self.mcp_bin): return self.mcp_bin
        for p in [str(REPO_ROOT/"target"/"release"/"codelattice"),
                  str(REPO_ROOT/"target"/"debug"/"codelattice")]:
            if os.path.isfile(p): self.mcp_bin=p; return p
        return None

    def _mcp_health(self):
        if self._mcp_ok is not None: return self._mcp_ok
        bin = self._find_mcp()
        if not bin: self._mcp_ok=False; return False
        try:
            r=subprocess.run([bin,"mcp"], input='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n',capture_output=True,text=True,timeout=10)
            for line in r.stdout.split("\n"):
                try: d=json.loads(line.strip()); self._mcp_ok=(d.get("id")==1 and "result" in d); break
                except: pass
        except: self._mcp_ok=False
        return self._mcp_ok

    def _mcp_call(self, tool, params, timeout=120):
        bin = self._find_mcp()
        if not bin: raise Exception("MCP binary not found")
        p=subprocess.Popen([bin,"mcp"], stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        # initialize
        p.stdin.write('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n')
        p.stdin.flush()
        lines=[]; start=time.time()
        while time.time()-start<timeout:
            l=p.stdout.readline(); lines.append(l)
            try: d=json.loads(l); break
            except: pass
        # send tools/call
        req = {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":tool,"arguments":params}}
        p.stdin.write(json.dumps(req)+"\n"); p.stdin.flush()
        resp=""; start2=time.time()
        while time.time()-start2<timeout:
            l=p.stdout.readline(); resp+=l
            try:
                d=json.loads(l)
                p.terminate()
                return d.get("result",{}).get("content",[{}])[0].get("text","") if d.get("result") else d
            except: pass
        p.terminate()
        return {"error":"timeout","hint":f"timed out after {str(timeout)}s"}

    def _mcp_list_tools(self):
        if self._mcp_tools: return self._mcp_tools
        if not self._mcp_health(): return []
        try:
            bin=self._find_mcp()
            p=subprocess.Popen([bin,"mcp"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            p.stdin.write('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"runner","version":"1.0"}}}\n')
            p.stdin.flush()
            t0=time.time()
            while time.time()-t0<10:
                l=p.stdout.readline()
                try:
                    d=json.loads(l)
                    if d.get("id")==1 and "result" in d: break
                except: pass
            p.stdin.write('{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n'); p.stdin.flush()
            resp=""; t1=time.time()
            while time.time()-t1<10:
                l=p.stdout.readline(); resp+=l
                try:
                    d=json.loads(l)
                    p.terminate()
                    tools=d.get("result",{}).get("tools",[])
                    self._mcp_tools=tools
                    return tools
                except: pass
            p.terminate()
        except: pass
        return []

    def _mcp_status(self):
        ok=self._mcp_health(); tools=self._mcp_list_tools() if ok else []
        return ok({"available":ok,"toolCount":len(tools),"serverVersion":"v0.12","binary":self._find_mcp() or "","mode":"jsonrpc-stdio"})

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
            result=self._mcp_call(tool,params)
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
            "release_check":("codelattice_production_assist",{"root":root,"language":lang}),
        }
        if wf in WF_MAP:
            tool=WF_MAP[wf][0]; params=WF_MAP[wf][1]
        elif wf=="custom_tool":
            if not tool or not tool.startswith("codelattice_"): return err("custom tool must start with codelattice_",400)
            params=params if isinstance(params,dict) else {}
        else:
            return err(f"unsupported workflow: {wf}",400,f"supported: {','.join(WF_MAP.keys())},custom_tool")

        if not self._mcp_health(): return err("MCP server not available",503,"Run 'cargo build --release --bins' first")

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
