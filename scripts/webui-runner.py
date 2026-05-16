#!/usr/bin/env python3
"""CodeLattice WebUI Runner — Local HTTP server + snapshot library API.

Uses Python stdlib only. Binds 127.0.0.1. Serves webui/snapshot-viewer/
as static files, plus REST API for health, snapshot generation, and library.

Usage:
  python3 scripts/webui-runner.py [--port PORT] [--static-dir PATH] [--snapshot-dir PATH]
"""

import http.server
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.parse
import hashlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
STATIC_DIR = REPO_ROOT / "webui" / "snapshot-viewer"
SNAPSHOT_SCRIPT = REPO_ROOT / "scripts" / "webui-snapshot.sh"
DEFAULT_LIBRARY = REPO_ROOT / ".codelattice-webui" / "snapshots"
INDEX_FILE = "index.json"
GENERATE_TIMEOUT = 120  # seconds


def parse_args():
    port = 8765
    static_dir = str(STATIC_DIR)
    snapshot_dir = str(DEFAULT_LIBRARY)
    i = 1
    while i < len(sys.argv):
        arg = sys.argv[i]
        if arg == "--port" and i + 1 < len(sys.argv):
            port = int(sys.argv[i + 1]); i += 2
        elif arg == "--static-dir" and i + 1 < len(sys.argv):
            static_dir = sys.argv[i + 1]; i += 2
        elif arg == "--snapshot-dir" and i + 1 < len(sys.argv):
            snapshot_dir = sys.argv[i + 1]; i += 2
        else:
            i += 1
    return port, static_dir, snapshot_dir


class RunnerHandler(http.server.SimpleHTTPRequestHandler):
    static_dir = str(STATIC_DIR)
    snapshot_dir = str(DEFAULT_LIBRARY)

    def __init__(self, *args, **kwargs):
        self.static_dir = RunnerHandler.static_dir
        self.snapshot_dir = RunnerHandler.snapshot_dir
        super().__init__(*args, directory=self.static_dir, **kwargs)

    def log_message(self, format, *args):
        if "/api/" in (args[0] if args else ""):
            sys.stderr.write(f"[api] {args[0]}\n")

    def _json(self, data, status=200):
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", len(body))
        self.end_headers()
        self.wfile.write(body)

    def _json_error(self, msg, status=400, detail=None):
        d = {"error": msg, "status": status}
        if detail: d["detail"] = str(detail)
        self._json(d, status)

    def _read_body(self):
        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            return {}
        try:
            return json.loads(self.rfile.read(length))
        except json.JSONDecodeError as e:
            return {"_parse_error": str(e)}

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        # API routes
        if path == "/api/health":
            return self._handle_health()
        if path == "/api/snapshots":
            return self._handle_list_snapshots()
        if path.startswith("/api/snapshot/"):
            snap_id = path.split("/api/snapshot/", 1)[1]
            return self._handle_get_snapshot(snap_id)

        # Static file serving
        return super().do_GET()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/api/generate-snapshot":
            return self._handle_generate()
        return self._json_error("not found", 404)

    def do_DELETE(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path.startswith("/api/snapshot/"):
            snap_id = parsed.path.split("/api/snapshot/", 1)[1]
            return self._handle_delete_snapshot(snap_id)
        return self._json_error("not found", 404)

    # ── Handlers ──────────────────────────────────────────────────

    def _handle_health(self):
        self._json({
            "status": "ok",
            "mode": "runner",
            "snapshotDir": self.snapshot_dir,
            "staticDir": self.static_dir,
        })

    def _ensure_library(self):
        os.makedirs(self.snapshot_dir, exist_ok=True)

    def _load_index(self):
        self._ensure_library()
        ipath = os.path.join(self.snapshot_dir, INDEX_FILE)
        if not os.path.isfile(ipath):
            return []
        try:
            with open(ipath) as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError):
            return []

    def _save_index(self, data):
        self._ensure_library()
        with open(os.path.join(self.snapshot_dir, INDEX_FILE), "w") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    def _next_id(self):
        return hashlib.md5(str(time.time()).encode()).hexdigest()[:12]

    def _handle_list_snapshots(self):
        index = self._load_index()
        result = []
        for entry in index:
            snap_path = os.path.join(self.snapshot_dir, entry.get("filename", ""))
            summary = {}
            if os.path.isfile(snap_path):
                try:
                    with open(snap_path) as f:
                        d = json.load(f)
                    s = d.get("summary", {})
                    summary = {
                        "sourceFileCount": s.get("sourceFileCount", 0),
                        "symbolCount": s.get("symbolCount", 0),
                        "edgeCount": s.get("edgeCount", 0),
                        "nodeCount": s.get("nodeCount", 0),
                    }
                except (json.JSONDecodeError, OSError):
                    pass
            result.append({
                "id": entry["id"],
                "filename": entry.get("filename", ""),
                "createdAt": entry.get("createdAt", ""),
                "rootLabel": entry.get("rootLabel", ""),
                "language": entry.get("language", ""),
                "summary": summary,
            })
        self._json(result)

    def _handle_get_snapshot(self, snap_id):
        index = self._load_index()
        entry = next((e for e in index if e["id"] == snap_id), None)
        if not entry:
            return self._json_error("snapshot not found", 404)
        fpath = os.path.join(self.snapshot_dir, entry["filename"])
        if not os.path.isfile(fpath):
            return self._json_error("snapshot file missing", 404)
        try:
            with open(fpath) as f:
                return self._json(json.load(f))
        except (json.JSONDecodeError, OSError) as e:
            return self._json_error("read error", 500, str(e))

    def _handle_generate(self):
        body = self._read_body()
        root = body.get("root", "").strip()
        if not root:
            return self._json_error("root is required")
        if not os.path.isdir(root):
            return self._json_error(f"root directory not found: {root}")

        language = body.get("language", "auto").strip()
        do_full = body.get("full", True)
        do_redact = body.get("redactRoot", True)

        snap_id = self._next_id()
        filename = f"snapshot-{snap_id}.json"
        outpath = os.path.join(self.snapshot_dir, filename)
        self._ensure_library()

        cmd = [
            "bash", str(SNAPSHOT_SCRIPT),
            "--root", root,
            "--language", language,
            "--output", outpath,
        ]
        if do_full:
            cmd.append("--full")
        if do_redact:
            cmd.append("--redact-root")

        try:
            result = subprocess.run(
                cmd, capture_output=True, text=True,
                timeout=GENERATE_TIMEOUT, cwd=str(REPO_ROOT)
            )
            if result.returncode != 0:
                return self._json_error(
                    "snapshot generation failed",
                    500,
                    result.stderr[:500] if result.stderr else "unknown error"
                )
        except subprocess.TimeoutExpired:
            return self._json_error("timeout", 504, f"generation exceeded {GENERATE_TIMEOUT}s")
        except OSError as e:
            return self._json_error("command error", 500, str(e))

        if not os.path.isfile(outpath) or os.path.getsize(outpath) == 0:
            return self._json_error("generated snapshot is empty", 500)

        # Extract summary for index
        summary = {}
        try:
            with open(outpath) as f:
                d = json.load(f)
            s = d.get("summary", {})
            summary = {
                "sourceFileCount": s.get("sourceFileCount", 0),
                "symbolCount": s.get("symbolCount", 0),
                "edgeCount": s.get("edgeCount", 0),
                "nodeCount": s.get("nodeCount", 0),
            }
        except (json.JSONDecodeError, OSError):
            pass

        entry = {
            "id": snap_id,
            "filename": filename,
            "createdAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "rootLabel": os.path.basename(root) or root,
            "language": language,
        }

        index = self._load_index()
        index.append(entry)
        self._save_index(index)

        self._json({
            "ok": True,
            "id": snap_id,
            "filename": filename,
            "summary": summary,
        }, status=201)

    def _handle_delete_snapshot(self, snap_id):
        index = self._load_index()
        entry = next((e for e in index if e["id"] == snap_id), None)
        if not entry:
            return self._json_error("snapshot not found", 404)
        fpath = os.path.join(self.snapshot_dir, entry.get("filename", ""))
        if os.path.isfile(fpath):
            try:
                os.unlink(fpath)
            except OSError as e:
                return self._json_error("delete failed", 500, str(e))
        index = [e for e in index if e["id"] != snap_id]
        self._save_index(index)
        self._json({"ok": True, "deleted": snap_id})


def main():
    port, static_dir, snapshot_dir = parse_args()
    RunnerHandler.static_dir = static_dir
    RunnerHandler.snapshot_dir = snapshot_dir

    server = http.server.HTTPServer(("127.0.0.1", port), RunnerHandler)
    url = f"http://127.0.0.1:{port}"
    print(f"CodeLattice WebUI Runner")
    print(f"  URL:          {url}")
    print(f"  Static dir:   {static_dir}")
    print(f"  Snapshot dir: {snapshot_dir}")
    print(f"  API health:   {url}/api/health")
    sys.stdout.flush()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down.")
        server.server_close()


if __name__ == "__main__":
    main()
