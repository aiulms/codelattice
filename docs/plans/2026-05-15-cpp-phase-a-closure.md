# C++ Phase A Graph Support — Closure

> **日期：** 2026-05-15
> **状态：** Implementation complete, pending verification
> **范围：** CodeLattice C++ Phase A 静态代码图谱支持

---

## 状态摘要

C++ Phase A 实现已完成，等待最终验证。核心提取器、CLI 集成、MCP 工具注册和文档更新均已就绪。

---

## 创建/修改的文件

### 新增文件

- `crates/cpp-extractor/src/lib.rs` — C++ tree-sitter 提取器核心
- `crates/cpp-extractor/src/symbols.rs` — C++ 符号类型定义（namespace、class、struct、method、function、constructor、destructor、enum、using alias、typedef、macro）
- `crates/cpp-extractor/src/calls.rs` — 调用关系提取（same-file / cross-file heuristic）
- `crates/cpp-extractor/src/includes.rs` — `#include` 依赖提取
- `crates/cpp-extractor/Cargo.toml` — C++ 提取器 crate 配置
- `fixtures/cpp/portable-smoke/` — C++ fixture 项目（含 namespace、class、inheritance、template、macro 样本）
- `docs/plans/2026-05-15-cpp-phase-a-preflight.md` — 本计划 preflight
- `docs/plans/2026-05-15-cpp-phase-a-closure.md` — 本 closure 文档

### 修改文件

- `crates/cli/src/main.rs` — 新增 `--language cpp` 分支和 feature gate
- `crates/cli/src/mcp_server.rs` — 注册 C++ 相关 MCP 工具
- `crates/cli/Cargo.toml` — 添加 `tree-sitter-cpp` optional dependency
- `scripts/build.sh` — 添加 `--features tree-sitter-cpp` 构建路径
- `scripts/smoke.sh` — 添加 C++ fixture smoke 测试
- `scripts/package-release.sh` — 添加 C++ feature 到 release build
- `README.md` — 添加 C++ Phase A 到支持语言表、CLI 示例、已知边界
- `docs/architecture/mcp-v0-contract.md` — 添加 `cpp` 到 language enum、C++ Phase A 限制说明、`cpp_disabled` 错误码
- `docs/architecture/unified-output-contract.md` — 添加 C++ 到 language 枚举、GraphSummary 来源、质量门适用性、Node/Edge 差异表
- `docs/plans/README.md` — 添加 C++ Phase A 计划索引

---

## 测试结果

| 测试项 | 状态 | 备注 |
|--------|------|------|
| C++ fixture analyze | **TBD** | 待验证 |
| 质量门（0 duplicate / 0 dangling / deterministic） | **TBD** | 待验证 |
| MCP analyze / symbol_search / impact_preview | **TBD** | 待验证 |
| CLI `--format json` | **TBD** | 待验证 |
| CLI `--format gitnexus-rc` | **TBD** | 待验证 |
| Release smoke（C++ fixture） | **TBD** | 待验证 |
| 编译 `--features tree-sitter-cpp` | **TBD** | 待验证 |

---

## 已知问题

| 问题 | 严重度 | 状态 | 备注 |
|------|--------|------|------|
| 模板实例化未处理 | Low | 已知限制 | Phase A 范围外，模板定义提取为符号但不实例化 |
| 重载解析不完整 | Low | 已知限制 | 函数调用按名称匹配，参数类型不匹配时可能指向错误重载 |
| 虚函数派发静态解析 | Low | 已知限制 | 虚调用按声明类型解析，不做动态类型分析 |
| macro expansion 未执行 | Low | 已知限制 | `#define` 只做识别，不做展开；宏调用可能无法解析 |
| 无 compile_commands.json 支持 | Low | 已知限制 | 无法获取自定义 include path 和宏定义 |

---

## 后续工作

1. **验证完成后**更新本 closure 文档，填入实际测试数据
2. **Release Packaging**：确保 `scripts/package-release.sh` 包含 `--features tree-sitter-cpp`
3. **Smoke Matrix**：在 `docs/release/smoke-matrix.md` 添加 C++ 验证行
4. **Dogfood**：在真实 C++ 项目（如 gitnexus-rust-core 的 C++ 依赖）上运行验证

---

## 相关文档

- [Preflight](2026-05-15-cpp-phase-a-preflight.md)
- [MCP Contract](../architecture/mcp-v0-contract.md)
- [Unified Output Contract](../architecture/unified-output-contract.md)
- [README](../../README.md)
