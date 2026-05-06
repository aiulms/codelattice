# Phase 2 Slice 8 Preflight — Cangjie diagnostics runner (cjc/cjlint subprocess)

**日期：** 2026-05-06
**状态：** Preflight（待用户 gate）
**前置：** Slice 7（Cangjie graph output）✅

## 0. 问题定义

Slice 7 已完成 Cangjie graph output（Repository/Package/SourceFile/Symbol 节点 + ContainsPackage/OwnsSource/Defines 边）。当前 graph 缺少：

- **Diagnostic 节点**：cjc compiler errors/warnings、cjlint linter suggestions
- **ANNOTATES 边**：Diagnostic → Symbol（标注哪些符号有问题）

GitNexus-RC TS 侧已有完整的 diagnostics pipeline（cjc-runner.ts + cjlint-runner.ts + diagnostics/types.ts + graph-emitter.ts），产出 1,113 Diagnostic nodes + ANNOTATES edges。Rust-core 需要对应的 Rust-native 实现。

核心问题：**如何在 Rust 中安全、可降级地调用 Cangjie SDK 工具链（cjc/cjlint）并集成到 cangjie graph output？**

## 1. 背景：TS 侧已有参考实现

| TS 模块 | 行数 | 功能 |
|---------|------|------|
| cjc-runner.ts | ~130 | spawn cjc --diagnostic-format=json，parse JSON output，1-based→0-based normalize |
| cjlint-runner.ts | ~180 | spawn cjlint -r json -o tmpfile，defectLevel→severity map，DYLD_LIBRARY_PATH |
| diagnostics/types.ts | ~60 | NormalizedDiagnostic（filePath/severity/message/source/rule/startLine/startColumn/endLine/endColumn） |
| diagnostics/index.ts | ~20 | compose cjc + cjlint → unified diagnostics list |
| graph-emitter.ts | ~140 | emitDiagnosticGraph()：NormalizedDiagnostic[] → Diagnostic nodes + ANNOTATES edges |
| resolve-tool.ts | ~70 | CANGJIE_HOME / CANGJIE_SDK_HOME → resolve tool path，buildSpawnEnv |

总计 ~600 行可移植逻辑。

## 2. 方案分析

### 方案 A：Rust subprocess diagnostics runner（推荐）

**做法：** 在 cangjie crate 内新增 `diagnostics/` 模块，使用 `std::process::Command` 调用 cjc/cjlint。

**需要做的：**
1. 新增 `crates/cangjie/src/diagnostics/mod.rs` + `runner.rs` + `types.rs`
2. 实现 SDK tool discovery（CANGJIE_HOME → CANGJIE_SDK_HOME → PATH）
3. 实现 cjc runner：`cjc --diagnostic-format=json --output-type=staticlib <file>`
4. 实现 cjlint runner：`cjlint -r json -o <tmpfile> <repo_root>`
5. 定义 `CangjieDiagnostic` 类型（对齐 NormalizedDiagnostic）
6. 实现 `emit_cangjie_diagnostics()` → Diagnostic nodes + ANNOTATES edges
7. 集成到 `inspect_cangjie_project()`（opt-in feature gate 或默认）
8. Graceful degrade：SDK 不可用时 skip diagnostics，不崩溃

**优点：**
- 与 TS 侧架构一致（已验证可行）
- `std::process::Command` 是 Rust 标准库，零新增依赖
- Bounded slice：预计 300-500 行新增代码
- 不改 project-model / CLI / GitNexus-RC
- 可 feature-gate（`tree-sitter-cangjie` feature 已存在，可复用或新增 `cangjie-diagnostics`）

**缺点：**
- 依赖本机安装 Cangjie SDK（cjc + cjlint 二进制）
- macOS 需要 DYLD_LIBRARY_PATH 设置（与 TS 侧相同）
- subprocess 调用有平台差异（Windows vs Unix）
- cjc 对单文件编译可能因缺少依赖而失败（与 TS 侧相同限制）

### 方案 B：LSP client diagnostics（deferred）

**做法：** 实现 LSP client，通过 `textDocument/publishDiagnostics` 获取 diagnostics。

**优点：**
- 更丰富的诊断信息（与 IDE 体验一致）
- 不需要逐文件 spawn cjc

**缺点：**
- 工程量远超 diagnostics runner（需要 JSON-RPC framing、async I/O、进程管理）
- LSP server 需要 `cjpm build` 编译项目才能产出完整 diagnostics
- TS 侧已验证：minimal cjpm project 在 3s 内 LSP 不推送 diagnostics
- 不适合作为 bounded slice

**推荐：方案 A（diagnostics runner），方案 B 留待 Phase 3 LSP client preflight。**

### 方案 C：跳过 diagnostics，直接进入其他方向

**做法：** 不实现 diagnostics，转向其他 Cangjie 能力（如 reference extraction、import resolution）。

**缺点：**
- Graph output 不完整（缺少 Diagnostic 节点/ANNOTATES 边）
- 下游 consumer 无法感知代码质量问题
- TS 侧已验证 diagnostics 价值（1,113 diagnostics on production fixture）

**不推荐。** Diagnostics 是 graph output 的自然扩展，且 TS 侧已验证可行。

## 3. 推荐方案详细设计（方案 A）

### 3.1 Module 结构

```
crates/cangjie/src/diagnostics/
  mod.rs          # pub mod runner; pub mod types; re-exports
  runner.rs       # run_cjc_diagnostics() + run_cjlint_diagnostics()
  types.rs        # CangjieDiagnostic { file_path, severity, message, source, rule, start_line, start_column, end_line, end_column }
```

### 3.2 SDK tool discovery

```rust
/// Resolve path to a Cangjie SDK tool (cjc, cjlint, cjpm, etc.)
/// Priority: CANGJIE_HOME > CANGJIE_SDK_HOME > PATH
fn resolve_cangjie_tool(tool_name: &str) -> Option<PathBuf>;

/// Build environment variables for spawning Cangjie SDK tools
/// (adds DYLD_LIBRARY_PATH on macOS)
fn build_cangjie_spawn_env() -> HashMap<String, String>;
```

与 TS `resolve-tool.ts` 对齐。

### 3.3 cjc runner

```rust
/// Run cjc compiler diagnostics on a single source file.
/// Returns Vec<CangjieDiagnostic> or error if cjc not available.
/// cjc exit code 1 = has diagnostics (not a crash).
pub fn run_cjc_diagnostics(source_file: &Path) -> Result<Vec<CangjieDiagnostic>, CangjieDiagnosticError>;
```

调用：`cjc --diagnostic-format=json --output-type=staticlib <file>`
- stdout：JSON diagnostics array（exit code 0）
- stderr：JSON diagnostics array（exit code 1，有编译错误）
- 1-based line/column → 0-based normalize（与 TS 一致）

### 3.4 cjlint runner

```rust
/// Run cjlint on a project root directory.
/// Writes JSON output to temp file, parses, filters to project files.
pub fn run_cjlint_diagnostics(project_root: &Path) -> Result<Vec<CangjieDiagnostic>, CangjieDiagnosticError>;
```

调用：`cjlint -r json -o <tmpfile> <project_root>`
- 读取 tmpfile JSON，defectLevel → severity 映射
- 按 project_root 过滤（只保留项目文件，排除 SDK/builtin）

### 3.5 Diagnostic 类型

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CangjieDiagnostic {
    pub file_path: String,       // repo-relative
    pub severity: DiagnosticSeverity,  // Error/Warning/Note/Suggestion
    pub message: String,
    pub source: String,          // "cjc" | "cjlint"
    pub rule: Option<String>,    // cjlint rule code
    pub start_line: usize,       // 0-based
    pub start_column: usize,     // 0-based
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Serialize)]
pub enum DiagnosticSeverity { Error, Warning, Note, Suggestion }
```

### 3.6 Graph 集成

扩展 `graph.rs`：
- 新增 `NodeKind::Diagnostic`
- 新增 `EdgeKind::Annotates`（Diagnostic → Symbol）
- 新增 `emit_cangjie_diagnostics()` 函数

或保持 graph.rs 不变，在 diagnostics/ 模块内实现 graph emission。

### 3.7 入口集成

```rust
/// Extended inspect that includes diagnostics.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn inspect_cangjie_project_with_diagnostics(
    root: &Path,
    include_diagnostics: bool,
) -> Result<CangjieGraphOutput, ...>;
```

### 3.8 Graceful degrade

- SDK 不可用：返回空 diagnostics，不崩溃
- cjc 对单文件失败：返回 error diagnostic（source="cjc"）
- cjlint 无输出：返回空 diagnostics
- subprocess timeout：30s default
- 所有 subprocess 调用带 `#[cfg(not(test))]` 或 mockable trait

## 4. Forbidden（不可协商）

- 不改 project-model crate
- 不改 CLI crate（先不加 CLI flag，纯 library API）
- 不改 GitNexus-RC runtime / Tool / live repo
- 不嵌入 Cangjie 编译器逻辑（只做 subprocess + parse）
- 不新增 workspace 依赖
- 不新增外部 crate 依赖（只用 std::process::Command + serde_json）
- 不做 LSP client
- 不做 incremental diagnostics
- 不做 diagnostics auto-fix
- 不把 diagnostics 设为默认（opt-in feature gate 或 function parameter）

## 5. Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/diagnostics/mod.rs` | 新建 | 模块声明 + re-exports |
| `crates/cangjie/src/diagnostics/runner.rs` | 新建 | cjc/cjlint subprocess + tool discovery |
| `crates/cangjie/src/diagnostics/types.rs` | 新建 | CangjieDiagnostic + DiagnosticSeverity |
| `crates/cangjie/src/graph.rs` | 编辑 | 可选：新增 Diagnostic NodeKind + Annotates EdgeKind |
| `crates/cangjie/src/lib.rs` | 编辑 | 新增 `pub mod diagnostics` |
| `crates/cangjie/tests/diagnostics_smoke.rs` | 新建 | SDK-present 测试 + graceful degrade 测试 |
| `docs/plans/2026-05-06-cangjie-phase2-slice8-execution-card.md` | 新建 | 执行卡 |

## 6. Acceptance Criteria

- [ ] `cargo build` 成功（不启用 feature）
- [ ] `cargo build --features tree-sitter-cangjie` 成功
- [ ] `cargo test` 保持已有测试通过（142+）
- [ ] SDK-present 环境下 cjc diagnostics 测试通过
- [ ] SDK-present 环境下 cjlint diagnostics 测试通过
- [ ] SDK-absent 环境下 graceful degrade 测试通过
- [ ] Diagnostic JSON parse 正确（1-based→0-based normalize）
- [ ] Diagnostic graph nodes + ANNOTATES edges 产出正确
- [ ] `cargo fmt --check` clean
- [ ] 零新增依赖

## 7. Stop-line / Deferred

- 不做 LSP client（需单独 preflight）
- 不做 diagnostics auto-fix
- 不做 incremental diagnostics
- 不做 diagnostics 默认启用
- 不做 CLI flag（`--include-diagnostics`）
- 不做 cross-file diagnostics correlation
- 不改 project-model / CLI / GitNexus-RC

## 8. 风险

| 风险 | 缓解 |
|------|------|
| cjc 对单文件可能因缺依赖失败 | 记录为 known limitation；TS 侧有相同限制 |
| macOS DYLD_LIBRARY_PATH 设置 | 复用 TS 侧已知的 buildSpawnEnv 逻辑 |
| subprocess timeout 可能不够 | 30s default，可配置 |
| 测试需 SDK 环境 | 测试分为 SDK-present（需 envsetup.sh）+ SDK-absent（graceful degrade）两部分 |
| cjlint output JSON 格式可能变化 | 定义 CangjieDiagnostic 为 stable contract，上游变化时更新 parser |
| Diagnostic node 新增 NodeKind | 保持在 cangjie crate 内（方案 B2 策略），不影响 project-model schema |

## 9. 与已有 slices 的关系

| Slice | 产出 | Slice 8 依赖 |
|-------|------|-------------|
| Slice 1-3 | manifest + project model | 需要 project root 和 source_files 列表 |
| Slice 4-5 | tree-sitter parser | 不需要（diagnostics 走 subprocess） |
| Slice 6 | symbol extraction | 需要 symbols 做 ANNOTATES target |
| Slice 7 | graph output | 扩展 graph 类型（新增 Diagnostic + Annotates） |

## 10. 下一步

如果本 preflight 获批：
1. 写 Slice 8 execution card（冻结 write set + acceptance criteria）
2. 实现 `crates/cangjie/src/diagnostics/` 模块
3. 添加 diagnostics tests
4. 闭账 → 评估 Slice 9（LSP client preflight 或 reference extraction）

如果不获批：
- 可跳过 diagnostics，进入 reference extraction（import resolution / type annotation）
- 或暂停 B 线，转入 A 线或 Rust analysis 改善
