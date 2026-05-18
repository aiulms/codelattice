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
  let graphState = { selectedNodeId: null, focusNodeId: null, depth: 1, edgeMode: "all", layout: "galaxy", engine: "g6", zoomLocked: true, spotlight: false, nodeById: {}, allEdges: [] };

  // ── DOM Helpers ─────────────────────────────────────────────────────

  function $(sel) { return document.querySelector(sel); }
  function $$(sel) { return Array.from(document.querySelectorAll(sel)); }

  function show(id) {
    $$(`.view-section`).forEach(function (el) { el.style.display = "none"; });
    var view = $("#view-" + id);
    if (view) view.style.display = "";
    document.body.setAttribute("data-active-tab", id || "");
    $$(".tab-btn").forEach(function (btn) {
      var active = btn.getAttribute("data-tab") === id;
      btn.classList.toggle("active", active);
      btn.setAttribute("aria-selected", active ? "true" : "false");
    });
  }

  function esc(text) {
    var d = document.createElement("div");
    d.textContent = text == null ? "" : String(text);
    return d.innerHTML;
  }

  function t(key, params) {
    return window.CTL_I18N ? CTL_I18N.t(key, params) : key;
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

  // Graph filter event listeners (Phase B)
  var gs = $("#graph-search"); if (gs) gs.addEventListener("input", function() { if (currentSnapshot) renderGraph(currentSnapshot); });
  var gkf = $("#graph-kind-filter"); if (gkf) gkf.addEventListener("change", function() { if (currentSnapshot) renderGraph(currentSnapshot); });
  var gem = $("#graph-engine-mode"); if (gem) gem.addEventListener("change", function() { window.setGraphEngine(gem.value); });

  // Diff file input (Phase B)
  var dfi = $("#diff-file-input");
  if (dfi) dfi.addEventListener("change", function(e) {
    if (e.target.files && e.target.files[0]) loadDiffSnapshot(e.target.files[0]);
  });

  // ── File Load ───────────────────────────────────────────────────────

  function loadSnapshot(jsonText) {
    try {
      currentSnapshot = JSON.parse(jsonText);
    } catch (e) {
      showError(t("error.invalidJson", {message: e.message}));
      return;
    }
    if (!currentSnapshot.schemaVersion) {
      showError(t("error.invalidSnapshot"));
      return;
    }
    renderAll();
    $("#loaded-content").style.display = "";
    $("#welcome-view").style.display = "none";
    $("#error-view").style.display = "none";
    document.body.classList.add("has-snapshot");
    document.body.classList.remove("is-welcome", "has-error");
    updateCautionBanner();
  }

  function showError(msg, detail) {
    $("#error-view").style.display = "";
    $("#welcome-view").style.display = "none";
    $("#loaded-content").style.display = "none";
    document.body.classList.add("has-error");
    document.body.classList.remove("is-welcome", "has-snapshot");
    $("#error-message").textContent = msg;
    if (detail) $("#error-detail").textContent = detail;
  }

  function showWelcome() {
    $("#welcome-view").style.display = "";
    $("#error-view").style.display = "none";
    $("#loaded-content").style.display = "none";
    document.body.classList.add("is-welcome");
    document.body.classList.remove("has-snapshot", "has-error");
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
    renderGraph(currentSnapshot);
    renderCleanup();
    renderReleaseReview();
    renderAutomationGraph();
    renderWorkflowPresets();
    renderDiffTab();
    if (CTL.renderTimeline) CTL.renderTimeline();
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
    setText("hero-project-language", s.language || "-");
    setText("hero-project-files", s.sourceFileCount != null ? s.sourceFileCount : "-");
    setText("hero-project-symbols", s.symbolCount != null ? s.symbolCount : "-");
    setText("hero-project-edges", s.edgeCount != null ? s.edgeCount : "-");
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
    setText("dash-gate-summary", t("dashboard.passedFailed", {passed: passed, failed: failed}));

    var gateList = $("#dash-quality-gates");
    gateList.innerHTML = "";
    var gates = q.gates || [];
    if (gates.length === 0) {
      gateList.innerHTML = '<p class="text-muted text-sm">' + esc(t("dashboard.noQuality")) + '</p>';
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
      '<div class="meta-item"><strong>' + esc(t("dashboard.tool")) + ':</strong> ' + esc(gf.tool || "-") + '</div>' +
      '<div class="meta-item"><strong>' + esc(t("dashboard.version")) + ':</strong> ' + esc(gf.toolVersion || "-") + '</div>' +
      '<div class="meta-item"><strong>' + esc(t("dashboard.schema")) + ':</strong> ' + esc(gf.snapshotSchema || currentSnapshot.schemaVersion || "-") + '</div>' +
      '<div class="meta-item"><strong>' + esc(t("dashboard.staticAnalysis")) + ':</strong> ' +
        (gf.staticAnalysis ? badge(t("common.yes"), "badge-success") : badge(t("common.no"))) + '</div>' +
      '<div class="meta-item"><strong>' + esc(t("dashboard.runtimeVerified")) + ':</strong> ' +
        (gf.runtimeVerified === false ?
          badge(t("common.false"), "badge-danger") + ' <span class="text-muted text-sm">— ' + esc(t("dashboard.heuristicOnly")) + '</span>' :
          badge(t("common.unknown"))) + '</div>';

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
          "<strong>" + esc(t("dashboard.runtimeNotVerified")) + "</strong> " + esc(t("dashboard.noCodeExecuted")) :
          "") + "</li>" +
        "<li><strong>" + esc(t("dashboard.noCoverage")) + "</strong> " + esc(t("dashboard.testsNotRun")) + "</li>" +
        "<li><strong>" + esc(t("dashboard.deletionNotVerified")) + "</strong> " + esc(t("dashboard.deadCodeManualReview")) + "</li>";
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
    var opts = '<option value="">' + esc(t("explore.allKinds")) + ' (' + allSymbols.length + ")</option>";
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
        " / " + allSymbols.length : "");

    if (filteredSymbols.length === 0) {
      list.innerHTML = '<p class="text-muted text-center text-sm">' + esc(t("explore.noMatch")) + '</p>';
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
      detail.innerHTML = '<p class="text-muted text-center">' + esc(t("explore.symbolNotFound")) + '</p>';
      return;
    }

    var html =
      '<h4>' + esc(sym.name || sym.id) + '</h4>' +
      '<table class="detail-table"><tbody>' +
      row(t("common.kind"), (sym.kindLabel || sym.kind || "-") +
        (sym.exported ? " " + badge("exported", "badge-success") : "")) +
      row(t("common.id"), '<code class="code-inline">' + esc(sym.id) + '</code>') +
      row(t("common.file"), esc(sym.file || "-")) +
      row(t("common.line"), sym.line != null ? sym.line + (sym.endLine ? "-" + sym.endLine : "") : "-") +
      row(t("common.visibility"), sym.visibility || "-") +
      "</tbody></table>";

    detail.innerHTML = html;
  }

  function renderSourceFiles(exp) {
    var files = exp.sourceFiles || [];
    var container = $("#explore-file-list");
    setText("explore-file-count", "(" + files.length + ")");

    if (files.length === 0) {
      container.innerHTML = '<p class="text-muted text-sm">' + esc(t("explore.noSourceFiles")) + '</p>';
      return;
    }

    var html = "";
    files.forEach(function (f) {
      html +=
        '<div class="file-card">' +
        '<div class="file-path">' + esc(f.path || "-") + '</div>' +
        '<div class="file-stats">' +
        '<span>' + esc(t("explore.symbolCount")) + ': <strong>' + (f.symbolCount || 0) + '</strong></span>' +
        (f.riskHint ? '<span class="text-warning">' + esc(f.riskHint) + '</span>' : "") +
        "</div></div>";
    });
    container.innerHTML = html;
  }

  function renderTopFiles(exp) {
    var topFiles = exp.topFiles || [];
    var container = $("#top-files-list");

    if (topFiles.length === 0) {
      container.innerHTML = '<p class="text-muted text-sm">' + esc(t("explore.noTopFiles")) + '</p>';
      return;
    }

    var html = "";
    topFiles.forEach(function (f, i) {
      html +=
        '<div class="file-card">' +
        '<span class="rank-badge">' + (i + 1) + '</span>' +
        '<div class="file-path">' + esc(f.path || "-") + '</div>' +
        '<div class="file-stats">' + esc(t("explore.symbolCount")) + ': <strong>' + (f.symbolCount || 0) + '</strong>' +
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
        "<strong>" + cleanup.deadCodeCandidateCount + "</strong> " + esc(t("cleanup.candidates")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    setInfoCard("cleanup-reachability",
      cleanup.unreachableCandidateCount != null ?
        "<strong>" + cleanup.unreachableCandidateCount + "</strong> " + esc(t("cleanup.unreachableCount")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    setInfoCard("cleanup-external-api",
      cleanup.externalApiSurfaceCount != null ?
        "<strong>" + cleanup.externalApiSurfaceCount + "</strong> " + esc(t("cleanup.exportedSymbols")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    setInfoCard("cleanup-framework",
      cleanup.frameworkEntryHintCount != null ?
        "<strong>" + cleanup.frameworkEntryHintCount + "</strong> " + esc(t("cleanup.hints")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    // Cautions
    var cautionList = $("#cleanup-caution-list");
    var cautions = cleanup.cautions || [
      t("cleanup.defaultCaution1"),
      t("cleanup.defaultCaution2"),
      t("cleanup.defaultCaution3"),
      t("cleanup.defaultCaution4")
    ];
    cautionList.innerHTML = cautions.map(function (c) {
      return "<li>" + esc(c) + "</li>";
    }).join("");
  }

  // ── Release Review ──────────────────────────────────────────────────

  function renderReleaseReview() {
    var rr = currentSnapshot.releaseReview || {};
    var ag = getAutomationGraph();

    var riskHtml = rr.breakingChangeRisk ?
      '<span class="badge ' +
        (rr.breakingChangeRisk === "high" ? "badge-danger" :
         rr.breakingChangeRisk === "medium" ? "badge-warning" : "badge-success") +
        '">' + esc(rr.breakingChangeRisk) + "</span>" +
        (rr.breakingChangeSurface ?
          " <span class='text-muted text-sm'>" + rr.breakingChangeSurface + esc(t("release.publicSymbols")) + "</span>" :
        "") :
      '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>';
    setInfoCard("release-breaking", riskHtml);

    setInfoCard("release-docs",
      rr.staleDocCandidateCount != null ?
        "<strong>" + rr.staleDocCandidateCount + "</strong> " + esc(t("release.docsToReview")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    setInfoCard("release-config-examples",
      rr.configExampleIssueCount != null ?
        "<strong>" + rr.configExampleIssueCount + "</strong> " + esc(t("release.issuesFound")) :
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>');

    var agSummary = ag.summary || {};
    setInfoCard("release-automation",
      ag.status === "not_collected" ?
        '<span class="text-muted">' + esc(t("common.notCollected")) + '</span>' :
        "<strong>" + (agSummary.workflowCount || 0) + "</strong> " + esc(t("automation.workflows")) +
        " · <strong>" + (agSummary.riskCount || 0) + "</strong> " + esc(t("automation.risks")));

    // Release cautions
    var cautionList = $("#release-caution-list");
    var cautions = rr.cautions || [
      t("release.defaultCaution1"),
      t("release.defaultCaution2"),
      t("release.defaultCaution3"),
    ];
    cautionList.innerHTML = cautions.map(function (c) {
      return "<li>" + esc(c) + "</li>";
    }).join("");
  }

  // ── Automation Graph Review ────────────────────────────────────────

  function getAutomationGraph() {
    if (!currentSnapshot) return { status: "not_collected" };
    return currentSnapshot.automationGraph ||
      (currentSnapshot.releaseReview && currentSnapshot.releaseReview.automationGraph) ||
      { status: "not_collected" };
  }

  function automationRiskClass(level) {
    level = String(level || "").toLowerCase();
    if (level === "high" || level === "critical") return "badge-danger";
    if (level === "medium") return "badge-warning";
    return "badge-info";
  }

  function renderAutomationGraph() {
    var panel = $("#automation-panel");
    if (!panel) return;
    var ag = getAutomationGraph();
    var summary = ag.summary || {};
    var risks = ag.riskFindings || ag.risks || [];
    var workflows = ag.workflows || [];

    if (ag.status === "not_collected") {
      panel.innerHTML =
        '<div class="automation-header">' +
          '<div><h3>' + esc(t("automation.title")) + '</h3>' +
          '<p class="text-muted">' + esc(t("automation.empty")) + '</p></div>' +
          '<span class="badge badge-info">' + esc(t("common.notCollected")) + '</span>' +
        '</div>';
      return;
    }

    var cards = [
      [t("automation.workflows"), summary.workflowCount || workflows.length || 0],
      [t("automation.steps"), summary.stepCount || 0],
      [t("automation.risks"), summary.riskCount || risks.length || 0],
      [t("automation.highRisk"), summary.highRiskCount || 0]
    ].map(function(card) {
      return '<div class="automation-stat"><span>' + esc(card[0]) + '</span><strong>' + esc(card[1]) + '</strong></div>';
    }).join("");

    var riskHtml = risks.length ?
      risks.slice(0, 8).map(function(r) {
        return '<div class="automation-risk-row">' +
          '<span class="badge ' + automationRiskClass(r.level || r.severity || r.risk) + '">' + esc(r.level || r.severity || r.risk || t("common.unknown")) + '</span>' +
          '<span><strong>' + esc(r.workflow || r.file || r.name || t("automation.riskItem")) + '</strong><br>' +
          '<small class="text-muted">' + esc(r.reason || r.message || r.hint || "") + '</small></span>' +
        '</div>';
      }).join("") :
      '<p class="text-muted">' + esc(t("automation.noRisks")) + '</p>';

    var workflowHtml = workflows.length ?
      workflows.slice(0, 6).map(function(w) {
        var steps = w.steps || w.stepCount || [];
        var stepCount = Array.isArray(steps) ? steps.length : steps;
        return '<div class="automation-workflow-row">' +
          '<span><strong>' + esc(w.name || w.id || t("automation.workflow")) + '</strong><br>' +
          '<small class="text-muted">' + esc(w.file || w.kind || w.trigger || "") + '</small></span>' +
          '<span class="badge badge-info">' + esc(stepCount || 0) + ' ' + esc(t("automation.steps")) + '</span>' +
        '</div>';
      }).join("") :
      '<p class="text-muted">' + esc(t("automation.noWorkflows")) + '</p>';

    panel.innerHTML =
      '<div class="automation-header">' +
        '<div><h3>' + esc(t("automation.title")) + '</h3>' +
        '<p class="text-muted">' + esc(t("automation.subtitle")) + '</p></div>' +
        '<span class="badge badge-info">' + esc(t("automation.staticOnly")) + '</span>' +
      '</div>' +
      '<div class="automation-stats">' + cards + '</div>' +
      '<div class="automation-columns">' +
        '<div><h4>' + esc(t("automation.riskFindings")) + '</h4>' + riskHtml + '</div>' +
        '<div><h4>' + esc(t("automation.workflows")) + '</h4>' + workflowHtml + '</div>' +
      '</div>';
  }

  // ── Workflow Presets ────────────────────────────────────────────────

  function renderWorkflowPresets() {
    // Phase C: delegate to interactive checklist
    if (typeof CTL !== 'undefined' && CTL.renderWorkflowChecklist) {
      CTL.renderWorkflowChecklist();
      return;
    }
    var wp = currentSnapshot.workflowPresets || {};
    var presets = wp.presets || [];

    var container = $("#workflow-presets-list");

    if (wp.status === "not_collected" || presets.length === 0) {
      container.innerHTML = '<p class="text-muted">' + esc(t("common.notCollected")) + '</p>';
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
        '<div class="workflow-tools">' + tools + "</div>" +
        (stopLines.length > 0 ?
          '<div class="workflow-stop-lines"><strong>Stop-lines:</strong><ul>' +
            stopLines.map(function (sl) { return "<li>" + esc(sl) + "</li>"; }).join("") +
            "</ul></div>" : "") +
        "</div>";
    });
    container.innerHTML = html;
  }

  // ── Graph View (Phase B) ──────────────────────────────────────────────

  function graphEdgeVisible(edge) {
    if (graphState.edgeMode === "calls") return edge.kind === "calls";
    if (graphState.edgeMode === "structure") return edge.kind !== "calls";
    return true;
  }

  function graphNeighborIds(edges, nodeId, depth) {
    var seen = new Set([nodeId]);
    var frontier = new Set([nodeId]);
    for (var d = 0; d < depth; d++) {
      var next = new Set();
      edges.forEach(function(e) {
        if (!graphEdgeVisible(e)) return;
        if (frontier.has(e.source) && !seen.has(e.target)) next.add(e.target);
        if (frontier.has(e.target) && !seen.has(e.source)) next.add(e.source);
      });
      next.forEach(function(id) { seen.add(id); });
      frontier = next;
      if (frontier.size === 0) break;
    }
    return seen;
  }

  function renderGraph(data) {
    var g = data.graph || {};
    if (g.status !== "collected" || !g.nodes || g.nodes.length === 0) {
      $("#graph-summary-text").textContent = g.status === "not_collected" ? t("graph.notCollected") : t("graph.noNodes");
      $("#graph-visual").innerHTML = '<div class="graph-visual-empty">' + esc(t("graph.empty")) + '</div>';
      $("#graph-node-list").innerHTML = '<div class="text-muted" style="padding:24px;text-align:center;">' + esc(t("graph.empty")) + '</div>';
      $("#graph-edge-list").innerHTML = "";
      $("#graph-node-count").textContent = "(0)";
      $("#graph-edge-count").textContent = "(0)";
      return;
    }
    var nodes = g.nodes || [], allEdges = (g.edges || []).filter(graphEdgeVisible);
    var nodeById = {};
    nodes.forEach(function(n) { nodeById[n.id] = n; });
    graphState.nodeById = nodeById;
    graphState.allEdges = allEdges;

    $("#graph-summary-text").textContent = t("graph.summary", {nodes: g.summary.nodeCount, edges: g.summary.edgeCount, calls: g.summary.callEdgeCount});

    var search = ($("#graph-search").value || "").toLowerCase();
    var kindFilter = $("#graph-kind-filter").value;
    var focusIds = graphState.focusNodeId ? graphNeighborIds(allEdges, graphState.focusNodeId, graphState.depth) : null;

    var filtered = nodes.filter(function(n) {
      var nameMatch = !search || (n.label || "").toLowerCase().indexOf(search) >= 0 || (n.file || "").toLowerCase().indexOf(search) >= 0;
      var kindMatch = !kindFilter || n.kind === kindFilter;
      var focusMatch = !focusIds || focusIds.has(n.id);
      return nameMatch && kindMatch && focusMatch;
    });
    if (focusIds && graphState.focusNodeId && !filtered.some(function(n) { return n.id === graphState.focusNodeId; }) && nodeById[graphState.focusNodeId]) {
      filtered.unshift(nodeById[graphState.focusNodeId]);
    }

    var visibleIds = new Set(filtered.map(function(n) { return n.id; }));
    var filteredEdges = allEdges.filter(function(e) {
      return visibleIds.has(e.source) && visibleIds.has(e.target);
    });

    $("#graph-node-count").textContent = "(" + filtered.length + ")";
    $("#graph-edge-count").textContent = "(" + filteredEdges.length + ")";
    var focusStatus = $("#graph-focus-status");
    if (focusStatus) {
      var focusNode = graphState.focusNodeId ? nodeById[graphState.focusNodeId] : null;
      focusStatus.textContent = focusNode ? t("graph.focused", {name: focusNode.label || focusNode.id, depth: graphState.depth}) : t("graph.clickToDrill");
    }

    var kindColor = {symbol: "#2563eb", file: "#059669", package: "#d97706", entry: "#7c3aed", risk: "#dc2626"};
    $("#graph-node-list").innerHTML = filtered.map(function(n, i) {
      var c = kindColor[n.kind] || "#6b7280";
      var active = n.id === graphState.selectedNodeId ? ' style="cursor:pointer;background:#eff6ff;"' : ' style="cursor:pointer;"';
      return '<div class="symbol-item graph-node" data-node-idx="' + i + '" onclick="selectGraphNode(&quot;' + escAttr(n.id) + '&quot;)" ondblclick="focusGraphNode(&quot;' + escAttr(n.id) + '&quot;)"' + active + '>' +
        '<span style="display:inline-block;width:10px;height:10px;border-radius:50%;background:' + c + ';margin-right:6px;"></span>' +
        '<span class="sym-name">' + esc(n.label) + '</span>' +
        '<span class="sym-meta">' + esc(t("graph." + n.kind) || n.kind) + (n.file ? ' · ' + n.file.split('/').pop() : '') + '</span></div>';
    }).join("") || '<div class="text-muted" style="padding:24px;text-align:center;">' + esc(t("graph.noMatchingNodes")) + '</div>';

    renderGraphVisual(filtered, filteredEdges, nodes, allEdges);

    var eKindColor = {calls: "#f97316", defines: "#2563eb", imports: "#059669", owns: "#64748b", related: "#6b7280"};
    $("#graph-edge-list").innerHTML = filteredEdges.slice(0, 140).map(function(e) {
      var src = nodeById[e.source];
      var tgt = nodeById[e.target];
      var c = eKindColor[e.kind] || "#6b7280";
      var conf = e.confidence ? " · " + Number(e.confidence).toFixed(2) : "";
      var srcName = src ? src.label : e.source.slice(-30);
      var tgtName = tgt ? tgt.label : e.target.slice(-30);
      var active = graphState.selectedNodeId && (e.source === graphState.selectedNodeId || e.target === graphState.selectedNodeId);
      return '<div class="symbol-item" style="font-size:0.82em;' + (active ? 'background:#fff7ed;' : '') + '">' +
        '<span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:' + c + ';margin-right:4px;"></span>' +
        '<span>' + esc(srcName) + ' → ' + esc(tgtName) + '</span>' +
        '<span class="sym-meta">' + esc(t("graph.edge." + e.kind) || e.kind) + conf + '</span></div>';
    }).join("") || '<div class="text-muted" style="padding:24px;text-align:center;">' + esc(t("graph.noMatchingEdges")) + '</div>';

    if (graphState.selectedNodeId && nodeById[graphState.selectedNodeId]) {
      renderGraphNodeDetail(graphState.selectedNodeId, nodeById, allEdges);
    } else {
      $("#graph-selected-detail").style.display = "none";
    }
  }

  function renderGraphVisual(filteredNodes, filteredEdges, allNodes, allEdges) {
    var host = $("#graph-visual");
    if (!host) return;
    var layout = graphState.layout || "galaxy";
    updateGraphLayoutButtons();
    updateGraphZoomLockButton();
    if (graphState.engine !== "svg" && window.CodeLatticeG6Graph && CodeLatticeG6Graph.available()) {
      var usedG6 = CodeLatticeG6Graph.render({
        host: host,
        nodes: filteredNodes,
        edges: filteredEdges,
        allNodes: allNodes,
        allEdges: allEdges,
        layout: layout,
        selectedNodeId: graphState.selectedNodeId,
        focusNodeId: graphState.focusNodeId,
        depth: graphState.depth,
        zoomLocked: graphState.zoomLocked,
        onSelect: window.selectGraphNode,
        onFocus: window.focusGraphNode,
        onHover: window.showGraphNodeHover,
        onHoverEnd: window.hideGraphNodeHover
      });
      if (usedG6) {
        renderGraphShowcaseChrome(host, filteredNodes, filteredEdges, layout);
        return;
      }
    } else if (window.CodeLatticeG6Graph) {
      CodeLatticeG6Graph.destroy();
    }
    host.className = "graph-visual graph-layout-" + layout;
    var priority = {package: 0, file: 1, entry: 2, risk: 3, symbol: 4};
    var degree = {};
    (filteredEdges || []).forEach(function(e) {
      degree[e.source] = (degree[e.source] || 0) + 1;
      degree[e.target] = (degree[e.target] || 0) + 1;
    });
    var nodes = filteredNodes.slice().sort(function(a, b) {
      var pa = priority[a.kind] || 9, pb = priority[b.kind] || 9;
      if (pa !== pb) return pa - pb;
      return (degree[b.id] || 0) - (degree[a.id] || 0);
    }).slice(0, graphState.focusNodeId ? 150 : (layout === "galaxy" ? 150 : 120));
    if (nodes.length === 0) {
      host.innerHTML = '<div class="graph-visual-empty">' + esc(t("graph.noMatchingNodes")) + '</div>';
      return;
    }
    var visible = new Set(nodes.map(function(n) { return n.id; }));
    var edges = filteredEdges.filter(function(e) { return visible.has(e.source) && visible.has(e.target); }).slice(0, graphState.focusNodeId ? 280 : (layout === "galaxy" ? 260 : 190));
    var selectedNeighbors = graphState.selectedNodeId ? graphNeighborIds(allEdges || edges, graphState.selectedNodeId, 1) : new Set();

    var w = Math.max(920, host.clientWidth || 1040);
    var h = Math.max(540, host.clientHeight || 600);
    var cx = w / 2, cy = h / 2;
    var byId = {};
    nodes.forEach(function(n) { byId[n.id] = n; });

    var fileForSymbol = {};
    edges.forEach(function(e) {
      var s = byId[e.source], tgt = byId[e.target];
      if (!s || !tgt) return;
      if (e.kind === "defines" && s.kind === "file" && tgt.kind === "symbol") fileForSymbol[tgt.id] = s.id;
      if (e.kind === "defines" && s.kind === "symbol" && tgt.kind === "file") fileForSymbol[s.id] = tgt.id;
    });

    var packages = nodes.filter(function(n) { return n.kind === "package"; });
    var files = nodes.filter(function(n) { return n.kind === "file"; });
    var symbols = nodes.filter(function(n) { return n.kind === "symbol"; });
    var others = nodes.filter(function(n) { return n.kind !== "package" && n.kind !== "file" && n.kind !== "symbol"; });
    var pos = {};
    var symbolsByFile = {};
    symbols.forEach(function(n) {
      var fid = fileForSymbol[n.id] || "_orphan";
      (symbolsByFile[fid] = symbolsByFile[fid] || []).push(n);
    });

    function clamp(v, min, max) { return Math.max(min, Math.min(max, v)); }
    function placeRing(list, rx, ry, phase) {
      list.forEach(function(n, i) {
        var angle = (Math.PI * 2 * i / Math.max(1, list.length)) + (phase || 0);
        pos[n.id] = {x: clamp(cx + Math.cos(angle) * rx, 36, w - 36), y: clamp(cy + Math.sin(angle) * ry, 34, h - 34)};
      });
    }
    function placeOrbit() {
      placeRing(packages, packages.length <= 1 ? 0 : 50, packages.length <= 1 ? 0 : 50, -Math.PI / 2);
      placeRing(files, graphState.focusNodeId ? Math.min(w * 0.34, 310) : Math.min(w * 0.32, 290), graphState.focusNodeId ? Math.min(h * 0.30, 205) : Math.min(h * 0.31, 190), -Math.PI / 2);
      Object.keys(symbolsByFile).forEach(function(fid, groupIdx) {
        var group = symbolsByFile[fid];
        var anchor = pos[fid] || {x: cx + Math.cos(groupIdx) * w * 0.20, y: cy + Math.sin(groupIdx) * h * 0.20};
        group.forEach(function(n, i) {
          var angle = Math.PI * 2 * i / Math.max(1, group.length);
          var ring = graphState.focusNodeId ? 64 + Math.min(95, group.length * 2.6) : 38 + Math.min(64, group.length * 1.65);
          pos[n.id] = {x: clamp(anchor.x + Math.cos(angle) * ring, 30, w - 30), y: clamp(anchor.y + Math.sin(angle) * ring, 28, h - 28)};
        });
      });
      placeRing(others, w * 0.38, h * 0.36, 0);
    }
    function topFiles(limit) {
      return files.slice().sort(function(a, b) {
        return (symbolsByFile[b.id] || []).length - (symbolsByFile[a.id] || []).length || (degree[b.id] || 0) - (degree[a.id] || 0);
      }).slice(0, limit);
    }
    function placeGalaxy() {
      packages.forEach(function(n, i) {
        var angle = Math.PI * 2 * i / Math.max(1, packages.length);
        pos[n.id] = {x: cx + Math.cos(angle) * 34, y: cy + Math.sin(angle) * 28};
      });
      var hubs = topFiles(Math.min(18, Math.max(8, files.length)));
      var hubSet = new Set(hubs.map(function(n) { return n.id; }));
      hubs.forEach(function(n, i) {
        var angle = (Math.PI * 2 * i / Math.max(1, hubs.length)) - Math.PI / 2;
        var rx = Math.min(w * 0.39, 420), ry = Math.min(h * 0.35, 240);
        pos[n.id] = {x: clamp(cx + Math.cos(angle) * rx, 68, w - 68), y: clamp(cy + Math.sin(angle) * ry, 58, h - 58)};
      });
      files.filter(function(n) { return !hubSet.has(n.id); }).forEach(function(n, i) {
        var angle = (Math.PI * 2 * i / Math.max(1, files.length)) + Math.PI / 10;
        pos[n.id] = {x: clamp(cx + Math.cos(angle) * w * 0.45, 42, w - 42), y: clamp(cy + Math.sin(angle) * h * 0.39, 36, h - 36)};
      });
      Object.keys(symbolsByFile).forEach(function(fid, groupIdx) {
        var group = symbolsByFile[fid].slice().sort(function(a, b) { return (degree[b.id] || 0) - (degree[a.id] || 0); });
        var anchor = pos[fid] || {x: cx + Math.cos(groupIdx) * w * 0.30, y: cy + Math.sin(groupIdx) * h * 0.26};
        group.forEach(function(n, i) {
          var angle = (Math.PI * 2 * i / Math.max(1, group.length)) + groupIdx * 0.37;
          var ring = 34 + Math.min(118, Math.sqrt(group.length) * 14 + i * 0.9);
          pos[n.id] = {x: clamp(anchor.x + Math.cos(angle) * ring, 24, w - 24), y: clamp(anchor.y + Math.sin(angle) * ring, 24, h - 24)};
        });
      });
      placeRing(others, w * 0.44, h * 0.40, Math.PI / 5);
    }
    function placeCommunities() {
      var hubs = topFiles(Math.min(10, Math.max(4, files.length)));
      var cols = Math.ceil(Math.sqrt(Math.max(1, hubs.length)));
      hubs.forEach(function(n, i) {
        var col = i % cols, row = Math.floor(i / cols);
        var x = w * (0.16 + (col / Math.max(1, cols - 1)) * 0.68);
        var y = h * (0.22 + (row / Math.max(1, Math.ceil(hubs.length / cols) - 1)) * 0.56);
        pos[n.id] = {x: clamp(x, 70, w - 70), y: clamp(y, 60, h - 60)};
      });
      packages.forEach(function(n, i) { pos[n.id] = {x: cx + (i - packages.length / 2) * 34, y: 58}; });
      files.filter(function(n) { return !pos[n.id]; }).forEach(function(n, i) {
        var angle = Math.PI * 2 * i / Math.max(1, files.length);
        pos[n.id] = {x: clamp(cx + Math.cos(angle) * w * 0.44, 42, w - 42), y: clamp(cy + Math.sin(angle) * h * 0.38, 42, h - 42)};
      });
      Object.keys(symbolsByFile).forEach(function(fid, groupIdx) {
        var anchor = pos[fid] || {x: cx, y: cy};
        var group = symbolsByFile[fid];
        group.forEach(function(n, i) {
          var angle = (Math.PI * 2 * i / Math.max(1, group.length)) + (groupIdx * 0.19);
          var ring = 40 + Math.min(88, group.length * 2.1);
          pos[n.id] = {x: clamp(anchor.x + Math.cos(angle) * ring, 24, w - 24), y: clamp(anchor.y + Math.sin(angle) * ring, 24, h - 24)};
        });
      });
      placeRing(others, w * 0.42, h * 0.34, 0);
    }
    function distributeLayer(list, x, top, bottom) {
      list.forEach(function(n, i) {
        var t = list.length <= 1 ? 0.5 : i / (list.length - 1);
        pos[n.id] = {x: x, y: top + t * (bottom - top)};
      });
    }
    function placeFlow() {
      distributeLayer(packages, w * 0.10, h * 0.18, h * 0.82);
      distributeLayer(files, w * 0.38, h * 0.10, h * 0.90);
      var grouped = [];
      Object.keys(symbolsByFile).forEach(function(fid) { grouped = grouped.concat(symbolsByFile[fid]); });
      distributeLayer(grouped, w * 0.72, h * 0.08, h * 0.92);
      distributeLayer(others, w * 0.90, h * 0.18, h * 0.82);
    }
    function placeBlueprint() {
      placeFlow();
      Object.keys(pos).forEach(function(id, i) {
        var p = pos[id];
        p.x = clamp(p.x + ((i % 3) - 1) * 18, 24, w - 24);
        p.y = clamp(p.y + ((i % 5) - 2) * 7, 24, h - 24);
      });
    }
    function placeHeatmap() {
      packages.forEach(function(n, i) { pos[n.id] = {x: cx + (i - packages.length / 2) * 42, y: 54}; });
      var cols = Math.max(4, Math.ceil(Math.sqrt(Math.max(1, files.length))));
      files.forEach(function(n, i) {
        var col = i % cols, row = Math.floor(i / cols);
        var rows = Math.ceil(files.length / cols);
        var x = w * (0.10 + (col / Math.max(1, cols - 1)) * 0.80);
        var y = h * (0.18 + (row / Math.max(1, rows - 1)) * 0.66);
        pos[n.id] = {x: clamp(x, 56, w - 56), y: clamp(y, 66, h - 46)};
      });
      Object.keys(symbolsByFile).forEach(function(fid, groupIdx) {
        var group = symbolsByFile[fid];
        var anchor = pos[fid] || {x: cx, y: cy};
        var cols2 = Math.max(3, Math.ceil(Math.sqrt(Math.max(1, group.length))));
        group.forEach(function(n, i) {
          var col = i % cols2, row = Math.floor(i / cols2);
          var spread = 18 + Math.min(42, group.length);
          pos[n.id] = {
            x: clamp(anchor.x + (col - (cols2 - 1) / 2) * spread, 24, w - 24),
            y: clamp(anchor.y + 36 + row * Math.min(24, spread), 24, h - 24)
          };
        });
      });
      placeRing(others, w * 0.42, h * 0.36, Math.PI / 4);
    }
    if (layout === "orbit") placeOrbit();
    else if (layout === "communities") placeCommunities();
    else if (layout === "flow") placeFlow();
    else if (layout === "blueprint") placeBlueprint();
    else if (layout === "heatmap") placeHeatmap();
    else placeGalaxy();

    var palettes = {
      galaxy: {symbol:"#6ee7f9", file:"#8b5cf6", package:"#f97316", entry:"#22c55e", risk:"#ef4444", bgText:"#0f172a", edge:"#38bdf8"},
      communities: {symbol:"#86efac", file:"#60a5fa", package:"#facc15", entry:"#c084fc", risk:"#fb7185", bgText:"#111827", edge:"#22c55e"},
      flow: {symbol:"#bfdbfe", file:"#fde68a", package:"#a7f3d0", entry:"#ddd6fe", risk:"#fecaca", bgText:"#1f2937", edge:"#2563eb"},
      blueprint: {symbol:"#fde047", file:"#93c5fd", package:"#fb7185", entry:"#4ade80", risk:"#f97316", bgText:"#e0f2fe", edge:"#facc15"},
      heatmap: {symbol:"#e0f2fe", file:"#22d3ee", package:"#f59e0b", entry:"#a78bfa", risk:"#fb7185", bgText:"#e0f2fe", edge:"#67e8f9"},
      orbit: {symbol:"#bfdbfe", file:"#bbf7d0", package:"#fed7aa", entry:"#ddd6fe", risk:"#fecaca", bgText:"#1f2937", edge:"#3b82f6"}
    };
    var color = palettes[layout] || palettes.galaxy;
    var strokeColor = {symbol:"#2563eb", file:"#059669", package:"#d97706", entry:"#7c3aed", risk:"#dc2626"};
    if (layout === "blueprint") strokeColor = {symbol:"#fef08a", file:"#bfdbfe", package:"#fb7185", entry:"#86efac", risk:"#fdba74"};
    function radiusFor(n) {
      var d = degree[n.id] || 0;
      var base = n.kind === "package" ? 24 : n.kind === "file" ? 14 : n.kind === "symbol" ? 6 : 10;
      if (layout === "galaxy" || layout === "communities") base += Math.min(18, Math.sqrt(d) * 3.6);
      if (layout === "flow" || layout === "blueprint") base += Math.min(8, Math.sqrt(d) * 1.8);
      return base;
    }
    function labelText(n) { var s = n.label || n.id || ""; return s.length > 24 ? s.slice(0, 22) + "…" : s; }
    var grid = "";
    if (layout === "orbit" || layout === "flow" || layout === "blueprint" || layout === "heatmap") {
      for (var gx = 80; gx < w; gx += 120) grid += '<line class="graph-grid-line" x1="' + gx + '" y1="0" x2="' + gx + '" y2="' + h + '"></line>';
      for (var gy = 80; gy < h; gy += 120) grid += '<line class="graph-grid-line" x1="0" y1="' + gy + '" x2="' + w + '" y2="' + gy + '"></line>';
    }
    var backdrop = "";
    if (layout === "galaxy" || layout === "communities") {
      backdrop += '<circle class="graph-backdrop-orb" cx="' + cx + '" cy="' + cy + '" r="' + Math.min(w, h) * 0.36 + '"></circle>';
    }
    if (layout === "blueprint") {
      backdrop += '<rect class="graph-blueprint-vignette" x="0" y="0" width="' + w + '" height="' + h + '"></rect>';
    }

    var edgeHtml = edges.map(function(e) {
      var a = pos[e.source], b = pos[e.target];
      if (!a || !b) return "";
      var highlight = graphState.selectedNodeId && (e.source === graphState.selectedNodeId || e.target === graphState.selectedNodeId);
      var dim = graphState.selectedNodeId && !highlight;
      var d;
      if (layout === "flow" || layout === "blueprint") {
        var mid = Math.max(50, Math.abs(b.x - a.x) * 0.52);
        d = 'M ' + a.x.toFixed(1) + ' ' + a.y.toFixed(1) + ' C ' + (a.x + mid).toFixed(1) + ' ' + a.y.toFixed(1) + ', ' + (b.x - mid).toFixed(1) + ' ' + b.y.toFixed(1) + ', ' + b.x.toFixed(1) + ' ' + b.y.toFixed(1);
      } else if (layout === "galaxy" || layout === "communities") {
        var mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
        var bend = ((a.x - cx) * (b.y - cy) - (a.y - cy) * (b.x - cx)) > 0 ? 26 : -26;
        d = 'M ' + a.x.toFixed(1) + ' ' + a.y.toFixed(1) + ' Q ' + (mx + bend).toFixed(1) + ' ' + (my - bend).toFixed(1) + ' ' + b.x.toFixed(1) + ' ' + b.y.toFixed(1);
      } else {
        d = 'M ' + a.x.toFixed(1) + ' ' + a.y.toFixed(1) + ' L ' + b.x.toFixed(1) + ' ' + b.y.toFixed(1);
      }
      return '<path class="graph-edge-line ' + escAttr(e.kind || "related") + (highlight ? ' highlight' : '') + (dim ? ' dimmed' : '') + '" data-source="' + escAttr(e.source) + '" data-target="' + escAttr(e.target) + '" d="' + d + '"><title>' + esc(e.kind || "related") + '</title></path>';
    }).join("");
    var nodeHtml = nodes.map(function(n) {
      var p = pos[n.id] || {x: cx, y: cy};
      var r = radiusFor(n);
      var dy = n.kind === "package" ? -22 : 18;
      var selected = n.id === graphState.selectedNodeId;
      var neighbor = !selected && selectedNeighbors.has(n.id);
      var dim = graphState.selectedNodeId && !selected && !neighbor;
      var big = r >= 14 || (degree[n.id] || 0) >= 4;
      var showFileLabel = layout === "orbit" || layout === "flow" || layout === "heatmap";
      var posterLabel = (layout === "galaxy" || layout === "communities") && big;
      var blueprintLabel = layout === "blueprint" && n.kind !== "file" && ((degree[n.id] || 0) >= 2 || big);
      var showLabel = selected || neighbor || n.kind === "package" || graphState.focusNodeId || (showFileLabel && n.kind === "file") || posterLabel || blueprintLabel;
      return '<g class="graph-node-g ' + (selected ? 'selected ' : '') + (neighbor ? 'neighbor ' : '') + (dim ? 'dimmed' : '') + '" onclick="selectGraphNode(&quot;' + escAttr(n.id) + '&quot;)" ondblclick="focusGraphNode(&quot;' + escAttr(n.id) + '&quot;)" onmouseenter="showGraphNodeHover(&quot;' + escAttr(n.id) + '&quot;, event)" onmouseleave="hideGraphNodeHover()">' +
        '<circle class="graph-node-circle" cx="' + p.x.toFixed(1) + '" cy="' + p.y.toFixed(1) + '" r="' + r.toFixed(1) + '" fill="' + (color[n.kind] || "#e5e7eb") + '" stroke="' + (strokeColor[n.kind] || "#64748b") + '"><title>' + esc(n.label || n.id) + '</title></circle>' +
        '<circle class="graph-node-core" cx="' + p.x.toFixed(1) + '" cy="' + p.y.toFixed(1) + '" r="' + Math.max(2.1, Math.min(4.2, r * 0.22)).toFixed(1) + '"></circle>' +
        '<text class="graph-node-label ' + escAttr(n.kind || "") + (big ? ' big' : '') + (showLabel ? '' : ' hidden') + '" x="' + p.x.toFixed(1) + '" y="' + (p.y + dy).toFixed(1) + '" text-anchor="middle">' + esc(labelText(n)) + '</text>' +
        '</g>';
    }).join("");
    var legend = '<div class="graph-legend"><span><i style="background:' + color.file + '"></i>' + esc(t("graph.file")) + '</span><span><i style="background:' + color.symbol + '"></i>' + esc(t("graph.symbol")) + '</span><span><i style="background:' + color.package + '"></i>' + esc(t("graph.package")) + '</span><span style="color:' + color.edge + ';">─ ' + esc(t("graph.edge.calls")) + '</span></div>';
    host.innerHTML = '<svg viewBox="0 0 ' + w + ' ' + h + '" role="img" aria-label="' + escAttr(t("graph.visual")) + '">' + backdrop + grid + edgeHtml + nodeHtml + '</svg>' + legend;
    renderGraphShowcaseChrome(host, nodes, edges, layout);
  }

  function renderGraphShowcaseChrome(host, nodes, edges, layout) {
    if (!host) return;
    var oldOverlay = host.querySelector(".graph-showcase-overlay");
    var oldHover = host.querySelector(".graph-hover-card");
    if (oldOverlay) oldOverlay.remove();
    if (oldHover) oldHover.remove();
    var callCount = (edges || []).filter(function(e) { return e.kind === "calls"; }).length;
    var fileCount = (nodes || []).filter(function(n) { return n.kind === "file"; }).length;
    var symbolCount = (nodes || []).filter(function(n) { return n.kind === "symbol"; }).length;
    var overlay = document.createElement("div");
    overlay.className = "graph-showcase-overlay";
    overlay.innerHTML =
      '<div><strong>CodeLattice</strong><span>' + esc(t("graph.layout" + layout.charAt(0).toUpperCase() + layout.slice(1)) || layout) + '</span></div>' +
      '<div class="graph-showcase-metrics">' +
        '<span>' + nodes.length + ' nodes</span>' +
        '<span>' + edges.length + ' edges</span>' +
        '<span>' + callCount + ' calls</span>' +
        '<span>' + fileCount + ' files</span>' +
        '<span>' + symbolCount + ' symbols</span>' +
      '</div>';
    host.appendChild(overlay);
    var hover = document.createElement("div");
    hover.className = "graph-hover-card";
    hover.id = "graph-hover-card";
    hover.style.display = "none";
    host.appendChild(hover);
  }

  window.showGraphNodeHover = function(nodeId, event) {
    var node = graphState.nodeById && graphState.nodeById[nodeId];
    var host = $("#graph-visual");
    if (!node || !host) return;
    var card = $("#graph-hover-card");
    if (!card) {
      renderGraphShowcaseChrome(host, Object.values(graphState.nodeById || {}), graphState.allEdges || [], graphState.layout || "galaxy");
      card = $("#graph-hover-card");
    }
    if (!card) return;
    var edges = (graphState.allEdges || []).filter(function(e) { return e.source === nodeId || e.target === nodeId; });
    var calls = edges.filter(function(e) { return e.kind === "calls"; }).length;
    card.innerHTML =
      '<strong>' + esc(node.label || node.id) + '</strong>' +
      '<div class="graph-hover-meta">' + esc(t("graph." + node.kind) || node.kind) +
        (node.file ? ' · ' + esc(node.file.split('/').pop()) : '') + '</div>' +
      '<div class="graph-hover-stats"><span>' + esc(t("graph.hoverDegree")) + ': ' + edges.length + '</span><span>' + esc(t("graph.edge.calls")) + ': ' + calls + '</span></div>' +
      '<div class="graph-hover-hint">' + esc(t("graph.hoverHint")) + '</div>';
    var rect = host.getBoundingClientRect();
    var x = event && event.clientX ? event.clientX - rect.left + 18 : rect.width - 280;
    var y = event && event.clientY ? event.clientY - rect.top + 18 : 72;
    card.style.left = Math.max(12, Math.min(rect.width - 290, x)) + "px";
    card.style.top = Math.max(12, Math.min(rect.height - 140, y)) + "px";
    card.style.display = "";
  };

  window.hideGraphNodeHover = function() {
    var card = $("#graph-hover-card");
    if (card) card.style.display = "none";
  };

  function renderGraphNodeDetail(nodeId, nodeById, allEdges) {
    var node = nodeById[nodeId];
    if (!node) return;
    var kindColor = {symbol: "#2563eb", file: "#059669", package: "#d97706", entry: "#7c3aed", risk: "#dc2626"};
    var c = kindColor[node.kind] || "#6b7280";
    var edges = (allEdges || []).filter(function(e) { return e.source === nodeId || e.target === nodeId; });
    var neighborIds = graphNeighborIds(allEdges || [], nodeId, 1);
    var html = '<h4 style="margin-bottom:8px;">' +
      '<span style="display:inline-block;width:12px;height:12px;border-radius:50%;background:' + c + ';margin-right:6px;"></span>' +
      esc(node.label) + ' <span class="badge badge-info">' + esc(node.kind) + '</span></h4>';
    html += '<table class="detail-table">';
    if (node.file) html += '<tr><td class="detail-label">' + esc(t("common.file")) + '</td><td>' + esc(node.file) + '</td></tr>';
    if (node.line) html += '<tr><td class="detail-label">' + esc(t("common.line")) + '</td><td>' + node.line + '</td></tr>';
    if (node.visibility) html += '<tr><td class="detail-label">' + esc(t("common.visibility")) + '</td><td>' + esc(node.visibility) + '</td></tr>';
    html += '<tr><td class="detail-label">' + esc(t("graph.connectedEdges")) + '</td><td>' + edges.length + '</td></tr>';
    html += '<tr><td class="detail-label">' + esc(t("graph.neighborCount")) + '</td><td>' + Math.max(0, neighborIds.size - 1) + '</td></tr>';
    html += '</table><div class="graph-detail-actions"><button class="btn btn-sm btn-primary" onclick="focusGraphNode(&quot;' + escAttr(node.id) + '&quot;)">' + esc(t("graph.drill")) + '</button><button class="btn btn-sm btn-secondary" onclick="resetGraphFocus()">' + esc(t("graph.resetFocus")) + '</button></div>';
    function relationRows(title, list, outgoing) {
      var rows = list.slice(0, 8).map(function(e) {
        var otherId = outgoing ? e.target : e.source;
        var other = nodeById[otherId];
        var label = other ? other.label : otherId;
        var kind = e.kind || "related";
        var conf = e.confidence ? " · " + Number(e.confidence).toFixed(2) : "";
        return '<button class="graph-relation-row" onclick="selectGraphNode(&quot;' + escAttr(otherId) + '&quot;)">' +
          '<span class="graph-relation-main">' + esc(label || otherId) + '</span>' +
          '<span class="graph-relation-meta">' + esc(t("graph.edge." + kind) || kind) + conf + '</span>' +
          '<strong>' + esc(t("graph.viewNeighbor")) + '</strong>' +
        '</button>';
      }).join("");
      return '<div class="graph-relation-section"><h5>' + esc(title) + ' <span class="text-muted text-sm">(' + list.length + ')</span></h5>' +
        (rows || '<div class="text-muted text-sm">' + esc(t("graph.noRelations")) + '</div>') + '</div>';
    }
    html += '<div class="graph-relation-grid">' +
      relationRows(t("graph.incoming"), edges.filter(function(e) { return e.target === nodeId; }), false) +
      relationRows(t("graph.outgoing"), edges.filter(function(e) { return e.source === nodeId; }), true) +
    '</div>';
    var detail = $("#graph-selected-detail");
    detail.innerHTML = html;
    detail.style.display = "";
  }

  window.selectGraphNode = function(nodeId) {
    graphState.selectedNodeId = nodeId;
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.focusGraphNode = function(nodeId) {
    graphState.selectedNodeId = nodeId;
    graphState.focusNodeId = nodeId;
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.focusSelectedGraphNode = function() {
    if (graphState.selectedNodeId) window.focusGraphNode(graphState.selectedNodeId);
  };
  window.resetGraphFocus = function() {
    graphState.focusNodeId = null;
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.setGraphDepth = function(depth) {
    graphState.depth = depth === 2 ? 2 : 1;
    var depthFilter = $("#graph-depth-filter");
    if (depthFilter) depthFilter.value = String(graphState.depth);
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.setGraphEdgeMode = function(mode) {
    graphState.edgeMode = mode || "all";
    var edgeMode = $("#graph-edge-mode");
    if (edgeMode) edgeMode.value = graphState.edgeMode;
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.setGraphLayout = function(layout) {
    graphState.layout = layout || "galaxy";
    var layoutMode = $("#graph-layout-mode");
    if (layoutMode) layoutMode.value = graphState.layout;
    updateGraphLayoutButtons();
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.setGraphEngine = function(engine) {
    graphState.engine = engine === "svg" ? "svg" : "g6";
    var engineMode = $("#graph-engine-mode");
    if (engineMode) engineMode.value = graphState.engine;
    if (window.CodeLatticeG6Graph && graphState.engine === "svg") CodeLatticeG6Graph.destroy();
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  function updateGraphLayoutButtons() {
    $$("#graph-layout-buttons .segment").forEach(function(btn) {
      btn.classList.toggle("active", btn.getAttribute("data-layout") === graphState.layout);
    });
  }
  function updateGraphZoomLockButton() {
    var btn = $("#graph-zoom-lock-btn");
    if (!btn) return;
    btn.classList.toggle("is-locked", !!graphState.zoomLocked);
    btn.textContent = graphState.zoomLocked ? t("graph.zoomLocked") : t("graph.zoomUnlocked");
    btn.setAttribute("aria-pressed", graphState.zoomLocked ? "true" : "false");
  }
  window.toggleGraphZoomLock = function() {
    graphState.zoomLocked = !graphState.zoomLocked;
    updateGraphZoomLockButton();
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.toggleGraphPosterMode = function() {
    var view = $("#view-graph");
    var btn = $("#graph-poster-btn");
    if (!view) return;
    var enabled = !view.classList.contains("graph-poster-mode");
    view.classList.toggle("graph-poster-mode", enabled);
    if (btn) btn.textContent = enabled ? t("graph.exitPosterMode") : t("graph.posterMode");
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.toggleGraphSpotlightMode = function() {
    var view = $("#view-graph");
    var btn = $("#graph-spotlight-btn");
    if (!view) return;
    graphState.spotlight = !view.classList.contains("graph-spotlight-mode");
    view.classList.toggle("graph-spotlight-mode", graphState.spotlight);
    if (btn) btn.textContent = graphState.spotlight ? t("graph.exitSpotlightMode") : t("graph.spotlightMode");
    if (currentSnapshot) renderGraph(currentSnapshot);
  };
  window.downloadGraphPoster = function() {
    var host = $("#graph-visual");
    if (!host) return;
    var fileName = "codelattice-graph-" + (graphState.layout || "graph") + ".png";
    var canvas = host.querySelector("canvas");
    if (canvas && canvas.toBlob) {
      try {
        canvas.toBlob(function(blob) {
          if (!blob) return alert(t("graph.exportFailed"));
          downloadBlob(blob, fileName);
        }, "image/png");
        return;
      } catch (_) {}
    }
    var svg = host.querySelector("svg");
    if (!svg) return alert(t("graph.exportFailed"));
    var xml = new XMLSerializer().serializeToString(svg);
    var img = new Image();
    var svgBlob = new Blob([xml], {type: "image/svg+xml;charset=utf-8"});
    var url = URL.createObjectURL(svgBlob);
    img.onload = function() {
      var canvas2 = document.createElement("canvas");
      var viewBox = svg.viewBox && svg.viewBox.baseVal;
      canvas2.width = Math.max(960, viewBox && viewBox.width || svg.clientWidth || 1200);
      canvas2.height = Math.max(620, viewBox && viewBox.height || svg.clientHeight || 760);
      var ctx = canvas2.getContext("2d");
      ctx.fillStyle = graphState.layout === "orbit" ? "#f8fbff" : "#071225";
      ctx.fillRect(0, 0, canvas2.width, canvas2.height);
      ctx.drawImage(img, 0, 0, canvas2.width, canvas2.height);
      URL.revokeObjectURL(url);
      canvas2.toBlob(function(blob) {
        if (!blob) return alert(t("graph.exportFailed"));
        downloadBlob(blob, fileName);
      }, "image/png");
    };
    img.onerror = function() { URL.revokeObjectURL(url); alert(t("graph.exportFailed")); };
    img.src = url;
  };
  function downloadBlob(blob, fileName) {
    var a = document.createElement("a");
    var url = URL.createObjectURL(blob);
    a.href = url;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(function() { URL.revokeObjectURL(url); }, 1000);
  }

  function renderDiff() {
    // This will be updated when a second snapshot is loaded
  }

  // ── Diff (Phase B) ────────────────────────────────────────────────────

  var diffSnapshot = null;
  var diffData = null;

  function clearDiff() {
    diffSnapshot = null; diffData = null;
    $("#diff-empty").style.display = "";
    $("#diff-results").style.display = "none";
    $("#diff-clear-btn").style.display = "none";
    $("#diff-compare-name").textContent = "";
  }

  window.clearDiff = clearDiff;

  function loadDiffSnapshot(file) {
    var reader = new FileReader();
    reader.onload = function(e) {
      try {
        diffSnapshot = JSON.parse(e.target.result);
        var name = file.name || "compare.json";
        $("#diff-compare-name").textContent = "vs " + name;
        $("#diff-clear-btn").style.display = "";
        computeAndRenderDiff();
      } catch(err) {
        $("#diff-compare-name").textContent = "Parse error: " + err.message;
      }
    };
    reader.readAsText(file);
  }

  function stableSymbolKey(sym) {
    return (sym.file || "") + "::" + (sym.name || sym.id || "") + "::" + (sym.kind || "");
  }

  function deltaBadge(baseVal, compVal) {
    var d = compVal - baseVal;
    if (d === 0) return '<span class="badge badge-info">=' + d + '</span>';
    if (d > 0) return '<span class="badge badge-danger">+' + d + '</span>';
    return '<span class="badge badge-success">' + d + '</span>';
  }

  function computeAndRenderDiff() {
    if (!currentSnapshot || !diffSnapshot) return;
    var base = currentSnapshot, comp = diffSnapshot;
    var bs = base.summary || {}, cs = comp.summary || {};
    var bq = base.quality || {}, cq = comp.quality || {};
    var be = base.explore || {}, ce = comp.explore || {};
    var bc = base.cleanup || {}, cc = comp.cleanup || {};

    var cards = [];
    cards.push({label: "Source Files", base: bs.sourceFileCount||0, comp: cs.sourceFileCount||0});
    cards.push({label: "Symbols", base: bs.symbolCount||0, comp: cs.symbolCount||0});
    cards.push({label: "Edges", base: bs.edgeCount||0, comp: cs.edgeCount||0});
    cards.push({label: "Nodes", base: bs.nodeCount||0, comp: cs.nodeCount||0});
    cards.push({label: "Quality Passed", base: bq.passedGateCount||0, comp: cq.passedGateCount||0});
    cards.push({label: "Quality Failed", base: bq.failedGateCount||0, comp: cq.failedGateCount||0});
    cards.push({label: "Call Edges", base: bs.callEdgeCount||0, comp: cs.callEdgeCount||0});
    cards.push({label: "Overall", base: bq.overall||"?", comp: cq.overall||"?", isStr: true});

    $("#diff-summary-cards").innerHTML = cards.map(function(c) {
      if (c.isStr) return '<div class="stat-card"><div class="stat-label">' + c.label + '</div><div class="stat-value" style="font-size:1em;">' + esc(String(c.base)) + ' → ' + esc(String(c.comp)) + '</div></div>';
      return '<div class="stat-card"><div class="stat-label">' + c.label + '</div><div class="stat-value">' + c.base + ' → ' + c.comp + ' ' + deltaBadge(c.base, c.comp) + '</div></div>';
    }).join("");

    // Symbol diff
    var baseSyms = be.symbols || [], compSyms = ce.symbols || [];
    var baseKeys = new Set(baseSyms.map(stableSymbolKey));
    var compKeys = new Set(compSyms.map(stableSymbolKey));
    var addedSyms = compSyms.filter(function(s) { return !baseKeys.has(stableSymbolKey(s)); });
    var removedSyms = baseSyms.filter(function(s) { return !compKeys.has(stableSymbolKey(s)); });
    $("#diff-added-symbols").innerHTML = (addedSyms.length > 0 ?
      addedSyms.slice(0, 20).map(function(s) { return '<div class="gate-item"><span>' + esc(s.name||s.id) + '</span><span class="badge badge-info">' + esc(s.kind||"?") + '</span></div>'; }).join("") :
      '<span class="text-muted">No symbols added</span>');
    $("#diff-removed-symbols").innerHTML = (removedSyms.length > 0 ?
      removedSyms.slice(0, 20).map(function(s) { return '<div class="gate-item"><span>' + esc(s.name||s.id) + '</span><span class="badge badge-warning">' + esc(s.kind||"?") + '</span></div>'; }).join("") :
      '<span class="text-muted">No symbols removed</span>');

    // File diff
    var baseFiles = new Set((be.sourceFiles||[]).map(function(f) { return f.path; }));
    var compFiles = new Set((ce.sourceFiles||[]).map(function(f) { return f.path; }));
    var addedFiles = (ce.sourceFiles||[]).filter(function(f) { return !baseFiles.has(f.path); });
    var removedFiles = (be.sourceFiles||[]).filter(function(f) { return !compFiles.has(f.path); });
    $("#diff-added-files").innerHTML = addedFiles.slice(0, 15).map(function(f) { return '<div class="gate-item"><span>' + esc(f.path) + '</span><span>' + (f.symbolCount||0) + ' syms</span></div>'; }).join("") || '<span class="text-muted">No files added</span>';
    $("#diff-removed-files").innerHTML = removedFiles.slice(0, 15).map(function(f) { return '<div class="gate-item"><span>' + esc(f.path) + '</span><span>' + (f.symbolCount||0) + ' syms</span></div>'; }).join("") || '<span class="text-muted">No files removed</span>';

    // Quality changes
    var bGates = (bq.gates||[]).map(function(g) { return g.gateName || g.name || ""; }).filter(Boolean);
    var cGates = (cq.gates||[]).map(function(g) { return g.gateName || g.name || ""; }).filter(Boolean);
    var qAdded = cGates.filter(function(g) { return bGates.indexOf(g) < 0; });
    var qRemoved = bGates.filter(function(g) { return cGates.indexOf(g) < 0; });
    var qChanged = [];
    (bq.gates||[]).forEach(function(bg) {
      var cg = (cq.gates||[]).find(function(g) { return (g.gateName||g.name) === (bg.gateName||bg.name); });
      if (cg && bg.passed !== cg.passed) qChanged.push({name: bg.gateName||bg.name, from: bg.passed, to: cg.passed});
    });
    var qHtml = qAdded.length > 0 ? '<p style="margin-bottom:4px;"><strong>Added:</strong> ' + qAdded.join(", ") + "</p>" : "";
    qHtml += qRemoved.length > 0 ? '<p style="margin-bottom:4px;"><strong>Removed:</strong> ' + qRemoved.join(", ") + "</p>" : "";
    qHtml += qChanged.map(function(c) {
      return '<div class="gate-item"><span>' + esc(c.name) + '</span><span class="badge ' + (c.to ? 'badge-success' : 'badge-danger') + '">' + (c.from ? "PASS" : "FAIL") + '→' + (c.to ? "PASS" : "FAIL") + '</span></div>';
    }).join("");
    $("#diff-quality-changes").innerHTML = qHtml || '<span class="text-muted">No quality gate changes</span>';

    // Limitations changed
    var bLim = base.limitations || {}, cLim = comp.limitations || {};
    var bNotes = bLim.notes || [], cNotes = cLim.notes || [];
    var limAdded = cNotes.filter(function(n) { return bNotes.indexOf(n) < 0; });
    var limRemoved = bNotes.filter(function(n) { return cNotes.indexOf(n) < 0; });
    var limHtml = limAdded.length > 0 ? '<p style="margin-bottom:4px;"><strong>+ Added:</strong></p><ul style="padding-left:20px;font-size:0.85em;">' + limAdded.map(function(n) { return "<li>" + esc(n) + "</li>"; }).join("") + "</ul>" : "";
    limHtml += limRemoved.length > 0 ? '<p style="margin-bottom:4px;"><strong>− Removed:</strong></p><ul style="padding-left:20px;font-size:0.85em;">' + limRemoved.map(function(n) { return "<li>" + esc(n) + "</li>"; }).join("") + "</ul>" : "";
    $("#diff-limits-changes").innerHTML = limHtml || '<span class="text-muted">No limitation changes</span>';

    $("#diff-empty").style.display = "none";
    $("#diff-results").style.display = "";
  }

  function renderDiffTab() {
    if (!diffSnapshot) {
      $("#diff-empty").style.display = "";
      $("#diff-results").style.display = "none";
    } else {
      computeAndRenderDiff();
    }
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

  // Phase I/H: runner/live/report scripts are loaded as plain browser scripts.
  // Keep these helpers explicit on window so those modules do not depend on
  // accidental globals from this IIFE.
  window.show = show;
  window.esc = esc;
  window.renderAll = renderAll;
  window.updateCautionBanner = updateCautionBanner;
  window.computeAndRenderDiff = computeAndRenderDiff;
  window.loadSnapshot = loadSnapshot;
  window.showWelcome = showWelcome;
  window.showError = showError;
  Object.defineProperty(window, "currentSnapshot", {
    configurable: true,
    get: function () { return currentSnapshot; },
    set: function (value) { currentSnapshot = value; }
  });
  Object.defineProperty(window, "diffSnapshot", {
    configurable: true,
    get: function () { return diffSnapshot; },
    set: function (value) { diffSnapshot = value; }
  });

})();
