# 工作台 CLI 构建（全语言二进制）

桌面工作台 spawn 的是 `target/debug/codelattice`（见
`apps/desktop/src-tauri/src/commands/common.rs` 的 `repo_root().join("target/debug/codelattice")`）。
默认构建（`cargo build`）只带 `tree-sitter-extraction`（rust）+ shell（非 optional），
inspect 会把其余可分析语言报为 `language-support-disabled-in-this-binary`，
挑选器里相应行灰显。

## 构建方法

```bash
scripts/codelattice-build-workbench-cli.sh
```

等价于：

```bash
cargo build -p gitnexus-rust-core-cli --features "\
tree-sitter-extraction,\
tree-sitter-typescript,\
tree-sitter-javascript,\
tree-sitter-python,\
tree-sitter-c,\
tree-sitter-cpp,\
tree-sitter-cangjie,\
tree-sitter-arkts"
```

- **显式列表，禁止 `--all-features`**：workspace 以后新增 optional feature
  会被默默编进来；「工作台支持哪些语言」应是显式决策。
- debug profile，对齐桌面 dev 现状；产物直接落在桌面 spawn 的路径，桌面零改动。
- 与 `codelattice-precommit-check.sh` 互不影响：precommit 用默认 feature。

## feature 清单（2026-08-25 实编）

| feature | 语言 | 状态 |
|---|---|---|
| `tree-sitter-extraction` | rust | ✅ 编入（默认 feature） |
| `tree-sitter-javascript` | javascript（.js 走 typescript 口径） | ✅ 编入 |
| `tree-sitter-typescript` | typescript / arkts 依赖 | ✅ 编入 |
| `tree-sitter-python` | python | ✅ 编入 |
| `tree-sitter-c` | c | ✅ 编入 |
| `tree-sitter-cpp` | cpp | ✅ 编入 |
| `tree-sitter-cangjie` | cangjie | ✅ 编入（本机 macOS arm64 编译通过） |
| `tree-sitter-arkts` | arkts | ✅ 编入（本机 macOS arm64 编译通过） |
| —（非 optional） | shell | 恒在 |

`codelattice inspect` 的 `analyzable` 字段按 `cfg!(feature = ...)` 编译期自适应，
feature 编进去即自动 `true`，无需任何源码改动或探测机制。

## 构建不过清单

记录本机编不过的 feature（若出现）：从脚本列表移除 + 在此逐条登记
（feature 名、编译错误摘要、日期）。inspect 会把该语言如实报
`language-support-disabled-in-this-binary`，这是设计好的降级行为。
**禁止改 vendor 语法源码或对应 crate 的 build.rs 绕过。**

| feature | 日期 | 错误摘要 |
|---|---|---|
| （暂无） | | |

> 2026-08-25：八个 feature 在 macOS arm64（darwin 25.6.0）全部编译通过，无登记项。
> cangjie / arkts 依赖 vendor 语法源码，换平台时若编不过按上表登记。
