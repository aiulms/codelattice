# Cangjie Phase 2 Slice 13 — Function Call Reference Extraction Preflight

**Date:** 2026-05-06
**Type:** preflight（docs-only）
**Status:** 进行中
**Author:** aiulms

## 1. Current State Audit（Phase 0 发现）

### 1.1 references.rs 当前能力

当前 AST walk（`walk()`）处理以下节点类型：

| 节点类型 | 提取内容 | Edge | Confidence |
|---------|---------|------|-----------|
| `variableDeclaration` → `userType` | 类型标注 | USES | 0.60 |
| `parameter` → `userType` | 参数类型标注 | USES | 0.60 |
| `returnType` → `userType` | 返回类型标注 | USES | 0.60 |
| `typeArguments` → `userType` | 泛型类型实参 | USES | 0.60 |
| `postfixExpression` → `fieldAccess`（无 callSuffix） | 字段读取 | ACCESSES | 0.65 |
| `assignmentExpression` → `atomicVariable` | 变量写入 | MODIFIES | 0.85 |
| `assignmentExpression` → `postfixExpression.fieldAccess` | 字段写入 | MODIFIES | 0.80 |

### 1.2 Function call gap

当前代码在 `postfixExpression` 处理中**显式检查并跳过**含 `callSuffix` 的节点：

```rust
// references.rs:727
children[children.len() - 2].kind() != "callSuffix"
```

这意味着以下 Cangjie 代码中的函数调用全部不可见：

```cj
let origin = Point(1, 2)         // constructor call → 不提取
let dist = distance(origin)      // function call → 不提取
obj.method(arg)                  // method call → 不提取
```

### 1.3 已有基础设施（可复用）

- **SameFileIndex**：文件内符号按名称索引（`resolve()` 返回唯一匹配）
- **ImportBindingTable**：import → (target_file, target_name) 映射
- **push_reference()**：两步 fallback（same-file → cross-file）
- **CrossFileSymbolIndex**：跨文件符号索引（`find_files_with_symbol_in_dir()`）
- **symbol extraction**：已支持 Function/Class/Struct/Enum/Interface/TypeAlias 7 种符号

### 1.4 Cangjie AST 结构（function call）

从 tree-sitter-cangjie grammar 分析，Cangjie 函数调用的 AST 结构：

**Simple function call** `func(args)`:
```
postfixExpression
  ├── atomicVariable
  │   └── varBindingPattern → "func"
  └── callSuffix
      └── callArgumentList
```

**Constructor call** `Point(1, 2)`:
```
postfixExpression
  ├── atomicVariable
  │   └── varBindingPattern → "Point"
  └── callSuffix
      └── callArgumentList
```

**Qualified call** `pkg.func(args)`:
```
postfixExpression
  ├── postfixExpression
  │   ├── atomicVariable → "pkg"
  │   └── fieldAccess
  │       └── atomicVariable → "func"
  └── callSuffix
```

**Method call** `obj.method(args)`:
```
postfixExpression
  ├── postfixExpression
  │   ├── atomicVariable → "obj"
  │   └── fieldAccess
  │       └── atomicVariable → "method"
  └── callSuffix
```

### 1.5 Warnings 现状

```
warning: function `package_name_from_target` is never used  (Slice 11, pre-existing)
warning: constant `BUILTIN_TYPES` is never used             (outside feature gate)
warning: function `is_builtin_type` is never used           (outside feature gate)
warning: constant `TYPE_DECLARATION_KINDS` is never used    (outside feature gate)
warning: function `type_name_kind` is never used            (outside feature gate)
warning: struct `FuncContext` is never constructed          (outside feature gate)
warning: struct `SameFileIndex` is never constructed        (outside feature gate)
warning: associated items `build` and `resolve` are never used (outside feature gate)
```

全部是 feature gate 外的 pre-existing warnings，Slice 13 不会消化它们（item/func 结构只在 `#[cfg(feature = "tree-sitter-cangjie")]` 内使用，`cargo check` 无 feature 时报告）。

## 2. MVP Scope

### 2.1 支持哪些 function call forms

| Form | Example | MVP |
|------|---------|-----|
| Simple function call | `distance(origin)` | ✅ YES |
| Constructor call | `Point(1, 2)` | ✅ YES |
| Qualified call (single segment) | `pkg.func()` | ✅ YES — extract last segment `func` |
| Same-file function call | `helper()` in same file | ✅ YES — SameFileIndex |
| Cross-file via explicit import | `import pkg.{add}` → `add(1,2)` | ✅ YES — ImportBindingTable |
| Cross-file via grouped import | `import pkg.{add, sub}` | ✅ YES (inherited from Slice 12) |

### 2.2 不支持哪些 forms

| Form | Example | 原因 |
|------|---------|------|
| Method call | `obj.method()` | 需要 receiver type inference（stop-line） |
| Method chain | `obj.method1().method2()` | 同上 |
| Wildcard import call | `import pkg.*; func()` | wildcard expansion 不支持 |
| Alias renamed import call | `import pkg.{add as plus}` | alias rename 不支持 |
| External version/git dep call | 第三方库函数调用 | 外部依赖符号不可解析 |
| Macro invocation | `macro!()` | macro expansion stop-line |
| Function pointer call | `f()` where f is var | 需要 type inference |
| Closure call | `|x| x + 1` 调用 | closure 识别不支持 |

### 2.3 输出 edge 类型

**方案 A（推荐）**：复用 `ReferenceKind::Uses` + 新 reason string `"cangjie-function-call"`

- 不新增 EdgeKind variant（零 schema change）
- graph emitter 映射保持不变（Uses → EdgeKind::Uses）
- 通过 reason 字符串区分 function call vs type annotation

**方案 B（备选）**：新增 `ReferenceKind::Calls` + graph EdgeKind::Calls

- 更精确语义，区分"使用类型"和"调用函数"
- 但是：EdgeKind 新增属于 schema change，影响所有 consumer
- 且 Cangjie graph 当前没有 CALLS edge（Rust-core project-model 有，但 cangjie 独立 graph）

**推荐方案 A**，原因：
- 最小化 blast radius（只改 references.rs + tests + fixture）
- 不改 graph schema / EdgeKind
- function call 本质是 USES edge（对函数符号的"使用"）
- 后续如需区分，可通过 reason 字段过滤

### 2.4 Confidence/reason policy

| Scenario | Confidence | Reason |
|----------|-----------|--------|
| Same-file exact match | 0.80 | `cangjie-function-call` |
| Cross-file via explicit import | 0.75 | `cangjie-function-call (cross-file via import)` |
| Ambiguous (multiple same-name) | no edge | — |
| Unresolved (no match) | no edge | — |

Confidence 低于 type annotation（0.60 same-file / 0.85 cross-file）的原因：
- Function call 的 callee name matching 比 type annotation 更容易 ambiguity（函数同名比类型同名更常见）
- 不做 parameter type matching（需要 type inference）
- 保守策略：宁可少给边，不给错边

## 3. Required Index Structures

无新增 index。复用现有：

- **SameFileIndex**（已有）：文件内符号按名查找
- **ImportBindingTable**（Slice 12）：import → target file mapping
- **CrossFileSymbolIndex**（Slice 12）：跨文件符号索引

### 3.1 push_reference 修改

`push_reference()` 现有逻辑不变：
1. same-file index 查找 → unique match → emit（confidence 0.80）
2. import binding table 查找 → unique match → emit（confidence 0.75, cross-file reason suffix）
3. ambiguous / no match → no edge

唯一区别：confidence 和 reason 值不同。

## 4. Required Write Set

| File | Change | Risk |
|------|--------|------|
| `crates/cangjie/src/extractors/references.rs` | ~80 行：新增 `postfixExpression` + `callSuffix` 处理分支 | LOW |
| `fixtures/cangjie/reference-function-call-basic/` | 新 fixture：cjpm.toml + src/main.cj（定义 + 调用函数） | LOW |
| `fixtures/cangjie/reference-function-call-cross-file/` | 新 fixture：跨文件 import → function call | LOW |
| `crates/cangjie/tests/function_call_reference.rs` | 新集成测试（feature-gated） | LOW |
| `docs/plans/2026-05-06-cangjie-phase2-slice13-*.md` | preflight + execution-card + closure-review | LOW |
| `docs/plans/README.md` | 更新 plans index | LOW |

**Estimated total lines:** ~350（~80 impl + ~120 tests + ~100 fixture + ~50 docs）

## 5. Forbidden Write Set

- GitNexus-RC runtime — NOT MODIFIED
- Tool checkout — NOT MODIFIED
- Cangjie live repo — NOT MODIFIED
- MCP server / HTTP API / UI — NOT MODIFIED
- Cangjie LSP client — NOT MODIFIED
- diagnostics runner — NOT MODIFIED
- graph.rs EdgeKind enum — NOT MODIFIED（复用 Uses）
- project model schema — NOT MODIFIED
- imports.rs — NOT MODIFIED（已有 API 足够）
- subprocess/cjpm_tree.rs — NOT MODIFIED
- manifest.rs — NOT MODIFIED
- Cargo.toml dependencies — NO NEW DEPENDENCIES

## 6. Acceptance Criteria

1. **Baseline tests still pass**: 95/95 without feature, 108/108 with feature
2. **Simple function call fixture**: `distance(origin)` → USES edge with reason `cangjie-function-call`, confidence 0.80
3. **Constructor call fixture**: `Point(1, 2)` → USES edge with reason `cangjie-function-call`
4. **Cross-file function call**: import → function call → cross-file USES edge with confidence 0.75
5. **No fake edge on unresolved**: calling undefined function → no USES edge
6. **No edge on method call**: `obj.method()` → no edge（method dispatch stop-line）
7. **Builtin type constructor**: `Array<Int64>(10)` → no edge for Array（builtin type 过滤）
8. **Endpoint integrity**: every USES edge source_id/target_id exists in graph
9. **Zero new dependencies**: `cargo check` no new crates
10. **Feature gate maintained**: all new logic behind `#[cfg(feature = "tree-sitter-cangjie")]`

## 7. Implementation Plan Preview

### Step 1: AST walk — postfixExpression + callSuffix handler

在 `walk()` 的 `postfixExpression` 分支中，现有代码跳过 `callSuffix`。新增分支：

```
if kind == "postfixExpression" && has callSuffix:
    extract callee name from:
        - simple: atomicVariable → varBindingPattern → name
        - qualified: last fieldAccess segment → name
    skip if:
        - method call (first child is postfixExpression with fieldAccess without callSuffix)
        - builtin type name
    push_reference(Uses, target_name, [Function, Class, Struct], ...)
```

### Step 2: Callee name extraction helper

新增 `extract_callee_name(node, source) -> Option<String>`:
- Simple call: 取第一个 named child 的 varBindingPattern
- Qualified call: 取最后一个 fieldAccess 的 varBindingPattern
- Method call: 返回 None（跳过）

### Step 3: Method call detection

新增 `is_method_call(node) -> bool`:
- postfixExpression 的第一个 named child 本身也是 postfixExpression
- 且该 child 的最后一个 named child 是 fieldAccess（不是 callSuffix）
- 这种情况下是 method call，跳过

### Step 4: New reason string

push_reference 新增 reason 参数支持，function call 使用 `cangjie-function-call`。

### Step 5: Fixtures + Tests

Fixtures:
1. `reference-function-call-basic/`：same-file function call + constructor call
2. `reference-function-call-cross-file/`：跨文件 import → function call

Tests:
1. Same-file function call produces USES edge
2. Constructor call produces USES edge
3. Cross-file function call via import binding
4. Unresolved function call → no edge
5. Method call → no edge
6. Builtin type constructor → no edge
7. Endpoint integrity

## 8. Stop-lines

- **No method dispatch**: 不根据 receiver type 解析 method call（需要 type inference）
- **No parameter matching**: 不根据 argument types 区分 overload（需要 type inference）
- **No new EdgeKind**: 不新增 graph edge type（复用 Uses）
- **No new dependencies**: 不引入新 crate
- **No external package resolution**: 不解析 git/version dep 中的函数
- **No LSP / diagnostics / schema migration**
- **No GitNexus-RC runtime / Tool / live repo modification**

## 9. Risk Assessment

| Risk | Level | Mitigation |
|------|-------|-----------|
| Function name ambiguity（同名函数多个） | LOW | SameFileIndex/ImportBindingTable 唯一匹配才 emit；ambiguous → no edge |
| Constructor vs function call 区分 | LOW | 两者在 AST 中都是 `postfixExpression(callSuffix)`，统一处理为 function call reference |
| Method call false positive | MEDIUM | 需要正确的 method call detection；MVP 只检测 fieldAccess pattern |
| Builtin type constructor false positive | LOW | 复用已有 `is_builtin_type()` 过滤 |
| Qualified call ambiguity（pkg.func 中 pkg 也是符号） | LOW | 只提取最后一段 `func` 作为 callee，忽略 package qualifier |

## 10. Recommendation

**PROCEED to execution card.**

理由：
- MVP scope bounded（~80 行 references.rs + ~120 行 tests + fixtures）
- 零新依赖、零 schema change、零跨 crate 影响
- 复用 Slice 10-12 所有基础设施
- Function call reference 是 Slice 10 same-file → Slice 12 cross-file 的自然延续
- 不改 graph edge type（只新增 reason string）
- Risk LOW
