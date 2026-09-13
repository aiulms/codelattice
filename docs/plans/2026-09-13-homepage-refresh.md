# 首页介绍更新 — execution card

## Preflight

- 目标：让初次访问者理解 CodeLattice 的用途、入口和 Beta 边界；同步 GitCode 项目简介。
- 风险：低，仅文档和仓库介绍；不更改符号、运行时、图谱契约或发行产物。
- 当前基线：HEAD `36fe87b8`；工作区已有 6 个源码/fixture 文件修改，全部保留且不纳入本次提交。
- 信息来源：当前 README、0.17.0-beta.2 release notes、Workbench P0 用户指南、安装脚本。

## 冻结范围

- Write set：`README.md`、`docs/guides/cli-reference.md`、`docs/project-description.md`、`docs/release-install.md`、本计划；必要的文档链接修正。
- 线上范围：`aiulms/codelattice` 的项目简介及上述文档提交，push `gitcode master`。
- Forbidden set：源码、fixture、目标项目、其他仓库、客户端配置、发行包、仓库权限设置。
- Stop-line：不把设计图标为真实截图，不把桌面 P0 标为已发行安装器，不宣称静态分析能证明运行时行为或自动重构安全，不声称远程模型模式完全离线。
- 文档策略：精简首页；详细 CLI / MCP / 语言与开发参考保留到专题文档；实际安装路径以脚本为准。

## 验证计划

- 检查 Markdown 相对链接、代码块和安装指令；运行 release metadata check。
- 运行 `cargo fmt --check`、`git diff --check` 及原生 staged detect-changes。完整 precommit 若受已有源码改动或构建成本影响，记录原因，使用原生 detect-changes 文档范围检查。
- 提交仅包含 write set；push 后核对远端提交与首页；线上简介若缺管理登录权限，保存可直接粘贴的文案并如实记录。

## Closure

- README 从 823 行缩至 153 行：首屏定位、3 个场景、流程图、源码与二进制入口、MCP、WebUI / Desktop、语言范围和隐私边界。
- 详细参考迁至 `docs/guides/cli-reference.md`；修正移动后的相对链接、默认磁盘缓存说明和过期的“无桌面壳”描述。
- 安装指南版本引用对齐线上已存在的 beta.2 附件；首页简介已用 Chrome 管理会话保存，并回到首页核对文本生效。GitCode 页面 SEO 标题仍为平台原有标题，基础设置未提供独立短标题字段，未改项目名称。
- PASS：59 个本地链接及标题锚点、Markdown fence 配对、release metadata check、安装脚本 dry-run。
- PASS：现有 release CLI 执行首页 portable fixture 示例，9 symbols / 2 source files / 25 edges；这是示例可执行性检查，未重新构建或安装产品。
- 原生 staged detect-changes：5 个 Markdown 文件、0 changed symbols，summary.riskLevel=low、crossProjectRisk=low。报告 risk reasons 仍夹带旧的 symbol/hunk 叙述（与 changedSymbols=[] 不一致），以本次 staged 文件列表与人工文档 diff 复核为准，不视为运行时证明。
- `cargo fmt --check` 与完整 precommit：在预先存在的 `crates/cli/src/lib.rs` 格式差异处失败，保留未改；完整脚本后续测试未运行。本次纯文档使用原生 staged detect-changes 与定向文档检查完成提交治理。
- 不运行全量编译/测试，不生成发行包；只提交上述 5 个文档。提交哈希、push 及线上 README 渲染核对由任务最终回执记录。
