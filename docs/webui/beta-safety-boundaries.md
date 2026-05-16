# CodeLattice WebUI — Beta Safety Boundaries

## 核心限制

1. **Static Analysis Only** — 所有结果来自源码静态分析，不执行项目代码。
2. **不提供运行时证明** — runtimeVerified 永远为 false。
3. **不提供外部使用证明** — externalUsageVerified 永远为 false。
4. **不提供测试覆盖证明** — coverageVerified 永远为 false。
5. **不提供安全删除证明** — deletionSafetyVerified 永远为 false。

## Runner 安全

- **只绑定 127.0.0.1** — 不暴露到局域网。
- **只读目标项目** — 不写入目标项目目录。
- **输出到管理目录** — snapshots 保存在 `.codelattice-webui/` 或指定 snapshot-dir。
- **不执行项目代码** — 不运行 build/test/package manager。
- **不调用外部服务** — 不访问网络 API。

## 数据

- **Profiles** 保存在 `.codelattice-webui/profiles.json`（gitignored）
- **Snapshots** 保存在 `.codelattice-webui/snapshots/`（gitignored）
- **Checklist** 保存在浏览器 localStorage
- **无远程上传** — 所有数据本地存储

## 不适用于

- 生产环境 release gate
- 安全审计
- 合规认证
- 替换 CI/CD 测试
- 自动删除代码
