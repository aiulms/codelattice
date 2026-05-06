# Phase 2 Slice 4 — tree-sitter Cangjie Vendor Gate / Feasibility

**日期：** 2026-05-06
**类型：** vendor gate（docs-only，需要用户批准）
**前置：** GitNexus-RC 已 vendor tree-sitter-cangjie（ABI 14，parser.c ~152K 行）
**参考：** Phase 1 preflight §5（tree-sitter 策略）

## 1. 上游来源审计

### 1.1 Repository

| 字段 | 值 |
|------|-----|
| **名称** | tree-sitter-cangjie |
| **上游 URL** | `https://gitcode.com/Cangjie-SIG/tree-sitter-cangjie` |
| **Maintainer** | vchuoshen6（vchuoshen6@163.com） |
| **当前版本** | 0.1.0 |
| **GitNexus-RC vendor commit** | `ff447b577b45e12a350398a672308174baa1c8ad` |
| **OpenSource 声明** | `README.OpenSource` 标注 "Mulan PSL2.0 License"，owner: vchuoshen6 |
| **所属组织** | Cangjie-SIG（Cangjie Special Interest Group） |

### 1.2 Artifacts

| 文件 | 行数 | 大小 | 说明 |
|------|------|------|------|
| `src/parser.c` | 152,495 | ~4.7 MB | tree-sitter CLI 生成的 C parser |
| `src/scanner.c` | 201 | ~5.7 KB | 手写 external scanner（处理 token-level logic） |
| `grammar.js` | 857 | ~25 KB | tree-sitter grammar DSL（源文件） |
| `grammar_*.js` | 5 文件 | ~35 KB | grammar 子模块 |
| `src/grammar.json` | — | ~145 KB | 生成的 grammar JSON |
| `src/node-types.json` | — | ~70 KB | 生成的 node type 定义 |
| `queries/` | 4 文件 | ~6 KB | highlights/indents/tags/textobjects 查询 |

### 1.3 Bindings

`tree-sitter.json` 声明支持以下 binding：C（原生）、Go、Node、Python、**Rust**、Swift。

Rust binding 标记为 `true`，理论上可生成 Rust crate，但**当前无 crates.io 发布**。

### 1.4 上游活跃度

- 版本 0.1.0，仍在早期阶段
- GitNexus-RC vendor 已稳定用于生产索引（cangjie-GitNexus-Index: 7,434 nodes / 15,415 edges）
- grammar 覆盖 Cangjie 0.51.x~0.56.x 语法
- 已知 parser bug（见 RISK_LEDGER §3.3），但 baseline symbol extraction 可用

## 2. License 评估

### 2.1 License 类型

**Mulan Permissive Software License v2.0（木兰宽松许可证 v2）**

中国开源许可证，OSI 批准的宽松许可证。核心条款：
- **允许**：复制、使用、修改、分发（含商业使用）
- **要求**：保留版权声明和免责声明
- **专利许可**：贡献者授予专利许可，但发起专利诉讼则终止
- **无担保**：AS-IS，不承担任何责任

### 2.2 与 Rust-core 兼容性

| 项目 | 状态 |
|------|------|
| Rust-core 当前 license | MIT |
| Mulan PSL v2 → MIT 兼容 | ✅ 宽松许可证，兼容 MIT |
| Vendor 后需要保留 LICENSE | ✅ 已包含在 vendor 目录 |
| 商业使用限制 | 无（Mulan PSL v2 允许商业使用） |

**结论：** Mulan PSL v2.0 是宽松开源许可证，与 MIT 兼容。vendoring 只需保留 LICENSE 文件，无其他限制。

## 3. ABI 兼容性分析

### 3.1 ABI 版本

| 组件 | ABI/Version |
|------|------------|
| tree-sitter-cangjie | ABI 14（用 tree-sitter-cli@0.21.0 生成） |
| GitNexus-RC tree-sitter runtime | 0.21.1（ABI 14） |
| Rust-core `tree-sitter` crate | **0.26** |

### 3.2 兼容性判断

tree-sitter ABI 向后兼容规则：高版本 runtime 可加载低版本 ABI grammar。

- tree-sitter 0.26 支持 ABI 14（已验证：tree-sitter-rust 0.24 在 0.26 下编译通过）
- tree-sitter-cangjie ABI 14 → tree-sitter 0.26：**预期兼容**

**唯一风险**：tree-sitter 0.26 对 ABI 14 的支持需要实际编译验证。如果遇到 ABI 不匹配，可选择：
- 升级 parser.c 到 ABI 15（需要 tree-sitter CLI generate）
- 降级 Rust-core tree-sitter 到 0.24（不推荐，可能影响 tree-sitter-rust）

### 3.3 已验证的先例

Rust-core 已有 `tree-sitter = "0.26"` + `tree-sitter-rust = "0.24"` 的组合在工作（89/89 tests pass，cargo build 成功）。这证明：
1. tree-sitter 0.26 可以加载 ABI 14 的 grammar
2. `cc` crate 编译 parser.c 在 macOS ARM64 上可用
3. `--no-default-features` 可以完全禁用 tree-sitter 依赖

## 4. 编译方案设计

### 4.1 方案：`cc` crate 编译 vendored C 源码

与 tree-sitter-rust crate 内部使用的方案一致。cangjie crate 新增：

```
crates/cangjie/
  vendor/
    tree-sitter-cangjie/
      src/
        parser.c         # ← vendor from GitNexus-RC
        scanner.c        # ← vendor from GitNexus-RC
        grammar.json     # ← vendor（不编译，仅记录）
        node-types.json  # ← vendor（不编译，仅记录）
      LICENSE            # ← vendor（保留 license）
      README.OpenSource  # ← vendor（保留 upstream info）
  build.rs               # ← 新增：cc::Build 编译 parser.c + scanner.c
```

### 4.2 `build.rs` 设计

```rust
// crates/cangjie/build.rs
fn main() {
    #[cfg(feature = "tree-sitter-cangjie")]
    {
        cc::Build::new()
            .include("vendor/tree-sitter-cangjie/src")
            .file("vendor/tree-sitter-cangjie/src/parser.c")
            .file("vendor/tree-sitter-cangjie/src/scanner.c")
            .compile("tree-sitter-cangjie");
    }
}
```

### 4.3 Cargo.toml feature gate 设计

```toml
[features]
default = []
tree-sitter-cangjie = ["dep:tree-sitter", "dep:cc"]

[dependencies]
tree-sitter = { version = "0.26", optional = true }

[build-dependencies]
cc = { version = "1", optional = true }
```

**不加入 `default` features**，与 project-model 不同：
- project-model 的 `tree-sitter-extraction` 对 Rust 分析已足够成熟
- cangjie crate 的 tree-sitter 支持是新引入的，应显式 opt-in
- 等 Cangjie symbol extraction 稳定后再考虑加入 default

### 4.4 与 project-model 的 feature 隔离

| crate | feature | default | 依赖 |
|-------|---------|---------|------|
| project-model | `tree-sitter-extraction` | **yes** | tree-sitter, tree-sitter-rust |
| cangjie | `tree-sitter-cangjie` | **no** | tree-sitter, cc (build) |

两个 feature 独立，互不影响。`--no-default-features` 只影响 project-model，不影响 cangjie（除非手动开启）。

### 4.5 Rust 侧语言加载

```rust
// crates/cangjie/src/extractors/mod.rs（示意，本 slice 不实现）

#[cfg(feature = "tree-sitter-cangjie")]
fn try_init_cangjie_parser() -> Option<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    // 需要 extern "C" 声明 tree_sitter_cangjie() 函数
    extern "C" { fn tree_sitter_cangjie() -> tree_sitter::Language; }
    let language = unsafe { tree_sitter_cangjie() };
    if parser.set_language(&language).is_ok() {
        Some(parser)
    } else {
        None
    }
}
```

**注意**：与 tree-sitter-rust 不同，tree-sitter-cangjie 没有预编译的 Rust binding crate。需要通过 `extern "C"` 声明链接到 `cc` crate 编译的 C 符号。

`tree-sitter-cangjie` 的 `tree-sitter.json` 中 `camelcase: "Cangjie"` → 生成的 C 函数名为 `tree_sitter_cangjie()`。

## 5. 风险评估

### 5.1 文件大小

| 文件 | 大小 | 影响 |
|------|------|------|
| parser.c | ~4.7 MB | Git 仓库膨胀、clone 时间增加 |
| scanner.c | ~5.7 KB | 可忽略 |
| 总计 | ~4.7 MB | 类似 tree-sitter-rust 的 parser.c |

**对比：** tree-sitter-rust 的 parser.c 约 ~3.5 MB（crates.io 预编译），vendored tree-sitter-cangjie 约 ~4.7 MB。增量可接受。

### 5.2 编译时间

- parser.c ~152K 行编译预计需 5-15 秒（macOS ARM64）
- `cc` crate 有缓存（`target/` 目录中增量编译）
- 仅在 `tree-sitter-cangjie` feature 启用时编译
- CI 影响：需在 CI 环境中安装 C 编译器（Linux: gcc/clang, macOS: Xcode CLI）

### 5.3 平台兼容性

| 平台 | 预期 | 风险 |
|------|------|------|
| macOS ARM64 | ✅ 已验证（tree-sitter-rust 编译） | 低 |
| Linux x86_64 | ✅ CI 标准环境 | 低 |
| Windows MSVC | ⚠️ 未验证 | 中（cc crate 支持 MSVC，但 scanner.c 可能依赖 POSIX API） |
| musl | ⚠️ 未验证 | 中 |

### 5.4 上游更新风险

- tree-sitter-cangjie 版本 0.1.0，未来可能 breaking change
- 上游 commit `ff447b57` 是已知 good version（GitNexus-RC 已验证）
- 更新策略：pin 特定 commit，手动同步更新

### 5.5 已知 parser bug

GitNexus-RC RISK_LEDGER §3.3 记录了 4 个已知 bug：
1. bare func + public interface 多行体解析错误
2. static + public interface 多行体解析错误
3. sealed interface 多行体解析错误
4. match case range pattern 不支持

这些 bug **不影响 baseline symbol extraction**（class/struct/enum/interface/function/method），但在编写 fixture 时需要注意 workaround。

## 6. 替代方案评估

### 6.1 完全不做 tree-sitter-cangjie

| 能做 | 不能做 |
|------|--------|
| cjpm.toml/lock 解析 ✅（Slice 1-2 已完成） | AST 级别符号提取 ❌ |
| project model（Slice 3 已完成） | 类型引用提取（ACCESSES/USES edges）❌ |
| cjc/cjlint diagnostics（subprocess，不需要 tree-sitter） | 调用图提取 ❌ |
| cjpm tree → 传递依赖（subprocess） | import 解析（需要 AST 分析 import 语句）❌ |
| LSP client（subprocess） | 命名绑定合成 ❌ |

**结论：** 不做 tree-sitter-cangjie，Cangjie 支持只能停留在 manifest/project model 层。无法产出 graph nodes/edges（DEFINES/HAS_METHOD/IMPORTS/CALLS 等），也无法做 import resolution 或 reference extraction。

### 6.2 Text-level regex fallback（类似 TextItemExtractor）

对 `.cj` 文件做类似 Rust `TextItemExtractor` 的逐行正则扫描：
- **能做**：提取 top-level `class`/`struct`/`enum`/`interface`/`func` 名称和位置
- **精度低**：无法处理嵌套结构、impl 块、extension 等
- **无 AST 信息**：无法提取 imports、type annotations、method bodies
- **开发成本**：需要专门为 Cangjie 语法编写 regex 模式（与 Rust 语法不同）

这是 **不引入 tree-sitter 情况下的最小替代方案**，但 scope 有限。

### 6.3 等待上游 crates.io 发布

- 如果 tree-sitter-cangjie 未来发布到 crates.io，可直接引用（无需 vendor）
- 但上游发布时间不确定，可能数月或更久
- 不推荐作为当前策略

### 6.4 Submodule 替代 vendor

- 用 git submodule 引用 `https://gitcode.com/Cangjie-SIG/tree-sitter-cangjie`
- 优点：不复制 parser.c 到仓库，只引用 commit hash
- 缺点：gitcode 外部仓库可用性不确定；需要 CI 配置 submodule init
- 与 GitNexus-RC 的 vendor 策略不一致（GitNexus-RC 选择的是 vendor）

## 7. 决策点

**用户需在以下选项中做出选择：**

| 选项 | 描述 | 风险 | 推荐 |
|------|------|------|------|
| **A** | 批准 vendor + feature gate，进入 Slice 5（tree-sitter-cangjie 集成） | 中 | **推荐** |
| **B** | 先做 text-level regex fallback，推迟 tree-sitter vendor | 低（但能力上限低） | 保守 |
| **C** | 等待上游 crates.io 发布 | 低（但时间不确定） | 不推荐 |
| **D** | 不做 Cangjie tree-sitter 集成（停留在 manifest/project model 层） | 无 | 不推荐 |

### 推荐：选项 A

理由：
1. GitNexus-RC 已 vendor 同一 commit 的 parser.c，在生产索引中运行稳定
2. Mulan PSL v2.0 与 MIT 兼容，无 license 风险
3. Rust-core 已有 tree-sitter 0.26 + `cc` crate 编译 tree-sitter-rust 的先例
4. vendor 只增加 ~4.7 MB，可接受
5. 不做 tree-sitter 就无法产出 graph nodes/edges，Cangjie 支持停留在"只有 manifest 解析"阶段

### 批准后下一步（Slice 5）

1. 从 GitNexus-RC `gitnexus/vendor/tree-sitter-cangjie/` 复制 parser.c + scanner.c + LICENSE + README.OpenSource 到 Rust-core
2. 新增 `crates/cangjie/build.rs`（`cc::Build`）
3. 更新 `crates/cangjie/Cargo.toml`（feature gate + dependency）
4. 新增 `crates/cangjie/src/extractors/` 模块（symbol extraction via tree-sitter）
5. 最小 fixture + 测试验证 AST parse 成功

## 8. 验证

- [x] 上游来源已审计（gitcode.com/Cangjie-SIG/tree-sitter-cangjie）
- [x] License 已评估（Mulan PSL v2.0，与 MIT 兼容）
- [x] ABI 兼容性已分析（ABI 14，tree-sitter 0.26 预期兼容）
- [x] 编译方案已设计（`cc` crate + build.rs + feature gate）
- [x] Feature gate 已设计（`tree-sitter-cangjie`，默认关闭）
- [x] 风险评估已完成（文件大小/编译时间/平台兼容性）
- [x] 替代方案已评估（text-level fallback / 等待 upstream / 不做）
- [x] 决策点已明确（4 个选项，推荐选项 A）
- [x] 本 slice 不改任何 .rs / Cargo.toml / vendor 文件
- [x] Rust-core cargo fmt/check/test 保持 123/123 pass
