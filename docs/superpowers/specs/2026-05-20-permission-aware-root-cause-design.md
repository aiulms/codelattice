# Permission-aware Root Cause Evidence Design

Date: 2026-05-20
Status: Approved by user direction

## Goal

Make CodeLattice less annoying for high-permission AI sessions, and give AI agents a structured way to move complex bugs from guessing toward evidence-backed root-cause hypotheses.

## Permission Model

CodeLattice should not ask users to re-authorize work that is already inside the AI client's granted capability envelope. The MCP server cannot override a client approval UI, but it can publish clear tool metadata so clients and agents know which tools are read-only, cache-only, artifact-writing, or smoke/debug oriented.

Every MCP tool receives:

- standard `annotations` with `readOnlyHint`, `destructiveHint`, `idempotentHint`, and `openWorldHint`;
- `x-codelattice-permissionProfile` with source-write, project-code-execution, network, cache-write, temp-artifact, and sensitivity fields.

## Root Cause Evidence Loop

`codelattice_root_cause_assistant` is an advisory, read-only tool. It accepts a bug report and optional capability/evidence fields, then combines static graph data with capability-aware evidence planning.

The output separates:

- what AI can already see;
- static hypotheses;
- what evidence is missing;
- the one best next action;
- optional probe plans;
- privacy/safety boundaries;
- likely fix areas;
- next verification.

If runtime evidence is already supplied, the tool raises confidence and summarizes what it supports. If no runtime evidence is supplied, it gives the smallest next evidence action instead of asking the user to manually assemble a long data checklist.

## Safety Boundary

The tool never edits source, installs probes, runs project code, opens browsers, calls HTTP endpoints, or starts watchers. It only tells the AI what it can do next within its already granted permissions.

