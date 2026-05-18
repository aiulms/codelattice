# Automation Graph Pack Closure

Date: 2026-05-18

## Result

Delivered `codelattice_automation_graph` as MCP tool #38.

The tool statically scans repository automation surfaces:

- CI workflow files (`.github/workflows`, GitLab/GitCode/Gitee workflow files, Jenkinsfile)
- `package.json` scripts
- Makefile targets
- Dockerfile steps
- Shell scripts (`.sh/.bash/.zsh/.ksh/.bats`)

It returns workflow summaries, step nodes, workflow-to-step edges, script invocation edges,
entry files, and risk findings. It never executes scripts, builds, Docker commands, package
scripts, or CI jobs.

## Risk Signals

Initial static risk patterns:

- `curl | sh` / `wget | sh`
- `pull_request_target`
- mutable workflow action refs such as `@main` / `@master`
- `rm -rf`
- `sudo`
- `chmod 777`
- `docker run --privileged`
- secret-like `echo $TOKEN` / `echo $SECRET`

All findings are review leads only, not exploit proofs.

## Verification

- `cargo fmt --check`: PASS
- `git diff --check`: PASS
- `cargo test --test mcp_server`: PASS, 120/120
- `cargo test`: PASS
- `bash scripts/install-mcp.sh --build`: PASS, all-language release binary
- `bash scripts/codelattice-mcp.sh --self-test`: PASS, 38 tools, all optional language adapters compiled
- `bash scripts/install-mcp.sh --doctor`: PASS, 8/8
- `bash scripts/mcp-dogfood.sh`: PASS, 38/38
- GitNexus detect-changes: LOW risk, 15 files / 29 symbols / 0 affected processes

## Boundaries

Not touched:

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- CodeLattice-Tool stable runtime
- AI client configs
- live repos

## Notes

`target/release/codelattice` was rebuilt with all optional language adapters after a plain release
build temporarily produced a 38-tool binary without optional parser features. The final self-test and
doctor both confirm the full language profile is restored.
