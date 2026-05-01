# Rust ProjectModel Fixtures

> **日期：** 2026-05-01
> **类型：** fixture 状态
> **状态：** Source 在 GitNexus-RC

---

## 状态

Golden fixture 源文件**当前在 GitNexus-RC 中**，不在此 workspace。

GitNexus-RC 中的位置：
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

## expected.json 状态

所有 14 个 `expected.json` 文件在 GitNexus-RC：
```
gitnexus/test/fixtures/lang-resolution/rust-*/expected.json
```

这些文件是 Rust-core ProjectModel 实现的 **golden truth**。

---

## 未来同步策略

此目录未来可：
1. **Vendor fixture snapshots** — 复制 `input/` 文件用于离线 Rust-core 测试
2. **Reference expected.json** — 指向 GitNexus-RC 获取 expected 输出

**不要复制历史日志。** 历史留在 GitNexus-RC。

---

## 相关文档

- [Fixture 索引](../../docs/fixtures/fixture-index.md)
- [expected.json Schema](../../docs/fixtures/expected-json-schema.md)
- [GitNexus-RC Golden Fixture Spec](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-golden-fixture-spec.md)
