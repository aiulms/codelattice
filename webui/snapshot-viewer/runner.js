// runner.js — CodeLattice WebUI Runner Client (Phase D)

var RUNNER = window.RUNNER || {}; window.RUNNER = RUNNER;

RUNNER.base = "";
RUNNER.connected = false;
RUNNER.snapshots = [];

function runnerApi(path, opts) {
  opts = opts || {};
  var url = RUNNER.base + path;
  var init = { method: opts.method || "GET", headers: { "Content-Type": "application/json" } };
  if (opts.body) init.body = JSON.stringify(opts.body);
  return fetch(url, init).then(function(r) {
    if (opts.raw) return r;
    return r.json().then(function(d) { if (!r.ok) throw new Error(d.error || r.statusText); return d; });
  });
}

function runnerCheckHealth() {
  var origin = window.location.origin || "http://127.0.0.1:8765";
  RUNNER.base = origin;
  return runnerApi("/api/health").then(function(d) {
    RUNNER.connected = true;
    var badge = document.getElementById("runner-mode-badge");
    if (badge) badge.style.display = "";
    var panel = document.getElementById("runner-panel");
    if (panel) panel.style.display = "";
    // hide static file mode badge
    var sb = document.getElementById("static-mode-badge");
    if (sb) sb.style.display = "none";
    return true;
  }).catch(function() {
    RUNNER.connected = false;
    var sb = document.getElementById("static-mode-badge");
    if (sb) sb.style.display = "";
    var panel = document.getElementById("runner-panel");
    if (panel) panel.style.display = "none";
    var runnerLib = document.getElementById("runner-library-list");
    if (runnerLib) runnerLib.innerHTML = '<span class="text-muted text-sm">Start <code>bash scripts/webui-runner.sh</code> to enable local analysis.</span>';
    return false;
  });
}

function runnerGenerate() {
  if (!RUNNER.connected) { alert("Runner not connected. Start scripts/webui-runner.sh first."); return; }
  var root = (document.getElementById("runner-root-input").value || "").trim();
  if (!root) { alert("Enter a project root path."); return; }
  var lang = document.getElementById("runner-lang-select").value;
  var st = document.getElementById("runner-status");
  if (st) st.textContent = "Generating...";
  return runnerApi("/api/generate-snapshot", {
    method: "POST",
    body: { root: root, language: lang, full: true, redactRoot: true }
  }).then(function(d) {
    if (st) st.textContent = "Done: " + d.id + " (" + (d.summary.symbolCount||0) + " symbols)";
    runnerLoadLibrary();
    return d;
  }).catch(function(e) {
    if (st) st.textContent = "Error: " + e.message;
  });
}

function runnerLoadLibrary() {
  if (!RUNNER.connected) return;
  return runnerApi("/api/snapshots").then(function(list) {
    RUNNER.snapshots = list || [];
    renderSnapshotLibrary();
    return list;
  });
}

function renderSnapshotLibrary() {
  var el = document.getElementById("runner-library-list");
  if (!el) return;
  var snaps = RUNNER.snapshots;
  if (snaps.length === 0) {
    el.innerHTML = '<span class="text-muted text-sm">No snapshots in library. Generate one above.</span>';
    return;
  }
  el.innerHTML = '<div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:4px;">' +
    snaps.map(function(s) {
      var syms = (s.summary||{}).symbolCount || "-";
      return '<div class="snapshot-library-item" style="padding:6px 10px;background:#f8fafc;border:1px solid #e5e7eb;border-radius:4px;font-size:0.85em;display:flex;gap:6px;align-items:center;">' +
        '<span><strong>' + esc(s.rootLabel||s.id) + '</strong> <span class="badge badge-lang">' + esc(s.language||'?') + '</span> ' + syms + ' sym</span>' +
        '<span style="font-size:0.75em;color:#9ca3af;">' + (s.createdAt||"").slice(0,10) + '</span>' +
        '<button class="btn btn-sm btn-secondary" onclick="runnerLoadSnapshot(&quot;' + escAttr(s.id) + '&quot;)" title="Load">Load</button>' +
        '<button class="btn btn-sm btn-secondary" onclick="runnerCompareSnapshot(&quot;' + escAttr(s.id) + '&quot;)" title="Compare">Diff</button>' +
        '<button class="btn btn-sm btn-secondary" onclick="runnerAddTimeline(&quot;' + escAttr(s.id) + '&quot;)" title="Timeline">+TL</button>' +
        '</div>';
    }).join("") + '</div>';
}

function runnerLoadSnapshot(snapId) {
  if (!RUNNER.connected) return;
  return runnerApi("/api/snapshot/" + snapId).then(function(data) {
    currentSnapshot = data;
    renderAll();
    document.getElementById("loaded-content").style.display = "";
    document.getElementById("welcome-view").style.display = "none";
    document.getElementById("error-view").style.display = "none";
    updateCautionBanner();
    show("dashboard");
  }).catch(function(e) {
    alert("Failed to load snapshot: " + e.message);
  });
}

function runnerCompareSnapshot(snapId) {
  if (!RUNNER.connected) return;
  return runnerApi("/api/snapshot/" + snapId).then(function(data) {
    diffSnapshot = data;
    document.getElementById("diff-compare-name").textContent = "vs " + snapId;
    document.getElementById("diff-clear-btn").style.display = "";
    computeAndRenderDiff();
    show("diff");
  }).catch(function(e) {
    alert("Failed to load compare: " + e.message);
  });
}

function runnerAddTimeline(snapId) {
  if (!RUNNER.connected || typeof CTL === "undefined") return;
  return runnerApi("/api/snapshot/" + snapId).then(function(data) {
    var name = snapId + ".json";
    CTL.timelineSnapshots = CTL.timelineSnapshots || [];
    CTL.timelineSnapshots.push({ name: name, data: data });
    CTL.timelineSnapshots.sort(function(a,b) { return (a.data.generatedAt||"").localeCompare(b.data.generatedAt||""); });
    document.getElementById("timeline-snapshot-count").textContent = CTL.timelineSnapshots.length + " snapshots";
    document.getElementById("timeline-clear-btn").style.display = "";
    CTL.renderTimeline();
    show("timeline");
  }).catch(function(e) { alert("Failed: " + e.message); });
}

// Auto-detect runner on page load
document.addEventListener("DOMContentLoaded", function() {
  // Delay to allow DOM to finish
  setTimeout(runnerCheckHealth, 500);
});
