# Framework Entry Hints — Preflight Card

**Date**: 2026-05-16
**Author**: Sisyphus (AI orchestrated)
**Branch**: master (HEAD: 2879f29)
**Target**: CodeLattice v0.22

## 1. What & Why

### Problem
`reachability_map` and `dead_code_candidates` use static call-graph reachability: symbols reachable from detected entry points = "alive", unreachable = "dead candidate". This works for ordinary call chains but fails when symbols are invoked by frameworks, runtimes, decorators, or callback registries — patterns where no static call edge exists.

### Goal
Add a **framework entry hints** layer that identifies symbols likely invoked by framework routing, decorator-based registration, callback tables, or CLI command registration. These hints reduce false positives in dead-code/reachability analysis by marking framework-called symbols with cautions.

### What this IS
- Static heuristics identifying symbols with framework/callback *patterns*
- Caution annotations injected into existing tools
- Deterministic, read-only, no runtime execution

### What this is NOT
- Runtime proof (never claims verified external usage)
- Framework parser (never executes decorators, macros, or config)
- Deletion safety assessment
- WebUI or interactive dashboard

## 2. Support Scope

### Languages supported
| Language | Route Hints | CLI Hints | Component Hints | Callback Hints | Lifecycle Hints |
|---|---|---|---|---|---|
| Python | FastAPI/Flask-style decorators, route file paths | click/typer command decorators | — | callback/handler names | — |
| TypeScript | Express/Next.js-style routes, file-based route paths | — | React/TSX PascalCase exports | Next.js GET/POST/loader/action | — |
| ArkTS | — | — | @Entry, build() | — | aboutToAppear, aboutToDisappear, onPageShow, onPageHide |
| Rust | route/handler/command names, Router/get()/post() text | clap/subcommand names | — | callback/handler names | — |
| C/C++ | — | — | — | callback/handler/hook/init names, function pointer patterns | — |
| Cangjie | route/handler/controller names | — | component/page names | — | — |

### Not in scope
- Framework-specific config parsing (next.config.js, FastAPI app init, axum Router::new())
- Type inference for decorator return types
- Macro expansion for Rust #[route] or #[get]
- C/C++ function pointer target resolution (only flag the symbol as callback-like)
- Full lifecycle method detection (only ArkTS well-known lifecycle names)

## 3. Scoring Strategy

### Positive signals
| Signal | Score | Language |
|---|---|---|
| Explicit decorator/annotation (@app.get, @click.command, @Entry) | +0.40 | Python, ArkTS |
| Route file path (routes/, pages/, api/, app/) | +0.25 | All |
| Entry-like exported function | +0.20 | All |
| Callback/handler name pattern | +0.15 | All |
| Public/exported symbol | +0.15 | All |
| Docs mention route/API/handler | +0.10 | All |
| Source text contains route registration pattern | +0.30 | Python, TS, Rust |

### Negative signals
| Signal | Score |
|---|---|
| Private/internal name pattern (_prefix, internal) | -0.20 |
| Test/generated/vendor path | exclude (unless includeTests) |

### Confidence levels
- **high**: score ≥ 0.80
- **medium**: score ≥ 0.55
- **low**: score < 0.55
- Output threshold: score ≥ 0.35

## 4.How Existing Tools Are Enhanced

### codelattice_reachability_map
- `summary` gains: `frameworkEntryHintCount`, `routeHintCount`, `callbackHintCount`
- `entryPoints` may include high-confidence framework hints
- Unreachable candidates tagged: `framework-callback-may-hide-callers`

### codelattice_dead_code_candidates
- Symbols matching framework hints: dead-code score reduced, confidence capped at medium
- Reasons/cautions include: `framework-entry-hint`, `framework-callback-may-hide-callers`
- Recommended verification includes route/handler checks

### codelattice_external_api_surface
- Component/route/handler symbols get additional surface caution

### codelattice_review_plan
- release_check / before_edit: checklist includes "Check framework route/callback registration"

### codelattice_project_insights
- readFirst/reviewFirst may prioritize route/handler files

## 5. Write Set / Forbidden Set

### Write Set
- `crates/cli/src/mcp_server.rs`: new handler + schema + dispatch + integration
- `crates/cli/tests/mcp_server.rs`: 10+ new tests
- `fixtures/python/framework-entry-hints/`: Python fixture (5+ files)
- `fixtures/typescript/framework-entry-hints/`: TypeScript fixture (7+ files)
- `docs/plans/`: preflight + closure docs
- `docs/architecture/mcp-v0-contract.md`: tool #33
- `CHANGELOG.md`, `README.md`
- `scripts/mcp-dogfood.sh`, `scripts/codelattice-mcp.sh`
- `.gitignore`: `__pycache__/` if not present

### Forbidden Set
- No GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool modifications
- No Codex/opencode/Claude config changes
- No new dependencies
- No real project source modifications
- No npm/pip/pytest/tsc execution
- No WebUI

## 6. Stop-Line
Stop and ask if:
- Baseline tests fail before changes
- `cargo check` produces errors that can't be resolved in 3 attempts
- Any forbidden file is touched accidentally
- Tool count does not match expectations (33 tools expected)
- Integration breaks any existing tool output contract

## 7. Verification Plan
1. `cargo fmt --check` — zero diff
2. `git diff --check` — no whitespace issues
3. `cargo test -p gitnexus-rust-core-cli --features tree-sitter-typescript --test mcp_server` — all tests pass
4. `cargo test -p gitnexus-rust-core-cli --features tree-sitter-python,tree-sitter-typescript --test mcp_server` — new tests pass
5. `scripts/mcp-dogfood.sh` — 33/33
6. `scripts/codelattice-mcp.sh --self-test` — tool count >= 33
7. `cargo check -p gitnexus-rust-core-cli` — zero errors (default + full features)
