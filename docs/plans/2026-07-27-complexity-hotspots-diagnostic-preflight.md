# 复杂度热点诊断（complexity_hotspots）Preflight

> **日期：** 2026-07-27
> **状态：** Preflight / 设计 + 风险评估（docs-only，不改代码）
> **目的：** 为新增 MCP 诊断工具 `codelattice_complexity_hotspots` 设计输出 schema、评分模型、实现路径与 v2 扩展点。
> **类型：** 新功能（非重构）

---

## 一、为什么需要这个诊断

现有 ~25 个诊断工具覆盖了**结构正确性**（cycle/impact/api/breaking-change）和**变更风险**（risk_hotspots），但缺少**代码内部复杂度**维度：

| 现有诊断 | 维度 | 复杂度诊断填补 |
|----------|------|----------------|
| risk_hotspots | fan-in/fan-out 耦合风险 | ❌ 不看函数内部复杂度 |
| architecture_drift | 模块间层级违规 | ❌ 不看单函数复杂度 |
| dead_code | 可达性 | ❌ 不看复杂度 |

**真实场景缺口**：开发者接手大仓时，"哪些函数太长/参数太多/过度耦合，需要优先重构"是高频问题，现有工具答不了。复杂度诊断直接回答这个。

---

## 二、与 risk_hotspots 的差异化定位（避免重叠）

| 指标 | risk_hotspots | complexity_hotspots |
|------|---------------|---------------------|
| fan-in / fan-out | ✅ 核心指标 | ✅ 作为组合维度之一 |
| 高 fan-in/fan-out 双高 | ✅ | — |
| **函数长度**（line span）| ❌ 未用 | ✅ 核心指标 |
| **参数数量** | ❌ 未用 | ✅ 核心指标 |
| **async/unsafe 加权** | ❌ 未用 | ✅ 风险加权 |
| **圈复杂度（McCabe）** | ❌ | ⏳ v2（需提取器扩展）|

**定位差异**：risk_hotspots 答"改这个符号影响多大"（耦合向外扩散风险）；complexity_hotspots 答"这个函数本身多难维护"（内部复杂度）。两者互补，不重叠。

---

## 三、路径 1（v1）设计：纯 graph + span 查询

### 3.1 数据来源（全部现有数据，无需新解析）

| 维度 | 数据来源 | 计算 | 语言覆盖 |
|------|----------|------|----------|
| 函数长度 | Symbol.line_start / line_end | `line_end - line_start + 1`（**原始行跨度，含注释/空行**） | 全语言 ✅ |
| 参数数量 | Symbol.type_annotations | `filter(annotationKind == "param-type").count()` | **仅 Rust**（见 3.6）|
| fan-in | GraphView.incoming | 复用 `compute_symbol_hotspot_score` 的 fan_in 计算 | 全语言 ✅ |
| fan-out | GraphView.outgoing | 复用 fan_out 计算 | 全语言 ✅ |
| async/unsafe | Symbol.is_async / is_unsafe | 风险加权 | Rust/TS（语法相关）|

### 3.6 paramCount 语言覆盖裁决（P1 修正）

**事实**：`param-type` 注解只有 Rust 提取器（`item.rs:1559`）发，cangjie / typescript 提取器不发。直接用 `count()` 会导致 TS/Cangjie 项目的 `paramCount` 恒为 0，多参数函数被**静默漏报**（假阴性伪装成健康）。

**裁决（采用评审选项 1）**：
- `metrics.paramCount` 字段类型改 `Option<u32>`：Rust 输出实际数，无注解语言输出 `null`
- 输出 schema 的 `summary` 增加 `coverage` 字段说明各维度可用性：
  ```json
  "summary": {
    "complexityCoverage": {
      "length": true,
      "paramCount": true,   // false 时表示该语言无参数注解，paramCount 维度未参与评分
      "fanIn": true,
      "fanOut": true
    }
  }
  ```
- 评分时：`paramCount == null` 则 `param_score` 维度跳过（不贡献 0 也不贡献满分），并在 `recommendation` 注明"参数维度对该语言不可用"
- `drivers` 数组里 paramCount 相关 driver 仅在 `paramCount.is_some()` 时出现

**不采用选项 3**（给 TS 提取器补 param-type）：扩 write set 违反路径 1 初衷，留作 v2。

### 3.7 函数长度的 raw span 语义（P3 文档修正）

**事实**：函数长度 = `line_end - line_start + 1`，是**原始行跨度**，包含注释和空行。一个 300 行但 200 行是 doc comment 的函数会被排高位。

**处理**：v1 接受此语义（去注释的"逻辑行数"需源码扫描，超出路径 1 范围），但必须**在输出 schema 明示**：
- `metrics.lengthLines` 字段重命名为 `metrics.lengthLinesRaw` 或在 schema 加注释 `"lengthLines": "raw line span including comments/blank lines"`
- 文档（tool description / preflight）写明"raw span"语义，避免用户对排行失去信任
- v2 可考虑提取"非空非注释行数"作为补充维度

### 3.2 复杂度评分模型（v1）

复合分数 = 加权和，每项归一化到贡献区间。**权重用命名常量**（便于调参时 fixture diff 可读）：

```rust
// —— 评分权重常量（命名 + rationale）——
// 长度维度：长函数是最直接的维护负担信号，权重最高
const W_LENGTH: f64 = 4.0;        // length_score 上限
// 参数维度：过多参数是经典代码坏味道（Effective Rust / Refactoring）
const W_PARAMS: f64 = 2.5;        // param_score 上限（paramCount 为 None 时跳过此维度）
// 耦合维度：复用 risk_hotspots 的 fan 判定，但权重低于长度
const W_FAN: f64 = 1.5;           // fan_score 上限
// 修饰符风险加权：async/unsafe 增加认知负担
const W_ASYNC: f64 = 0.5;
const W_UNSAFE: f64 = 0.5;
```

阈值（基于常见代码规范，可参数化）：
- **length**：≤20 行=0，21-50=0.5，51-100=1.5，101-200=2.5，>200=W_LENGTH(4.0)
- **params**（仅 Rust，`paramCount.is_some()`）：≤3=0，4-5=0.5，6-8=1.5，>8=W_PARAMS(2.5)
- **fan**：复用现有 high_fan_in(>5)/high_fan_out(>5) 阈值，命中各贡献 W_FAN/3(0.5)，双高额外 +0.5
- **modifier**：async +W_ASYNC(0.5)，unsafe +W_UNSAFE(0.5)

总分范围 0-8.0+（params 维度不可用时上限降至 5.5），映射等级：
- ≥6.0 → `critical`（强烈建议拆分）
- ≥4.0 → `high`
- ≥2.0 → `medium`
- <2.0 → 不报告（控制噪音）

**调权影响**：权重常量改动会触发 fixture golden 重新生成——命名常量让那次 diff 可读（这是本仓库 golden 测试风格的诉求）。

### 3.3 输出 schema（对齐现有诊断格式）

```json
{
  "complexityHotspots": [
    {
      "symbol": "crate::module::function_name",
      "name": "function_name",
      "kind": "function",
      "sourcePath": "src/lib.rs",
      "lineStart": 120,
      "lineEnd": 280,
      "complexityScore": 6.5,
      "complexityLevel": "critical",
      "metrics": {
        "lengthLinesRaw": 161,
        "paramCount": 7,
        "fanIn": 8,
        "fanOut": 12,
        "isAsync": true,
        "isUnsafe": false
      },
      "drivers": ["excessive-length", "too-many-params", "high-fan-out", "async"],
      "recommendation": "考虑拆分为更小的函数；参数过多可封装为 struct"
    }
  ],
  "summary": {
    "totalSymbols": 2763,
    "analyzedSymbols": 1840,
    "criticalCount": 3,
    "highCount": 12,
    "mediumCount": 45,
    "complexityCoverage": {
      "length": true,
      "paramCount": true,
      "fanIn": true,
      "fanOut": true
    }
  }
}
```

**字段语义**：
- `metrics.lengthLinesRaw`：**原始行跨度**（含注释/空行），非逻辑行数
- `metrics.paramCount`：`Option<u32>`——Rust 输出实际数，无 param-type 注解的语言输出 `null`
- `summary.complexityCoverage`：各评分维度是否实际参与（paramCount 维度对该语言不可用时为 false）

### 3.4 参数

| 参数 | 默认 | 说明 |
|------|------|------|
| `root` | 必填 | 项目根 |
| `language` | "auto" | 语言 |
| `maxResults` | 50 | 最多返回的热点数 |
| `minLevel` | "medium" | 最低报告等级 |
| `includeTests` | false | 是否包含 test 符号 |
| `compact` | true | 复用 facade compact 策略 |

### 3.5 Toolset 归属裁决（P2 修正）

**事实**：`McpToolset` 有 Ai(6) / Core / Full 三档。工具默认进 Full，仅当显式加入 `AI_TOOLSET_TOOLS` 或 `CORE_EXTRA_TOOLS` 白名单才在对应档可见。现有诊断工具（dead_code / risk_hotspots / architecture_drift / impact_analysis 等）**都不在 AI/CORE 白名单**，即诊断类工具惯例只在 Full 档可见。

**裁决**：`codelattice_complexity_hotspots` **进 Full 档**，不加入任何白名单（与同类诊断工具一致）。

**后果**：
- Full 工具数 49 → **50**（触发 P1-① smoke 脚本审计，见 §4.2）
- Ai 档（6 个）不变，Core 档（现有数）不变
- `codelattice-mcp-facade-smoke.sh:41` 的 core 区间断言 `[ "$T" -lt 49 ]` 逻辑上不受影响（core 数不变），但语义上 49 应改为 50 以保持"core < full"的意图——审计时一并更新

---

## 四、实现位置与 write set

### 4.1 实现位置

新增 handler `handle_complexity_hotspots` + 评分函数 `compute_complexity_score`，放在 mcp_server.rs 的诊断区（与 risk_hotspots / dead_code 相邻，约 L14811-16622 的 v0.11 区段附近）。

**注意**：这会往 mcp_server.rs 加 ~150-200 行。按 AGENTS.md 质量要求，新增逻辑应尽量独立。评分函数 `compute_complexity_score` 设计为**纯函数**（输入 symbol node + graph metrics，输出 score + drivers），便于未来抽到独立模块（顺带推进 mcp_server 拆分）。

### 4.2 Write Set

**核心实现（3 文件）**：

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cli/src/mcp_server.rs` | 修改 | +`handle_complexity_hotspots` +`compute_complexity_score`（纯函数）+ 评分权重常量 + 注册到 tools_list + dispatch + `permission_profile_for_tool` |
| `crates/cli/tests/mcp_server.rs` | 修改 | +测试用例（评分逻辑、paramCount=null 路径、过滤、输出 schema）|
| `fixtures/` | 新增 | +复杂度 fixture（含长函数/多参数/高耦合样本）|

**P1-① smoke 脚本工具数断言审计（5 处，49 → 50）**：

| 文件 | 行 | 现断言 | 改为 |
|------|----|--------|------|
| `scripts/codelattice-installed-acceptance.sh` | 179 | `section "Dev: Full Tools (expect 49)"` | `expect 50` |
| `scripts/codelattice-installed-acceptance.sh` | 185 | `assert_eq "Full toolset count" "49"` | `"50"` |
| `scripts/codelattice-mcp-facade-smoke.sh` | 33-34 | `Test 2: full toolset has 49 unique tools` | `50 unique tools` |
| `scripts/codelattice-mcp-facade-smoke.sh` | 36 | `[ "$T" = "49" ] && pass "full-toolset-49"` | `= "50"` / `full-toolset-50` |
| `scripts/codelattice-mcp-facade-smoke.sh` | 41 | `[ "$T" -lt 49 ] ... core-toolset-middle` | `-lt 50`（core 数不变，仅同步上界语义）|
| `scripts/codelattice-mcp.sh` | 38 | `MIN_EXPECTED_TOOLS=49` | `=50` |
| `scripts/codelattice-open-nwe-readonly-smoke.sh` | 175-176 | `assert full_count == 49` / `exposes 49 tools` | `== 50` / `50 tools` |
| `scripts/fresh-clone-smoke.sh` | 127 | `if [[ "${count:-0}" -lt 49 ]]` | `-lt 50` |

> 注：`codelattice-mcp-facade-smoke.sh:41` 的 core 区间断言改为 `-lt 50` 是语义同步（core < full），core 实际数不变。审计时逐一 grep `49` 确认无遗漏（已知 5 文件 8 处）。

**P2-② CHANGELOG / 契约版本处理（裁决：本轮只加 CHANGELOG 条目，不动契约版本）**：

- 本轮在 `CHANGELOG.md` 的 Unreleased 区加一条 `feat(mcp): add codelattice_complexity_hotspots diagnostic`
- **不改 MCP contract 版本号**（schemaVersion / mcp contract version）——新增工具是 additive change，不破坏现有契约，按本仓库 release-versioning policy，版本号由下一次 release 战役统一提升
- **不动 release metadata checker**（`scripts/check-release-metadata.sh`）——它校验的是 release 包元数据，与新增工具无关
- 若 release checker 有 toolCount 硬编码，审计时一并发现并更新（当前未知，实施时 grep 确认）

### 4.3 Forbidden Set

- 不改 Symbol schema / TypeAnnotation schema（路径 1 不需要）
- 不改 item.rs / 提取器（路径 1 不需要新解析；**不给 TS/cangjie 提取器补 param-type**——P1-② 选项 3 明确否决）
- 不改 output.rs / graph.rs
- 不动 Cargo.toml
- 不改其他诊断的评分逻辑（risk_hotspots 等）
- 不破坏现有 MCP tool contract（新增工具是 additive）
- 不把 complexity_hotspots 加入 AI_TOOLSET_TOOLS / CORE_EXTRA_TOOLS 白名单（保持诊断工具 Full-only 惯例）
- 不改 MCP contract / schemaVersion（由 release 战役统一处理）
- 除 §4.2 列出的 8 处 smoke 断言外，不扩大 smoke 脚本改动范围

### 4.4 Stop Line

- `cargo test` 任何已有用例由 pass 转 fail
- 现有诊断输出变化（除新增 complexity_hotspots 外）
- 发现需要改 schema 才能实现（说明路径 1 选错，应转路径 2）
- **smoke 脚本断言未全部更新**就跑 smoke（必须先完成 §4.2 的 8 处审计再 smoke）
- grep `49` 在 scripts/ 下仍有与工具数相关的遗漏（审计不完整）

---

## 五、v2 扩展点（圈复杂度，预留）

路径 1 设计预留 v2 扩展：
- `metrics` 对象预留 `cyclomaticComplexity` 字段（v1 不输出或输出 null）
- v2 在 item.rs 提取阶段，用 tree-sitter 遍历函数 body，数 `if_expression`/`for_expression`/`while_expression`/`match_expression`/`binary_expression(&&)`/`binary_expression(||)` 节点，算 McCabe CC = 1 + decision_points
- v2 需扩 Symbol schema（加 `cyclomatic_complexity: Option<u32>`），触发 output contract 更新 + fixture golden 更新

v2 触发条件：v1 落地后，若用户反馈"长度/参数维度不够，需要更精确的分支复杂度"，再启动 v2。

---

## 六、风险

| 风险 | 级别 | 缓解 |
|------|------|------|
| **smoke 脚本工具数断言遗漏**（P1-①）| **HIGH** | §4.2 已列 5 文件 8 处；实施时逐一改 + grep `49` 复核 |
| **paramCount 假阴性**（P1-②）| **HIGH** | paramCount 改 Option + coverage 声明；TS/cangjie 明确标注不可用而非静默 0 |
| 参数数量统计不准（Rust 内 type_annotations 可能漏）| LOW | 标注"基于显式类型注解"；v2 从 signature 精确提取 |
| fan-in/fan-out 与 risk_hotspots 数值不一致 | LOW | 复用同一计算函数，保证一致 |
| mcp_server.rs 再次膨胀 | LOW | 评分函数设计为纯函数，未来可抽模块；+200 行可接受 |
| 评分阈值主观 | LOW | 权重命名常量化（§3.2），maxResults/minLevel 参数化 |
| 函数长度含注释/空行误导（P3）| LOW | 字段名 `lengthLinesRaw` + 文档明示 raw span 语义 |
| toolset 归属误放（P2-①）| LOW | §3.5 裁决 Full-only，不进白名单 |
| 契约版本误动（P2-②）| LOW | §4.2 裁决只加 CHANGELOG，不动 schemaVersion |

---

## 七、验证策略

1. **单元测试**：`compute_complexity_score` 纯函数测试（不同 length/param/fan 组合的评分）
2. **fixture 测试**：新 fixture 含已知复杂度的函数，验证排行与等级
3. **self-analysis**：对 CodeLattice 自身跑 complexity_hotspots，人工核查 top 10 是否合理（应包含 mcp_server.rs 的大函数、calls.rs 等）
4. **不回归**：现有 738 test 全过

---

## 八、下一步

本 preflight 不启动代码改动。确认设计后，起 execution card 实施。
