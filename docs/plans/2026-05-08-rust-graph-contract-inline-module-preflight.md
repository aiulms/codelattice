# Preflight: 第5个 Rust Graph Contract Fixture — inline-module

日期：2026-05-08
状态：Preflight
优先级：Priority 2 — Rust graph contract / fixture expansion

## 动机

现有 4 个 Rust graph contract fixture：
1. `portable-smoke` — 基础节点/边类型 + 跨 target calls
2. `imports-cross-crate` — 外部 symbol node + ACCESSES
3. `multi-module` — 跨文件 crate:: path CALLS
4. `module-hierarchy` — 多级模块树 + super:: path

这些 fixture 都使用文件级模块（`mod foo;` → `foo.rs`）。
缺少对 inline module（`mod foo { ... }` 直接内联定义）的 contract 覆盖。

Inline module 是 Rust 的重要特性：
- `self::` 路径在 inline module 内的行为与文件级模块一致
- `super::` 路径逐级向上引用
- `crate::` 路径从 inline module 内引用 crate root

## 设计

### Fixture: `inline-module`（compile-valid）

```
fixtures/rust/inline-module/
  Cargo.toml
  src/
    lib.rs
```

lib.rs 包含：
- 顶层函数 `root_fn()` 
- inline module `inner`：包含函数 `inner_fn()`，通过 `super::root_fn()` 调用顶层函数
- inline module 内嵌套的 inline module `nested`：包含函数 `nested_fn()`
  通过 `crate::root_fn()` 和 `super::inner_fn()` 调用
- 顶层函数 `exercise_self()`：在 inline module 内通过 `self::` 调用同模块函数

### 预期 Graph 产出

- Nodes: Repository(1), Package(1), Target(1), SourceFile(1), Symbol(6+)
- Edges: CONTAINS_PACKAGE(1), HAS_TARGET(1), OWNS_SOURCE(1), DEFINES(6+), CALLS(3+)
- CALLS edges: crate-path(1), super-path(1), self-path(1)
- 0 duplicate, 0 dangling, deterministic

### Write Set

- `fixtures/rust/inline-module/` — 新建 fixture
- `crates/cli/tests/project_model_graph_contract.rs` — 新增 5+ contract tests

### Forbidden Set

- 不修改 calls.rs / model.rs / graph.rs
- 不新增依赖
- 不修改 GitNexus-RC / Tool / live repo

### Acceptance Criteria

1. Contract tests 验证所有节点/边类型
2. 验证 known symbol IDs + known CALLS edges
3. quality gates: 0 dup nodes, 0 dup edges, 0 dangling source/target, deterministic
4. 全部现有测试通过
5. `cargo fmt --check` + `git diff --check` clean
