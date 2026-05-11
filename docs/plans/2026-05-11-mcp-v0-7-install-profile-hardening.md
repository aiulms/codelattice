# CodeLattice MCP v0.7 — Install/Profile Hardening + Cangjie Feature Binary Reliability

> **日期：** 2026-05-11
> **版本：** v0.7.0
> **状态：** Complete

---

## 一、问题

v0.6 补核验发现：opencode session 的 MCP server 进程加载了缺少 `tree-sitter-cangjie` feature 的 binary，导致 Cangjie MCP 调用在 session 内不可用。根因：

1. `install-mcp.sh --build` 不带 `--features tree-sitter-cangjie`
2. `codelattice-mcp.sh` 的 binary 选择不检测 feature 支持
3. `cargo run` fallback 不带 cangjie feature
4. `--doctor` / `--self-test` 不检测 cangjie 支持
5. 无 profile 输出机制 — 无法从 MCP 层面判断 binary 能力

---

## 二、变更

### 2.1 MCP serverInfo 增强

`initialize` 响应增加 `cangjieSupport` 和 `toolCount` 字段：

```json
{
  "serverInfo": {
    "name": "codelattice",
    "version": "0.7.0",
    "cangjieSupport": true,
    "toolCount": 21
  }
}
```

`cangjieSupport` 通过 `#[cfg(feature = "tree-sitter-cangjie")]` 在编译时确定。

### 2.2 install-mcp.sh 加固

- `--build` 默认带 `--features tree-sitter-cangjie`
- 新增 `--rust-only` 选项（不带 cangjie feature）
- `--doctor` 新增检查：
  - cangjieSupport 检测（从 initialize 响应读取）
  - Cangjie symbol_search(init) smoke test
- `--print-config` 始终推荐 wrapper 路径（不直接指向 binary）
- 通过时 7/7 checks（binary、wrapper、handshake、tools、cache、cangjie、smoke）

### 2.3 codelattice-mcp.sh 加固

- `--version` 输出完整 profile：version、cangjieSupport、toolCount
- `--self-test` 新增 cangjieSupport 检查
- Binary 选择逻辑：优先选 cangjie-enabled binary（检测 initialize 响应）
- 非最优 binary 时输出 warning 和修复命令
- `cargo run` fallback 带 `--features tree-sitter-cangjie`

### 2.4 mcp-dogfood.sh 更新

- 打印 profile 信息（version、cangjieSupport、toolCount）
- 新增第 22 步：profile cangjie support check
- 版本号更新为 v0.7

### 2.5 文档更新

- `mcp-local-client-setup.md`: v0.7 profile 说明、rebuild 方法、opencode 重启说明
- `mcp-v0-contract.md`: v0.7 changelog、§3.23 Profile Detection spec

---

## 三、测试

- MCP tests: 52/52 pass
- Productization tests: 19/19 pass
- `install-mcp.sh --doctor`: 7/7 pass（含 cangjie smoke）
- `codelattice-mcp.sh --self-test`: 21 tools + cangjieSupport=True
- `codelattice-mcp.sh --version`: profile 正确显示
- `mcp-dogfood.sh`: 22/22 pass

---

## 四、关键决策

| 决策 | 理由 |
|------|------|
| cangjieSupport 在编译时确定 | cfg(feature) 是最可靠的方式 |
| 从 initialize 响应读取 profile | 不需要新 tool，任何 MCP 客户端都能读取 |
| wrapper 优先选 cangjie-enabled binary | 开发期间 debug binary 可能带 cangjie 但 release 不带 |
| cargo run fallback 带 cangjie feature | 确保零配置也能工作 |
| --print-config 始终推荐 wrapper | wrapper 有 profile 检测，直接用 binary 没有 |
