# Path Portability After CodeLattice Rename — Closure

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Closure Review
> **关联：** [CodeLattice Rename Follow-up](2026-05-09-codelattice-rename-followup-closure.md)

---

## 一、问题来源

项目从 `gitnexus-rust-core` 改名为 CodeLattice（本地路径 `/Users/jiangxuanyang/Desktop/codelattice`）后，`target/` 目录随目录一起移动。由于 `env!("CARGO_MANIFEST_DIR")` 在编译时将旧路径 `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/crates/cli` 烘焙进二进制，导致以下测试失败：

- `project_model_call_expected_compare`：7 tests FAILED
- `project_model_import_expected_compare`：部分 tests FAILED
- `project_model_graph_expected_compare`：部分 tests FAILED
- `project_model_symbol_expected_compare`：部分 tests FAILED
- `project_model_inspect`：部分 tests FAILED

---

## 二、根因分析

测试代码**没有硬编码旧绝对路径**。`workspace_root()` 函数正确使用 `env!("CARGO_MANIFEST_DIR")` + `.parent().parent()` 推导 workspace root。

问题是 **stale compilation cache**：
1. 代码在 `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/` 编译过
2. `env!("CARGO_MANIFEST_DIR")` 在编译时展开为旧路径
3. 整个目录被重命名为 `/Users/jiangxuanyang/Desktop/codelattice/`
4. `target/` 缓存仍包含旧路径
5. `cargo test` 未检测到需要重编译（源文件未变），直接用缓存的二进制运行

---

## 三、修复方式

**`cargo clean`** — 清除 stale 编译缓存，强制重编译。

**无需修改任何源代码。** 测试路径推导逻辑正确：

```rust
fn workspace_root() -> PathBuf {
    if let Ok(root) = std::env::var("GITNEXUS_RUST_CORE_ROOT") {
        PathBuf::from(root)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }
}
```

---

## 四、为什么不使用新绝对路径

测试代码从未使用绝对路径。它通过 `CARGO_MANIFEST_DIR`（编译时宏）动态推导 workspace root，这是 Cargo 项目的标准做法。问题不在代码设计，而在编译缓存。

---

## 五、是否改变 runtime 行为

**否。** `cargo clean` 只清除编译缓存，不改变任何源代码或运行时行为。

---

## 六、是否改变 graph/calls 语义

**否。** 无代码变更。

---

## 七、验证结果

| 验证项 | 结果 |
|--------|------|
| cargo test (no feature) | 全部 PASS，0 failures |
| cargo test --features tree-sitter-cangjie | 全部 PASS，0 failures |
| project_model_call_expected_compare | 7/7 ✅ |
| project_model_import_expected_compare | 12/12 ✅ |
| project_model_graph_contract | 10/10 ✅ |
| project_model_graph_expected_compare | 12/12 ✅ |
| project_model_symbol_expected_compare | 4/4 ✅ |
| project_model_inspect | 5/5 ✅ |
| bridge_roundtrip (both) | 26/26 ✅ |
| productization_commands (both) | 30/30 ✅ |
| cross_file_import_confidence | 7/7 ✅ |
| alias_reference | 11/11 ✅ |
| alpha-trial-smoke (full) | 8/8 ✅ |
| Tool status | up-to-date ✅ |

---

## 八、剩余旧名分类

| 类别 | 数量 | 处理 |
|------|------|------|
| Cargo binary name `gitnexus-rust-core-cli` | ~25 处 | **兼容保留**（constraint: 不重命名 Cargo package/binary） |
| Historical docs (GOVERNANCE/PROVENANCE/QUALITY/RISK_LEDGER) | ~10 处 | **历史事实，保留** |
| Old execution cards (2026-05-04~07) | ~6 处 | **历史记录，保留** |
| Help text `-p gitnexus-rust-core-cli` | 1 处 | **匹配 binary name，保留** |

**无必须修复项。** 所有当前运行代码和测试均正确。

---

## 九、后续建议

1. **在 CI 中添加 `cargo clean` 后缀检查**：如果 repo 路径变化（如 CI runner 不同 workdir），确保 `env!("CARGO_MANIFEST_DIR")` 是正确的
2. **或将 `target/` 加入 `.gitignore`**（如果尚未加入）—— 已在 `.gitignore` 中
3. ** rename follow-up 文档已正确分类所有旧名残留** — 本轮确认无需额外修复
