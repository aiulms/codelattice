# CodeLattice Guides

These guides turn CodeLattice's MCP tools into repeatable AI workflows.

| Guide | Use For |
|-------|---------|
| [AI MCP Tool Guide](ai-mcp-tool-guide.md) | 默认 6 个 facade 工具的选择规则、配置、job 模式、busy 处理。 |
| [AI Prompt Cookbook](ai-prompt-cookbook.md) | Copyable prompts for onboarding, edits, dead-code review, release checks, and legacy cleanup. |
| [Workflow Presets](workflow-presets.md) | Scenario-to-tool mapping for `codelattice_workflow_presets` (+ facade equivalents for the default 6-tool surface). |
| [MCP AI Usage Guide](../mcp/ai-usage-guide.md) | CLI token profiles, compact payload semantics, decision guidance, and full-mode caveats. |
| [MCP v0 Contract](../architecture/mcp-v0-contract.md) | 权威工具输入输出契约、Known Limitations、envelope schema。 |

CodeLattice is a local static code intelligence engine. The workflows in these
guides do not execute project code and do not prove runtime behavior. Treat all
dead-code, framework-entry, external API, stale-doc, and config/example findings
as candidates that need engineering review.
