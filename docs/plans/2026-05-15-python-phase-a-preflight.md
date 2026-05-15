# Python Phase A Preflight

Date: 2026-05-15

## Goal

Add Python Phase A static code graph support to CodeLattice for local Python project analysis.

## Context

- C++ Phase A committed: `da451e0`
- tree-sitter-python 0.25.0 compatible with tree-sitter 0.26
- Python fixture covers: package, module imports, from imports, aliased imports, functions, classes, methods, constructor, async function, constants, test functions, entry point

## Design Decisions

1. **Crate**: `crates/python/` with feature `tree-sitter-python`
2. **Dependencies**: `tree-sitter = "0.26"`, `tree-sitter-python = "0.25"`
3. **Project detection**: pyproject.toml > setup.py > setup.cfg > requirements.txt > lockfile > plain (.py files)
4. **File extensions**: `.py` (source), `.pyi` (stubs, optional)
5. **Excluded dirs**: __pycache__, .venv, venv, .tox, site-packages, etc.
6. **Symbol kinds**: Module, Function, AsyncFunction, Class, Method, Constructor, Variable, Constant, TestFunction, Decorator
7. **Visibility**: Public (default), Private (_prefix), Dunder (__name__)
8. **Import kinds**: Import, ImportAs, FromImport, FromImportAs, RelativeImport, StarImport
9. **Call confidence tiers**:
   - 0.90: direct same-file/imported function call
   - 0.80: imported function call, module-qualified project call
   - 0.75: class constructor name match
   - 0.60: name-only cross-file candidate
   - 0.45: receiver method name only
   - 0.20: star-import ambiguous (diagnostic)
10. **Bridge format**: Same pattern as C++ bridge

## Fixture

`fixtures/python/portable-smoke/`:
- pyproject.toml
- src/sample_app/__init__.py
- src/sample_app/main.py
- src/sample_app/math_utils.py
- src/sample_app/service.py
- tests/test_math_utils.py

Expected output: ~29 nodes, ~46 edges, ~23 symbols, 5 source files, ~18 call edges

## Scope

- Phases 1-13: crate, project discovery, fixture, extractors, graph, CLI, MCP, tests, scripts, docs, verification, index refresh, commit
