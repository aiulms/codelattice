# Preflight: MCP v0 Thin stdio Wrapper

> **日期：** 2026-05-10
> **类型：** Preflight
> **状态：** ✅ Pass — Implementation confirmed safe
> **Base commit：** `582014e`

---

## 1. Goal

Add a minimal MCP v0 stdio server to CodeLattice, enabling AI agents to call `analyze`, `quality`, `summary`, and `smoke` via MCP JSON-RPC protocol over stdin/stdout.

## 2. Current CLI Structure

### 2.1 CLI Crate

- **Package**: `gitnexus-rust-core-cli` in `crates/cli/`
- **Dependencies**: `clap` 4, `serde` 1, `serde_json` 1, `gitnexus-project-model`, optional `gitnexus-cangjie`
- **Entry point**: `crates/cli/src/main.rs` (1138 lines)
- **Subcommands**: `project-model inspect`, `cangjie inspect/graph`, `analyze`, `quality`, `summary`

### 2.2 Existing Command Handlers

All analysis functions are private in `main.rs`:
- `run_rust_analysis(root)` → `Result<(Value, Vec<Value>, Vec<Value>), String>`
- `run_cangjie_analysis(root)` → same (feature-gated)
- `compute_rust_quality_gates(nodes, edges)` → `Vec<QualityGateResult>`
- `compute_cangjie_quality_gates(nodes, edges)` → same (feature-gated)
- `build_rust_summary(json_val, nodes, edges)` → `GraphSummary`
- `build_cangjie_summary(nodes, edges)` → `GraphSummary` (feature-gated)
- `check_root(root)` → `Result<&Path, String>`
- `resolve_language(lang_arg, root)` → `Result<String, String>`
- `now_iso8601()` → `String`

### 2.3 Reuse Strategy

Since existing functions are private in `main.rs`, the MCP server will use **subprocess approach**:
- Spawn `current_exe()` with appropriate subcommand (analyze/quality/summary)
- Parse JSON stdout from subprocess
- For smoke, call `bash scripts/alpha-trial-smoke.sh` directly

This avoids refactoring main.rs and keeps MCP as a true thin wrapper.

## 3. MCP Protocol Design

### 3.1 Transport

**Newline-delimited JSON-RPC over stdio.** Each message is a single JSON object on one line.

- Input: read from stdin line by line
- Output: write to stdout (JSON only)
- Logging: stderr only

### 3.2 Methods

| Method | Purpose |
|--------|---------|
| `initialize` | Server handshake, return capabilities |
| `notifications/initialized` | Client notification, no response |
| `tools/list` | Return 4 tool definitions with JSON Schema |
| `tools/call` | Execute a tool |
| `shutdown` / `exit` | Graceful termination |

### 3.3 Tools

| Tool | Input | Output |
|------|-------|--------|
| `codelattice_analyze` | root, language?, strict?, includeGraph? | summary + quality + optional graph |
| `codelattice_quality` | root, language? | pass/fail per gate |
| `codelattice_summary` | root, language? | compact stats + quality summary |
| `codelattice_smoke` | mode? (rust-only/cangjie-only/full) | pass/fail/skip counts |

## 4. Dependencies

**No new dependencies.** MCP framing implemented with:
- `serde_json` for JSON parsing/serialization
- `std::io::{BufRead, Write}` for stdio I/O
- `std::process::Command` for subprocess calls
- `std::time::Duration` + `child.wait_with_timeout()` pattern for timeouts

## 5. Path Safety

### 5.1 Allowlist Strategy

Default-deny for explicitly forbidden paths, default-allow for everything else:

| Path | Status |
|------|--------|
| `/Users/jiangxuanyang/Desktop/cangjie` | **DENIED** (live repo) |
| Any other existing directory | ALLOWED |

### 5.2 Validation

```rust
fn validate_root_path(root: &str) -> Result<PathBuf, McpError>
```

1. Check path exists
2. Check path is a directory
3. Check against deny list
4. Return canonicalized path

## 6. Error Handling

Structured error responses:

| Error | When |
|-------|------|
| `path_denied` | Root path is on deny list |
| `command_failed` | Subprocess exit non-zero |
| `timeout` | Subprocess exceeded time limit |
| `json_parse_failed` | Subprocess output not valid JSON |
| `unsupported_language` | Language not "rust"/"cangjie"/"auto" |
| `smoke_failed` | Smoke script failed |
| `cangjie_disabled` | Cangjie requested but feature not compiled |

## 7. Stop-line Check

| Stop-line | Status |
|-----------|--------|
| No new dependencies | ✅ |
| No live repo writes | ✅ Read-only analysis |
| No default tool switch | ✅ MCP is explicit opt-in |
| No GitNexus-RC modification | ✅ |
| No runtime behavior change | ✅ Thin wrapper only |
| No stdout pollution | ✅ stderr for logging |
| No recursive MCP calls | ✅ Uses subcommands |

## 8. Test Strategy

| Test | What |
|------|------|
| tools/list | Verify 4 tools with correct schemas |
| codelattice_analyze (Rust) | portable-smoke fixture returns summary |
| codelattice_quality (Rust) | portable-smoke fixture returns pass |
| codelattice_summary (Rust) | portable-smoke fixture returns summary |
| codelattice_smoke (rust-only) | Returns pass counts |
| path_denied | Live repo rejected |
| nonexistent_path | Non-existent directory rejected |
| MCP framing | JSON-RPC id matching, sequence |

## 9. Verdict

**PASS.** Implementation is safe:
- Thin wrapper over existing CLI subcommands
- No new dependencies
- No behavior changes
- Clear safety boundaries (path deny list, read-only, no recursion)
- Subprocess approach keeps MCP isolated from core logic
