# Breaking-Change Review — Preflight Card

**Date**: 2026-05-16
**Author**: Sisyphus (AI orchestrated)
**Branch**: master (HEAD: 311494f)
**Target**: CodeLattice v0.23

## 1. What & Why

### Problem
When AI agents modify code, there's no unified way to ask "will this change break external consumers, framework routes, public APIs, or callback registries?" CodeLattice has the signals (external_api_surface, framework_entry_hints, reachability_map, changed_symbols, docs) but they're disconnected.

### Goal
Add `codelattice_breaking_change_review` — an orchestration tool that combines existing signals into a single compatibility risk review. Given a list of changed symbols (or auto-detected from git diff), it cross-references each changed symbol against all caution systems and produces a structured risk report.

### What this IS
- Static graph + git diff + public surface + framework hints = heuristic compatibility review
- Risk classification with actionable checklist
- Release notes hints based on what changed

### What this is NOT
- Compiler/semver/runtime proof
- Auto version bump
- Auto release notes generation
- Deletion safety assessment
- WebUI

## 2. Support Scope

### Inputs
- `changedSymbols`: explicit list of symbols/files to review
- `diffMode`: auto-detect from git working tree (fallback)
- `includeExternalApi`, `includeFrameworkEntries`, `includeReachability`, `includeDocs`: toggles

### Risk Classification
| Risk | When |
|---|---|
| **critical** | Changed external API high + framework entry high + deleted/renamed public API |
| **high** | Changed external API high, changed framework route/CLI/component high, changed C/C++ header API, changed Rust lib.rs public, TS package exports, Python __init__.py re-export |
| **medium** | Changed reachable high fan-in symbol, changed public-like but undocumented, ambiguous changed symbol |
| **low** | Changed private/internal with low fan-in, no external/framework caution |
| **unknown** | No graph, no changed symbols, unable to classify |

## 3. Signal Integration
- **external_api_surface**: changed symbol appears → → changedExternalApi section, high risk
- **framework_entry_hints**: changed symbol appears → → changedFrameworkEntries section, route/callback verification
- **reachability_map**: symbol reachable from entries → → affectedReachability
- **docs**: symbol mentioned in docs → → docUpdateLikely, releaseNotesHints
- **changed_symbols**: auto-detect from git diff if no explicit list

## 4. Write Set / Forbidden Set
Same as previous packs — only CodeLattice repo, no new deps, no runtime execution, no WebUI.

## 5. Stop-Line
Stop if: baseline fails, cargo check errors > 3 attempts, any forbidden file touched.

## 6. Verification Plan
Same as previous — fmt, diff, tests (172→~182), dogfood (33→34), contract doc, real corpus smoke.
