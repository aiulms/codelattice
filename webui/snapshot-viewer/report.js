// report.js — CodeLattice Snapshot Viewer Report + Workflow Checklist (Phase C)

var CTL = window.CTL || {}; window.CTL = CTL;

// ── Workflow Checklist ───────────────────────────────────────────────────

CTL.checklist = {};
CTL.checklistInitialized = false;

// Load from localStorage if available
function checklistLoad() {
  try {
    var s = localStorage.getItem("codelattice-checklist");
    if (s) CTL.checklist = JSON.parse(s);
  } catch(e) {}
  CTL.checklistInitialized = true;
}
function checklistSave() {
  try { localStorage.setItem("codelattice-checklist", JSON.stringify(CTL.checklist)); } catch(e) {}
}
checklistLoad();

CTL.workflowPresets = [
  { id:"onboarding", name:"Project Onboarding", tools:["project_overview","reachability_map","external_api_surface","review_plan"], items: ["Reviewed project overview & stats","Checked entry points & reachability","Reviewed external API surface","Noted framework entry hints","Recorded manual observations"] },
  { id:"before_edit", name:"Before Edit", tools:["changed_symbols","impact_preview","impact_analysis","breaking_change_review"], items: ["Identified changed symbols","Reviewed impact preview","Checked risk reasons","Verified no breaking changes to public API","Recorded pre-edit observations"] },
  { id:"after_edit", name:"After Edit", tools:["changed_symbols","impact_preview","consistency_review","quality"], items: ["Re-identified all changed symbols","Reviewed post-edit impact","Checked consistency (docs/tests)","Verified quality gates still pass","Recorded post-edit observations"] },
  { id:"delete_code", name:"Delete Code Assessment", tools:["reachability_map","dead_code_candidates","external_api_surface","calls_to"], items: ["Checked reachability (unreachable candidates)","Reviewed dead code candidates with cautions","Verified no external API consumers","Checked framework entry hints","Confirmed NOT deletion-proof manually"] },
  { id:"release_check", name:"Release Check", tools:["quality","production_assist","automation_graph","breaking_change_review","consistency_review","config_examples_review"], items: ["Verified all quality gates pass","Reviewed production assist summary","Checked automation graph workflows","Checked breaking change review","Reviewed consistency (docs/tests sync)","Checked config/example references","Recorded release readiness notes"] },
  { id:"legacy_cleanup", name:"Legacy Cleanup", tools:["project_insights","dead_code_candidates","reachability_map","risk_hotspots","architecture_drift"], items: ["Reviewed project insights (legacy mode)","Checked dead code candidates","Reviewed unreachable symbols with cautions","Checked risk hotspots","Reviewed architecture drift","Recorded cleanup plan"] },
  { id:"public_api_change", name:"Public API Change", tools:["external_api_surface","impact_preview","breaking_change_review","consistency_review"], items: ["Reviewed full external API surface","Checked impact of API changes","Reviewed breaking change risks","Updated documentation references","Noted downstream consumers manually"] },
  { id:"framework_route_change", name:"Framework Route Change", tools:["framework_entry_hints","external_api_surface","impact_preview","reachability_map"], items: ["Reviewed all framework entry hints","Checked route/callback impacts","Verified no dangling routes","Updated framework config references","Tested route changes manually"] },
  { id:"docs_tests_sync", name:"Docs-Tests Sync", tools:["consistency_review","config_examples_review","project_overview"], items: ["Checked stale doc candidates","Reviewed missing test candidates","Verified config references","Updated relevant docs","Noted test gaps for follow-up"] },
  { id:"config_examples_sync", name:"Config-Examples Sync", tools:["config_examples_review","consistency_review","project_overview"], items: ["Checked stale config references","Reviewed example code blocks","Verified CI/Docker refs consistent","Updated outdated examples","Noted config drift for follow-up"] }
];

CTL.buildWorkflowChecklist = function() {
  return CTL.workflowPresets.map(function(p) {
    var checked = CTL.checklist[p.id] || {};
    return {
      id: p.id,
      name: p.name,
      tools: p.tools,
      items: p.items.map(function(item, i) {
        return {text: item, checked: !!checked[i]};
      }),
      allChecked: p.items.every(function(_, i) { return !!checked[i]; }),
      checkedCount: p.items.filter(function(_, i) { return !!checked[i]; }).length,
      totalCount: p.items.length
    };
  });
};

CTL.renderWorkflowChecklist = function() {
  var list = CTL.buildWorkflowChecklist();
  var container = document.getElementById("workflow-presets-list");
  if (!container) return;
  var html = '<p class="text-muted" style="margin-bottom:8px;">Checklist is a human review aid, not proof that checks passed. Checked items are saved to browser localStorage.</p>' +
    '<div style="margin-bottom:10px;"><button class="btn btn-sm btn-secondary" onclick="CTL.resetWorkflowChecklist()">Reset All</button></div>';
  html += list.map(function(sc, si) {
    var cardClass = sc.allChecked ? "workflow-card checked" : "workflow-card";
    return '<div class="workflow-card" id="wf-' + sc.id + '">' +
      '<div class="workflow-header" style="display:flex;justify-content:space-between;align-items:center;">' +
        '<h4 class="workflow-name">' + esc(sc.name) + ' <span class="text-muted text-sm">('+sc.checkedCount+'/'+sc.totalCount+')</span></h4>' +
        '<span class="text-muted text-sm">' + sc.tools.map(function(t) { return '<code>' + t + '</code>'; }).join(', ') + '</span>' +
      '</div>' +
      '<div class="workflow-checklist-items" style="margin-top:8px;">' +
        sc.items.map(function(item, ii) {
          var cid = sc.id + "-" + ii;
          return '<label style="display:flex;align-items:flex-start;gap:6px;padding:4px 0;cursor:pointer;font-size:0.88em;" for="ci-' + cid + '">' +
            '<input type="checkbox" id="ci-' + cid + '" ' + (item.checked?'checked':'') + ' onchange="CTL.toggleChecklistItem(&quot;' + cid + '&quot;)" style="margin-top:2px;">' +
            '<span>' + esc(item.text) + '</span></label>';
        }).join("") +
      '</div></div>';
  }).join("");
  container.innerHTML = html;
};

CTL.toggleChecklistItem = function(cid) {
  var parts = cid.split("-"), scId = parts[0], ii = parseInt(parts[1],10);
  var checked = document.getElementById("ci-" + cid);
  if (!checked) return;
  CTL.checklist[scId] = CTL.checklist[scId] || {};
  CTL.checklist[scId][ii] = checked.checked;
  checklistSave();
  CTL.renderWorkflowChecklist();
};

CTL.resetWorkflowChecklist = function() {
  CTL.checklist = {};
  checklistSave();
  CTL.renderWorkflowChecklist();
};

// ── Markdown Report Generation ────────────────────────────────────────────

CTL.collectReportContext = function() {
  var snap = window.currentSnapshot;
  if (!snap) return null;
  var ctx = {snapshot: snap};

  // Dashboard stats
  var s = snap.summary || {};
  ctx.summary = {
    sourceFiles: s.sourceFileCount||0, symbols: s.symbolCount||0,
    edges: s.edgeCount||0, nodes: s.nodeCount||0,
    language: s.language||snap.language||"", packages: s.packageCount||0,
    callEdges: s.callEdgeCount||0
  };

  // Quality
  var q = snap.quality || {};
  ctx.quality = {
    overall: q.overall||"?", total: q.totalGates||0, passed: q.passedGateCount||0, failed: q.failedGateCount||0,
    gates: q.gates||[]
  };

  // Graph
  var g = snap.graph || {};
  ctx.graph = {
    nodes: (g.summary||{}).nodeCount||0, edges: (g.summary||{}).edgeCount||0,
    callEdges: (g.summary||{}).callEdgeCount||0,
    fileNodes: (g.summary||{}).fileNodeCount||0, symbolNodes: (g.summary||{}).symbolNodeCount||0
  };

  // Diff
  if (window.diffSnapshot) {
    var ds = window.diffSnapshot.summary||{};
    ctx.diff = { compareLoaded: true,
      sourceFilesDelta: (ds.sourceFileCount||0) - ctx.summary.sourceFiles,
      symbolsDelta: (ds.symbolCount||0) - ctx.summary.symbols,
      edgesDelta: (ds.edgeCount||0) - ctx.summary.edges,
    };
  } else { ctx.diff = {compareLoaded: false}; }

  // Timeline
  var td = CTL.buildTimelineData ? CTL.buildTimelineData() : null;
  ctx.timeline = td ? {count: td.count, metrics: CTL.timelineMetrics, rows: td.snapshots} : null;

  // Cleanup
  var c = snap.cleanup || {};
  ctx.cleanup = {
    deadCode: ((c.deadCodeCandidates||{}).summary||{}).candidateSymbolCount || (c.deadCodeCandidates||{}).candidateSymbolCount || null,
    unreachable: ((c.reachability||{}).summary||{}).unreachableCandidateCount || null,
    externalApi: (c.externalApiSurface||{}).externalSurfaceSymbolCount || null,
    framework: ((c.frameworkEntries||{}).summary||{}).frameworkEntryHintCount || null,
  };

  // Release
  var rr = snap.releaseReview || {};
  ctx.release = {
    breakingRisk: (rr.breakingChange||{}).compatibilityRisk || "N/A",
    breakingSurface: (rr.breakingChange||{}).breakingChangeSurface || null,
    staleDocs: ((rr.consistency||{}).staleDocCandidates||[]).length || null,
  };

  var ag = snap.automationGraph || (rr && rr.automationGraph) || {};
  var ags = ag.summary || {};
  ctx.automationGraph = {
    status: ag.status || "collected",
    workflowCount: ags.workflowCount || (ag.workflows || []).length || 0,
    stepCount: ags.stepCount || 0,
    riskCount: ags.riskCount || (ag.riskFindings || []).length || 0,
    highRiskCount: ags.highRiskCount || 0,
    riskFindings: ag.riskFindings || [],
    workflows: ag.workflows || [],
    staticOnly: true
  };

  // Workflow checklist
  ctx.checklist = CTL.buildWorkflowChecklist ? CTL.buildWorkflowChecklist() : [];

  // Limitations
  var lim = snap.limitations || {};
  ctx.limitations = lim.notes || (Array.isArray(lim) ? lim : []);

  ctx.meta = {
    generatedAt: snap.generatedAt||"",
    toolVersion: (snap.generatedFrom||{}).toolVersion||"",
    schemaVersion: snap.schemaVersion||"",
    root: snap.root||(snap.summary||{}).root||"",
    reportGeneratedAt: new Date().toISOString(),
  };

  return ctx;
};

CTL.generateMarkdownReport = function() {
  var ctx = CTL.collectReportContext();
  if (!ctx) return "# CodeLattice Review Report\n\nNo snapshot loaded.\n";

  var lines = [];
  lines.push("# CodeLattice Snapshot Review Report");
  lines.push("");
  lines.push("**Generated:** " + ctx.meta.reportGeneratedAt + " | **Snapshot:** " + ctx.meta.generatedAt);
  lines.push("**Tool:** " + ctx.meta.toolVersion + " | **Schema:** " + ctx.meta.schemaVersion);
  lines.push("");

  lines.push("## Caution");
  lines.push("");
  lines.push("- staticAnalysis: true");
  lines.push("- runtimeVerified: **false** — no project code was executed");
  lines.push("- externalUsageVerified: **false**");
  lines.push("- coverageVerified: **false**");
  lines.push("- deletionSafetyVerified: **false**");
  lines.push("");
  lines.push("**This is a static analysis report, not a release gate or safety proof.**");
  lines.push("");

  lines.push("## Dashboard Summary");
  lines.push("");
  lines.push("| Metric | Value |");
  lines.push("|---|---|");
  lines.push("| Source Files | " + ctx.summary.sourceFiles + " |");
  lines.push("| Symbols | " + ctx.summary.symbols + " |");
  lines.push("| Nodes | " + ctx.summary.nodes + " |");
  lines.push("| Edges | " + ctx.summary.edges + " |");
  lines.push("| Call Edges | " + ctx.summary.callEdges + " |");
  lines.push("| Packages | " + ctx.summary.packages + " |");
  lines.push("| Language | " + ctx.summary.language + " |");
  lines.push("");

  lines.push("## Quality Gates");
  lines.push("");
  lines.push("Overall: **" + ctx.quality.overall + "** | " + ctx.quality.passed + " passed / " + ctx.quality.failed + " failed / " + ctx.quality.total + " total");
  lines.push("");
  if (ctx.quality.gates.length > 0) {
    lines.push("| Gate | Status | Detail |");
    lines.push("|---|---|---|");
    ctx.quality.gates.forEach(function(g) {
      lines.push("| " + (g.gateName||g.name||"") + " | " + (g.passed ? "✅ PASS" : "❌ FAIL") + " | " + (g.detail||g.message||"") + " |");
    });
  }
  lines.push("");

  lines.push("## Graph Summary");
  lines.push("");
  lines.push("| Metric | Value |");
  lines.push("|---|---|");
  lines.push("| Graph Nodes | " + ctx.graph.nodes + " |");
  lines.push("| Graph Edges | " + ctx.graph.edges + " |");
  lines.push("| Call Edges | " + ctx.graph.callEdges + " |");
  lines.push("| File Nodes | " + ctx.graph.fileNodes + " |");
  lines.push("| Symbol Nodes | " + ctx.graph.symbolNodes + " |");
  lines.push("");

  lines.push("## Diff Comparison");
  lines.push("");
  if (ctx.diff.compareLoaded) {
    lines.push("| Metric | Baseline | Compare | Delta |");
    lines.push("|---|---|---|---|");
    lines.push("| Source Files | " + ctx.summary.sourceFiles + " | " + (ctx.summary.sourceFiles+ctx.diff.sourceFilesDelta) + " | " + (ctx.diff.sourceFilesDelta>=0?"+":"") + ctx.diff.sourceFilesDelta + " |");
    lines.push("| Symbols | " + ctx.summary.symbols + " | " + (ctx.summary.symbols+ctx.diff.symbolsDelta) + " | " + (ctx.diff.symbolsDelta>=0?"+":"") + ctx.diff.symbolsDelta + " |");
    lines.push("| Edges | " + ctx.summary.edges + " | " + (ctx.summary.edges+ctx.diff.edgesDelta) + " | " + (ctx.diff.edgesDelta>=0?"+":"") + ctx.diff.edgesDelta + " |");
  } else {
    lines.push("No compare snapshot loaded.");
  }
  lines.push("");

  lines.push("## Timeline");
  lines.push("");
  if (ctx.timeline && ctx.timeline.count >= 2) {
    var rows = ctx.timeline.rows;
    lines.push("| Metric | " + rows.map(function(r){return r.label;}).join(" | ") + " |");
    lines.push("|" + rows.map(function(){return "---";}).join("|") + "---|");
    ctx.timeline.metrics.forEach(function(m) {
      lines.push("| " + m.label + " | " + rows.map(function(r){return r[m.key]||0;}).join(" | ") + " |");
    });
  } else {
    lines.push("No timeline snapshots loaded.");
  }
  lines.push("");

  lines.push("## Cleanup Summary");
  lines.push("");
  lines.push("| Category | Count |");
  lines.push("|---|---|");
  lines.push("| Dead Code Candidates | " + (ctx.cleanup.deadCode||"N/A") + " |");
  lines.push("| Unreachable Candidates | " + (ctx.cleanup.unreachable||"N/A") + " |");
  lines.push("| External API Surface | " + (ctx.cleanup.externalApi||"N/A") + " |");
  lines.push("| Framework Entry Hints | " + (ctx.cleanup.framework||"N/A") + " |");
  lines.push("");

  lines.push("## Release Review Summary");
  lines.push("");
  lines.push("| Check | Value |");
  lines.push("|---|---|");
  lines.push("| Breaking Change Risk | " + ctx.release.breakingRisk + " |");
  lines.push("| Breaking Change Surface | " + (ctx.release.breakingSurface||"N/A") + " |");
  lines.push("| Stale Doc Candidates | " + (ctx.release.staleDocs||"N/A") + " |");
  lines.push("");

  lines.push("## Automation Graph Review");
  lines.push("");
  if (ctx.automationGraph.status === "not_collected") {
    lines.push("Automation graph was not collected in this snapshot.");
  } else {
    lines.push("| Metric | Value |");
    lines.push("|---|---|");
    lines.push("| Workflows | " + ctx.automationGraph.workflowCount + " |");
    lines.push("| Steps | " + ctx.automationGraph.stepCount + " |");
    lines.push("| Risk Findings | " + ctx.automationGraph.riskCount + " |");
    lines.push("| High Risk Findings | " + ctx.automationGraph.highRiskCount + " |");
    if (ctx.automationGraph.riskFindings.length > 0) {
      lines.push("");
      lines.push("| Risk | Item | Reason |");
      lines.push("|---|---|---|");
      ctx.automationGraph.riskFindings.slice(0, 10).forEach(function(r) {
        lines.push("| " + (r.level||r.severity||r.risk||"unknown") + " | " + (r.workflow||r.file||r.name||"") + " | " + (r.reason||r.message||r.hint||"") + " |");
      });
    }
  }
  lines.push("");

  lines.push("## Workflow Review Checklist");
  lines.push("");
  ctx.checklist.forEach(function(sc) {
    lines.push("### " + sc.name + " (" + sc.checkedCount + "/" + sc.totalCount + ")");
    lines.push("");
    sc.items.forEach(function(item) {
      lines.push("- [" + (item.checked?"x":" ") + "] " + item.text);
    });
    lines.push("");
  });

  lines.push("## Limitations");
  lines.push("");
  ctx.limitations.forEach(function(l) {
    lines.push("- " + l);
  });
  lines.push("");

  lines.push("## Recommended Manual Verification");
  lines.push("");
  lines.push("- [ ] Run project tests/builds outside CodeLattice");
  lines.push("- [ ] Review framework routes/callbacks manually");
  lines.push("- [ ] Verify public API consumers manually");
  lines.push("- [ ] Check integration with external dependencies");
  lines.push("- [ ] Confirm no runtime regressions via manual or CI testing");

  return lines.join("\n");
};

CTL.selectedTemplate = "general_snapshot_review";

CTL.getReportTemplates = function() {
  return [
    {id:"general_snapshot_review", name:"General Review", desc:"Standard snapshot overview with all sections."},
    {id:"onboarding_report", name:"Onboarding Report", desc:"First-time project analysis: structure, hotspots, entry points."},
    {id:"before_edit_risk_report", name:"Before Edit Risk Report", desc:"Pre-change impact assessment with diff if loaded."},
    {id:"release_review_report", name:"Release Review Report", desc:"Pre-release check: quality gates, breaking changes, docs."},
    {id:"legacy_cleanup_report", name:"Legacy Cleanup Report", desc:"Dead code, reachability, architecture drift analysis."},
    {id:"delete_code_review_report", name:"Delete Code Review Report", desc:"Safety assessment with NOT-deletion-proof warnings."}
  ];
};

CTL.renderReport = function() {
  var container = document.getElementById("report-content");
  if (!container) return;
  var templates = CTL.getReportTemplates();
  var tmplHTML = '<div style="margin-bottom:8px;"><select id="report-template-select" class="filter-select" onchange="CTL.selectedTemplate=this.value;CTL.renderReport()">' +
    templates.map(function(t){return '<option value="'+t.id+'"'+(CTL.selectedTemplate===t.id?' selected':'')+'>'+t.name+'</option>';}).join("")+'</select>' +
    '<span class="text-muted text-sm" style="margin-left:8px;">'+templates.find(function(t){return t.id===CTL.selectedTemplate;}).desc+'</span></div>';
  var report = CTL.generateMarkdownReport();
  container.innerHTML = tmplHTML +
    '<div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap;">' +
    '<button class="btn btn-sm btn-primary" onclick="CTL.copyReport()">Copy Report</button>' +
    '<button class="btn btn-sm btn-secondary" onclick="CTL.downloadReport()">Download .md</button>' +
    '</div>' +
    '<pre class="code-block" id="report-md" style="max-height:600px;overflow:auto;white-space:pre-wrap;font-size:.82em;line-height:1.45;">' + esc(report) + '</pre>';
};

CTL.generateTemplateReport = CTL.generateMarkdownReport;

CTL.copyReport = function() {
  var report = CTL.generateMarkdownReport();
  try {
    navigator.clipboard.writeText(report).then(function() {
      alert("Report copied to clipboard.");
    }, function() {
      // Fallback: select text in pre
      var pre = document.getElementById("report-md");
      if (pre) {
        var range = document.createRange();
        range.selectNodeContents(pre);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        alert("Report selected. Press Cmd+C / Ctrl+C to copy.");
      }
    });
  } catch(e) {
    var pre = document.getElementById("report-md");
    if (pre) {
      var range = document.createRange();
      range.selectNodeContents(pre);
      var sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
      alert("Could not copy. Select text manually or press Cmd+C.");
    }
  }
};

CTL.downloadReport = function() {
  var report = CTL.generateMarkdownReport();
  var blob = new Blob([report], {type: "text/markdown;charset=utf-8"});
  var url = URL.createObjectURL(blob);
  var a = document.createElement("a");
  a.href = url;
  a.download = "codelattice-review-" + new Date().toISOString().slice(0,10) + ".md";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

CTL.renderTimeline = CTL.renderTimeline || function(){};
CTL.renderReport = CTL.renderReport || function(){};
CTL.buildWorkflowChecklist = CTL.buildWorkflowChecklist || function(){return[];};
CTL.checklistLoad = checklistLoad;
