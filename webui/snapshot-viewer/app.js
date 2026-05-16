/* ============================================================
   CodeLattice Snapshot Viewer — Application Logic
   Pure vanilla JS, no dependencies.
   Functions: loadSnapshot, validateSnapshot, normalizeSnapshot,
   renderDashboard, renderExplore, renderImpact,
   renderCleanupRelease, renderError
   ============================================================ */

// ---- Global State ----
let currentSnapshot = null;
let currentView = 'dashboard';

// ---- Constants ----
const SCHEMA_VERSION = 'webui.snapshot.v1';
const TABS = ['dashboard', 'explore', 'impact', 'cleanup', 'release'];

// ============================================================
// 1. INITIALIZATION & EVENT BINDING
// ============================================================

document.addEventListener('DOMContentLoaded', () => {
  // File input
  const fileInput = document.getElementById('file-input');
  fileInput.addEventListener('change', (e) => {
    if (e.target.files && e.target.files[0]) {
      loadSnapshotFromFile(e.target.files[0]);
    }
  });

  // Drag and drop on body
  const dropZone = document.getElementById('drop-zone');
  let dragCounter = 0;

  document.body.addEventListener('dragenter', (e) => {
    e.preventDefault();
    dragCounter++;
    dropZone.style.display = 'flex';
  });

  document.body.addEventListener('dragleave', (e) => {
    e.preventDefault();
    dragCounter--;
    if (dragCounter <= 0) { dragCounter = 0; dropZone.style.display = 'none'; }
  });

  document.body.addEventListener('dragover', (e) => e.preventDefault());

  document.body.addEventListener('drop', (e) => {
    e.preventDefault();
    dragCounter = 0;
    dropZone.style.display = 'none';
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      loadSnapshotFromFile(e.dataTransfer.files[0]);
    }
  });

  // Tab switching
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => switchTab(btn.dataset.tab));
  });

  // Explore search / filter
  const searchInput = document.getElementById('explore-search');
  let searchTimeout = null;
  searchInput.addEventListener('input', () => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => renderExplore(currentSnapshot), 200);
  });

  const kindFilter = document.getElementById('explore-kind-filter');
  kindFilter.addEventListener('change', () => renderExplore(currentSnapshot));

  // Check URL query for snapshot path
  checkUrlQuery();
});

function checkUrlQuery() {
  const params = new URLSearchParams(window.location.search);
  const snapshotPath = params.get('snapshot');
  if (snapshotPath) {
    fetch(snapshotPath)
      .then(r => r.json())
      .then(data => handleLoadedData(data))
      .catch(err => renderError(
        `Failed to fetch: ${snapshotPath}`,
        err.message + '\n\nNote: Browser security (CORS) may block file:// fetches.\nUse the "Load Snapshot" button instead.'
      ));
  }
}

// ============================================================
// 2. SNAPSHOT LOADING
// ============================================================

function loadSnapshotFromFile(file) {
  if (!file.name.endsWith('.json') && !file.type.includes('json')) {
    renderError('Invalid file type', `Expected a JSON file (.json), got "${file.name}"`);
    return;
  }

  const reader = new FileReader();
  reader.onload = (e) => {
    try {
      const data = JSON.parse(e.target.result);
      handleLoadedData(data);
    } catch (parseErr) {
      renderError('JSON Parse Error', parseErr.message);
    }
  };
  reader.onerror = () => renderError('File Read Error', 'Could not read the selected file.');
  reader.readAsText(file);
}

function handleLoadedData(data) {
  const validation = validateSnapshot(data);
  if (!validation.ok) {
    renderError(validation.error, validation.detail || '');
    return;
  }

  currentSnapshot = normalizeSnapshot(data);

  // Show UI elements that need data
  document.getElementById('caution-banner').style.display = '';
  document.getElementById('project-info').style.display = '';
  document.getElementById('welcome-view').style.display = 'none';
  document.getElementById('error-view').style.display = 'none';
  document.getElementById('loaded-content').style.display = '';

  // Populate header
  document.getElementById('hdr-language').textContent = currentSnapshot.language.toUpperCase();
  document.getElementById('hdr-schema-version').textContent = currentSnapshot.schemaVersion;
  document.getElementById('hdr-generated-at').textContent = formatDate(currentSnapshot.generatedAt);

  // Render generatedFrom bar
  renderGenfromBar();

  // Render all views
  renderDashboard(currentSnapshot);
  renderExplore(currentSnapshot);
  renderImpact(currentSnapshot);
  renderCleanupRelease(currentSnapshot);

  // Switch to dashboard
  switchTab('dashboard');
}

// ============================================================
// 3. VALIDATION
// ============================================================

function validateSnapshot(data) {
  if (!data || typeof data !== 'object')
    return { ok: false, error: 'Input is not a valid JSON object' };

  if (!data.schemaVersion)
    return { ok: false, error: 'Missing schemaVersion field' };

  if (data.schemaVersion !== SCHEMA_VERSION)
    return { ok: false, error: `Schema version mismatch`, detail: `Expected "${SCHEMA_VERSION}", got "${data.schemaVersion}". This viewer only supports V1 snapshots.` };

  return { ok: true };
}

// ============================================================
// 4. NORMALIZATION — fill missing fields with safe defaults
// ============================================================

function normalizeSnapshot(data) {
  const d = JSON.parse(JSON.stringify(data)); // deep clone

  // metadata defaults
  d.generatedAt = d.generatedAt || '';
  d.generatorVersion = d.generatorVersion || '';
  d.root = d.root || '';
  d.language = d.language || 'unknown';

  // generatedFrom defaults
  const gf = d.generatedFrom || {};
  d.generatedFrom = {
    staticAnalysis: gf.staticAnalysis !== false,
    runtimeVerified: gf.runtimeVerified || false,
    externalUsageVerified: gf.externalUsageVerified || false,
    coverageVerified: gf.coverageVerified || false,
    deletionSafetyVerified: gf.deletionSafetyVerified || false
  };

  // summary defaults
  d.summary = d.summary || {};
  setDefault(d.summary, 'nodeCount', 0);
  setDefault(d.summary, 'edgeCount', 0);
  setDefault(d.summary, 'symbolCount', 0);
  setDefault(d.summary, 'sourceFileCount', 0);
  setDefault(d.summary, 'packageCount', 0);
  setDefault(d.summary, 'diagnosticCount', 0);
  setDefault(d.summary, 'callEdgeCount', 0);
  setDefault(d.summary, 'topNodeKinds', []);
  setDefault(d.summary, 'topEdgeKinds', []);

  // quality defaults
  d.quality = d.quality || {};
  setDefault(d.quality, 'overall', 'unknown');
  setDefault(d.quality, 'totalGates', 0);
  setDefault(d.quality, 'passedGates', 0);
  setDefault(d.quality, 'failedGates', 0);
  setDefault(d.quality, 'gates', []);
  setDefault(d.quality, 'metrics', {});
  setDefault(d.quality, 'diagnosticsSummary', {});

  // insights defaults
  d.insights = d.insights || {};
  setDefault(d.insights, 'status', 'not_collected');

  // explore defaults
  d.explore = d.explore || {};
  setDefault(d.explore, 'status', 'not_collected');
  setDefault(d.explore, 'symbols', []);
  d.explore.searchMeta = d.explore.searchMeta || {};
  setDefault(d.explore.searchMeta, 'totalSymbols', 0);
  setDefault(d.explore.searchMeta, 'availableKinds', []);

  // impact defaults
  d.impact = d.impact || {};
  setDefault(d.impact, 'status', 'not_collected');
  setDefault(d.impact, 'reason', '');
  setDefault(d.impact, 'entries', []);

  // cleanup defaults
  d.cleanup = d.cleanup || {};
  for (const sub of ['deadCodeCandidates', 'reachability', 'externalApiSurface', 'frameworkEntries']) {
    d.cleanup[sub] = d.cleanup[sub] || {};
    setDefault(d.cleanup[sub], 'status', 'not_collected');
    setDefault(d.cleanup[sub], 'reason', '');
  }

  // releaseReview defaults
  d.releaseReview = d.releaseReview || {};
  for (const sub of ['breakingChange', 'consistency', 'configExamples']) {
    d.releaseReview[sub] = d.releaseReview[sub] || {};
    setDefault(d.releaseReview[sub], 'status', 'not_collected');
    setDefault(d.releaseReview[sub], 'reason', '');
  }

  // docsTestsConfig defaults
  d.docsTestsConfig = d.docsTestsConfig || {};
  setDefault(d.docsTestsConfig, 'status', 'not_collected');

  // workflowPresets defaults
  d.workflowPresets = d.workflowPresets || {};
  setDefault(d.workflowPresets, 'status', 'not_collected');

  // limitations defaults
  d.limitations = d.limitations || [];

  return d;
}

function setDefault(obj, key, val) {
  if (obj[key] === undefined || obj[key] === null) obj[key] = val;
}

// ============================================================
// 5. TAB SWITCHING
// ============================================================

function switchTab(tabId) {
  currentView = tabId;

  // Update tab buttons
  document.querySelectorAll('.tab-btn').forEach(btn => {
    const isActive = btn.dataset.tab === tabId;
    btn.classList.toggle('active', isActive);
    btn.setAttribute('aria-selected', isActive ? 'true' : 'false');
  });

  // Show/hide view sections
  TABS.forEach(id => {
    const el = document.getElementById(`view-${id}`);
    if (el) el.style.display = id === tabId ? '' : 'none';
  });
}

// ============================================================
// 6. GENERATEDFROM BAR
// ============================================================

function renderGenfromBar() {
  const bar = document.getElementById('genfrom-bar');
  const gf = currentSnapshot.generatedFrom;

  const tags = [
    { key: 'staticAnalysis', label: 'Static Analysis' },
    { key: 'runtimeVerified', label: 'Runtime Verified' },
    { key: 'externalUsageVerified', label: 'External Usage Verified' },
    { key: 'coverageVerified', label: 'Coverage Verified' },
    { key: 'deletionSafetyVerified', label: 'Deletion Safety Verified' }
  ];

  bar.innerHTML = tags.map(t =>
    `<span class="gen-tag ${gf[t.key] ? 'true' : 'false'}">${t.label}: <strong>${gf[t.key]}</strong></span>`
  ).join('');
}

// ============================================================
// 7. DASHBOARD VIEW
// ============================================================

function renderSnapshotDashboard(data) {
  const s = data.summary;
  setText('dash-source-files', s.sourceFileCount);
  setText('dash-symbols', s.symbolCount);
  setText('dash-call-edges', s.callEdgeCount);
  setText('dash-packages', s.packageCount);

  // Quality status badge
  const q = data.quality;
  const statusEl = document.getElementById('dash-quality-status');
  statusEl.textContent = q.overall || 'unknown';
  statusEl.className = 'badge ' + (
    q.overall === 'pass' ? 'badge-success' :
    q.overall === 'fail' ? 'badge-danger' :
    q.overall === 'warn' ? 'badge-warning' : 'badge-info'
  );

  // Quality gates list
  const gatesList = document.getElementById('dash-quality-gates');
  if (q.gates && q.gates.length > 0) {
    gatesList.innerHTML = q.gates.map(g => `
      <div class="gate-item">
        <span>
          <span class="gate-name">${escHtml(g.gateName)}</span>
          <span class="gate-detail">${escHtml(g.detail || '')}</span>
        </span>
        <span class="${g.passed ? 'gate-pass' : 'gate-fail'}">${g.passed ? '&#10003; PASS' : '&#10007; FAIL'}</span>
      </div>
    `).join('');
  } else {
    gatesList.innerHTML = '<p class="text-muted text-sm">No quality gate data available.</p>';
  }

  // Limitations
  const limList = document.getElementById('dash-limitations');
  if (data.limitations && data.limitations.length > 0) {
    limList.innerHTML = data.limitations.map(l =>
      `<li>${escHtml(l)}</li>`
    ).join('');
  } else {
    limList.innerHTML = '<li class="text-muted" style="font-style:italic;">No limitations recorded.</li>';
  }
}

// ============================================================
// 8. EXPLORE VIEW
// ============================================================

function renderExplore(data) {
  const exp = data.explore;
  const container = document.getElementById('explore-symbol-list');
  const countEl = document.getElementById('explore-count');
  const kindFilter = document.getElementById('explore-kind-filter');
  const searchTerm = (document.getElementById('explore-search').value || '').toLowerCase();
  const kindFilterVal = kindFilter.value;

  // If not collected, show message
  if (exp.status !== 'collected' || !exp.symbols || exp.symbols.length === 0) {
    container.innerHTML = `<div class="text-muted text-center" style="padding:24px;">
      <strong>Symbol data not collected</strong><br>
      <span class="not-collected-reason">${escHtml(exp.reason || 'Use MCP tools for symbol-level exploration.')}</span>
    </div>`;
    countEl.textContent = '(0)';
    updateKindFilterOptions([]);
    return;
  }

  // Filter symbols
  let symbols = exp.symbols.filter(sym => {
    const nameMatch = !searchTerm ||
      (sym.name || '').toLowerCase().includes(searchTerm) ||
      (sym.file || '').toLowerCase().includes(searchTerm);
    const kindMatch = !kindFilterVal || sym.kind === kindFilterVal;
    return nameMatch && kindMatch;
  });

  countEl.textContent = `(${symbols.length} of ${exp.searchMeta.totalSymbols || exp.symbols.length})`;

  // Update kind filter options
  const kinds = [...new Set(exp.symbols.map(s => s.kind).filter(Boolean))].sort();
  updateKindFilterOptions(kinds);

  // Render symbol list
  container.innerHTML = symbols.map((sym, idx) => `
    <div class="symbol-item" data-index="${idx}" onclick="selectSymbol(${idx})">
      <span class="sym-name">${escHtml(sym.name || '(unnamed)')}</span>
      <span class="sym-meta">${escHtml(sym.kind || '')}${sym.file ? ' · ' + escHtml(sym.file) : ''}</span>
    </div>
  `).join('');

  if (symbols.length === 0) {
    container.innerHTML = `<div class="text-muted text-center" style="padding:24px;">No matching symbols.</div>`;
  }
}

function selectSymbol(index) {
  const exp = currentSnapshot.explore;
  if (!exp.symbols || index >= exp.symbols.length) return;

  // Highlight selection
  document.querySelectorAll('.symbol-item').forEach(el => el.classList.remove('selected'));
  const selected = document.querySelector(`.symbol-item[data-index="${index}"]`);
  if (selected) selected.classList.add('selected');

  const sym = exp.symbols[index];
  const detail = document.getElementById('explore-detail');

  detail.innerHTML = `
    <h3 style="margin-bottom:12px;font-size:1.05em;">
      ${escHtml(sym.name || '(unnamed)')}
      <span class="badge badge-lang">${escHtml(sym.kind || '?')}</span>
      ${sym.visibility === 'public' ? '<span class="badge badge-warning">pub</span>' : ''}
    </h3>

    ${sym.file ? renderDetailRow('Location', `${escHtml(sym.file)}${sym.line ? ':' + sym.line : ''}`) : ''}
    ${sym.sourceSnippet ? `
      <div class="detail-row">
        <div class="detail-label">Source Snippet</div>
        <div class="source-snippet">${escHtml(sym.sourceSnippet.lines || '')}</div>
      </div>
    ` : ''}

    ${(sym.outgoingEdges && Object.keys(sym.outgoingEdges).length > 0) ? renderDetailRow('Outgoing Edges', formatEdgeMap(sym.outgoingEdges)) : ''}
    ${(sym.incomingEdges && Object.keys(sym.incomingEdges).length > 0) ? renderDetailRow('Incoming Edges', formatEdgeMap(sym.incomingEdges)) : ''}

    ${renderConfidenceSamples(sym.confidenceSamples)}
  `;
}

function renderConfidenceSamples(samples) {
  if (!samples || samples.length === 0) return '';
  const items = samples.map(cs => {
    const cls = cs.confidence >= 0.8 ? 'badge-success' : cs.confidence >= 0.5 ? 'badge-warning' : 'badge-danger';
    return `<span class="badge ${cls}" title="${escHtml(cs.reason || '')}">${cs.confidence} ${escHtml(cs.reason || '')}</span>`;
  }).join(' ');
  return `<div class="detail-row"><div class="detail-label">Confidence Samples</div><div class="detail-value">${items}</div></div>`;
}

function updateKindFilterOptions(kinds) {
  const sel = document.getElementById('explore-kind-filter');
  const currentVal = sel.value;
  const firstOption = sel.querySelector('option[value=""]');
  sel.innerHTML = '';
  if (firstOption) sel.appendChild(firstOption);
  else {
    const opt = document.createElement('option');
    opt.value = ''; opt.textContent = 'All Kinds';
    sel.appendChild(opt);
  }
  kinds.forEach(k => {
    const opt = document.createElement('option');
    opt.value = k; opt.textContent = k;
    sel.appendChild(opt);
  });
  if (kinds.includes(currentVal)) sel.value = currentVal;
}

// ============================================================
// 9. IMPACT VIEW
// ============================================================

function renderImpact(data) {
  const imp = data.impact;
  const container = document.getElementById('impact-content');

  if (imp.status !== 'collected' || !imp.entries || imp.entries.length === 0) {
    container.innerHTML = `
      <div class="impact-empty">
        <h3>&#128270; Impact Analysis — On Demand</h3>
        <p>Impact analysis requires a target symbol and is not pre-computed into this snapshot.</p>
        <p class="text-sm">To get impact data, use the MCP <code>impact_preview</code> tool or generate an enriched snapshot.</p>
        <div class="not-collected-reason" style="margin-top:8px;">${escHtml(imp.reason || '')}</div>
      </div>
    `;
    return;
  }

  container.innerHTML = imp.entries.map(entry => `
    <div class="section-block">
      <h3 class="section-title">
        Impact: <code>${escHtml(entry.targetSymbol || entry.targetId || '?')}</code>
        <span class="badge ${riskBadgeClass(entry.risk)}">${entry.risk || '?'}</span>
      </h3>
      ${entry.riskReasons && entry.riskReasons.length > 0 ?
        `<ul style="list-style:disc;padding-left:20px;margin-bottom:10px;">
          ${entry.riskReasons.map(r => `<li>${escHtml(r)}</li>`).join('')}
        </ul>` : ''}

      <div class="card-grid card-grid-3">
        ${renderImpactMetric('Callers', entry.impactMetrics?.callerCount)}
        ${renderImpactMetric('Impacted Files', entry.impactMetrics?.impactedFileCount)}
        ${renderImpactMetric('Cross-file', entry.impactMetrics?.crossFileCount)}
        ${renderImpactMetric('Low Conf Edges', entry.impactMetrics?.lowConfidenceEdgeCount)}
        ${renderImpactMetric('Min Confidence', entry.confidenceSummary?.minConfidence)}
        ${renderImpactMetric('Avg Confidence', entry.confidenceSummary?.avgConfidence)}
      </div>

      ${entry.directCallers && entry.directCallers.length > 0 ? `
        <h4 style="margin:10px 0 6px;font-size:.9em;">Direct Callers</h4>
        <div class="gate-list">${entry.directCallers.map(c => `
          <div class="gate-item">
            <span><span class="gate-name">${escHtml(c.name)}</span>
              <span class="gate-detail">${c.file ? escHtml(c.file) : ''}</span></span>
            <span class="badge ${c.confidence >= 0.8 ? 'badge-success' : c.confidence >= 0.5 ? 'badge-warning' : 'badge-danger'}">${c.confidence}</span>
          </div>`).join('')}
        </div>` : ''}
    </div>
  `).join('');
}

function riskBadgeClass(risk) {
  switch ((risk || '').toUpperCase()) {
    case 'LOW': return 'badge-success';
    case 'MEDIUM': return 'badge-warning';
    case 'HIGH': case 'CRITICAL': return 'badge-danger';
    default: return 'badge-info';
  }
}

function impactRenderMetric(label, value) {
  if (value === undefined || value === null) return '';
  return `
    <div class="stat-card">
      <div class="stat-label">${label}</div>
      <div class="stat-value">${value}</div>
    </div>
  `;
}

// ============================================================
// 10. CLEANUP + RELEASE REVIEW VIEWS
// ============================================================

function renderCleanupRelease(data) {
  renderCleanupSection(data);
  renderReleaseSection(data);
}

function renderCleanupSection(data) {
  const cleanup = data.cleanup;

  // Dead Code Candidates
  renderInfoCardBody('cleanup-dead-code', cleanup.deadCodeCandidates, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const s = d.summary || {};
    return `
      <div class="stat-value" style="font-size:1.3em;margin-bottom:6px;">${s.candidateSymbolCount || 0} candidates</div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;">
        <span class="badge badge-success">${s.highConfidenceCount || 0} high</span>
        <span class="badge badge-warning">${s.mediumConfidenceCount || 0} medium</span>
        <span class="badge badge-danger">${s.lowConfidenceCount || 0} low</span>
      </div>
      ${d.deletionSafe === false ? '<p class="not-collected-reason" style="margin-top:6px;"><strong>&#9888; NOT deletion-safe</strong></p>' : ''}
    `;
  });

  // Reachability
  renderInfoCardBody('cleanup-reachability', cleanup.reachability, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const s = d.summary || {};
    return `
      <div class="stat-value" style="font-size:1.3em;margin-bottom:6px;">${s.unreachableCandidateCount || 0} unreachable</div>
      <div class="text-muted text-sm">${s.entryPointCount || 0} entry points &middot; ${s.reachableFileCount || 0}/${s.totalFiles || 0} files reachable</div>
    `;
  });

  // External API Surface
  renderInfoCardBody('cleanup-external-api', cleanup.externalApiSurface, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const s = d.summary || {};
    return `
      <div class="stat-value" style="font-size:1.3em;margin-bottom:6px;">${s.externalSurfaceSymbolCount || 0} surface symbols</div>
      <span class="badge badge-${cautionLevelClass(s.averageCautionScore)}">Caution: ${s.cautionLevel || 'unknown'}</span>
    `;
  });

  // Framework Entries
  renderInfoCardBody('cleanup-framework', cleanup.frameworkEntries, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const s = d.summary || {};
    return `
      <div class="stat-value" style="font-size:1.3em;margin-bottom:6px;">${s.frameworkEntryHintCount || 0} hints</div>
      <div class="text-muted text-sm">
        ${s.routeHintCount || 0} routes &middot;
        ${s.callbackHintCount || 0} callbacks &middot;
        ${s.componentHintCount || 0} components &middot;
        ${s.cliHintCount || 0} CLI
      </div>
    `;
  });
}

function renderReleaseSection(data) {
  const rr = data.releaseReview;

  renderInfoCardBody('release-breaking', rr.breakingChange, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    return `
      <span class="badge badge-${cautionLevelClass(d.compatibilityRisk)}">Risk: ${d.compatibilityRisk || 'unknown'}</span>
      ${d.changedExternalApi ? '<br><span class="badge badge-danger" style="margin-top:4px;">Public API changed</span>' : ''}
      ${d.reviewChecklist && d.reviewChecklist.length > 0 ? `
        <ol style="padding-left:18px;margin-top:8px;font-size:.85em;">
          ${d.reviewChecklist.slice(0, 5).map(c => `<li>${escHtml(c.item || c.priority + ': ' + (c.description || ''))}</li>`).join('')}
        </ol>` : ''}
    `;
  });

  renderInfoCardBody('release-consistency', rr.consistency, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const items = [];
    if (d.staleDocCandidates && d.staleDocCandidates.length > 0) items.push(`${d.staleDocCandidates.length} stale doc(s)`);
    if (d.missingTestCandidates && d.missingTestCandidates.length > 0) items.push(`${d.missingTestCandidates.length} missing test(s)`);
    return items.length > 0 ? items.join('<br>') : '<span class="not-collected-msg">No issues found</span>';
  });

  renderInfoCardBody('release-config-examples', rr.configExamples, (d) => {
    if (d.status !== 'collected') return notCollectedHtml(d.reason);
    const items = [];
    if (d.staleExamples && d.staleExamples.length > 0) items.push(`${d.staleExamples.length} stale example(s)`);
    if (d.staleConfigReferences && d.staleConfigReferences.length > 0) items.push(`${d.staleConfigReferences.length} stale config ref(s)`);
    return items.length > 0 ? items.join('<br>') : '<span class="not-collected-msg">No issues found</span>';
  });
}

function renderInfoCardBody(elementId, sectionData, rendererFn) {
  const el = document.getElementById(elementId);
  if (!el) return;
  try {
    el.innerHTML = rendererFn(sectionData || {});
  } catch (err) {
    el.innerHTML = '<span class="not-collected-msg">Render error</span>';
  }
}

function notCollectedHtml(reason) {
  return `<div class="not-collected-msg">&#128274; Not collected</div>
    ${reason ? `<div class="not-collected-reason">${escHtml(reason)}</div>` : ''}`;
}

function cautionLevelClass(score) {
  if (score === undefined || score === null) return 'info';
  if (score >= 0.7) return 'danger';
  if (score >= 0.4) return 'warning';
  return 'success';
}

// ============================================================
// 11. ERROR VIEW
// ============================================================

function showWelcome() {
  document.getElementById('welcome-view').style.display = '';
  document.getElementById('error-view').style.display = 'none';
  document.getElementById('loaded-content').style.display = 'none';
  document.getElementById('caution-banner').style.display = 'none';
  currentSnapshot = null;
}

function renderError(message, detail) {
  document.getElementById('welcome-view').style.display = 'none';
  document.getElementById('error-view').style.display = '';
  document.getElementById('loaded-content').style.display = 'none';
  document.getElementById('caution-banner').style.display = 'none';

  document.getElementById('error-message').textContent = message;
  document.getElementById('error-detail').textContent = detail || '';
  currentSnapshot = null;
}

// ============================================================
// 12. UTILITY HELPERS
// ============================================================

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value ?? '-';
}

function escHtml(str) {
  if (str === null || str === undefined) return '';
  const div = document.createElement('div');
  div.appendChild(document.createTextNode(String(str)));
  return div.innerHTML;
}

function formatDate(isoStr) {
  if (!isoStr) return '';
  try {
    const d = new Date(isoStr);
    return d.toLocaleString();
  } catch {
    return isoStr;
  }
}

function formatEdgeMap(edges) {
  if (!edges) return '';
  return Object.entries(edges)
    .filter(([, v]) => v > 0)
    .map(([k, v]) => `${k}: ${v}`)
    .join(' | ');
}

function renderDetailRow(label, value) {
  return `
    <div class="detail-row">
      <div class="detail-label">${label}</div>
      <div class="detail-value">${typeof value === 'string' ? value : JSON.stringify(value)}</div>
    </div>
  `;
}
