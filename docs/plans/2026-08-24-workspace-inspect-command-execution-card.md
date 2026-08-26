# 执行卡 v2：`codelattice inspect` 文件夹体检动词（多语言卡 2）

日期：2026-08-24 · 来源：preflight v2 §3 · 前置：卡 1（快照语言身份）已交付验收
v1 → v2：按第二轮评审修订——嵌套规则改为"只压同语言 L3 重复上报"、冻结整张信封、
v1 明确不做 L2 config 检测、命令名/新文件/复用遍历常量的实现约束、共享点清单勘误。

## 任务

新增顶层 CLI 动词（**禁止**改已 hide 的 `project-model inspect` / `cangjie inspect`）：

```
codelattice inspect --root <PATH> [--format json]
→ schemaVersion: "codelattice.workspaceInspection.v1"
```

回答"这个文件夹里有什么"。不改 analyze / autoEntry / MCP 的任何现有行为与输出。

## 信封（整张冻死，禁止发明新桶名）

```jsonc
{
  "schemaVersion": "codelattice.workspaceInspection.v1",
  "root": "...",
  "generatedAt": "...",
  "generatedFrom": {"staticAnalysis": true, "projectContentRead": false, "scriptsExecuted": false},
  "projects": [          // L1 manifest 背书且可分析
    {"name": "backend", "relativePath": "backend", "language": "rust",
     "confidence": "certain",
     "evidence": {"kind": "manifest", "file": "Cargo.toml"},
     "sourceFileCount": 42, "analyzable": true}
  ],
  "sourceOnlyAreas": [   // L3 扩展名直方图、可分析（≥2 个同语言源文件）
    {"relativePath": "scripts/tools", "language": "python", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".py", "count": 17},
     "sourceFileCount": 17, "analyzable": true},
    {"relativePath": "scripts/tools", "language": "shell", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".sh", "count": 4},
     "sourceFileCount": 4, "analyzable": true}
  ],
  "unsupportedAreas": [  // 不可分析：known-unsupported（认得出）/ unrecognized（不认识）
    {"relativePath": "legacy", "language": "java", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".java", "count": 12},
     "sourceFileCount": 12, "analyzable": false,
     "reason": "language-not-supported", "recognition": "known-unsupported"},
    {"relativePath": "gen", "confidence": "low",
     "evidence": {"kind": "extension-histogram", "extension": ".xyzfoo", "count": 2},
     "sourceFileCount": 2, "analyzable": false,
     "reason": "language-not-supported", "recognition": "unrecognized"}
     // unrecognized：language 字段省略，recognition 必有
  ],
  "cautions": ["Directory/manifest-level static scan only; no source content was read.", "..."],
  "recommendedNextActions": ["..."]
}
```

行级规则：

1. **一行 = `(relativePath, language)`**；同目录多语言多行。
2. **confidence 三档**：L1 manifest → `certain`；L3 直方图 → `medium`；
   unrecognized → `low`。
3. **`reason` 只在 `analyzable: false` 时出现**；`analyzable: true` 的行禁止带 reason。
4. **阈值**：unsupported **≥1** 个文件就报（1 个 .java 也要看见）；
   L3 可分析源码区 **≥2**（对齐现有扫描器）；生产者不按"够不够格展示"过滤，
   每行都带 `sourceFileCount`。
5. **嵌套规则（v2 修正，防"Java 被吃掉"）**：manifest 项目只压**同一套语言的
   L3 重复上报**（backend 是 rust 项目 → 不再报 backend/src 为 rust sourceOnly）；
   **不同语言、尤其 unsupported / unrecognized，即使在 manifest 树里也单独成行**
   （backend/legacy/Foo.java 必须出现）。

## analyzable 判定（cli 层，映射表冻死）

workspace-model 层只产语言与证据；analyzable 由 cli 层按当前二进制 feature 判定：

| language | 判定 |
|---|---|
| rust | `cfg!(feature = "tree-sitter-extraction")`（default，通常 true） |
| shell | true（gitnexus-shell 非 optional） |
| typescript / python / c / cpp / cangjie / arkts | `cfg!(feature = "tree-sitter-<lang>")` |
| 不在支持列表 | `false` + `language-not-supported` |

`.js` 在扫描器里算 typescript，analyzability 跟 TS feature，**不要另开 javascript 行**
（改扩展名表是另一张卡的事，本卡禁止）。feature 关闭时
`reason: "language-support-disabled-in-this-binary"`。

## L2 config 检测：v1 不做（已拍板）

现有 `detect_project_at` 只认 SUPPORTED/UNSUPPORTED_MANIFESTS + 目录内扩展名，
不认 `tsconfig.json`（822 行那个 tsconfig 是 graph 的 import 边逻辑，不是项目检测）。
**v1 不实现 L2**：fixture 不放 config 区；TS 无 package.json 时靠 L3（≥2 个 .ts）。
将来要做 L2 时另冻文件名单，并写明只 `exists()`、不读内容。

## 实现约束（防堆 lib.rs / 防扫描爆炸）

1. `crates/cli/src/lib.rs` 只加 `Commands::Inspect` 变体 + 一行分发；JSON 组装放
   **新文件 `crates/cli/src/workspace_inspect.rs`**（lib.rs 已 5700+ 行）。
2. workspace-model 新增独立入口函数（建议 `inspect_workspace_inventory`），内部复用
   遍历与 manifest 表；**必须复用 `SKIP_DIRS`、`MAX_WALK_DEPTH=5`、`MAX_ENTRIES=5000`**
   （否则对仓库自身 smoke 会把 node_modules/target 扫成 typescript 区）。
3. 直方图按**当前目录的文件**计（与 detect_by_extensions 一致），不递归子目录。
4. 1 个 `.py` 不报（L3 ≥2）；写进测试，避免和 java ≥1 混淆。

## 隔离规则（强制）

`scan_workspace_inventory` 共享点清单（勘误后）：

| 位置 | 实际是什么 |
|---|---|
| lib.rs:710 | CLI autoEntry 扫描 |
| lib.rs:1218 | 前缀匹配（`redact_root: false`，和别处不同，隔离时也别改这个参数） |
| mcp_server.rs:745 | MCP autoEntry 扫描 |
| mcp_server.rs:786 | MCP autoEntry **信封组装**（不是扫描调用点，也别动它的 JSON） |
| mcp_server.rs:23343 | diagnose_root_* 扫描 |
| mcp_job.rs:1193 | workspace job 扫描 |

- 上述函数与 `detect_by_extensions` 的行为语义零改动；共享代码只加内部字段 /
  新函数 / 新 flag。
- autoEntry 序列化测试、MCP 测试必须继续绿。

## Write set

- `crates/workspace-model/src/lib.rs`（新 inspection 入口 + 其单元测试；不改既有函数语义）
- `crates/cli/src/workspace_inspect.rs`（新）
- `crates/cli/src/lib.rs`（仅变体 + 分发）
- `crates/cli/tests/`（新集成测试文件）
- `fixtures/mixed/inspect-smoke/`（新增：manifest 项目 + 多语言源码区 + java 区 +
  unrecognized 区 + manifest 树内嵌 java；**不放 config 区**）
- `docs/plans/`（偏差记录）

## Forbidden set

- 不碰 calls.rs；不改 0.3.0 / autoEntry / MCP 任何输出；不改扩展名表内容
- 不读源码内容做检测（`projectContentRead: false`）
- 不做 L2 config 检测、不做合并快照、不接桌面 UI（卡 3）
- 不改已有 fixtures；不提交 git；不碰前序会话的未提交改动

## TDD 要点（先红后绿）

- 一行一语言：同目录 17 py + 4 sh → 两行，count 各自正确（**文件放同一层目录**）
- 嵌套规则：rust manifest 项目内嵌 1 个 .java → java 行必须在场；rust 子目录不重复上报
- unsupported ≥1：单个 .java 出现；1 个 .py 不出现
- recognition 两档 + unrecognized 省略 language
- analyzable：用 `cfg!(feature = ...)` 自适应断言，禁止写死 true/false
- 安全段在场：generatedFrom.projectContentRead == false、cautions 非空
- 回归：autoEntry 序列化测试、MCP 测试继续绿

## 验收

紧循环：`cargo test -p gitnexus-workspace-model` +
`cargo test -p gitnexus-rust-core-cli --test <新集成测试文件>`
全量放最后：`cargo test`、`cargo fmt --check`、`git diff --check`

手动 smoke：`codelattice inspect --root fixtures/mixed/inspect-smoke` 输出符合信封；
再对仓库自身跑一次确认不崩、node_modules/target 被跳过、unsupported 区可见。

## 执行记录（2026-08-24，closure）

全绿：workspace-model 18/18（新增 5）；`--test workspace_inspect` 7/7（新增）；
根 workspace 全量 64 suite 无失败；MCP server 339/339、productization 22/22
（autoEntry/MCP 回归绿）；fmt --check / git diff --check 干净。
手动 smoke：fixture 信封四区全中；仓库自身 exit=0、139 项目行、
node_modules/target 零出现、unsupported 区可见（unrecognized 行按
"生产者不滤、UI 滤"评审结论全量输出）。

偏差（均在冻结规则推论内，未发明新桶名/字段名）：

1. `lib.rs` 除变体 + 分发外还加了 `pub mod workspace_inspect;` 声明——新文件的
   必要接线；pub 是因为集成测试的 analyzable 断言必须借 lib 的
   `language_analyzable`（与 bin 同 feature 编译）自适应，测试 crate 自身的
   cfg! feature 集恒为空，不可直接写。
2. manifest 行 `sourceFileCount` 口径（卡未冻结算法）：项目树内该语言直方图
   求和（backend=2），对齐信封样例语义；单测+集成测试锁定。
3. unsupported manifest（go.mod 等）→ unsupportedAreas、confidence certain、
   evidence manifest、known-unsupported；fixture 按卡不放该区，单测补锁。
4. 直方图跳过命中 manifest 表的文件名（防 Cargo.toml 被报成 unrecognized
   .toml 行）；无扩展名文件跳过（无证据不发明）。
5. 嵌套压制作用于**两个桶**的直方图行（sourceOnlyAreas 的可分析行 +
   unsupportedAreas 的表内语言行），规则同为"manifest 项目语言相等 + 树内
   前缀"；unrecognized 行 language 缺席天然不压。
