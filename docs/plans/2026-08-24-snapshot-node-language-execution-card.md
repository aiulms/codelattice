# 执行卡：快照节点/模块语言身份（多语言卡 1）

日期：2026-08-24 · 来源：preflight v2《多语言项目的语言身份与文件夹体检能力》G2 ·
评审状态：G2 经第三方评审确认无大改，允许单独冻结先行

## 任务

转换器（`crates/cli/src/webui_snapshot.rs`）给 webui.snapshot.v1 产物加语言身份：

- 文件节点 `language`：优先 `properties.language`，回退扩展名映射
- 符号节点 `language`：跟随所属文件（按节点 `file` 字段查扩展名）；`file` 空则省略
- package 节点：不带 `language`
- moduleGraph 模块 `languages: string[]`：模块内节点语言字母序去重；全未知省略字段
- limitations 追加 `.h→c` 已知误伤说明

## 冻结规则（评审确认，契约级）

- 缺席即未知；**禁止 `""`、禁止 `null`**（TS 中 null ≠ 缺席）
- 扩展名表复用 `workspace-model::SOURCE_EXTENSIONS`（导出为 pub，加法改动），
  不在转换器另写一份；表含不可分析语言（csharp/go/java/…），身份标注 ≠ 可分析性
- 不写进 0.3.0 analyze 图节点（那是 MCP 读取面）
- 前端 `SnapshotNode.language?: string` + `ModuleGraphModule.languages?: string[]`，
  仅加法；本卡只做数据生产 + 类型声明，徽标渲染属展示层小改可同卡带（结构树 + 图谱角标）

## Write set

- `crates/workspace-model/src/lib.rs`（导出 SOURCE_EXTENSIONS 或 pub 访问器）
- `crates/cli/src/webui_snapshot.rs`（生产 language / languages）
- `crates/cli/tests/webui_snapshot_format.rs`（集成测试加 language 断言）
- `apps/desktop/src/types.ts`（可选字段声明）
- `apps/desktop/src/panels/structure-tree.tsx`（或实际渲染文件节点的组件）+ 图谱节点角标
- `docs/webui/webui-snapshot-contract.md`（节点表加 language 行 —— 加法）

## Forbidden set

- 不碰 inspect / 扫描器语义（卡 2 的事）
- 不重新生成 fixtures（单语言 fixture 的节点本就全 rust，重新生成属卡 2 后统一处理）
- 不碰 calls.rs、不改 0.3.0 图节点、不提交 git

## TDD

转换器先红后绿：rust fixture 符号节点 language=rust；无 file 的符号省略；
moduleGraph languages 字母序；`.h` 文件节点标 c 且 limitations 含误伤说明。

## 验收

`cargo test`（含新增断言）、`npx vitest run` 不回退、`npx tsc --noEmit`、
`cargo fmt --check`（两个 workspace）、`git diff --check`

## 执行记录（2026-08-24，closure）

全绿：根 workspace `cargo test`、`--lib webui_snapshot` 13/13（新增 4）、
`--test webui_snapshot_format` 4/4（新增 1，含 0.3.0 图节点不带 language 的反向断言）、
src-tauri 18/18、vitest 126/126 不回退、tsc 干净、两个 workspace fmt --check、
`git diff --check`。CLI 实产物（portable-smoke）四项判定全部满足。

偏差（均在冻结规则内）：

1. Write set 多了 `apps/desktop/src/state/structure-tree.ts`：徽标需要数据字段
   （`StructureTreeItem.language/languages`），该类型在数据层模块定义，是
   "structure-tree.tsx 或实际渲染文件节点的组件"的必要延伸，纯加法。
2. 测试fixture 发现的既有行为：无点文件名（如 `Makefile`）被模块归属启发式当作
   目录段（模块 id=Makefile 而非 (root)）。属既有语义，本卡不改，测试按现状断言。
3. 契约文档没有现成的 graph 节点字段表：以新小节 §3.12（纯加法）+ §3.11 表加
   `modules[].languages` 行的方式记录冻结规则。
