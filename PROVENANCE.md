# PROVENANCE

> 最后更新：2026-05-09

## 项目来源

本项目（gitnexus-rust-core）最初作为 GitNexus 生态的 Rust 语言分析核心启动。在开发过程中，逐步形成了独立的 Rust/Cangjie 本地代码上下文分析能力，路线从"复刻某个现有工具形态"收束为"独立 Rust/Cangjie 本地代码上下文核心"。

## 治理关系

- **GitNexus-RC**（`https://gitcode.com/aiulms/gitnexus-rc`）是本项目的治理来源和历史参考。语言支持决策、fixture 设计、confidence/reason 策略初期源自 GitNexus-RC `docs/language-support/`。
- 本项目**不是** GitNexus-RC 的替代品或竞争产品。它是一个独立的 Rust 工具链，专注于本地代码智能分析。
- 两项目各自独立发展：GitNexus-RC 继续作为 TypeScript 全栈代码智能平台；gitnexus-rust-core 专注于 Rust/Cangjie 本地分析核心。
- `--format gitnexus-rc` bridge 格式保留了与 GitNexus-RC 消费侧的兼容通道，但该格式名仅为 legacy compatibility alias，不代表项目长期产品身份。

## 当前定位

- **独立 Rust 工具链**：不依赖 GitNexus-RC runtime、不依赖 Node.js/npm 生态。
- **本地代码上下文核心**：为 AI 辅助开发、代码理解、影响分析提供结构化、可验证的图数据。
- **Alpha production trial 阶段**：核心能力（Rust + Cangjie 分析、quality gates、bridge 输出）已稳定，但尚未达到 v1.0 生产承诺。

## 技术路线

- **Rust 分析**：基于 tree-sitter AST，不做完整类型推断、trait solving、宏展开。
- **Cangjie 分析**：基于 tree-sitter-cangjie AST，不做完整方法派发、类型推断。
- **输出协议**：统一 JSON、quality JSON、bridge JSON 三种格式，均有 contract 文档和回归测试覆盖。
- **质量门**：duplicate/dangling/deterministic/synthetic 等门，由 CI/smoke 自动化验证。

## 许可证

本项目采用 MIT License（详见 [LICENSE](LICENSE)）。

tree-sitter-cangjie parser（`crates/cangjie/vendor/tree-sitter-cangjie/`）来自 [Cangjie-SIG/tree-sitter-cangjie](https://gitcode.com/Cangjie-SIG/tree-sitter-cangjie)，采用 Mulan PSL v2.0 许可证。

## 误解澄清

1. **这不是 GitNexus-RC 的 Rust 重写版。** 路线已独立，不再以复刻 GitNexus-RC 全功能为目标。
2. **这不是生产发布。** 当前为 alpha production trial，输出格式和部分语义策略仍可能调整。
3. **这不是商业化产品。** 无 UI、无 Web 服务、无 MCP server、无云端部署承诺。
4. **bridge format 不是长期品牌名。** `--format gitnexus-rc` 保留为 legacy compatibility alias，未来可能新增中性格式名。
