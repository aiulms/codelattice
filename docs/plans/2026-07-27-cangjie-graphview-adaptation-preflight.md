# Cangjie GraphView 适配 Preflight（多语言诊断推广前置）

> **日期：** 2026-07-27
> **状态：** Preflight / 调研修正 + 适配方案（docs-only）
> **目的：** 让 MCP 诊断（complexity/risk_hotspots/impact/dead_code 等）对 Cangjie 可用。
> **背景：** 初版调研误判为"8 语言碎片化"，经复核 + 证伪测试修正为"Cangjie 专项适配"。

---

## 一、调研修正（事实核对）

### 1.1 证伪测试结果（2026-07-27）

| 语言 | EdgeKind serde | 序列化值 | GraphView 大写匹配 | 初版判断 | 修正 |
|------|----------------|----------|---------------------|----------|------|
| Rust | `SCREAMING_SNAKE_CASE` | `CALLS` | ✅ | ✅ 正确 | — |
| **TS** | `SCREAMING_SNAKE_CASE` (`graph.rs:43`) | `CALLS` | ✅ | ❌ 错（误称 camelCase）| **fan 实测正常** |
| JS/Python/C/CPP/Shell/ArkTS | 全部 `SCREAMING_SNAKE_CASE` | `CALLS` 等 | ✅ | ❓ 未查 | ✅ 全通 |
| **Cangjie** | `camelCase` (`graph.rs:57`) | `uses`/`accesses`/`modifies` | ❌ | ✅ 正确 | **唯一真实缺口** |

**证伪证据**：用 `--features tree-sitter-typescript` 编译跑 TS fixture，`complexity_fan_in_out` 计算出 2 个 symbol fan≠0——证明 TS 诊断今天就是通的，初版"TS fan=0 跑不了"为假。

### 1.2 初版错误根因

看到 `cli/src/lib.rs:4091` 兼容代码匹配 `calls`/`Calls`/`CALLS` 三种形式，误推断"有语言输出小写 calls"。实际那段兼容是为 bridge/legacy 格式准备，TS 原生输出走 `SCREAMING_SNAKE_CASE`，根本不输出小写。**未核查各语言 EdgeKind 头部的 serde 属性**，一步之差结论全歪。

### 1.3 真实图景

**不是"8 语言严重碎片化"——7/8 语言序列化完全一致，GraphView 对它们直接可用。唯一缺口是 Cangjie**，且缺口是**词汇映射 + 字段名差异**，不是数据缺失（Cangjie 的 Uses/Accesses/Modifies 比 CALLS 更细粒度）。

---

## 二、Cangjie 与 GraphView 契约的具体差异

### 2.1 Edge kind（camelCase vs 大写）

| Cangjie EdgeKind | 序列化值 | GraphView 期望 | 映射 |
|------------------|----------|----------------|------|
| `Uses` | `uses` | — | → `REFERENCES`（**不映射 CALLS**，见 2.3）|
| `Accesses` | `accesses` | `ACCESSES` | → `REFERENCES` 或 `ACCESSES` |
| `Modifies` | `modifies` | — | → `REFERENCES` |
| `Imports` | `imports` | `IMPORTS` | → `IMPORTS`（大小写）|
| `ContainsPackage`/`OwnsSource`/`Defines`/`Annotates` | camelCase | 大写 | 结构边，不影响 fan 诊断 |

### 2.2 Symbol node properties 字段名差异

| 维度 | Cangjie 字段 | GraphView/诊断期望 | 影响 |
|------|--------------|---------------------|------|
| symbol 类型 | `"kind"`（如 `"Function"`）| `"symbolKind"` | complexity 按字段名取不到 |
| 起始行 | `"startLine"` | `"lineStart"` | complexity 长度维度取不到 |
| 结束行 | `"endLine"` | `"lineEnd"` | 同上 |
| 名称 | 在 `label`，无 `"name"` | `"name"` | 显示名取不到 |
| 源文件 | 无 `"sourcePath"` | `"sourcePath"` | 文件定位取不到 |
| async/unsafe | 无 | `"isAsync"`/`"isUnsafe"` | modifier 维度不可用（同 Rust paramCount 处理）|

### 2.3 语义注意事项（关键）

**Cangjie 的 `Uses` 包含类型注解引用**（如 `let x: Foo` 产生 Uses 边），**不是纯函数调用**。GraphView 现有 fan 过滤器（`mcp_server.rs:14297`）匹配 `CALLS`/`REFERENCES`/`IMPORTS`。

映射决策：
- `Uses`/`Accesses`/`Modifies` → 统一映射为 `REFERENCES`（而非 `CALLS`）
- 理由：塞进 CALLS 会污染调用语义（类型注解不是调用）；REFERENCES 语义更宽，GraphView fan 过滤器已含它
- 代价：Cangjie 的 fan 是 "referenced-by"（含类型引用），不是严格 "callers"。**必须在诊断输出文档注明**，避免用户误解 fan 含义

---

## 三、适配方案：GraphView 摄入层归一化（单点收口）

### 3.1 Seam 选择

在 `GraphView::build`（`mcp_server.rs:5540`）摄入 graph 数据时，对 Cangjie 的 edge kind 和 node properties 做归一化映射。**不动各语言 emitter、不碰 golden fixture、不改持久化契约**——这是单点收口的最优 seam。

### 3.2 归一化映射实现

**Edge kind 映射**（在 GraphView::build 构建 incoming/outgoing 时）：
```rust
fn normalize_edge_kind(raw: &str) -> &str {
    match raw {
        // Cangjie camelCase → 大写（GraphView fan 过滤器兼容）
        "uses" | "accesses" | "modifies" => "REFERENCES",
        "imports" => "IMPORTS",
        "containsPackage" => "CONTAINS_PACKAGE",
        "ownsSource" => "OWNS_SOURCE",
        "defines" => "DEFINES",
        "annotates" => "ANNOTATES",
        // 其他语言已是 SCREAMING_SNAKE_CASE，原样返回
        other => other,
    }
}
```

**Node properties 别名**（在 node clone 时补全）：
- 检测 Cangjie 特征（`"startLine"` 存在但 `"lineStart"` 不存在）→ 补 `"lineStart"`/`"lineEnd"` 别名
- `"kind"` 存在但 `"symbolKind"` 不存在 → 补 `"symbolKind"` 别名
- `label` 存在但 `"name"` 不存在 → 补 `"name"` 别名
- `"sourcePath"` 缺失 → 从 file 节点关系推断或留空

### 3.3 诊断影响

适配后，以下诊断对 Cangjie 自动可用（无需改诊断逻辑）：
- `codelattice_complexity_hotspots`（length + fan 维度；modifier 维度因无 async/unsafe 不可用，coverage 标注）
- `codelattice_risk_hotspots`
- `codelattice_impact_analysis`
- `codelattice_dead_code_candidates`（依赖 reachability，需确认 entry point 检测兼容）
- 其他依赖 fan 的诊断

### 3.4 文档与输出说明

诊断输出需注明 Cangjie fan 的语义差异：
- `complexityHotspots` / `riskHotspots` 的 fan 字段对 Cangjie 是 "referenced-by"（含类型引用），非纯 "callers"
- 在 `summary` 或 `notes` 加语言特定说明

---

## 四、Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cli/src/mcp_server.rs` | 修改 | +`normalize_edge_kind` + 在 `GraphView::build` 接入归一化 + node properties 别名补全 |
| `crates/cli/tests/mcp_server.rs` | 修改 | +Cangjie fixture 诊断测试（验证 fan≠0、字段映射）|
| `docs/` | 修改 | 注明 Cangjie fan 语义差异 |

### 4.1 Forbidden Set

- 不改 Cangjie emitter（`crates/cangjie/src/graph.rs`）——保持 emitter 输出原样，归一化只在 GraphView 摄入层
- 不改其他语言 emitter
- 不改 Rust graph schema / EdgeKind
- 不改持久化契约 / output contract
- 不改诊断逻辑本身（complexity/risk 等不动）
- 不动 Cargo.toml
- 不把 Cangjie 的 Uses 映射成 CALLS（语义污染）

### 4.2 Stop Line

- `cargo test` 任何已有用例由 pass 转 fail
- Rust/TS 诊断输出变化（归一化不能影响已通的语言）
- 发现 Cangjie 适配需要改 emitter（说明 seam 选错）
- Cangjie dev binary 禁用导致无法本地验证（需用 feature 编译或跳过 Cangjie 测试）

---

## 五、验证挑战

**关键约束**：Cangjie 在 dev binary 禁用（需 `--features tree-sitter-cangjie`），且 Cangjie SDK 可能不在本机。

验证策略：
1. **归一化函数单元测试**：纯函数测试 `normalize_edge_kind` 各输入映射（不依赖 Cangjie runtime）
2. **Cangjie fixture 测试**：用 `--features tree-sitter-cangjie` 编译，跑现有 Cangjie fixture（`fixtures/cangjie/reference-cross-file-basic`），验证诊断 fan≠0
3. **不回归**：Rust/TS 诊断输出零变化（归一化对 SCREAMING_SNAKE_CASE 是 no-op）

---

## 六、scope 与价值

| 维度 | 评估 |
|------|------|
| 工作量 | ~百行（归一化函数 + 接入 + 测试 + 文档）|
| 风险 | 低（单点收口，不动 emitter/诊断逻辑）|
| 价值 | 让所有 fan 依赖诊断对 Cangjie 可用；统一 graph 摄入契约 |
| 不做的事 | 不扩诊断、不改 schema、不碰持久化 |

---

## 七、下一步

本 preflight 修正了初版调研的事实错误（TS 判断错误），scope 从"8 语言归一化中间件"缩到"Cangjie 专项适配"。确认后实施。
