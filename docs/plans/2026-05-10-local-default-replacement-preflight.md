# Local Default Replacement Preflight

Date: 2026-05-10 12:45 CST

Executor: local AI preflight investigation, docs-only

Scope: evaluate whether this machine can make CodeLattice the default local analysis path for Rust and Cangjie projects, while preserving GitNexus-RC as fallback. This preflight did not enable any default switch.

## Truth Gate

| Workspace | HEAD | Branch | Remote | Status |
|---|---:|---|---|---|
| `/Users/jiangxuanyang/Desktop/codelattice` | `0a2390192eb66619152b9da8bfce425a5fead6a8` | `master` | `gitcode https://gitcode.com/aiulms/codelattice.git` | clean, only ignored `.DS_Store` / `.gitnexus/` artifacts |
| `/Users/jiangxuanyang/Desktop/GitNexus-RC` | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` | `main` | `gitcode` / `origin` -> `https://gitcode.com/aiulms/gitnexus-rc.git` | pre-existing untracked local artifacts; not touched |
| `/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool` | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` | `main` | `origin https://gitcode.com/aiulms/gitnexus-rc.git` | clean |
| `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index` | `9b29db6b7599547205e15034489e7c4e13f879f3` | detached HEAD | `origin https://gitcode.com/aiulms/cjgui.git` | clean, only ignored `.gitnexus/` |

CodeLattice is at `0a23901` or later. Tool status reports `/Users/jiangxuanyang/Desktop/codelattice` indexed as `codelattice`, indexed commit `0a23901`, current commit `0a23901`, status up to date.

Tool list currently includes `open-nwe`, `gitnexus-rc`, two `cjgui` entries, a worktree `open-nwe`, and `codelattice`. The global registry file is `~/.gitnexus/registry.json` and currently has 6 entries.

No CodeLattice `.claude/`, `CLAUDE.md`, `.sisyphus/`, temporary bridge JSON, or run JSON artifacts were present during the preflight scan.

## Current Entrypoints

| Entrypoint | Current target | Used by whom | Can switch? | Fallback needed? | Risk | Notes |
|---|---|---|---|---|---|---|
| `~/.codex/config.toml` `[mcp_servers.gitnexus]` | `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js mcp` | Codex MCP tools | No for this phase | Yes | High | MCP serves all indexed repos and tools such as `query`, `context`, `impact`, `cypher`, and `detect_changes`. CodeLattice does not replace the MCP server. |
| `~/.config/opencode/opencode.json` MCP entry | same Tool CLI `mcp` command | OpenCode / OpenAgent style clients | No for this phase | Yes | High | This is a client default. Switching it would globally affect all repos and languages. |
| `~/.claude.json` / `~/.claude/settings.json` | no active GitNexus server entry found in root-level relevant scan | Claude Code | Not applicable now | Yes if later added | Medium | Historical project/session files contain old GitNexus guidance; root-level active config did not show a GitNexus default to switch. |
| `~/.agents/skills/gitnexus-*` | GitNexus MCP first, Tool CLI absolute path fallback | Agent skill workflows | Partial, documentation only | Yes | Medium | These skills cover exploration, debugging, PR review, impact analysis, refactoring, guide, and CLI commands. Rust/Cangjie analyze generation can be delegated to CodeLattice later, but graph query/refactor workflows still depend on GitNexus-RC. |
| CodeLattice `AGENTS.md` GitNexus block | indexed repo `codelattice`, Tool CLI absolute path | Agents working inside CodeLattice | Partial, docs only | Yes | Medium | It already says no production replacement and no default tool switch. Future docs can add language-aware local command authority, but not silently flip default MCP. |
| GitNexus-RC `AGENTS.md` / `GUARDRAILS.md` | GitNexus-RC Tool CLI absolute path | GitNexus-RC maintainers and agents | No | Yes | High | Governance source for GitNexus. Must keep Tool path and no-`npx gitnexus` production rule. |
| Repo-local `scripts/alpha-trial-smoke.sh` | CodeLattice `cargo run` bridge JSON -> Tool import | Alpha trial validation | Yes, as model for future smoke | Yes | Low | Already performs Rust/Cangjie bridge generation, Tool ingestion, cleanup, and registry restore without changing defaults. |
| Repo-local `scripts/build.sh` | builds `gitnexus-rust-core-cli` | CodeLattice local build users | Yes | No for build | Low | Good source for the eventual CodeLattice CLI path. Binary name remains compatibility name. |
| CodeLattice CLI via Cargo | `cargo run -p gitnexus-rust-core-cli -- analyze/quality/summary` | Local Rust/Cangjie analysis | Yes for Rust/Cangjie | Yes | Medium | Supports `--language rust`, `--language cangjie`, `--language auto`, `--format gitnexus-rc`, `--strict`, `quality`, and `summary`. |
| Built binary path | `/Users/jiangxuanyang/Desktop/codelattice/target/release/gitnexus-rust-core-cli` after `scripts/build.sh` | Future local wrapper | Yes | Yes | Medium | Build output is not a committed dependency. Switch script should detect and rebuild or fall back to `cargo run`. |
| GitNexus Tool CLI | `/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js` | Production analyze/status/list/context/impact/detect-changes/wiki/MCP | No hard replace | Yes | High | Still required for registry, MCP, WebUI server, query/context/impact, wiki, embeddings, non-Rust/Cangjie languages, and bridge ingestion. |
| Bridge import flag | `--experimental-rust-core-bridge-graph <json>` | Tool ingestion of CodeLattice output | Keep | Yes | Medium | This remains the integration seam. Do not rename or loosen validator. |
| GitNexus WebUI | `node .../index.js serve` / `gitnexus-web` | Browser UI | No | Yes | High | WebUI expects GitNexus-RC HTTP API and graph store. CodeLattice can feed the store through bridge import, but does not replace WebUI runtime. |
| Shell profiles `~/.zshrc`, `~/.bashrc`, `~/.bash_profile`, `~/.profile` | no GitNexus/CodeLattice hits found | Interactive shell | Possible later | Yes | Low | Good candidate for a user-approved wrapper alias, but this preflight did not edit profiles. |
| `~/.gitnexus/config.json` | provider config only | GitNexus wiki / config | No | Yes | Medium | Do not overwrite. It may hold provider choices and future credentials. |
| `~/.gitnexus/registry.json` | multi-repo registry | Tool and MCP discovery | No direct switch | Yes | High | Switch flow can restore the `codelattice` name after bridge import, but must not purge existing indexes without explicit approval. |
| GitNexus-RC package `gitnexus` | `dist/cli/index.js`, commands `analyze`, `mcp`, `serve`, `list`, `status`, `context`, `impact`, `cypher`, `detect-changes`, `wiki`, groups | GitNexus production features | No | Yes | High | This is the broad multi-language graph platform and fallback surface. |
| GitNexus-RC-Tool checkout | built distribution of same package | Production command authority | No modification | Yes | High | This preflight treated it as read-only. |

## Replacement Boundary

CodeLattice can replace the local analysis-generation step for:

- Rust project analysis: `cargo run -p gitnexus-rust-core-cli -- analyze --root <repo> --language rust ...`
- Cangjie project analysis: `cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze --root <repo> --language cangjie ...`
- GitNexus-RC bridge JSON generation via `--format gitnexus-rc --strict`
- Local `quality` and `summary` checks for Rust/Cangjie
- Tool ingestion through `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph <json>`

CodeLattice should not directly replace:

- Other language analysis paths in GitNexus-RC
- GitNexus-RC MCP server defaults
- GitNexus WebUI default HTTP flow
- Registry management, `status`, `list`, `detect-changes`, `context`, `impact`, `query`, `cypher`, group, wiki, and embedding behavior
- Rename/refactor tools that depend on GitNexus-RC graph editing semantics
- Historical GitNexus features and language adapters not covered by Rust/Cangjie alpha evidence

Recommended boundary: CodeLattice becomes the preferred producer for Rust/Cangjie bridge graphs, while GitNexus-RC remains the graph store, query/refactor/MCP/WebUI runtime, and fallback analyzer.

## Replacement Strategy

Use a language-aware default instead of a global hard replacement:

1. Detect target language from explicit `--language`, then from project markers.
2. If target is Rust (`Cargo.toml`) or Cangjie (`cjpm.toml` / `.cj` roots), run CodeLattice bridge generation.
3. Import the bridge JSON into the Tool with the existing experimental bridge flag.
4. Restore or preserve the intended repo name in the Tool registry.
5. For non-Rust/Cangjie, mixed unknown repos, WebUI, MCP, wiki, embeddings, and refactor/query workflows, call the GitNexus-RC Tool CLI unchanged.
6. Keep an explicit fallback command path visible in every wrapper output.

Success definition for local replacement:

- Rust and Cangjie projects default to CodeLattice analysis generation.
- Other languages and old GitNexus features still use GitNexus-RC.
- `status`, `detect-changes`, and `context` remain usable through the Tool after bridge import.
- One rollback command restores the previous alias/config state.
- No live repo, Tool dist, GitNexus-RC runtime/schema/WebUI, MCP config, or registry content is modified without explicit approval.

## Switch Script Design

Future script proposal: `scripts/local-default-switch.sh`.

This preflight does not create the script.

Required modes:

- `--status`: print detected shell/agent/client config, current wrapper target, Tool registry repo names, CodeLattice build availability, and fallback path.
- `--dry-run`: show every proposed write with old/new values and backup path; perform no writes.
- `--enable`: install or update only approved local wrapper/config entries; record pre-change state first.
- `--rollback`: restore the pre-change wrapper/config state from the recorded snapshot.
- `--smoke`: run the post-switch smoke plan without changing defaults.

Suggested behavior:

- Write an auditable state file such as `~/.config/codelattice/local-default-switch/state.json`.
- Write backups under a timestamped directory such as `~/.config/codelattice/local-default-switch/backups/<timestamp>/`.
- Prefer a wrapper command over editing the Tool dist, for example a local `codelattice-analyze` wrapper or an approved shell alias.
- Wrapper chooses CodeLattice only for Rust/Cangjie analysis generation.
- Wrapper calls the Tool absolute path for bridge import and all graph queries.
- If CodeLattice analysis, JSON purity validation, Tool import, or smoke fails, leave the prior default active and print the rollback command.
- Never delete GitNexus-RC indexes during enable or rollback.
- Never edit `/Users/jiangxuanyang/Desktop/cangjie`, open-nwe, or other live repos.

Possible future write targets, only after explicit user approval:

- A wrapper alias or shell profile block, guarded by begin/end markers.
- An optional local config file under `~/.config/codelattice/`.
- CodeLattice docs/AGENTS command-authority wording.

Forbidden future write targets for the first implementation:

- GitNexus-RC runtime/schema/WebUI.
- GitNexus-RC-Tool `dist`.
- MCP configs such as `~/.codex/config.toml`, `~/.config/opencode/opencode.json`, or `~/.claude.json`, unless a later task explicitly approves MCP switching.
- Live project repositories.

## Smoke Plan

Post-switch smoke should run with temporary JSON files and delete them afterward:

1. CodeLattice self Rust analyze:
   ```bash
   cargo run -p gitnexus-rust-core-cli -- analyze \
     --root /Users/jiangxuanyang/Desktop/codelattice \
     --language rust \
     --format gitnexus-rc \
     --strict
   ```
2. Cangjie cjgui analyze:
   ```bash
   cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
     --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui \
     --language cangjie \
     --format gitnexus-rc \
     --strict
   ```
3. Validate stdout JSON purity: first byte is `{`; `python3 -m json.tool <json> >/dev/null`; no `sed` cleanup.
4. Import bridge JSON through:
   ```bash
   node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze \
     --force \
     --experimental-rust-core-bridge-graph <json>
   ```
5. Run Tool `status`.
6. Run Tool `detect-changes --repo codelattice --scope all`.
7. Run Tool `detect-changes --repo cjgui --scope all`.
8. Run Tool `context main --repo cjgui`.
9. Run fallback GitNexus-RC `detect-changes --repo gitnexus-rc --scope all`.
10. Restore CodeLattice registry name:
    ```bash
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze \
      /Users/jiangxuanyang/Desktop/codelattice \
      --force \
      --skip-agents-md \
      --name codelattice
    ```

The existing `scripts/alpha-trial-smoke.sh --rust-only` and `--cangjie-only` already cover a smaller portable version of this flow and should stay in the implementation smoke.

## Rollback Plan

Rollback must be one command and must not depend on network access:

1. Restore saved alias/wrapper/config files from `~/.config/codelattice/local-default-switch/backups/<timestamp>/`.
2. Restore GitNexus command authority to:
   ```bash
   node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js ...
   ```
3. Remove temporary bridge JSON files created by the switch or smoke.
4. Re-index CodeLattice with the canonical repo name if the registry was temporarily changed:
   ```bash
   node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze \
     /Users/jiangxuanyang/Desktop/codelattice \
     --force \
     --skip-agents-md \
     --name codelattice
   ```
5. Delete untracked generated `.claude/` or `CLAUDE.md` only after confirming they are not tracked.
6. Do not clear GitNexus-RC existing indexes unless the user explicitly requests that.

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| Global MCP switch breaks non-Rust/Cangjie workflows | High | Do not switch MCP in first implementation. Keep GitNexus-RC MCP as default. |
| Registry name drift after bridge import | Medium | Always restore `codelattice` with `--name codelattice --skip-agents-md`; add smoke assertion. |
| Tool bridge adapter remains experimental | Medium | Keep the flag name unchanged; fallback to GitNexus-RC analyze when bridge import fails. |
| Binary path missing or stale | Medium | Wrapper checks built binary first, then falls back to `cargo run`; `--status` reports both. |
| Shell/client config edits are hard to audit | Medium | Use begin/end markers plus timestamped backups and `--dry-run`. |
| Agent skills still describe GitNexus-first workflows | Medium | Treat Rust/Cangjie analyze as a producer substitution only; leave query/refactor skills on GitNexus-RC. |
| Secrets in local config files | Medium | Switch script must never copy full config files into repo docs or logs; backup locally only. |
| Live repo accidental writes | High | Use Cangjie index checkout for smoke; never target `/Users/jiangxuanyang/Desktop/cangjie` for writes. |

## Go / No-Go Recommendation

Recommendation: GO for a future implementation of an opt-in, language-aware local wrapper script after explicit user approval.

Recommendation: NO-GO for a global hard replacement of GitNexus-RC, MCP defaults, WebUI defaults, Tool dist, or registry ownership.

The first implementation should be conservative:

- Add `scripts/local-default-switch.sh` with `--status`, `--dry-run`, `--enable`, `--rollback`, and `--smoke`.
- Default to dry-run-first behavior.
- Only write local wrapper/config surfaces approved by the user.
- Keep GitNexus-RC Tool CLI as the fallback and graph-query runtime.
- Run the smoke plan before declaring the switch enabled.

## Preflight Result

Status: PASS as investigation-only preflight.

Default switch enabled: NO.

Runtime changes: none.

Tool checkout changes: none.

Live repo changes: none.

Recommended next step: implement the switch script only after explicit user approval, with no MCP switch in the first pass.
