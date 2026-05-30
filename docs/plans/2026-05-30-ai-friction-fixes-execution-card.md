# AI 使用摩擦修复：CLI 输出档位 / Workspace 过度拆分 / Clean fast-path / Feature 提示

> **状态**: Execution Card · **日期**: 2026-05-30
> **目标**: 修复 4 个 AI 日常使用摩擦，提交推送 gitcode master

---

## Write Set（修改范围）

| 文件 | 改动 |
|------|------|
| `crates/cli/src/lib.rs` | 任务 1（--profile 参数 + 输出过滤）、任务 3（clean fast-path）、任务 4（错误提示） |
| `crates/workspace-model/src/lib.rs` | 任务 2（ProjectInfo 区分 manifest-backed / source-only） |
| `crates/cli/src/mcp_server.rs` | 任务 2（workspace auto-entry taxonomy 修正） |
| `Cargo.toml` (cli crate) | 任务 4（默认 feature 调整，如选方案 A） |
| `crates/cli/tests/lib.rs` 或新 test 文件 | 回归测试 |

## Forbidden Set（禁止改动）

- 不改 open-nwe / cangjie 等 live repo
- 不改 GitNexus-RC
- 不新增 MCP tool 数量（默认 6，full 49）
- 不同步 CodeLattice-Tool
- 不改 graph schema（NodeKind / EdgeKind 语义）

---

## 任务 1: CLI analyze --profile 参数

### 改动点

1. `Commands::Analyze` struct 增加 `--profile` 参数（`full|compact|symbols|modules`，默认 `full`）
2. Rust/Cangjie/ArkTS/TS/C/Cpp/Python/Shell 各语言分析完成后，根据 profile 过滤输出
3. `--profile full` 保持旧行为完全不变

### Profile 输出规格

**symbols**:
```json
{
  "schemaVersion": "codelattice.analyzeSymbols.v1",
  "root": "...",
  "language": "rust",
  "generatedFrom": { ... },
  "stats": { "symbolCount": 2598, "sourceFileCount": 139, "nodeCount": 5516, "edgeCount": 6429 },
  "symbols": [
    { "id": "...", "name": "...", "kind": "function", "file": "...", "line": 123, "modulePath": "...", "visibility": "pub" }
  ]
}
```

**modules**:
```json
{
  "schemaVersion": "codelattice.analyzeModules.v1",
  "root": "...",
  "language": "rust",
  "modules": [
    { "module": "crate::mcp_server", "fileCount": 1, "symbolCount": 42, "publicSymbolCount": 5, "riskHints": [], "readFirst": true }
  ]
}
```

**compact**:
```json
{
  "schemaVersion": "codelattice.analyzeCompact.v1",
  "root": "...",
  "language": "rust",
  "summary": { ... },
  "topModules": [ ... ],
  "topPublicSymbols": [ ... ],
  "entryPoints": [ ... ],
  "topRisks": [ ... ],
  "omitted": { "fullGraphEdges": true, "diagnostics": true, "detailHint": "Use --profile full for complete graph" }
}
```

### 实现

- 分析完成后统一做后处理过滤，不侵入各语言分析逻辑
- `fn filter_analyze_profile(result: &Value, profile: &str) -> Value` 
- symbols 从 `result.graph.nodes` 中提取 kind=symbol 的节点
- modules 从 nodes 的 modulePath/file 聚合
- compact 在 summary 基础上加 top-N 切片

---

## 任务 2: Workspace taxonomy 修正

### 根因

`workspace-model/src/lib.rs:327` 的 `detect_by_extensions()` 把任何含 2+ 源文件的目录当作 project。这是 `frontend/src/components/mission-center` 被识别为独立 project 的原因。

### 改动点

1. `ProjectInfo` 增加 `is_manifest_backed: bool` 字段
2. `detect_by_extensions` 返回的 `ProjectInfo` 标记 `is_manifest_backed = false`
3. `detect_project_at` 通过 manifest 检测的标记 `is_manifest_backed = true`
4. `build_workspace_auto_entry` 区分处理：
   - `supportedProjects` / `recommendedProjectCount` 只包含 `is_manifest_backed = true`
   - 新增 `sourceOnlyAreas` 列表（compact 模式只返回 top 5 + summary）
5. `scan_workspace_inventory` 的返回结构保持 `Vec<ProjectInfo>` 不变（向后兼容），消费侧按 `is_manifest_backed` 过滤

### 数据流

```
scan_workspace_inventory()
  → Vec<ProjectInfo>  // is_manifest_backed 字段区分
    → CLI: supportedProjects 只取 manifest-backed
    → MCP: 同上 + sourceOnlyAreas 列表
```

---

## 任务 3: detect-changes clean fast-path

### 改动点

1. `build_detect_changes_report` 开头判断 clean 条件：
   - `changed_file_count + unknown_hunk_count + untracked_file_count == 0`
2. Clean 条件下：
   - `summary.riskLevel` = `"none"`（不可能是 high）
   - compact 模式只输出最小字段：schemaVersion/status/root/language/scope/summary/changedSymbols/risk/generatedFrom/detailHint
3. 非 compact 模式保持完整输出

### 实现

在 `build_detect_changes_report` 函数中，report 构建完成后，插入 clean fast-path 逻辑：

```rust
let is_clean = changed_file_count == 0 
    && changed_symbol_count == 0 
    && unknown_hunk_count == 0 
    && untracked_file_count == 0;

if is_clean && compact {
    return json!({
        "schemaVersion": "codelattice.detectChanges.v1",
        "status": "clean",
        "root": root.to_string_lossy(),
        "language": language,
        "scope": scope,
        "summary": { "riskLevel": "none", ...counts },
        "changedSymbols": [],
        "risk": { "overallRisk": "none" },
        "generatedFrom": { ... },
        "detailHint": "No changes detected. Use --compact=false for full report."
    });
}
```

---

## 任务 4: Feature 提示优化

### 方案选择：方案 B（改提示，不改默认 feature）

理由：不改 release/install scripts，不影响现有 CI。

### 改动点

1. 各语言 bridge 的 disabled 错误提示统一格式：
   ```
   This dev binary was built without {language} support.
   CodeLattice-Tool installed binary includes {language}.
   For source builds: cargo build --features tree-sitter-{lang} or ALL_LANGUAGE_FEATURES.
   ```
2. `crates/cli/src/lib.rs` 或各 `*_bridge.rs` 中的提示字符串更新

### 涉及文件

- `crates/cli/src/python_bridge.rs`
- `crates/cli/src/javascript_bridge.rs` (TypeScript)
- 其他有类似 disabled 提示的 bridge 文件

---

## 测试计划

1. `cargo fmt --check`
2. `cargo test`
3. `codelattice analyze --root . --language rust --profile symbols` — 不含完整 graph edges
4. `codelattice analyze --root . --language rust --profile compact` — payload < 阈值
5. `codelattice analyze --root /path/to/open-nwe --language auto` — project count 只数 manifest-backed
6. `codelattice detect-changes --root . --language rust --scope all --compact` — clean 时 status=clean
7. Toolset 验证：默认 AI 6，full 49

---

## Stop-lines

- 不做 trait solving / type inference / macro expansion
- 不改 graph schema 语义
- 不改 GitNexus-RC
- 不改 live repos
