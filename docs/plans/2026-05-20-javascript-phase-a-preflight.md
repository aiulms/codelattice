# JavaScript Phase A Preflight

> **日期：** 2026-05-20
> **类型：** 执行计划
> **状态：** Draft
> **执行者：** AI Assistant

---

## 1. 背景

CodeLattice 已支持 Rust、Cangjie、ArkTS、TypeScript、C、C++、Python、Shell 八种语言。TypeScript adapter 已有 tree-sitter-typescript、TSX parser、module resolution、path alias / monorepo 支持。

JavaScript Phase A 目标是复用 TypeScript 适配器能力，快速为 JavaScript 项目提供静态图谱分析，避免大规模重造。

---

## 2. 复用策略

### 2.1 TypeScript 适配器复用

| TypeScript 能力 | JavaScript 复用策略 |
|-----------------|-------------------|
| tree-sitter-typescript grammar | ✅ 直接复用（JavaScript grammar 与 TypeScript grammar 共享基础语法） |
| TSX/JSX parser | ✅ 直接复用（.jsx 使用 typescript 语言的 jsx 方言） |
| symbol extractor | ✅ 复用（JavaScript 子集，移除类型注解相关逻辑） |
| import extractor | ✅ 复用（添加 CommonJS 支持） |
| module resolution | ✅ 复用（去除 tsconfig.json 特定逻辑） |
| tsconfig parser | ❌ 不复用（JavaScript 项目通常无 tsconfig.json） |

### 2.2 隔离策略

| 模块 | 隔离原因 |
|------|----------|
| JavaScript project detection | 避免 JS/TS 项目混淆 |
| package.json manifest | JavaScript 特有 |
| CommonJS export handling | TypeScript 通常不使用 |
| dynamic import/require diagnostic | JavaScript 特有 limitation |

---

## 3. 技术设计

### 3.1 Parser 选择

```rust
// .js / .mjs / .cjs → tree-sitter-typescript 的 typescript grammar
// .jsx → tree-sitter-typescript 的 tsx grammar
```

tree-sitter-typescript 的 `typescript` grammar 已经支持纯 JavaScript 语法：
- 函数声明、箭头函数、class
- ES6 import/export
- 模板字符串、解构赋值
- 异步函数、生成器

### 3.2 支持的文件类型

| 扩展名 | Parser | 说明 |
|--------|--------|------|
| `.js` | typescript grammar | 标准 JavaScript |
| `.jsx` | tsx grammar | React JSX 文件 |
| `.mjs` | typescript grammar | ES Module 规范 |
| `.cjs` | typescript grammar | CommonJS 规范 |

### 3.3 项目识别规则

```rust
// JavaScript 项目识别
fn detect_javascript_project(root: &Path) -> bool {
    has_package_json(root) &&
    has_js_source_files(root) &&
    !has_tsconfig_json(root)  // 避免把 TS 项目降级
}

// 混合项目策略
// --language javascript: 只分析 .js/.jsx/.mjs/.cjs
// --language typescript: 只分析 .ts/.tsx（保持原行为）
```

### 3.4 符号抽取覆盖

| 符号类型 | AST Node | 示例 |
|----------|----------|------|
| function declaration | `function_declaration` | `function foo() {}` |
| arrow function | `arrow_function` | `const foo = () => {}` |
| function expression | `function_expression` | `const foo = function() {}` |
| class declaration | `class_declaration` | `class Foo {}` |
| method definition | `method_definition` | `class { foo() {} }` |
| object method | `pair` + function | `{ foo() {} }` |
| exported symbol | `export` + above | `export function foo() {}` |
| default export | `export_default` | `export default foo` |
| CommonJS export | `assignment_expression` | `module.exports = ...` |
| CommonJS named export | `assignment_expression` | `exports.foo = ...` |

### 3.5 import/require 覆盖

| 类型 | 示例 | 处理 |
|------|------|------|
| ESM import | `import x from "y"` | 解析 |
| ESM export from | `export { x } from "y"` | 解析 |
| ESM dynamic import | `import("x")` | diagnostic (low-confidence) |
| CommonJS require | `const x = require("y")` | 解析 |
| CommonJS module.exports | `module.exports = ...` | 解析 |
| CommonJS exports | `exports.foo = ...` | 解析 |

### 3.6 Module Resolution

```rust
// 相对路径解析
"./foo" → ["./foo.js", "./foo.jsx", "./foo/index.js", ...]
"../foo" → ["../foo.js", "../foo.mjs", ...]

// package.json 基础识别
main: "dist/index.js" → package entry
module: "dist/module.js" → ESM entry
exports: { ".": "..." } → conditional exports (Phase A 基础支持)
bin: { "cmd": "bin/cmd.js" } → CLI entry
```

---

## 4. 模块结构

```
crates/javascript/
├── src/
│   ├── lib.rs              # 模块入口，导出 analyze_project
│   ├── extractors/
│   │   ├── mod.rs
│   │   ├── symbol.rs      # 符号抽取（复用 TS + JS 适配）
│   │   ├── imports.rs     # import/require 抽取（+ CommonJS）
│   │   └── references.rs  # 引用抽取
│   ├── graph.rs           # graph 输出
│   ├── project.rs         # project detection + manifest
│   └── module_resolution.rs # 相对路径 + package.json 解析
├── tests/
│   ├── symbol_extraction.rs
│   ├── import_resolution.rs
│   └── graph_output.rs
└── Cargo.toml
```

---

## 5. 硬边界（Stop-lines）

- ❌ 不执行 npm/yarn/pnpm/node build/test
- ❌ 不做 runtime type inference
- ❌ 不解析 eval、动态 require、动态 import
- ❌ 不深度索引 node_modules
- ❌ 不破坏 TypeScript / ArkTS 现有行为
- ❌ 不修改 GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool

---

## 6. Limitation 记录

| 限制 | 说明 | 置信度/处理 |
|------|------|--------------|
| dynamic import() | 运行时才能确定路径 | diagnostic (info) |
| dynamic require() | 运行时字符串拼接 | diagnostic (info) |
| 变量类型推断 | 不做 JS type inference | 不推断 |
| this binding | 运行时确定 | 保守策略 |
| prototype chain | 不做原型链解析 | 保守策略 |
| eval() | 不执行 | diagnostic (warning) |
| node_modules | 不索引外部包 | external node |

---

## 7. Fixture 设计

### 7.1 JavaScript Portable Smoke

```
fixtures/javascript/portable-smoke/
├── package.json
├── src/
│   ├── index.js           # ESM import/export, arrow function
│   ├── math.js            # function declaration, class
│   ├── logger.cjs         # CommonJS require/module.exports
│   ├── component.jsx      # React component (JSX)
│   ├── utils.js          # object method, default export
│   └── dynamic.js        # dynamic import (diagnostic)
├── config/
│   └── vite.config.js
└── tests/
    └── math.test.js
```

### 7.2 覆盖矩阵

| 测试点 | 文件 | 预期 |
|--------|------|------|
| ESM import | index.js | 解析 |
| CommonJS require | logger.cjs | 解析 |
| JSX component | component.jsx | 解析（TSX parser） |
| arrow function | index.js | 抽取 |
| class + method | math.js | 抽取 |
| default export | utils.js | 抽取 |
| dynamic import | dynamic.js | diagnostic |
| package.json main | package.json | 识别 |
| relative path resolution | index.js → math.js | 解析 |

---

## 8. 验证计划

### 8.1 必运行测试

```bash
cargo fmt --check
git diff --check
cargo test
cargo test --test mcp_server
cargo test --features tree-sitter-typescript
python3 -m py_compile scripts/webui-runner.py
node --check webui/snapshot-viewer/app.js
node --check webui/snapshot-viewer/runner.js
scripts/codelattice-mcp.sh --self-test
scripts/mcp-dogfood.sh
scripts/webui-snapshot-smoke.sh
scripts/webui-viewer-smoke.sh --skip-browser
scripts/webui-workspace-smoke.sh
scripts/codelattice-precommit-check.sh
```

### 8.2 预期结果

- 所有测试通过
- JavaScript fixture smoke 通过
- TypeScript 现有测试不回归
- ArkTS 现有测试不回归

---

## 9. 风险评估

| 风险 | 级别 | 缓解策略 |
|------|------|----------|
| JS/TS 项目混淆 | medium | 显式 `--language` 优先；auto detection 保守 |
| tree-sitter-typescript grammar 限制 | low | Phase A 只支持标准 JS；记录 limitation |
| CommonJS/ESM 混淆 | medium | 保守置信度；不强制解混淆 |
| 破坏现有 TypeScript | low | 隔离 crate；分别测试 |

---

## 10. Commit Plan

```bash
git add .
git commit -m "feat(javascript): add Phase A JS/JSX graph support"
git push gitcode master
```

---

## 11. 执行卡（Execution Card）

### Write Set
- `crates/javascript/` (新目录)
- `crates/cli/src/` (更新)
- `fixtures/javascript/` (新目录)
- `docs/` (更新)
- `Cargo.toml` (workspace 更新)
- `README.md`, `CHANGELOG.md`

### Forbidden Set
- `GitNexus-RC/` (只读)
- `GitNexus-RC-Tool/` (只读)
- `CodeLattice-Tool/` (只读)
- 真实项目源码 (禁止)
- AI client 配置 (禁止)

### Stop-lines
- TypeScript 测试回归 → 回滚
- ArkTS 测试回归 → 回滚
- cargo build 失败 → 修复
- precommit 失败 → 修复

### Acceptance Criteria
- [ ] `cargo test` 全部通过
- [ ] JavaScript fixture 分析产生 nonzero output
- [ ] `--language javascript` CLI 可用
- [ ] MCP tools 接受 `language=javascript`
- [ ] WebUI 显示 JavaScript 选项
- [ ] precommit 检测 risk ≤ medium

---

## Changelog

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-20 | 0.1 | 初始 preflight |
