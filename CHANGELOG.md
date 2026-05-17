# CodeLattice Changelog

All notable CodeLattice release changes are tracked here.

This project follows the release policy in `docs/release-versioning.md`. The product version comes from Cargo `workspace.package.version`; MCP `serverVersion` is a separate runtime/tool-profile version.

## [Unreleased]

### Fixed

- **WebUI graph controls and mixed-repo project selection**: replaced the native graph layout dropdown with segmented buttons so browser menus no longer cover the canvas, added a default-on wheel lock for G6 graphs, guarded G6 rendering against duplicate node/edge IDs in large Cangjie snapshots, and surfaced clickable candidate subprojects for monorepo roots while marking unsupported C# modules as unsupported instead of pretending to analyze them.
- **WebUI graph interaction polish**: the Graph tab now behaves as an exploration surface rather than a static picture: click selects and highlights a node, double-click or "drill down" focuses its 1/2-hop neighborhood, edge-mode filters switch between all/calls/structure, selected-node details show connected edges and neighbor count, and the visual density/labels/legend were tightened for large project graphs.
- **WebUI graph/network visualization and Cangjie retry UX**: the Graph tab now renders an SVG relationship network in addition to node/edge lists. Snapshot generation failures now surface the real analyze error, expand the workbench browse panel, and suggest candidate child projects instead of leaving users with a vague `generation failed`. Cangjie `sourceFile`/`symbol` node shapes are now counted correctly in WebUI snapshots.
- **WebUI loaded workbench project selection**: the post-analysis workbench runner panel now includes the same folder-selection path as the first screen: a native "Choose Folder" action plus an in-page directory browser with quick roots. Users no longer have to manually type a path after a snapshot has loaded.
- **WebUI Chinese workbench localization**: localized the loaded workbench path (runner panels, snapshot library, Live MCP controls, tab bar, dashboard/explore/graph/cleanup/release views) so Chinese mode no longer leaves the main analysis screen in English.
- **WebUI refresh-safe analysis state**: one-click analysis and snapshot-library loads now persist `snapshot=<id>` and `tab=<view>` in the URL/localStorage, so browser refresh restores the current analysis instead of returning to the project picker. Runner-served static assets now send no-store cache headers to avoid stale English JavaScript.
- **WebUI Project Picker folder selection**: the main "Choose Folder" action now uses the local runner to open a native macOS folder picker, with the in-page directory browser as fallback. Also fixed the default language precedence so a saved English preference is respected.
- **WebUI first-screen runner wiring**: exported shared viewer helpers for runner/live/report scripts, preserved empty-array API responses, and added cache-busting script URLs so the browser does not keep stale JavaScript after an update.
- **WebUI snapshot language display**: snapshot generation now falls back to the requested language when CLI metadata does not populate a language value, so Dashboard/Header no longer show `unknown` after a one-click analysis.
- **WebUI large-project snapshot generation**: fixed false `analyze_failed` results for large JSON output by removing `pipefail`-sensitive `echo | head | grep` checks, and surfaced generation errors inline instead of a context-free browser alert.
- **WebUI auto-language quality and graph context**: one-click `auto` analysis now reuses the language detected by `analyze` for `quality`, and graph snapshots keep package/file context before symbol nodes so large ArkTS projects show meaningful file/definition edges.

### Added

- **WebUI G6 Graph Engine Upgrade**: vendored AntV G6 5.1.1 as the default advanced graph renderer while keeping the existing SVG graph as fallback. Graph view now has a `G6 advanced graph / SVG fallback graph` engine selector, canvas-based zoom/pan/drag interaction, click-to-select, double-click drill-down, richer presentation backgrounds, and smoke/browser checks for the G6 adapter and vendor bundle.
- **WebUI graph presentation templates**: added multiple visual graph templates for product demos and social sharing: Code Galaxy, Module Clusters, Call Flow, Blueprint, and Engineering mode. Graph view also gains Poster Mode, larger SVG canvas styling, curved relation paths, weighted bubble sizing, and bilingual layout labels while preserving click, double-click drill-down, 1/2-hop focus, and edge filtering.
- **WebUI Phase I — Project Picker + One-Click Analyze + i18n (Chinese/English)**: Safe directory browser, one-click project analysis, full bilingual UI.
  - `webui/snapshot-viewer/i18n.js` — 100+ zh/en key translations, language toggle, localStorage persistence, auto-detect from browser language.
  - `scripts/webui-runner.py` — FS API (safe dir browse: `/api/fs/roots`, `/api/fs/list`, `/api/fs/validate-root`), quick-analyze endpoint (`POST /api/quick-analyze`: auto-create profile + generate snapshot + return result).
  - `webui/snapshot-viewer/runner.js` — Project Picker (recent profiles, directory browser dialog, one-click analyze), auto-loads result to Dashboard.
  - `scripts/webui-i18n-smoke.sh` — 23 checks: i18n syntax, zh/en messages, 15 key translations, picker UI.

- **WebUI Phase G — Live MCP Job Mode**: True MCP tool calls via runner, job lifecycle management, live result rendering in WebUI.
  - `scripts/webui-runner.py` — MCP backend: JSON-RPC stdio protocol calling `codelattice mcp`, initialize+list tools+call via subprocess. 6 workflow mappings (project_overview/symbol_search/impact_preview/project_insights/dead_code_candidates/release_check + custom_tool). Job lifecycle: queued→running→succeeded/failed/cancelled with thread worker. APIs: GET /api/mcp/status, GET /api/mcp/tools, GET/POST/DELETE /api/mcp/jobs, GET /api/mcp/job/<id>, POST /api/mcp/job/<id>/cancel.
  - `webui/snapshot-viewer/live.js` — Frontend: auto-detect MCP status, workflow selector, Run button, job list with status badges, poll interval, result viewer, cancel/delete, report integration.
  - `scripts/webui-live-mcp-smoke.sh` — Smoke test: MCP status, tools list (37 tools), create project_overview job, poll until success, list jobs, error cases (bad workflow, missing root), cancel, delete (10+ checks).
  - `scripts/webui-viewer-smoke.sh` — Phase G: live.js + 9 live functions (61 total checks).

- **WebUI Phase F — Beta Readiness + Product Hardening**: Contract test suite, browser smoke, beta sanity, beta user docs.
  - `scripts/webui-runner-contract-test.sh` — 21 API contract tests: 10 happy path + 11 error path (invalid JSON, missing root, root not found, root is file, unsupported lang, path traversal, missing snap/profile, delete missing, corrupt rebuild). All verify unified `{success,data,error,hint}` response format.
  - `scripts/webui-browser-smoke.sh` — Browser smoke: 10 static HTTP checks (HTML serves, JS assets 200, health API), page content checks (Profiles/Library/Guided/Report/Caution/Generate text), graceful browser skip (12+1 checks).
  - `scripts/webui-beta-sanity.sh` — Beta readiness aggregator: .gitignore check, runner host 127.0.0.1, subprocess.run usage, no shell=True, fixture path leak check (5 languages), no npm files, runs all 6 smoke scripts.
  - `docs/webui/beta-user-guide.md` — Beta user guide: quick start, 10 usage steps, cleanup.
  - `docs/webui/beta-safety-boundaries.md` — Safety boundaries: static-only, 127.0.0.1, no project writes, no code execution.
  - `docs/webui/troubleshooting.md` — Troubleshooting: port in use, generation failed, root not found, file vs runner mode, empty states.
  - `scripts/webui-runner-smoke.sh` — Rewritten for Phase E API format (8 checks).

- **WebUI Phase E — Project Workbench + Guided Review + Profiles**: 15-endpoint hardened runner API, project profiles, guided review workflows, report templates, workbench trial.
  - `scripts/webui-runner.py` — Rewritten: unified `{success,data,error,hint}` response structure; Project Profiles CRUD (list/create/get/update/delete); generate-snapshot for profile; snapshot library with search/filter/sort/pagination; index rebuild; path safety (sanitize IDs, validate roots); error handling (timeout/invalid JSON/unsupported language/missing root).
  - `webui/snapshot-viewer/runner.js` — Enhanced: profile list/create/select/delete/generate; snapshot library with search/filter-by-language/sort/Load/Diff/Timeline/Download/Delete operations; Guided Review (6 scenarios with purpose/tabs/steps/checklist/report, localStorage persistence).
  - `webui/snapshot-viewer/report.js` — Report Templates: template selector dropdown (6 templates: general/onboarding/before_edit/release/legacy/delete_code), guided report generation.
  - `scripts/webui-workbench-trial.sh` — End-to-end trial: creates 2 profiles, generates snapshots, filters by profile/language, validates schema/path-leak/no-error, rebuilds index, tests error cases (15 checks).
  - `scripts/webui-viewer-smoke.sh` — Phase E checks: profiles UI, guided review UI, report templates, 10+ Phase E functions (56 total checks, Matrix 5/5).

- **WebUI Phase D — Local Runner + Snapshot Library + Live-lite Analysis**: Python stdlib HTTP server, snapshot generation API, managed library with history.
  - `scripts/webui-runner.py` — Python HTTP server (127.0.0.1 only), serves webui/snapshot-viewer/ + REST API: health, generate-snapshot, snapshots list, snapshot detail, delete. Calls webui-snapshot.sh via subprocess, 120s timeout, JSON error responses.
  - `scripts/webui-runner.sh` — Shell wrapper: port selection, browser open, snapshot-dir config.
  - `webui/snapshot-viewer/runner.js` — Frontend client: auto-detect runner mode, project root input + language select + Generate button, Snapshot Library panel (load/compare/timeline from library). Graceful fallback to static file mode.
  - `scripts/webui-runner-smoke.sh` — Smoke test: starts runner on random port, checks health, generates Rust fixture snapshot, lists snapshots, validates detail (schema+data+no path leak), cleans up (6 checks).
  - `.gitignore` — Added `.codelattice-webui/` to exclude runner output.
  - `scripts/webui-viewer-smoke.sh` — Phase D checks: runner.js + 8 functions + runner-panel HTML (46 total checks).

- **WebUI Phase C — Timeline + Report Export + Review Workflow**: SVG trend charts, Markdown report generation, interactive review checklist.
  - `webui/snapshot-viewer/timeline.js` — Loads 2+ snapshots, builds metric timeline (8 metrics: source files/symbols/edges/graph nodes/graph edges/quality failed/dead code/unreachable), renders SVG line chart with value labels + metric selector buttons, computes first-to-last delta.
  - `webui/snapshot-viewer/report.js` — Markdown report generation (Dashboard/Quality/Graph/Diff/Timeline/Cleanup/Release/Checklist/Limitations/Recommended Verification), clipboard copy + .md download; interactive workflow checklist (10 scenarios, 5 items each, localStorage persistence, check all/reset toggle).
  - `webui/snapshot-viewer/styles.css` — Checklist hover states, checked-card left-border highlight.
  - `scripts/webui-viewer-smoke.sh` — Phase C: timeline.js/report.js existence + syntax + 15 core function checks (40 total checks).

- **WebUI Phase B — Graph Visualization + Snapshot Diff + Smoke Hardening**: Native graph rendering, two-snapshot diff comparison, hardened smoke validation.
  - `scripts/codelattice-snapshot-gen.py` — build_graph_section() extracts nodes/edges from CLI JSON with configurable limits (default 150 nodes/300 edges), computes call/file/symbol counts, marks stability=preview.
  - `webui/snapshot-viewer/index.html` — Added Graph tab (node/edge lists + detail panel) and Diff tab (summary delta cards, added/removed symbols/files, quality gate changes, limitation changes).
  - `webui/snapshot-viewer/app.js` — Added renderGraph (search/filter by kind, node detail), selectGraphNode (connected edge count), loadDiffSnapshot, computeAndRenderDiff (delta summary, symbol diff, file diff, quality changes), stableSymbolKey.
  - `webui/snapshot-viewer/styles.css` — Graph (node-spans, edge items, detail table) and Diff (controls, summary cards) styles.
  - `scripts/webui-viewer-smoke.sh` — Rewritten with subshell safety (no pipe-to-while-read), Phase B graph/diff UI element checks, 5-language fixture matrix validation with leak detection.
  - `scripts/webui-snapshot-smoke.sh` — Rewritten with standalone Python validator (avoids subshell counter loss), 5-language requirement enforcement.
  - All 5 fixture snapshots regenerated with graph section, zero path leaks.

- **WebUI Phase A — Rich Snapshot Viewer + Export Pipeline**: Snapshot enrichment from CLI analyze data, multi-language fixture matrix, 6-view enhanced viewer, workflow presets.
  - `scripts/codelattice-snapshot-gen.py` — Python enrichment engine: extracts explore symbols/source files from CLI JSON, computes heuristic cleanup/release/insight summaries, embeds 10 workflow presets. No external deps, stdin/stdout clean.
  - `scripts/webui-snapshot.sh` — Rewritten: new params `--full/--include-explore/--include-review/--include-workflows/--redact-root/--no-enrichment`. Safe bash → Python temp-file bridge. Default: full enrichment.
  - `webui/snapshot-viewer/app.js` — Rewritten (~250 lines): renderDashboard (quality passed/failed + generatedFrom), renderExplore (search/filter/sort/symbol detail/source files/top files), renderCleanup (4 subsections + cautions), renderReleaseReview (3 subsections), renderWorkflowPresets (10 preset cards).
  - `webui/snapshot-viewer/index.html` — Added Workflows tab, enhanced Dashboard (nodes/edges + metadata), enhanced Explore (source files + top files + sort), enhanced Cleanup/Release (caution lists).
  - `webui/snapshot-viewer/styles.css` — New components: workflow cards, file grid, symbol kind badges, detail table, caution list, meta list, rank badges.
  - `scripts/webui-viewer-smoke.sh` — Enhanced: multi-language matrix checks (Rust/TS/C/C++/Python), Phase A CSS/HTML/JS structure validation, path leak detection (35+ checks).
  - `fixtures/webui-snapshots/` — Multi-language fixture snapshot matrix: rust/typescript/c/cpp/python-portable-smoke.snapshot.json (all enriched, all redacted vs path leaks).
  - `docs/plans/2026-05-16-webui-phase-a-preflight.md` — Scope lock with stop-lines, acceptance criteria, execution plan.
  - `docs/plans/2026-05-16-webui-phase-a-closure.md` — Closure review with scope compliance, known limitations, next steps.

- **WebUI Snapshot Viewer MVP**: First static, read-only WebUI for visualizing CodeLattice snapshot JSON.
  - `webui/snapshot-viewer/index.html` — Main page: 5 views (Dashboard/Explore/Impact/Cleanup/Release), tab navigation, file/drag-drop loading, caution banner.
  - `webui/snapshot-viewer/styles.css` — Local dev tool aesthetic, responsive layout, CSS variables, zero dependencies.
  - `webui/snapshot-viewer/app.js` — Application logic (~540 lines): load/validate/normalize/render functions, search/filter, error handling.
  - `webui/snapshot-viewer/README.md` — Usage guide, loading methods, view descriptions.
  - `scripts/webui-viewer-smoke.sh` — Automated smoke test (34 checks): file existence, JS syntax, HTML structure, CSS features, JSON validation, contract compliance.
  - `docs/plans/2026-05-16-webui-snapshot-viewer-preflight.md` — Scope lock preflight.
  - `docs/plans/2026-05-16-webui-snapshot-viewer-closure.md` — Closure review.

### Changed

- README.md: Updated WebUI section to "Phase A — Rich Snapshot Viewer" status; added multi-language matrix, new CLI params, feature table with 6 views.

- **WebUI Snapshot Readiness**: New `docs/webui/` documentation pack with snapshot contract, MVP view specifications, and caution rendering guidelines for a future human-facing project visualization layer.
  - `docs/webui/README.md` — WebUI readiness overview, 5-view architecture (Dashboard, Explore, Impact, Cleanup, Release Review), MCP vs WebUI relationship.
  - `docs/webui/webui-mvp.md` — Detailed MVP view specs with layout suggestions, required data sections, stability labels, and per-view caution rendering rules.
  - `docs/webui/webui-snapshot-contract.md` — Full `CodeLatticeWebSnapshotV1` JSON contract definition with field stability labels (stable/preview/heuristic), section-by-section schema from MCP tool sources, minimal and full example structures.
  - `scripts/webui-snapshot.sh` — Snapshot generation script: aggregates CLI analyze + quality output into contract-compliant JSON; supports `--root`, `--language`, `--output` (file or stdout), `--compact`; uses bash + Python stdlib only; no new dependencies.
  - `scripts/webui-snapshot-smoke.sh` — Automated smoke test: generates Rust and TypeScript fixture snapshots, validates JSON parse, schemaVersion, generatedFrom flags, summary counts, quality section, and limitations section.
  - `fixtures/webui-snapshots/rust-portable-smoke.snapshot.json` — Pre-generated Rust fixture snapshot (5.2 KB).
  - `fixtures/webui-snapshots/typescript-portable-smoke.snapshot.json` — Pre-generated TypeScript fixture snapshot (4.7 KB).
  - `docs/plans/2026-05-16-webui-readiness-preflight.md` — Pre-flight plan document.
  - `docs/plans/2026-05-16-webui-readiness-closure.md` — Closure review document.

### Changed

- README.md: Added "WebUI Readiness" section with quick-start commands, documentation index, and hard boundaries.

## [0.15.0-beta.1] - 2026-05-16

### Added

- **AI Prompt Cookbook**: New user guides under `docs/guides/` with copyable prompts and workflow preset explanations for onboarding, before/after edit review, dead-code investigation, release checks, legacy cleanup, public API changes, framework-route changes, and docs/tests/config synchronization.

- **AI Workflow Presets** (v0.26): New MCP tool — returns suggested MCP workflow steps for 10 common scenarios. Does not execute analysis (presetOnly=true).
  - Scenarios: onboarding, before_edit, after_edit, delete_code, release_check, legacy_cleanup, public_api_change, framework_route_change, docs_tests_sync, config_examples_sync.
  - 10 integration tests. MCP tool count: 36 → 37.

- **Config/Examples Review** (v0.25): New MCP tool `codelattice_config_examples_review` — scans package.json, tsconfig, Cargo.toml, pyproject.toml, CI, Docker, examples, and docs code blocks for stale references. Never executes scripts or builds.
  - 10 new integration tests. MCP tool count: 35 → 36.

- **Consistency Review (Docs & Tests)** (v0.24): New MCP tool `codelattice_consistency_review` — cross-references changed symbols against documentation and test files to flag stale docs, missing docs, related tests, missing tests, and stale tests.
  - File-based doc scanner (README.md, docs/*.md) and test file discovery.
  - Consistency risk levels: critical/high/medium/low.
  - Review checklist with P0/P1 priorities.
  - All output: coverageVerified=false, runtimeVerified=false — never runs tests or claims coverage.
  - New fixture: `fixtures/typescript/consistency-review/` (10 files).
  - 10 new integration tests.
  - MCP tool count: 34 → 35.
  - No new dependencies.

- **Breaking-Change Review** (v0.23): New MCP tool `codelattice_breaking_change_review` — cross-references changed symbols against public API surface, framework entry hints, and documentation to assess compatibility risk.
  - Orchestrates external_api_surface, framework_entry_hints, README docs, and graph metadata.
  - Compatibility risk levels: critical/high/medium/low/unknown.
  - Review checklist with P0/P1/P2 priorities and release notes hints.
  - All output: externalUsageVerified=false, runtimeVerified=false — never claims proof.
  - New fixture: `fixtures/typescript/breaking-change-review/` (8 files).
  - 10 new integration tests.
  - MCP tool count: 33 → 34.
  - No new dependencies.

- **Framework Entry Hints / Callback Entry Caution** (v0.22): New MCP tool `codelattice_framework_entry_hints` — identifies symbols likely invoked by framework routing, decorators, callback registries, or CLI commands via static heuristics.
  - Language-specific detection: Python (routes.py/cli.py patterns), TypeScript (Next.js/Express file-based routes, React TSX components, Next.js GET/POST handlers), ArkTS (@Entry/lifecycle methods), Rust (handler/main patterns), C/C++ (callback/hook naming, header API), Cangjie (handler/component naming).
  - Scoring based on file path patterns, symbol naming conventions, public/exported status.
  - Caution levels and recommended verification steps per hint kind (route/cli/component/callback/lifecycle).
  - Compact mode for AI-agent-friendly output.
  - All output includes `generatedFrom: { runtimeVerified: false, heuristic: true, compilerVerified: false }` — never claims runtime proof.
  - Framework entry hints integrated into `codelattice_reachability_map` summary and `codelattice_dead_code_candidates` caution system.
  - New fixtures: `fixtures/python/framework-entry-hints/` (5 files), `fixtures/typescript/framework-entry-hints/` (8 files).
  - 10 new MCP integration tests.
  - MCP tool count: 32 → 33.
  - No new dependencies.

- **External API Surface / Public API Caution** (v0.21): New MCP tool `codelattice_external_api_surface` — identifies symbols likely exposed to external consumers, with caution levels and recommended verification steps.
  - Scoring heuristics for Rust (`pub` visibility, `lib.rs`, `pub use` re-exports) and TypeScript/ArkTS (`export`, `index.ts`, re-exports, `package.json` exports/bin, TSX components).
  - Package metadata integration: reads `package.json` exports/main/types/bin fields and `Cargo.toml` lib/bin targets.
  - README/Doc cross-reference: symbols mentioned in documentation get additional confidence.
  - Caution levels (high/medium/low) based on cumulative score, with per-symbol reasons.
  - Compact mode for AI-agent-friendly output.
  - All output includes `generatedFrom: { externalUsageVerified: false, heuristic: true, compilerVerified: false }` — no proof/guarantee/deletion-safe language.
  - New fixture: `fixtures/typescript/external-api-surface/` (10 files with package.json exports/bin, index.ts re-exports, public/internal split, CLI entry, TSX component).
  - 10 new MCP integration tests.
  - MCP tool count: 31 → 32.
  - No new dependencies.

- **Entry Point & Reachability Map** (v0.20): New MCP tool `codelattice_reachability_map` — multi-language entry point detection + static BFS reachability analysis.
  - Entry point detection heuristics for Rust, Python, TypeScript/ArkTS, C, C++, and Cangjie (main, lib.rs, index.ts, __init__.py, etc.), with confidence scores and reason tags.
  - BFS reachability traversal from detected entry points (configurable max depth 1–32, default 8), following CALLS/REFERENCES/IMPORTS/INCLUDES/DEFINES edges.
  - Unreachable candidate classification: symbols not reachable from any entry point, with cautions for public API exposure and dynamic dispatch patterns.
  - Compact mode for AI-agent-friendly output (omits verbose IDs).
  - All output includes `generatedFrom: { staticAnalysisOnly: true, heuristic: true, compilerVerified: false }` — no proof/guarantee/deletion-safe language.
  - Reachability summary integrated into `codelattice_dead_code_candidates`, `codelattice_project_insights` (release_check mode), and `codelattice_review_plan`.
  - New fixture: `fixtures/typescript/reachability-map/` (9 files with layered architecture, public API surface, dynamic dispatch patterns).
  - 10 new MCP integration tests.
  - MCP tool count: 30 → 31.
  - No new dependencies.

- **Graph Diagnostics Pack** (v0.19): 5 new MCP tools that package existing static graph capabilities into user-scenario-oriented diagnostic tools, turning CodeLattice from a "code graph analyzer" into a "local code diagnostics engine."
  - `codelattice_impact_analysis` — Change impact analysis: find direct callers/callees, upstream/downstream paths, entry point reachability, risk scoring, read-first and review-first recommendations.
  - `codelattice_risk_hotspots` — Project risk hotspot detection: identify high fan-in/fan-out symbols and files, cross-module dependencies, public API exposure, and quality metric anomalies.
  - `codelattice_architecture_drift` — Architecture health analysis: detect cycle candidates, cross-layer calls (with user-provided layer rules), boundary leaks, and overly coupled modules.
  - `codelattice_ai_context_pack` — AI editing context: given a task description or target symbols, output relevant files, key symbols, call chains, suggested read order, and do-not-assume warnings — ready to feed directly into AI assistants.
  - `codelattice_review_gate` — Diff-based review gate: analyze git diff or specified changed files for touched symbols, hotspot exposure, impact summary, and review checklist.
  - All 5 tools follow unified output contract: `generatedFrom.staticAnalysisOnly=true`, `heuristic=true`, `compilerVerified=false`. No proof/guarantee/deletion-safe language.
  - New fixture: `fixtures/typescript/graph-diagnostics/` (8 files with layered architecture: api → service → domain → infra, cycle candidates, test file).
  - 20 new MCP integration tests.
  - MCP tool count: 25 → 30.
  - No new dependencies.

- **Dead Code Candidate Analysis** (v0.18): New MCP tool `codelattice_dead_code_candidates` identifies statically unreachable symbols and files via graph-based reachability analysis.
  - Scoring algorithm: per-symbol and per-file candidate scoring with positive signals (no incoming edges, not reachable from entry points, private visibility) and negative signals (public/exported, entry-like name, dynamic patterns).
  - Confidence tiers: high (>=0.80), medium (>=0.55), low (<0.55). Candidates below 0.45 are excluded.
  - Entry point detection: language-specific heuristics for main/lib.rs/index.ts etc., user-provided `entryHints`, BFS reachability traversal (max depth 8).
  - Public API cautions: exported/public symbols get confidence capped at "medium" with `public-api-may-have-external-callers` caution.
  - Dynamic feature cautions: registry/plugin/route patterns get `dynamic-dispatch-may-hide-callers` caution and -0.15 score penalty.
  - All output explicitly states `deletionSafe: false` and includes `static-analysis-only` caution — never claims deletion proof.
  - New fixture: `fixtures/typescript/dead-code-candidates/` (7 files covering live, legacy, public-api, dynamic, and test scenarios).
  - 9 new MCP integration tests (feature-gated behind `tree-sitter-typescript`).
  - MCP tool count: 24 → 25.
  - No new dependencies.

### Changed

- README / GitCode homepage now describes the current master as a diagnostics-oriented beta candidate with 37 MCP tools, while keeping `v0.14.0-beta.1` as the latest published GitCode Release until a new release page is created.
- Packaging and smoke thresholds now expect the current 37-tool MCP profile for local `0.15.0-beta.1` candidate artifacts.

### Breaking Changes

- None.

### Known Limitations

- All new diagnostic and review tools are static, heuristic, and not compiler verified.
- They do not prove runtime behavior, external API usage, test coverage, or safe deletion.
- This package candidate is not GA and has not been published as a GitCode Release page yet.

## [0.14.0-beta.1] - 2026-05-16

### Added

- **Unified quality metrics** across MCP outputs: `qualityMetrics` is now available in project overview, project insights, review plan release checks, and production assist.
- **Real-project corpus baseline** for beta validation, covering Redis (C), Catch2 (C++), and pip (Python) as the default non-vendored smoke/baseline set.
- **TypeScript path alias and monorepo import resolution**: tsconfig `baseUrl` / `paths`, `extends`, extensionless imports, index resolution, and workspace package imports now resolve to real files where possible.
- **Python import resolution refinement**: package-aware module index, src-layout / flat-layout detection, relative imports, parent-relative imports, aliases, and simple `__init__.py` re-export chains.
- **C and C++ compile_commands include resolution**: `-I`, `-iquote`, `-isystem`, forced includes, diagnostics for unresolved/system includes, and no synthetic unresolved include targets.
- **Release beta notes** for `0.14.0-beta.1` and a real corpus baseline report for external beta validation.

### Changed

- README front matter is now product-facing Chinese copy: CodeLattice is presented as a local code intelligence engine for large, legacy, and complex codebases.
- Release artifact and release smoke now target the full seven-language beta set: Rust, Cangjie, ArkTS, TypeScript, C, C++, Python.
- Release manifests now record source commit, product version, MCP serverVersion, language support flags, tool count, and build features.
- MCP/install documentation now uses parameterized stable wrapper paths such as `/path/to/CodeLattice-Tool/codelattice-mcp.sh`.

### Fixed

- Local promotion, install, and release packaging scripts build and validate the full optional language set: Cangjie, ArkTS, TypeScript, C, C++, and Python.
- Default `cargo test` no longer compiles C/C++ graph integration tests that require `tree-sitter-c` / `tree-sitter-cpp` unless those features are enabled. The same graph tests still run under `cargo test --all-features`.

### Breaking Changes

- None.

### Known Limitations

- CodeLattice is not a compiler, IDE, language server, or hosted upload service.
- Rust does not perform full type inference, trait solving, or macro expansion.
- C/C++ do not perform full preprocessing, template instantiation, overload resolution, or virtual dispatch resolution.
- TypeScript does not run `tsc` and does not provide type-system guarantees.
- Python analysis is static and does not execute imports, virtual environments, monkey patches, or dynamic import code.
- Beta users should pin versions and run self-test / release smoke after upgrades.

## [0.13.0-beta.2] - 2026-05-15

### Added

- Release artifact now builds with `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript`, so the published macOS Apple Silicon tarball includes Rust, Cangjie, ArkTS, and TypeScript adapters.
- Packaged release smoke now includes ArkTS and TypeScript portable fixtures and verifies they analyze successfully from the unpacked tarball.
- MCP `initialize.serverInfo` now reports `arktsSupport` and `typescriptSupport` alongside `cangjieSupport`, making packaged language capability drift visible.

### Fixed

- Fixed `v0.13.0-beta.1` packaging drift where README/CHANGELOG advertised ArkTS production-trial support but the published binary was built without `tree-sitter-arkts`.

### Notes

- `v0.13.0-beta.1` remains immutable for checksum integrity. `v0.13.0-beta.2` supersedes it for external beta users who need ArkTS or TypeScript from the prebuilt package.

## [0.13.0-beta.1] - 2026-05-15

### Added

- `feat(mcp)`: add compact AI-sidecar outputs (`a7b1652`) - compact mode for MCP tools returns stripped-down results for AI context efficiency.
- `feat(arkts)`: complete production trial analysis path (`559f44a`) - ArkTS/HarmonyOS analysis works end-to-end via tree-sitter-typescript, component/buildMethod extraction.
- `feat(mcp)`: detect changed symbols from git diff (`9d0b157`) - auto-detect changed symbols from unstaged/staged/all git diff, map hunks to graph symbols.
- `feat(mcp)`: explain impact risk for AI review (`c674d19`) - impact preview returns riskReasons, impactMetrics, confidenceSummary, reviewFocus.
- `feat(mcp)`: associate code changes with docs (`7c19d41`) - static doc graph, DocScanner, code ↔ docs association for AI sidecar.
- `feat(typescript)`: add Phase A local graph support (`fb3719c`) - TypeScript language adapter, symbols, imports, calls.
- `feat(mcp)`: add persistent analysis cache (`c44b51d`) - two-layer cache (memory LRU + persistent disk), fingerprint stale detection, structured staleReasons.

### Changed

- (No breaking changes in this release cycle.)

### Documentation

- (Documentation updates tracked per feature above.)

### Known Limitations

- **TypeScript**: no path alias resolution, no monorepo/workspace support, no TSX framework hints.
- **ArkTS**: struct keyword parsed as ERROR by tree-sitter-typescript, no @Builder/@Extend, no full ArkUI declarative syntax tree.
- **Persistent cache**: no per-symbol incremental recompute.
- **Call edges** are heuristic with confidence/reason, not compiler-verified.
- **No project script execution**.
- **Not a compiler, IDE, language server, or hosted service**.

## [0.1.0] - 2026-05-11

### Added

- Public `codelattice` release binary, while retaining `gitnexus-rust-core-cli` as a compatibility binary.
- Portable release tarball packaging with `manifest.json`, stable MCP wrapper, checksums, docs, and Rust/Cangjie smoke fixtures.
- Release smoke validation for packaged binaries, wrapper self-test, MCP `tools/list`, and portable Rust/Cangjie fixture analysis.
- Fresh clone smoke workflow for external-user setup validation without writing AI client configuration files.
- Portable MCP install/promote scripts with configurable install directories.

### Changed

- README and getting-started docs now present CodeLattice as a standalone local code intelligence engine for Rust and Cangjie projects.
- MCP setup docs and generated config snippets now prefer stable promoted runtime paths over developer checkout wrappers.

### Fixed

- Cangjie `project_overview` compact output now reports nonzero top-level symbol, source file, and edge counts for populated projects.
- Install and promote scripts no longer assume the original author's machine path.

### Compatibility

- The Cargo package and compatibility binary name `gitnexus-rust-core-cli` remain available for existing scripts.
- The public command surface should prefer `codelattice` for new usage.
