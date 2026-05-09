# CodeLattice Local Rename and Index Refresh Closure

Date: 2026-05-09

## Scope

本轮只处理 public identity 的本地落点和 GitNexus/Tool 索引身份：

- 本地目录：`/Users/jiangxuanyang/Desktop/gitnexus-rust-core` -> `/Users/jiangxuanyang/Desktop/codelattice`
- GitCode remote：`https://gitcode.com/aiulms/gitnexus-rust-core.git` -> `https://gitcode.com/aiulms/codelattice.git`
- Tool index：刷新为 repo `codelattice`
- Public-facing 入口：`README.md`、`AGENTS.md`、构建/smoke 脚本展示名改为 CodeLattice

## Explicit Non-goals

- 不重命名 Cargo package / binary：`gitnexus-rust-core-cli` 暂时保留为 alpha 兼容名。
- 不重命名兼容格式：`--format gitnexus-rc` 保留。
- 不重命名 Tool opt-in flag：`--experimental-rust-core-bridge-graph` 保留。
- 不清理历史 closure review 中的旧名事实。
- 不修改 GitNexus-RC runtime、Tool checkout、WebUI 或 live repo。

## Index Result

`node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force`

Result:

- Repo name: `codelattice`
- Path: `/Users/jiangxuanyang/Desktop/codelattice`
- Commit: `5e647fa`
- Symbols: 4104
- Relationships: 7170
- Clusters: 118
- Flows: 157

`.gitnexus/meta.json` now points at `/Users/jiangxuanyang/Desktop/codelattice`.

## Command Authority Fix

The generated GitNexus block suggested `npx gitnexus analyze`. This was corrected to the Tool CLI absolute path:

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze
```

This preserves the existing command authority rule and avoids accidentally using the old npm-published CLI.

## Remaining Legacy Names

The following names are intentionally retained:

- `gitnexus-rust-core-cli` package/binary
- `--format gitnexus-rc`
- `--experimental-rust-core-bridge-graph`
- `rust-core-bridge-adapter` references in GitNexus-RC adapter history
- Historical plan/closure references to the former working name

These are compatibility or historical names, not current project identity.

## Current Identity

CodeLattice is the project identity going forward. `gitnexus-rust-core` is now only the former working name.
