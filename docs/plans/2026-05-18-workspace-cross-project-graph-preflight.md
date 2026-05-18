# Workspace Cross-Project Graph — Preflight

日期：2026-05-18
状态：preflight → implementation

## 目标

把 CodeLattice 从"能发现多个子项目"推进到"能理解多个子项目之间的关系"。
新增 workspace-level cross-project graph，包含节点和边，静态识别子项目间关系。

## 硬边界

- 只修改 CodeLattice repo
- 不执行目标项目代码 / build / test / CI / Docker
- 尽量只读取目录结构和 manifest/config 文件
- 读取源码内容必须克制，有上限和注释
- 输出服务 AI 和后续 WebUI，本轮不做炫酷图谱 UI
- 所有输出标注 static-only / heuristic

## Workspace Graph Schema (CodeLatticeWorkspaceGraphV1)

### Node Kinds

| kind | 说明 | confidence |
|------|------|-----------|
| workspace | 工作区根节点 | 1.0 |
| project | 已支持的子项目 | 1.0 |
| package | 包/模块（如果 manifest 中有） | 0.85-1.0 |
| config | 配置文件节点 | 1.0 |
| script | 脚本文件节点 | 1.0 |
| workflow | CI/自动化工作流节点 | 1.0 |
| unsupported | 暂不支持的模块 | 1.0 |

### Edge Kinds

| kind | 说明 | confidence range |
|------|------|-----------------|
| contains | 包含关系（workspace→project, project→config/script） | 1.0 |
| depends_on | 依赖关系（manifest path dep, workspace member） | 0.85-0.95 |
| imports | 跨项目 import/include/source | 0.65-0.85 |
| script_refs | 脚本引用（package.json scripts, Makefile target） | 0.75 |
| config_refs | 配置引用（CI引用本地路径, Dockerfile COPY） | 0.75 |
| adjacent_to | 相邻关系（同父目录下的兄弟模块） | 0.35-0.5 |
| unsupported_boundary | 支持/不支持模块边界 | 0.45-0.6 |

### Confidence/Reason 策略

- **1.0**: contains / direct manifest workspace member listing
- **0.85**: manifest path dependency / workspace package dependency
- **0.75**: config/script 明确本地路径引用
- **0.65**: source/include 明确相对路径引用（读取少量源码，有上限）
- **0.45**: name-only package match（不读源码）
- **0.35**: adjacency-only

### Evidence 结构

每条边携带 evidence：`{file, field, value}`，说明该边来源。

## Static Relationship Extraction 策略

### depends_on

1. **Rust** (Cargo.toml):
   - workspace.members → contains edges
   - [dependencies] 中的 path = "..." → depends_on
   - 轻量 TOML 解析：只读 [workspace] 和 [dependencies] 段

2. **TypeScript/ArkTS** (package.json):
   - workspaces 字段 → contains
   - dependencies/devDependencies 中指向本地路径 → depends_on
   - tsconfig.json paths/baseUrl → imports

3. **Python** (pyproject.toml / requirements.txt):
   - 本地 path 或 editable 引用 → depends_on

4. **C/C++** (CMakeLists.txt / Makefile):
   - add_subdirectory / include 指向本地目录 → depends_on
   - 只做文本级匹配，不做完整 CMake 解析

5. **Cangjie** (cjpm.toml):
   - dependencies 中 path = "..." → depends_on

6. **Shell**:
   - source ./script.sh → imports（仅限显式相对路径）

### script_refs / config_refs

- package.json "scripts" 字段引用本地路径
- Makefile target 引用本地脚本
- .github/workflows/*.yml 引用本地路径
- Dockerfile COPY 引用本地路径
- Shell source 本地脚本

### adjacent_to

- 同一父目录下的 supported project 与 unsupported module
- confidence 0.35-0.5
- reason: sibling-module-boundary

### unsupported_boundary

- supported project 附近存在 unsupported module
- 标记语言和 reason
- 用于提示"此处可能有理解盲区"

## Manifest/Config 读取边界

- **只读**：Cargo.toml, package.json, tsconfig.json, pyproject.toml, setup.cfg,
  requirements.txt, CMakeLists.txt, Makefile, cjpm.toml, .github/workflows/*.yml,
  Dockerfile, *.sh
- **不读源码**：原则上不读 .rs/.ts/.py/.c/.cpp/.cj 源文件
- **例外**：如果 manifest 中没有足够信息，可读少量（≤20个文件×≤50行）源文件头部
  来识别跨项目 import，必须注释说明原因
- **限制**：每个项目最多读 20 个文件，每个文件最多读前 50 行

## Node ID 策略

- deterministic：基于 relative path + kind 的 hash
- workspace node: `ws:<root_hash[:8]>`
- project node: `proj:<relative_path_hash[:8]>`
- package node: `pkg:<relative_path>/<name_hash[:8]>`
- config node: `cfg:<relative_path_hash[:8]>`
- script node: `scr:<relative_path_hash[:8]>`
- workflow node: `wf:<relative_path_hash[:8]>`
- unsupported node: `unsup:<relative_path_hash[:8]>`

## WebUI 最小展示

- Workspace tab 增加 Cross-Project Graph Summary 区块
- 数字卡片：nodeCount / edgeCount / crossProjectEdgeCount / unsupportedBoundaryCount
- Top connected projects 列表
- Bridge scripts/configs 列表
- Unsupported boundaries 列表
- "加载工作区关系图" 按钮
- "复制工作区图谱摘要给 AI" 按钮
- 边列表用 table：source → target / kind / confidence / reason
- 不做新图谱视觉引擎
- 不做大规模 CSS 重构

## Smoke 验证计划

`scripts/webui-workspace-graph-smoke.sh`:
1. 构造 mixed workspace fixture（Rust + TS + Shell + unsupported C# + unsupported Go + CI config）
2. runner 启动
3. inventory 检测
4. analyze recommended
5. GET /api/workspace/graph 返回成功
6. schemaVersion == workspace.graph.v1
7. nodeCount > 0, edgeCount > 0
8. contains edges > 0
9. unsupported_boundary or adjacent_to edges > 0
10. 所有 edge source/target 存在于 node ids
11. generatedFrom.scriptsExecuted == false
12. generatedFrom.runtimeVerified == false
13. insights 包含 crossProjectGraphSummary
14. missing runId 返回 404 structured error
15. 无绝对路径泄露

## 实施计划

1. 写 preflight（本文档）
2. Runner 新增 `_workspace_graph()` 函数族
3. Runner 新增 GET/POST `/api/workspace/graph` 路由
4. 增强 `_workspace_insights()` 加 crossProjectGraphSummary
5. runner.js 新增 `workspaceGraphLoad()` 和 `workspaceGraphCopyAiSummary()`
6. app.js 新增 graph summary 渲染
7. i18n.js 新增 12+ keys
8. report.js 增强 AI summary
9. 写 smoke test
10. 更新 docs
11. 全量验证
12. Commit + push
