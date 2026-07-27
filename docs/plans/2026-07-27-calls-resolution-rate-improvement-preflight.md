# CALLS Resolution Rate 提升 Preflight

> **日期：** 2026-07-27
> **状态：** Preflight / 提升方案评估（docs-only，不改代码）
> **目的：** 量化 Rust CALLS resolution rate 的提升空间，识别 stop-line 内可做的策略，给出推荐实现顺序。
> **前置：** calls.rs 第二刀拆分已完成（2026-07-26，commit `915caf8`），AGENTS.md「扩 CALLS 前必须先拆 calls.rs」阻塞已解除。

---

## 一、当前实测基线（2026-07-27 self-analysis）

对 CodeLattice 自身仓库跑 `project-model inspect --include calls`，结果：

| 指标 | 值 |
|------|-----|
| 总 call sites | 9157 |
| resolved | 5537 (60.5%) |
| unresolved | 3620 (39.5%) |

> 注：历史 baseline 记录的 65.7%（2370/3609）是 2026-05-08 的数据；当前分母增大至 9157（代码增长 + 更完整的提取），rate 降至 60.5%。这不是退化，是统计口径变化。

### unresolved 构成

| callKind | unresolved 数 | 占比 |
|----------|---------------|------|
| **method-call** | **3376** | **93.2%** |
| free-function | 149 | 4.1% |
| qualified-path | 59 | 1.6% |
| associated-function | 20 | 0.6% |
| external-crate | 16 | 0.4% |

**结论：method-call 是唯一值得攻的大头。** 其余 244 个分散且多为 stop-line 外（局部闭包、cfg-gated、跨模块 variant）。

---

## 二、method-call 失败根因分析

### 2.1 现有解析逻辑（3 道防线）

`calls.rs` 的 `resolve_call_site` method-call 分支：

1. **crate 内唯一同名 method** → resolve（confidence 0.65）
2. **无匹配 → stdlib trait method**（如 `to_string`）→ resolve（0.55）
3. **无匹配 + known receiver type method**（如 `push`/`len`）→ 扫描 **same-function let 绑定** 类型注解推断 receiver type → resolve（0.65）

### 2.2 第 3 道防线的现状（重要修正）

> **2026-07-27 代码核查修正**：初步勘察误判方向 A「未实现」。实际核查 `stdlib_tables.rs:208-303` 发现，`scan_variable_type_annotation` **已经包含 Phase 2b：函数参数类型注解扫描**（L270-303），解析 `fn name(param: Type)` 并匹配 var_name。

也就是说，方向 A 的核心逻辑**已实现并接入** method-call 第 3 道防线。当前仍有 3376 unresolved 的真实原因是：

| receiver 类型来源 | 当前覆盖？ | 说明 |
|-------------------|-----------|------|
| 同函数 let 绑定显式类型 | ✅ Phase 1 | `let v: Vec<_> = ...` |
| 同函数 let 绑定 + 构造器 | ✅ Phase 1 | `let v = Vec::new()` |
| **函数参数类型注解** | ✅ **Phase 2b（已实现）** | `fn f(v: &Vec<u8>)` |
| self 方法 | ❌ 未覆盖 | 见方向 B |
| serde_json::Value 等外部 crate | ❌ stop-line 禁碰 | `d.and_then()` where `d: Value` |
| tree-sitter Node | ❌ stop-line 禁碰 | `node.kind()` |
| 跨函数返回值 / 链式 / trait solving | ❌ 需 type inference | stop-line 外 |

**实际样本核查**（2026-07-27）：抽样的 `d.and_then() @ reducer.rs:143` 实为 `serde_json::Value::and_then`（外部 crate，禁碰）。这印证了大量 unresolved 是外部 crate 方法，非静态可解。

### 2.3 unresolved method-call 样本分析（40 样本人工分类）

| 类别 | 占比 | stop-line 内可解？ |
|------|------|-------------------|
| 函数参数类型注解 receiver | ~25% | ✅ 可解（方向 A） |
| self 方法 | ~3% | ✅ 可解（方向 B） |
| 同函数 let 绑定（现有逻辑应覆盖但漏的） | ~10% | ✅ 可解（修 bug / 补全） |
| tree-sitter Node 方法（kind/walk/child/parent） | ~15% | ❌ external crate，禁碰 |
| stdlib type（Vec/Option/String/HashMap） | ~15% | ✅ 可解（扩表，方向 C） |
| serde_json::Value / 其他外部 crate | ~7% | ❌ external crate，禁碰 |
| 跨函数返回值 / trait solving | ~25% | ❌ 需 type inference，禁碰 |

### 2.4 量化潜力

| 项 | 数量 | 说明 |
|----|------|------|
| unresolved method-call 总数 | 3376 | — |
| 减去 tree-sitter + 外部 crate（禁碰） | -526 (~15.6%) | stop-line 外 |
| 剩余可尝试 | 2850 | — |
| 其中静态可解比例（样本推算） | ~62% | 函数参数 + self + let + stdlib |
| **理论上限（新增可解析数）** | **~1767** | 2850 × 62% |
| **理论 resolution rate 上限** | **~79.8%** | (5537+1767)/9157 |

**保守预估**：C1（~250）+ A（~844）+ B（~75）合计约 ~1169 个新增解析，resolution rate 提升至 ~73%。考虑边界情况折损，实际预期 68-72%。

---

## 三、三个提升方向（按 stop-line 合规性排序）

### 方向 A：函数参数类型注解（潜力修正：已部分实现，增量有限）

> **2026-07-27 修正**：核查发现 Phase 2b 已实现函数参数扫描。本方向的「新逻辑」空间大幅缩小。

**现状**：`scan_variable_type_annotation` 的 Phase 2b 已扫函数参数。但样本核查显示仍有大量 unresolved，根因是 receiver 类型来自**外部 crate（serde_json::Value / tree-sitter Node）**或**跨函数返回值**，这些是 stop-line 外。

**仍可改进的边界**（小增量）：
- A1：`fn ` 启发式定位（`prefix.rfind("fn ")`）可能被注释/字符串/属性里的 "fn " 干扰，导致 func_scope 提取错误 → 漏解。可改为基于 CallerIndex 的 enclosing fn 边界（已有 line_start/line_end）。
- A2：参数是 `self` 形式（`&self`/`&mut self`/`self`）时，Phase 2b 的 `param_name == var_name` 不匹配 "self"（因为 `self` 没有显式 `: Type`）。这和方向 B 重叠。

**预估收益（修正）**：A1 修 bug 约恢复几十到一百个；A2 归入方向 B。**原估 844 偏高，修正为 ~100-200。**

### 方向 B：self 方法解析（低难度，小收益）

**策略**：method-call 的 receiver 是 `self` 时，解析到 enclosing impl 块的同名 method symbol。

**合规性**：✅ stop-line 内。impl 块上下文是静态可见的。

**预估收益**：75 个（样本占 ~3%）。

**实现复杂度**：低。CallerIndex 已有 enclosing function 信息；Symbol 有 `impl_details`；只需在 method-call 分支加 `self` 特判，查 enclosing impl 的 method。

### 方向 C：补全 stdlib trait method 表（低难度，高性价比）

**策略**：补全 `lookup_stdlib_trait_method` 的 match 表。

**合规性验证结果（2026-07-27）**：当前表**只覆盖 3 个方法**（`to_string`/`clone`/`collect`）。高频的 Option/Result/Iterator trait method **全未覆盖**：

| 方法 | unresolved 数 | 可映射到 |
|------|---------------|----------|
| `unwrap_or` | 185 | `std::option::Option::unwrap_or` / `std::result::Result::unwrap_or` |
| `unwrap` | 80 | `std::option::Option::unwrap` / `std::result::Result::unwrap` |
| `map` | 66 | `std::option::Option::map` / `std::result::Result::map` / `std::iter::Iterator::map` |
| `and_then` | 66 | `std::option::Option::and_then` / `std::result::Result::and_then` |
| `count` | 64 | `std::iter::Iterator::count` |
| `unwrap_or_default` | 60 | `std::option::Option::unwrap_or_default` / `std::result::Result::unwrap_or_default` |
| `find` | 46 | `std::iter::Iterator::find` |
| `is_some` | 38 | `std::option::Option::is_some` |
| `cloned` | 19 | `std::iter::Iterator::cloned` |
| `map_or` | 8 | `std::option::Option::map_or` |
| `is_none` / `is_ok` / `is_err` | 14+ | Option/Result |
| **小计** | **~640+** | — |

**重要 nuance**：`unwrap`/`map` 等在 Option 和 Result（甚至 Iterator）上都有同名 method。映射时无法区分 receiver 实际是哪个类型（需 type inference，stop-line 外）。

**两种处理策略**：
- **C1（保守）**：只映射在 std 中**唯一定义**的 trait method（如 `count`/`find`/`cloned` 只在 Iterator；`is_some`/`is_none` 只在 Option）。这些可安全映射，confidence 0.55。预估 ~250 个。
- **C2（激进）**：Option/Result 同名 method 也映射，resolved_symbol_id 取**其中一个**（如优先 Option），confidence 降到 0.45 标注「receiver type 未验证，Option/Result 二选一」。预估 ~640 个，但准确度有损。

**推荐 C1**：保 stop-line 语义纯净，不引入「猜错类型」的风险。C2 的 0.45 confidence 在下游消费时可能误导。

**预估收益（C1）**：~250 个新增解析。resolution rate 60.5% → ~63.2%。

---

## 四、推荐执行顺序

按「收益 / 成本」比排序：

| 优先级 | 方向 | 预估收益 | 难度 | 风险 | 建议 |
|--------|------|----------|------|------|------|
| **P0** | **C1 补全唯一 trait method 表** | ~250 | **极低**（改 match 表） | 极低 | **首选**，最快见效 |
| **P1** | **A1 修 func_scope 定位 bug** | ~100-200 | 低-中 | 低 | 用 CallerIndex enclosing fn 边界替代 `rfind("fn ")` |
| P2 | B self 方法 | ~75 | 低 | 低 | `self`/`&self` 参数特判 |

**保守预估（修正）**：C1（~250）+ A1（~150）+ B（~75）合计约 ~475 个新增解析，resolution rate 提升至 ~65.7%。

**重要诚实结论**：原估的 60.5%→73% 难以达到。大量 unresolved 是 serde_json::Value / tree-sitter Node / 链式调用 / 跨函数返回值，这些在 stop-line 外。**在严格遵守 stop-line 的前提下，resolution rate 的现实天花板约 66-68%**。要突破必须放开 type inference 或 external crate 解析，那是另一条路线（需先修订 stop-line）。

---

## 五、Stop-Line 边界确认（严格）

本提升方案 **明确不触碰** 以下（AGENTS.md stop-line）：

- ❌ **不做 full receiver type inference**：不推断 `let v = get_vec();` 里 `v` 的类型（需跨表达式数据流）
- ❌ **不做 trait solving**：不解析 `impl Trait for T` 的 method dispatch
- ❌ **不解析 external crate API**：tree-sitter Node（526 个）、serde_json::Value 等不碰
- ❌ **不处理链式调用**：`a.b().c()` 不推断 `a.b()` 返回类型

方向 A/B/C **只利用静态可见的语法信息**（函数签名、impl 块、stdlib 表），完全在 stop-line 内。

---

## 六、验证策略

每个方向落地后必须验证：

1. **resolution rate 提升**：self-analysis 的 resolved 数应增加（A 预期 +800 左右）
2. **无回归**：23 个 call-resolution fixtures golden 零漂移（已有 method-call 的不能变 unresolved）
3. **confidence/reason 合规**：新增解析必须带合理 confidence（参数注解 0.65，与现有 receiver type method 一致）和 reason
4. **fixture 补充**：新增的解析策略必须配对应 fixture（如 `fn-param-vec-method`）+ golden，不能只靠真实项目统计证明

---

## 七、与 calls.rs 第三刀（text fallback）的关系

text fallback 提取是**纯结构重构**，不影响 resolution rate。本提升方案是**语义增强**，会增加 calls.rs 逻辑。

按 AGENTS.md 质量要求：「新增逻辑必须带 fixture/harness」「不能无界追加到 calls.rs」。

建议：本提升方案落地时，新增的解析逻辑（param type annotation、self method）应作为**独立的小函数**加入 calls.rs，若 calls.rs 因此再次超过 2000 行（当前 1984），则顺势触发 text fallback 第三刀拆分。两者可串行但不冲突。

---

## 八、下一步

本 preflight 不启动代码改动。确认方向后，建议起 execution card：

`2026-07-27-calls-method-call-param-type-resolution-execution-card.md`

覆盖方向 A + B，write set 限于 `calls.rs` + `calls_index.rs`（caller index 加参数签名）+ 新 fixture + 对应 golden。
