# Python Import Resolution Refinement Pack — Preflight

Date: 2026-05-16

## Goal

Enhance Python import resolution in CodeLattice to resolve relative imports, package imports, and simple `__init__.py` re-exports to real file/symbol targets instead of synthetic `py:mod:` nodes.

## Resolution Scope (what this pack resolves)

1. **Absolute module imports**: `import shop.config`, `import shop.config as config` → target `shop/config.py`
2. **Absolute from-imports**: `from shop.models import Order` → target `Order` symbol in `shop/models.py`
3. **Relative from-imports**: `from .services import OrderService` → resolve to parent package + module
4. **Sibling imports**: `from . import config` → resolve to sibling module file
5. **Parent relative imports**: `from ..config import DEFAULT_CURRENCY` → resolve to grandparent package + module
6. **`__init__.py` re-exports**: `from .api import create_order` in `__init__.py` → make `from shop import create_order` work
7. **Aliases**: `from .config import DEFAULT_CURRENCY as CURRENCY` → preserve alias, resolve target
8. **src-layout detection**: `src/shop/...` → package root is `src/`
9. **flat-layout detection**: `shop/...` → package root is project root

## NOT in scope

- Star import expansion (`from .models import *`) → diagnostic only
- Dynamic imports (`importlib.import_module`, `__import__`, `eval`, `getattr`) → diagnostic only
- `try/except` conditional re-exports → treated as unresolved
- site-packages / virtualenv resolution → never
- Type inference / type annotations → never
- Runtime execution of any Python code → never
- Cross-repo imports → never

## Confidence / Reason Strategy

| Resolution Type | Confidence | Reason |
|----------------|------------|--------|
| Exact module file match | 0.90 | `python-exact-module-import` |
| Exact symbol in module | 0.85 | `python-exact-symbol-import` |
| `__init__.py` re-export chain | 0.75 | `python-init-reexport` |
| Import alias resolved | 0.80 | `python-import-alias-resolved` |
| Unresolved (diagnostic only) | no edge | diagnostic reason codes below |

### Diagnostic reason codes (no edge created)

| Code | When |
|------|------|
| `python-import-module-not-found` | Module path has no matching file |
| `python-import-symbol-not-found` | Module found but symbol not in module |
| `python-relative-import-outside-package` | Relative import in non-package file |
| `python-star-import-not-expanded` | Star import detected |
| `python-dynamic-import-not-resolved` | importlib / eval / getattr detected |

### Edge creation rules

- Resolved module import → IMPORTS edge: `file_node → target_file_node`
- Resolved symbol import → IMPORTS edge: `file_node → target_symbol_node`
- Unresolved import → diagnostic only, **no edge** (no dangling edges)
- Star import → diagnostic only, no edge
- Dynamic import → diagnostic only, no edge

## Write Set

### New files
- `crates/python/src/module_resolution.rs` — PythonModuleIndex, resolution logic
- `crates/python/tests/import_resolution.rs` — crate-level tests (if tests dir doesn't exist, create it)
- `fixtures/python/import-resolution/` — 10-file fixture covering all import patterns
- `docs/plans/2026-05-16-python-import-resolution-preflight.md` (this file)
- `docs/plans/2026-05-16-python-import-resolution-closure.md` (to be created at end)

### Modified files
- `crates/python/src/lib.rs` — expose `module_resolution` module
- `crates/python/src/graph.rs` — use PythonModuleIndex in import edge building, add diagnostics
- `crates/python/src/extractors/call.rs` or `graph.rs` — import-aware call resolution
- `crates/cli/src/lib.rs` — build PythonModuleIndex and pass to build_python_graph
- `crates/cli/tests/mcp_server.rs` — new MCP tests for import resolution
- `docs/architecture/unified-output-contract.md` — Python resolution reason codes
- `docs/architecture/mcp-v0-contract.md` — Python import resolution behavior
- `CHANGELOG.md` — Unreleased section
- `README.md` — Python support scope update

## Forbidden Set

- Do NOT modify GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool
- Do NOT modify real project source code
- Do NOT add new Cargo dependencies
- Do NOT create dangling edges for unresolved imports
- Do NOT create synthetic `py:mod:` nodes for resolved imports (replace with real targets)
- Do NOT execute Python code or read site-packages
- Do NOT expand star imports
- Do NOT do type inference
- Do NOT change existing MCP field semantics
- Do NOT do TypeScript path alias, C/C++ include path, or any other language

## Stop-line

- If cargo test fails → fix before proceeding
- If existing MCP tool outputs break → stop and reassess
- If pip-python real-project compare regresses >20% in node/edge counts → investigate before proceeding

## Verification Plan

1. `cargo fmt --check`
2. `git diff --check`
3. `cargo test -p gitnexus-python --features tree-sitter-python`
4. `cargo test --test mcp_server --features tree-sitter-python`
5. `cargo test --all-features`
6. `python3 scripts/real-project-corpus-smoke-test.py`
7. Real-project pip-python cached compare (if cache exists)
8. Tool detect-changes

## Expected Impact on qualityMetrics

- `edgeCount` should increase for pip-python (more resolved import edges)
- `lowConfidenceEdgeRate` may decrease (resolved imports at 0.85-0.90 confidence)
- `danglingEdgeCount` should stay 0 (we don't create dangling edges)
- `unresolvedImportOrIncludeCount` may change (new diagnostics for star/dynamic imports)
- `callQuality.lowConfidenceCallRate` may decrease slightly (import-aware call resolution)
