// timeline.js — CodeLattice Snapshot Viewer Timeline (Phase C)
// Pure DOM/SVG, no external deps. Renders metric trends from multiple snapshots.

var CTL = window.CTL || {}; window.CTL = CTL;

CTL.timelineSnapshots = [];

CTL.loadTimelineFiles = function(files) {
  var loaded = [];
  var errors = [];
  Array.from(files).forEach(function(f) {
    var reader = new FileReader();
    reader.onload = function(e) {
      try {
        var snap = JSON.parse(e.target.result);
        if (!snap.schemaVersion) { errors.push(f.name + ": missing schemaVersion"); return; }
        loaded.push({name: f.name, data: snap});
      } catch(ex) { errors.push(f.name + ": " + ex.message); }
      if (loaded.length + errors.length === files.length) {
        CTL.timelineSnapshots = loaded.sort(function(a,b) {
          return (a.data.generatedAt||"").localeCompare(b.data.generatedAt||"");
        });
        CTL.renderTimeline();
      }
    };
    reader.readAsText(f);
  });
};

CTL.buildTimelineData = function() {
  var snaps = CTL.timelineSnapshots;
  if (snaps.length < 1) return null;
  var rows = snaps.map(function(s) {
    var d = s.data, sd = d.summary||{}, g = d.graph||{}, c = d.cleanup||{};
    var dc = c.deadCodeCandidates||{}, rc = c.reachability||{};
    var rr = d.releaseReview||{}, br = rr.breakingChange||{};
    return {
      label: s.name.replace(".json","").slice(0,30),
      generatedAt: d.generatedAt||"",
      language: sd.language||d.language||"",
      sourceFiles: sd.sourceFileCount||0,
      symbols: sd.symbolCount||0,
      edges: sd.edgeCount||0,
      graphNodes: (g.summary||{}).nodeCount||0,
      graphEdges: (g.summary||{}).edgeCount||0,
      qualityFailed: (d.quality||{}).failedGateCount||0,
      deadCode: (dc.summary||{}).candidateSymbolCount||(dc.candidateSymbolCount||0),
      unreachable: (rc.summary||{}).unreachableCandidateCount||(rc.unreachableCandidateCount||0),
    };
  });
  return {snapshots: rows, count: rows.length};
};

CTL.timelineMetrics = [
  {key:"sourceFiles", label:"Source Files"},
  {key:"symbols", label:"Symbols"},
  {key:"edges", label:"Edges"},
  {key:"graphNodes", label:"Graph Nodes"},
  {key:"graphEdges", label:"Graph Edges"},
  {key:"qualityFailed", label:"Quality Failed"},
  {key:"deadCode", label:"Dead Code Cand."},
  {key:"unreachable", label:"Unreachable"},
];
CTL.timelineSelectedMetric = "symbols";

CTL.timelineMetricValue = function(row, metric) { return row[metric] || 0; };

CTL.renderTimeline = function() {
  var data = CTL.buildTimelineData();
  var container = document.getElementById("timeline-content");
  var chartEl = document.getElementById("timeline-chart");
  var tableEl = document.getElementById("timeline-table");
  if (!container || !data || data.count < 1) {
    if (container) container.innerHTML = '<div class="impact-empty"><h3>Timeline</h3><p>Load 2 or more snapshots to build a timeline.</p></div>';
    return;
  }

  // Metric selector
  var metricHTML = '<div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px;">';
  CTL.timelineMetrics.forEach(function(m) {
    metricHTML += '<button class="btn ' + (CTL.timelineSelectedMetric===m.key?'btn-primary':'btn-secondary') + ' btn-sm" onclick="CTL.selectTimelineMetric(&quot;'+m.key+'&quot;)">' + m.label + '</button>';
  });
  metricHTML += '</div>';

  // SVG chart
  var svgHTML = CTL.renderTimelineChart(data, CTL.timelineSelectedMetric);

  // Table
  var rows = data.snapshots;
  var metricKey = CTL.timelineSelectedMetric;
  var maxVal = Math.max.apply(null, rows.map(function(r) { return CTL.timelineMetricValue(r, metricKey); })) || 1;
  var thHTML = '<tr><th style="text-align:left;padding:4px 8px;">Snapshot</th>' +
    rows.map(function(r) { return '<th style="padding:4px 8px;text-align:right;">' + esc(r.label) + '</th>'; }).join("") +
    '<th style="padding:4px 8px;text-align:right;">Delta</th></tr>';
  var bodyHTML = CTL.timelineMetrics.map(function(m) {
    var vals = rows.map(function(r) { return CTL.timelineMetricValue(r, m.key); });
    return '<tr><td style="padding:4px 8px;font-size:0.85em;">' + m.label + '</td>' +
      vals.map(function(v) { return '<td style="padding:4px 8px;text-align:right;font-size:0.85em;">' + v + '</td>'; }).join("") +
      '<td style="padding:4px 8px;text-align:right;font-size:0.85em;">' + CTL.deltaBadge(vals[0]||0, vals[vals.length-1]||0) + '</td></tr>';
  }).join("");
  tableEl.innerHTML = '<table style="width:100%;border-collapse:collapse;">' + thHTML + bodyHTML + '</table>';

  container.innerHTML = '<div style="margin-bottom:8px;font-size:0.85em;color:var(--clr-text-muted);">Timeline compares static snapshots only — does not prove behavior changed.</div>' +
    metricHTML + '<div id="timeline-chart">' + svgHTML + '</div><div style="margin-top:16px;">' + tableEl.outerHTML + '</div>';
};

CTL.renderTimelineChart = function(data, metricKey) {
  var rows = data.snapshots;
  var w = 700, h = 200, pad = {top:20, right:20, bottom:30, left:50};
  var vals = rows.map(function(r) { return CTL.timelineMetricValue(r, metricKey); });
  var maxVal = Math.max.apply(null, vals) || 1;
  if (maxVal === 0) maxVal = 1;
  var n = vals.length;
  var xStep = n > 1 ? (w - pad.left - pad.right) / (n - 1) : (w - pad.left - pad.right);
  if (n === 1) xStep = (w - pad.left - pad.right) / 2;

  var points = vals.map(function(v, i) {
    return {x: pad.left + i * xStep, y: pad.top + (h - pad.top - pad.bottom) * (1 - v/maxVal)};
  });

  var pathD = points.map(function(p, i) { return (i===0?'M':'L') + p.x.toFixed(0)+','+p.y.toFixed(0); }).join(" ");
  var dots = points.map(function(p, i) {
    return '<circle cx="'+p.x.toFixed(0)+'" cy="'+p.y.toFixed(0)+'" r="4" fill="#2563eb" stroke="#fff" stroke-width="1.5"/>' +
      '<text x="'+p.x.toFixed(0)+'" y="'+(p.y-8)+'" text-anchor="middle" font-size="10" fill="#374151">'+vals[i]+'</text>';
  }).join("");

  // Y axis labels
  var yLabels = "";
  for (var yv = 0; yv <= maxVal; yv += Math.ceil(maxVal/3) || 1) {
    var yp = pad.top + (h - pad.top - pad.bottom) * (1 - yv/maxVal);
    yLabels += '<text x="'+(pad.left-5)+'" y="'+(yp+4)+'" text-anchor="end" font-size="9" fill="#6b7280">'+yv+'</text>';
    yLabels += '<line x1="'+pad.left+'" y1="'+yp+'" x2="'+(w-pad.right)+'" y2="'+yp+'" stroke="#e5e7eb" stroke-width="0.5"/>';
  }
  // X axis labels
  var xLabels = rows.map(function(r,i) {
    return '<text x="'+(pad.left + i*xStep)+'" y="'+(h-5)+'" text-anchor="middle" font-size="9" fill="#6b7280" transform="rotate(-30,'+(pad.left+i*xStep)+','+(h-5)+')">'+r.label+'</text>';
  }).join("");

  return '<svg viewBox="0 0 '+w+' '+h+'" style="width:100%;max-width:700px;height:auto;background:#fff;border:1px solid #e5e7eb;border-radius:6px;">' +
    yLabels + xLabels +
    '<path d="'+pathD+'" fill="none" stroke="#2563eb" stroke-width="2" stroke-linejoin="round"/>'+
    '<path d="'+pathD+'" fill="url(#grad)" opacity="0.1"/>' +
    '<defs><linearGradient id="grad" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="#2563eb"/><stop offset="100%" stop-color="#2563eb" stop-opacity="0"/></linearGradient></defs>' +
    dots + '</svg>';
};

CTL.selectTimelineMetric = function(key) {
  CTL.timelineSelectedMetric = key;
  CTL.renderTimeline();
};
