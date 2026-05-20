# Permission-aware Root Cause Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP permission metadata and a read-only root-cause evidence assistant.

**Architecture:** Post-process `tools_list()` output with permission annotations, then add one AI-facing facade tool that reuses existing graph/cache data to produce hypotheses and evidence plans. Keep runtime capture advisory only.

**Tech Stack:** Rust 2021, serde_json, existing MCP stdio integration tests, no new dependencies.

---

### Task 1: Permission Metadata

**Files:**
- Modify: `crates/cli/src/mcp_server.rs`
- Modify: `crates/cli/tests/mcp_server.rs`

- [x] **Step 1: Write failing permission tests**

Add tests that assert `codelattice_change_review` advertises standard read-only annotations and that cache/export tools advertise cache/temp-artifact write scopes.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test mcp_server mcp_tools_list_permission -- --nocapture
```

Expected: fail because tools currently have no permission annotations.

- [x] **Step 3: Implement permission profile post-processing**

Add helper functions near `tools_list()` that attach standard MCP annotations and `x-codelattice-permissionProfile` to every returned tool.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test --test mcp_server mcp_tools_list_permission -- --nocapture
```

Expected: pass.

### Task 2: Root Cause Assistant

**Files:**
- Modify: `crates/cli/src/mcp_server.rs`
- Modify: `crates/cli/tests/mcp_server.rs`

- [x] **Step 1: Write failing root-cause assistant tests**

Add tests for `tools/list`, static mode without runtime evidence, and runtime-evidence mode.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test mcp_server mcp_root_cause -- --nocapture
```

Expected: fail because the tool does not exist.

- [x] **Step 3: Implement read-only root-cause assistant**

Add `codelattice_root_cause_assistant`, handler, AI toolset membership, full tool count updates, and workflow routing.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test --test mcp_server mcp_root_cause -- --nocapture
```

Expected: pass.

### Task 3: Docs, Scripts, and Verification

**Files:**
- Modify: `scripts/mcp-dogfood.sh`
- Modify: `scripts/codelattice-mcp.sh`
- Modify: `scripts/install-mcp.sh`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/README.md`
- Modify: `docs/architecture/mcp-v0-contract.md`
- Modify: `docs/architecture/mcp-local-client-setup.md`
- Modify: `docs/guides/ai-prompt-cookbook.md`
- Modify: `docs/plans/2026-05-20-permission-aware-root-cause-closure.md`

- [x] **Step 1: Update user-facing docs and tool counts**

Document permission-aware MCP metadata and the root-cause evidence loop.

- [x] **Step 2: Run verification**

Run the full verification plan from preflight.

- [x] **Step 3: Commit and push**

Commit with:

```bash
git commit -m "feat(mcp): add permission-aware root cause assistant"
git push gitcode master
```
