# CodeLattice Runtime Isolation Pack

Date: 2026-05-11
Status: Active

## Problem

Codex and opencode were configured to start CodeLattice MCP directly from the
development checkout:

```text
/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh
```

That made the AI IDE runtime depend on whatever binary or wrapper happened to be
in the active development workspace. A rebuild or wrapper edit in the dev checkout
could affect newly started AI sessions.

## Decision

Use a stable local runtime directory:

```text
/Users/jiangxuanyang/Desktop/CodeLattice-Tool
```

AI clients should point to:

```text
/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh
```

The dev checkout remains:

```text
/Users/jiangxuanyang/Desktop/codelattice
```

Changes in the dev checkout only affect AI clients after an explicit promotion.

## Promotion Command

```bash
cd /Users/jiangxuanyang/Desktop/codelattice
bash scripts/promote-to-local-tool.sh
```

The script:

- builds a release binary with `tree-sitter-cangjie`
- installs a stable wrapper and binary into `CodeLattice-Tool`
- writes a `manifest.json` with source commit and binary hash
- runs a self-test against the promoted runtime
- prints Codex/opencode config snippets

## Runtime Layout

```text
CodeLattice-Tool/
  codelattice-mcp.sh
  manifest.json
  bin/
    codelattice-cli
    gitnexus-rust-core-cli
```

`gitnexus-rust-core-cli` is kept as a compatibility binary name until the Cargo
package/bin rename is done.

## Safety Rules

- Codex/opencode must point at `CodeLattice-Tool`, not the dev checkout.
- Do not auto-promote on build or commit.
- Promote only after tests/smoke pass.
- Keep GitNexus-RC MCP side-by-side; CodeLattice remains a sidecar.
- Rollback is just restoring the previous AI client config backup or replacing
  the runtime directory with an older promoted copy.

## Verification

Minimum runtime verification:

```bash
/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh --version
/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh --self-test
```

Expected:

- `toolCount >= 21`
- `cangjieSupport: True`
- `Self-test passed`
