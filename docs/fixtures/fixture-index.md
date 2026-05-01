# Rust-core ProjectModel Fixture 索引

> **日期：** 2026-05-01
> **类型：** fixture 索引
> **状态：** Golden truth from GitNexus-RC

---

## 目的

本文档索引 14 个 Rust ProjectModel golden fixtures。这些 fixtures 是 Rust-core ProjectModel 实现的 **golden truth**。所有 fixtures 源自 GitNexus-RC，其 expected.json 文件是权威的 expected 输出。

---

## Fixture 语料库

| # | Fixture 名称 | 场景 | GitNexus-RC 路径 | Priority | expected.json |
|---|-------------|---------|-------------------|----------|--------------|
| A | rust-cargo-root-baseline | Repo root 单 package 带 lib target | `gitnexus/test/fixtures/lang-resolution/rust-cargo-root-baseline/` | P0 | ✅ |
| B | rust-cargo-root-subdirectory | 子目录 `backend/` 中的 package | `gitnexus/test/fixtures/lang-resolution/rust-cargo-root-subdirectory/` | P0 | ✅ |
| C | rust-workspace-explicit-member | Workspace 带显式成员 `backend` | `gitnexus/test/fixtures/lang-resolution/rust-workspace-explicit-member/` | P0 | ✅ |
| D | rust-virtual-workspace-glob | Virtual workspace 带 glob 成员 `crates/*` | `gitnexus/test/fixtures/lang-resolution/rust-virtual-workspace-glob/` | P0 | ✅ |
| E1 | rust-ambiguous-root-nested-package | Root package + nested backend package | `gitnexus/test/fixtures/lang-resolution/rust-ambiguous-root-nested-package/` | P0 | ✅ |
| E2 | rust-ambiguous-root-duplicate-member | 重复成员去重 | `gitnexus/test/fixtures/lang-resolution/rust-ambiguous-root-duplicate-member/` | P0 | ✅ |
| E3 | rust-cargo-root-missing | Source file 在任何 package 之外 | `gitnexus/test/fixtures/lang-resolution/rust-cargo-root-missing/` | P0 | ✅ |
| E4 | rust-virtual-workspace-not-crate-root | Virtual workspace root 不作为 crate root | `gitnexus/test/fixtures/lang-resolution/rust-virtual-workspace-not-crate-root/` | P0 | ✅ |
| T1 | rust-target-lib-and-main | Lib + main target 共存 | `gitnexus/test/fixtures/lang-resolution/rust-target-lib-and-main/` | P0 | ✅ |
| T2 | rust-target-bin-worker | Named bin target `src/bin/worker.rs` | `gitnexus/test/fixtures/lang-resolution/rust-target-bin-worker/` | P0 | ✅ |
| T3 | rust-target-shared-module-ambiguous | Shared module 解析到最短路径（lib） | `gitnexus/test/fixtures/lang-resolution/rust-target-shared-module-ambiguous/` | P0 | ✅ |
| E5a | rust-nested-package-unlisted | 未在 workspace 成员中的 nested package | `gitnexus/test/fixtures/lang-resolution/rust-nested-package-unlisted/` | P0 | ✅ |
| E5b | rust-nested-package-explicit-member | Nested + outer 显式成员 | `gitnexus/test/fixtures/lang-resolution/rust-nested-package-explicit-member/` | P0 | ✅ |
| E5c | rust-nested-package-glob-member | Explicit + glob 去重 | `gitnexus/test/fixtures/lang-resolution/rust-nested-package-glob-member/` | P0 | ✅ |

---

## Fixture 详情

### A: rust-cargo-root-baseline

- **场景：** Repo root 单 package 带 lib target
- **验证：** Repo root 的 baseline `Cargo.toml`，`src/lib.rs` 作为 lib target
- **Priority：** P0
- **Expected：** 1 package，1 target，`crate::` → `src/lib.rs`
- **GitNexus-RC：** `rust-subdirectory-cargo-root.test.ts`

### B: rust-cargo-root-subdirectory

- **场景：** 子目录 `backend/` 中的 package
- **验证：** 子目录 package 被发现，`crate::` 解析到 `backend/src/lib.rs`
- **Priority：** P0
- **Expected：** 1 package（backend），无 root fallback
- **GitNexus-RC：** `rust-subdirectory-cargo-root.test.ts`

### C: rust-workspace-explicit-member

- **场景：** Workspace 带显式成员 `backend`
- **验证：** Workspace `members = ["backend"]` 展开，`crate::` → member root
- **Priority：** P0
- **Expected：** 1 workspace，1 member，root 不作为 crate root
- **GitNexus-RC：** `rust-workspace-member-expansion.test.ts`

### D: rust-virtual-workspace-glob

- **场景：** Virtual workspace 带 glob 成员 `crates/*`
- **验证：** Simple glob 展开，virtual workspace root 不作为 crate root
- **Priority：** P0
- **Expected：** 1 virtual workspace，2+ members from glob，root 不作为 crate root
- **GitNexus-RC：** `rust-workspace-member-expansion.test.ts`

### E1: rust-ambiguous-root-nested-package

- **场景：** Root package + nested backend package
- **验证：** 两个 packages 都被发现，nearest wins，无 backend→root false edge
- **Priority：** P0
- **Expected：** 2 packages，backend source → backend，无 false edge to root
- **GitNexus-RC：** `rust-ambiguous-cargo-root-guard.test.ts`

### E2: rust-ambiguous-root-duplicate-member

- **场景：** 重复成员去重
- **验证：** Explicit + glob 重复去重，无 duplicate packages
- **Priority：** P0
- **Expected：** 1 package（backend，deduped），无 duplicate edges
- **GitNexus-RC：** `rust-ambiguous-cargo-root-guard.test.ts`

### E3: rust-cargo-root-missing

- **场景：** Source file 在任何 package 之外
- **验证：** `scripts/setup.rs` 被解析，外部 source 无 `crate::` edge
- **Priority：** P0
- **Expected：** 1 package（backend），外部 source → 无 high-confidence edge
- **GitNexus-RC：** `rust-ambiguous-cargo-root-guard.test.ts`

### E4: rust-virtual-workspace-not-crate-root

- **场景：** Virtual workspace root 不作为 crate root
- **验证：** Virtual workspace root（无 `[package]`）不作为 crate root
- **Priority：** P0
- **Expected：** 1 virtual workspace，member `api`，root `src/lib.rs` 不作为 crate root
- **GitNexus-RC：** `rust-ambiguous-cargo-root-guard.test.ts`

### T1: rust-target-lib-and-main

- **场景：** Lib + main target 共存
- **验证：** Lib target `crate::` → `src/lib.rs`，bin target `crate::` → `src/main.rs`
- **Priority：** P0
- **Expected：** 1 package，2 targets，无 cross-import
- **GitNexus-RC：** `rust-cargo-target-root-ambiguity.test.ts`

### T2: rust-target-bin-worker

- **场景：** Named bin target `src/bin/worker.rs`
- **验证：** `src/bin/worker.rs` 被索引，worker→local IMPORTS，`src/bin/` 未被忽略
- **Priority：** P0
- **Expected：** 2+ targets（lib，worker bin），worker → worker target
- **GitNexus-RC：** `rust-cargo-target-root-ambiguity.test.ts`

### T3: rust-target-shared-module-ambiguous

- **场景：** Shared module 解析到最短路径（lib）
- **验证：** `src/shared.rs` 解析到 lib target（最短路径），不是 main target
- **Priority：** P0
- **Expected：** Shared module → lib target，无 main target false edge
- **GitNexus-RC：** `rust-cargo-target-root-ambiguity.test.ts`

### E5a: rust-nested-package-unlisted

- **场景：** 未在 workspace 成员中的 nested package
- **验证：** `backend/tools/Cargo.toml` 被发现，不在 workspace members 中
- **Priority：** P0
- **Expected：** 2 packages（backend，tools），tools→tooling package，无 backend false edge
- **GitNexus-RC：** `rust-nested-package-workspace-member.test.ts`

### E5b: rust-nested-package-explicit-member

- **场景：** Nested + outer 显式成员
- **验证：** 两个 packages 都被发现，nested→nested package，无 false cross-edge
- **Priority：** P0
- **Expected：** 2 packages（backend，tooling），各自 ownership，无 false edge
- **GitNexus-RC：** `rust-nested-package-workspace-member.test.ts`

### E5c: rust-nested-package-glob-member

- **场景：** Explicit + glob 去重
- **验证：** Explicit + glob 都匹配 `tooling`，去重，无 duplicate edges
- **Priority：** P0
- **Expected：** 2 packages（backend，tooling），无 duplicate edges，无 false edge
- **GitNexus-RC：** `rust-nested-package-workspace-member.test.ts`

---

## Fixture Source 位置

所有 fixture 源文件在 GitNexus-RC：

```
GitNexus-RC/
  gitnexus/
    test/
      fixtures/
        lang-resolution/
          rust-cargo-root-baseline/
          rust-cargo-root-subdirectory/
          rust-workspace-explicit-member/
          rust-virtual-workspace-glob/
          rust-ambiguous-root-nested-package/
          rust-ambiguous-root-duplicate-member/
          rust-cargo-root-missing/
          rust-virtual-workspace-not-crate-root/
          rust-target-lib-and-main/
          rust-target-bin-worker/
          rust-target-shared-module-ambiguous/
          rust-nested-package-unlisted/
          rust-nested-package-explicit-member/
          rust-nested-package-glob-member/
```

---

## 未来同步策略

Fixture 源文件**留在 GitNexus-RC** 作为权威来源。此 workspace 未来可：

1. **Vendor fixture snapshots** — 复制 input/ 文件用于离线 Rust-core 测试
2. **Reference expected.json** — 使用 GitNexus-RC expected.json 作为 golden truth
3. **Track GitNexus-RC changes** — 监控 GitNexus-RC 的 fixture 更新

此 workspace **不复制** GitNexus-RC 历史日志、implementation notes 或 closure review 产物。

---

*来源：GitNexus-RC docs/language-support/plans/2026-05-01-rust-core-project-model-golden-fixture-spec.md*
*Golden truth：GitNexus-RC 中 14 个 expected.json 文件*
