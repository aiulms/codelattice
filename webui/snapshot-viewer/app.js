// app.js — CodeLattice Snapshot Viewer (Phase A Enhanced)
//
// Renders enriched CodeLatticeWebSnapshotV1 JSON into a multi-tab UI.
// Supports: Dashboard, Explore, Cleanup, Release Review, Workflow Presets.

(function () {
  "use strict";

  // ── State ───────────────────────────────────────────────────────────
  let currentSnapshot = null;
  let allSymbols = [];
  let filteredSymbols = [];
  let selectedSymbolId = null;

  // ── DOM Helpers ─────────────────────────────────────────────────────

  function $(sel) { return document.querySelector(sel); }
  function $$(sel) { return Array.from(document.querySelectorAll(sel)); }

  function show(id) {
    $$(`.view-section`).forEach(function (el) { el.style.display = "none"; });
    var view = $("#view-" + id);
    if (view) view.style.display = "";
    $$(".tab-btn").forEach(function (btn) {
      var active = btn.getAttribute("data-tab") === id;
      btn.classList.toggle("active", active);
      btn.setAttribute("aria-selected", active ? "true" : "false");
    });
  }

  function esc(text) {
    var d = document.createElement("div");
    d.textContent = text || "";
    return d.innerHTML;
  }

  function badge(text, cls) {
    cls = cls || "";
    return '<span class="badge ' + cls + '">' + esc(text) + "</span>";
  }

  // ── Tab Navigation ─────────────────────────────────────────────────

  $$(".tab-btn").forEach(function (btn) {
    btn.addEventListener("click", function () {
      show(btn.getAttribute("data-tab"));
    });
  });

  // Explore filter event listeners
  $("#explore-search").addEventListener("input", applyExploreFilter);
  $("#explore-kind-filter").addEventListener("change", applyExploreFilter);
  $("#explore-sort").addEventListener("change", applyExploreFilter);

  // ── File Load ───────────────────────────────────────────────────────

  function loadSnapshot(jsonText) {
    try {
      currentSnapshot = JSON.parse(jsonText);
    } catch (e) {
      showError("Invalid JSON: " + e.message);
      return;
    }
    if (!currentSnapshot.schemaVersion) {
      showError("Not a valid CodeLattice snapshot — missing schemaVersion.");
      return;
    }
    renderAll();
    $("#loaded-content").style.display = "";
    $("#welcome-view").style.display = "none";
    $("#error-view").style.display = "none";
    updateCautionBanner();
  }

  function showError(msg, detail) {
    $("#error-view").style.display = "";
    $("#welcome-view").style.display = "none";
    $("#loaded-content").style.display = "none";
    $("#error-message").textContent = msg;
    if (detail) $("#error-detail").textContent = detail;
  }

  function showWelcome() {
    $("#welcome-view").style.display = "";
    $("#error-view").style.display = "none";
    $("#loaded-content").style.display = "none";
  }

  // File input handler
  $("#file-input").addEventListener("change", function (e) {
    var file = e.target.files[0];
    if (!file) return;
    var reader = new FileReader();
    reader.onload = function (ev) { loadSnapshot(ev.target.result); };
    reader.readAsText(file);
    e.target.value = ""; // allow re-selecting same file
  });

  // Drag & drop
  var body = document.body;
  ["dragenter", "dragover"].forEach(function (ev) {
    body.addEventListener(ev, function (e) {
      e.preventDefault();
      $("#drop-zone").style.display = "";
    });
  });
  ["dragleave", "drop"].forEach(function (ev) {
    body.addEventListener(ev, function (e) {
      e.preventDefault();
      $("#drop-zone").style.display = "none";
    });
  });
  body.addEventListener("drop", function (e) {
    var file = e.dataTransfer.files[0];
    if (file) {
      var reader = new FileReader();
      reader.onload = function (ev) { loadSnapshot(ev.target.result); };
      reader.readAsText(file);
    }
  });

  // ── Caution Banner ──────────────────────────────────────────────────

  function updateCautionBanner() {
    var banner = $("#caution-banner");
    var lim = (currentSnapshot && currentSnapshot.limitations) || {};
    if (lim.runtimeVerified === false) {
      banner.style.display = "";
    } else {
      banner.style.display = "none";
    }
  }

  // ── Render All Sections ─────────────────────────────────────────────

  function renderAll() {
    renderHeader();
    renderDashboard();
    renderExplore();
    renderCleanup();
    renderReleaseReview();
    renderWorkflowPresets();
  }

  // ── Header ──────────────────────────────────────────────────────────

  function renderHeader() {
    var s = currentSnapshot.summary || {};
    var gf = currentSnapshot.generatedFrom || {};

    var info = $("#project-info");
    info.style.display = "";

    $("#hdr-language").textContent = s.language || gf.toolVersion || "-";
    $("#hdr-schema-version").textContent = currentSnapshot.schemaVersion || "";
    $("#hdr-generated-at").textContent = s.generatedAt || currentSnapshot.generatedAt || "";
  }

  // ── Dashboard ───────────────────────────────────────────────────────

  function renderDashboard() {
    var s = currentSnapshot.summary || {};
    var q = currentSnapshot.quality || {};
    var lim = currentSnapshot.limitations || {};
    var gf = currentSnapshot.generatedFrom || {};

    // Summary cards
    setText("dash-source-files", s.sourceFileCount != null ? s.sourceFileCount : "-");
    setText("dash-symbols", s.symbolCount != null ? s.symbolCount : "-");
    var ne = (s.nodeCount || 0) + " / " + (s.edgeCount || 0);
    setText("dash-nodes-edges", ne);
    setText("dash-language", s.language || "-");

    // Quality gates
    var qStatus = $("#dash-quality-status");
    var overall = q.overall || "unknown";
    qStatus.textContent = overall.toUpperCase();
    qStatus.className = "badge " + (overall === "pass" ? "badge-success" :
                                        overall === "fail" ? "badge-danger" : "badge-warning");

    var passed = q.passedGateCount != null ? q.passedGateCount : "?";
    var failed = q.failedGateCount != null ? q.failedGateCount : "?";
    setText("dash-gate-summary", passed + " passed, " + failed + " failed");

    var gateList = $("#dash-quality-gates");
    gateList.innerHTML = "";
    var gates = q.gates || [];
    if (gates.length === 0) {
      gateList.innerHTML = '<p class="text-muted text-sm">No quality gate data collected.</p>';
    } else {
      gates.forEach(function (g) {
        var name = g.name || g.label || "unnamed";
        var status = String(g.status || g.result || "unknown").toLowerCase();
        var ok = status === "pass" || status === "passed" || status === "ok" || status === "true";
        var cls = ok ? "gate-pass" : "gate-fail";
        gateList.innerHTML +=
          '<div class="gate-item ' + cls + '">' +
          '<span class="gate-name">' + esc(name) + '</span>' +
          '<span class="gate-status">' + esc(status) + '</span></div>';
      });
    }

    // Generated From metadata
    var metaList = $("#dash-generated-from");
    metaList.innerHTML =
      '<div class="meta-item"><strong>Tool:</strong> ' + esc(gf.tool || "-") + '</div>' +
      '<div class="meta-item"><strong>Version:</strong> ' + esc(gf.toolVersion || "-") + '</div>' +
      '<div class="meta-item"><strong>Schema:</strong> ' + esc(gf.snapshotSchema || currentSnapshot.schemaVersion || "-") + '</div>' +
      '<div class="meta-item"><strong>Static Analysis:</strong> ' +
        (gf.staticAnalysis ? badge("Yes", "badge-success") : badge("No")) + '</div>' +
      '<div class="meta-item"><strong>Runtime Verified:</strong> ' +
        (gf.runtimeVerified === false ?
          badge("False", "badge-danger") + ' <span class="text-muted text-sm">— results are heuristic only</span>' :
          badge("Unknown")) + '</div>';

    // Limitations
    var limList = $("#dash-limitations");
    limList.innerHTML = "";
    var notes = lim.notes || [];
    if (notes.length > 0) {
      notes.forEach(function (n) {
        limList.innerHTML += "<li>" + esc(n) + "</li>";
      });
    } else {
      limList.innerHTML =
        "<li>" + (lim.runtimeVerified !== true ?
          "<strong>Runtime not verified.</strong> No project code was executed." :
          "") + "</li>" +
        "<li><strong>No coverage data.</strong> Test execution was not performed.</li>" +
        "<li><strong>Deletion safety not verified.</strong> Dead-code candidates require manual review.</li>";
    }
  }

  // ── Explore ─────────────────────────────────────────────────────────

  function renderExplore() {
    var exp = currentSnapshot.explore || {};
    allSymbols = (exp.symbols || []).map(function (s, i) { s._idx = i; return s; });
    filteredSymbols = allSymbols.slice();

    // Populate kind filter
    var kindFilter = $("#explore-kind-filter");
    var currentValue = kindFilter.value;
    var kinds = {};
    allSymbols.forEach(function (s) { kinds[s.kind] = true; });
    var opts = '<option value="">All Kinds (' + allSymbols.length + ")</option>";
    Object.keys(kinds).sort().forEach(function (k) {
      var count = allSymbols.filter(function (s) { return s.kind === k; }).length;
      opts += '<option value="' + esc(k) + '">' + esc(k || "unknown") + " (" + count + ")</option>";
    });
    kindFilter.innerHTML = opts;
    kindFilter.value = currentValue;

    applyExploreFilter();

    // Source files
    renderSourceFiles(exp);

    // Top files
    renderTopFiles(exp);
  }

  function applyExploreFilter() {
    var search = ($("#explore-search").value || "").toLowerCase();
    var kind = $("#explore-kind-filter").value;
    var sort = $("#explore-sort").value;

    filteredSymbols = allSymbols.filter(function (s) {
      var nameMatch = !search ||
        (s.name || "").toLowerCase().indexOf(search) !== -1 ||
        (s.file || "").toLowerCase().indexOf(search) !== -1 ||
        (s.id || "").toLowerCase().indexOf(search) !== -1;
      var kindMatch = !kind || s.kind === kind;
      return nameMatch && kindMatch;
    });

    // Sort
    filteredSymbols.sort(function (a, b) {
      if (sort === "name") return (a.name || "").localeCompare(b.name || "");
      if (sort === "file") return (a.file || "").localeCompare(b.file || "");
      if (sort === "kind") return (a.kind || "").localeCompare(b.kind || "");
      return 0;
    });

    renderSymbolList();
  }

  function renderSymbolList() {
    var list = $("#explore-symbol-list");
    setText("explore-count", "(" + filteredSymbols.length + ")");
    setText("explore-total",
      filteredSymbols.length !== allSymbols.length ?
        " of " + allSymbols.total : "");

    if (filteredSymbols.length === 0) {
      list.innerHTML = '<p class="text-muted text-center text-sm">No symbols match filters.</p>';
      return;
    }

    var html = "";
    filteredSymbols.forEach(function (s) {
      var isSelected = s.id === selectedSymbolId;
      html +=
        '<div class="symbol-item' + (isSelected ? " selected" : '') + '" data-id="' + esc(s.id) + '">' +
        '<span class="sym-kind-badge ' + esc(s.kind || "") + '">' + esc(s.kindLabel || s.kind || "?") + '</span>' +
        '<span class="sym-name">' + esc(s.name || s.id || "?") + '</span>' +
        (s.exported ? '<span class="badge badge-xs">pub</span>' : "") +
        '<span class="sym-file text-sm text-muted">' + esc(s.file || "") + '</span></div>';
    });
    list.innerHTML = html;

    // Click handlers
    list.querySelectorAll(".symbol-item").forEach(function (item) {
      item.addEventListener("click", function () {
        selectSymbol(item.getAttribute("data-id"));
      });
    });
  }

  function selectSymbol(id) {
    selectedSymbolId = id;
    // Re-render list to update selection highlight
    renderSymbolList();

    var sym = null;
    for (var i = 0; i < allSymbols.length; i++) {
      if (allSymbols[i].id === id) { sym = allSymbols[i]; break; }
    }
    var detail = $("#explore-detail");
    if (!sym) {
      detail.innerHTML = '<p class="text-muted text-center">Symbol not found.</p>';
      return;
    }

    var html =
      '<h4>' + esc(sym.name || sym.id) + '</h4>' +
      '<table class="detail-table"><tbody>' +
      row("Kind", (sym.kindLabel || sym.kind || "-") +
        (sym.exported ? " " + badge("exported", "badge-success") : "")) +
      row("ID", '<code class="code-inline">' + esc(sym.id) + '</code>') +
      row("File", esc(sym.file || "-")) +
      row("Line", sym.line != null ? sym.line + (sym.endLine ? "-" + sym.endLine : "") : "-") +
      row("Visibility", sym.visibility || "-") +
      "</tbody></table>";

    detail.innerHTML = html;
  }

  function renderSourceFiles(exp) {
    var files = exp.sourceFiles || [];
    var container = $("#explore-file-list");
    setText("explore-file-count", "(" + files.length + ")");

    if (files.length === 0) {
      container.innerHTML = '<p class="text-muted text-sm">No source file data.</p>';
      return;
    }

    var html = "";
    files.forEach(function (f) {
      html +=
        '<div class="file-card">' +
        '<div class="file-path">' + esc(f.path || "-") + '</div>' +
        '<div class="file-stats">' +
        '<span>Symbols: <strong>' + (f.symbolCount || 0) + '</strong></span>' +
        (f.riskHint ? '<span class="text-warning">' + esc(f.riskHint) + '</span>' : "") +
        "</div></div>";
    });
    container.innerHTML = html;
  }

  function renderTopFiles(exp) {
    var topFiles = exp.topFiles || [];
    var container = $("#top-files-list");

    if (topFiles.length === 0) {
      container.innerHTML = '<p class="text-muted text-sm">No top file ranking data.</p>';
      return;
    }

    var html = "";
    topFiles.forEach(function (f, i) {
      html +=
        '<div class="file-card">' +
        '<span class="rank-badge">' + (i + 1) + '</span>' +
        '<div class="file-path">' + esc(f.path || "-") + '</div>' +
        '<div class="file-stats">Symbols: <strong>' + (f.symbolCount || 0) + '</strong>' +
        (f.reason ? '<span class="text-sm text-muted"> — ' + esc(f.reason) + '</span>' : "") +
        "</div></div>";
    });
    container.innerHTML = html;
  }

  // ── Cleanup ─────────────────────────────────────────────────────────

  function renderCleanup() {
    var cleanup = currentSnapshot.cleanup || {};

    setInfoCard("cleanup-dead-code",
      cleanup.deadCodeCandidateCount != null ?
        "<strong>" + cleanup.deadCodeCandidateCount + "</strong> candidates" :
        '<span class="text-muted">not_collected</span>');

    setInfoCard("cleanup-reachability",
      cleanup.unreachableCandidateCount != null ?
        "<strong>" + cleanup.unreachableCandidateCount + "</strong> unreachable" :
        '<span class="text-muted">not_collected</span>');

    setInfoCard("cleanup-external-api",
      cleanup.externalApiSurfaceCount != null ?
        "<strong>" + cleanup.externalApiSurfaceCount + "</strong> exported symbols" :
        '<span class="text-muted">not_collected</span>');

    setInfoCard("cleanup-framework",
      cleanup.frameworkEntryHintCount != null ?
        "<strong>" + cleanup.frameworkEntryHintCount + "</strong> hints" :
        '<span class="text-muted">not_collected</span>');

    // Cautions
    var cautionList = $("#cleanup-caution-list");
    var cautions = cleanup.cautions || [
      "Dead-code detection is heuristic-based on call-graph shape.",
      "Candidates are NOT proven unused — they may be called via reflection/dynamic dispatch.",
      "Public/exported symbols may be used by external crates not analyzed here.",
      "Auto-deletion is explicitly forbidden without human review."
    ];
    cautionList.innerHTML = cautions.map(function (c) {
      return "<li>" + esc(c) + "</li>";
    }).join("");
  }

  // ── Release Review ──────────────────────────────────────────────────

  function renderReleaseReview() {
    var rr = currentSnapshot.releaseReview || {};

    var riskHtml = rr.breakingChangeRisk ?
      '<span class="badge ' +
        (rr.breakingChangeRisk === "high" ? "badge-danger" :
         rr.breakingChangeRisk === "medium" ? "badge-warning" : "badge-success") +
        '">' + esc(rr.breakingChangeRisk) + "</span>" +
        (rr.breakingChangeSurface ?
          " <span class='text-muted text-sm'>" + rr.breakingChangeSurface + " public symbols</span>" :
        "") :
      '<span class="text-muted">not_collected</span>';
    setInfoCard("release-breaking", riskHtml);

    setInfoCard("release-docs",
      rr.staleDocCandidateCount != null ?
        "<strong>" + rr.staleDocCandidateCount + "</strong> candidate docs to review" :
        '<span class="text-muted">not_collected</span>');

    setInfoCard("release-config-examples",
      rr.configExampleIssueCount != null ?
        "<strong>" + rr.configExampleIssueCount + "</strong> issues found" :
        '<span class="text-muted">not_collected</span>');

    // Release cautions
    var cautionList = $("#release-caution-list");
    var cautions = rr.cautions || [
      "Release review is based on static analysis only — does not run tests or verify docs accuracy.",
      "Breaking-change risk assessment is heuristic; actual impact depends on downstream usage.",
      "Documentation staleness requires manual review.",
    ];
    cautionList.innerHTML = cautions.map(function (c) {
      return "<li>" + esc(c) + "</li>";
    }).join("");
  }

  // ── Workflow Presets ────────────────────────────────────────────────

  function renderWorkflowPresets() {
    var wp = currentSnapshot.workflowPresets || {};
    var presets = wp.presets || [];

    var container = $("#workflow-presets-list");

    if (wp.status === "not_collected" || presets.length === 0) {
      container.innerHTML = '<p class="text-muted">No workflow presets collected in this snapshot. Generate with --include-workflows for full recommendations.</p>';
      return;
    }

    var html = "";
    presets.forEach(function (p, i) {
      var tools = (p.tools || []).map(function (t) {
        return badge(t, "badge-xs");
      }).join(" ");
      var stopLines = p.stopLines || [];

      html +=
        '<div class="workflow-card">' +
        '<div class="workflow-header">' +
        '<h4 class="workflow-name">' + esc(p.name) + "</h4>" +
        '<span class="workflow-id text-muted text-sm">' + esc(p.id) + "</span>" +
        "</div>" +
        '<p class="workflow-desc">' + esc(p.description) + "</p>" +
        '<div class="workflow-tools">Recommended tools: ' + tools + "</div>" +
        (stopLines.length > 0 ?
          '<div class="workflow-stop-lines"><strong>Stop-lines:</strong><ul>' +
            stopLines.map(function (sl) { return "<li>" + esc(sl) + "</li>"; }).join("") +
            "</ul></div>" : "") +
        "</div>";
    });
    container.innerHTML = html;
  }

  // ── Utilities ────────────────────────────────────────────────────────

  function setText(id, text) {
    var el = $("#" + id);
    if (el) el.textContent = typeof text === "undefined" ? "" : text;
  }

  function setInfoCard(id, html) {
    var el = $("#" + id);
    if (el) el.innerHTML = html || '<span class="text-muted">-</span>';
  }

  function row(label, value) {
    return "<tr><td class='detail-label'>" + esc(label) + "</td><td>" + value + "</td></tr>";
  }

})();
