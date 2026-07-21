# 参与 mir2 开发

本项目同时维护 Crystal 参考实现、Rust 权威服务端、Player Web 和多套素材管线。提交应保持边界清晰、可复现，并且不能把受限素材或密钥带入 Git。

## 开始之前

1. 阅读 [`README.md`](README.md)。
2. 阅读 [`mir2-web3/docs/DEVELOPER-HANDOFF.md`](mir2-web3/docs/DEVELOPER-HANDOFF.md)。
3. Windows 开发使用 [`mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md`](mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md)。
4. 涉及素材时阅读 [`mir2-web3/docs/ASSET-CONSUMER-SETUP.md`](mir2-web3/docs/ASSET-CONSUMER-SETUP.md)。
5. 外部协作和素材授权阅读 [`mir2-web3/docs/LEGAL-AND-ASSET-RIGHTS.md`](mir2-web3/docs/LEGAL-AND-ASSET-RIGHTS.md)。
6. Agent 或多人并行开发必须遵守 [`mir2-web3/docs/AGENT-ORCHESTRATION.md`](mir2-web3/docs/AGENT-ORCHESTRATION.md)。

## 初始化

从仓库根目录进入项目后运行：

```powershell
cd mir2-web3
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap-developer.ps1
.\scripts\start-developer.ps1 -OpenBrowser
```

不要绕过 `bootstrap-developer.ps1` 自行猜测 Rust、Node 或子模块版本。需要跳过耗时步骤时可查看脚本参数：

```powershell
Get-Help .\scripts\bootstrap-developer.ps1 -Detailed
```

## 开发素材策略

| 任务 | 推荐模式 |
| --- | --- |
| 协议、Gateway、Simulation、普通 UI | Starter |
| 运营后台界面与管理 API | Starter |
| 角色、怪物、装备、特效和全地图视觉开发 | GitHub 私有开发素材包 |
| 远程验收、缓存、低端设备和 CDN 行为 | R2 CDN |
| 重建原始 Crystal 图集 | 本地合法取得的 Crystal Client 数据源 |

规则：

- 不要提交 `apps/web/public/generated/crystal-packs/full/`。
- 不要提交 `Crystal/Build/Client/Debug`、原始 `.Lib`、完整声音库或客户端可执行文件。
- 不要提交 `.env.local`、GitHub token、R2 Access Key、Secret 或 Worker upload secret。
- 私有开发素材包由 `scripts/package-developer-assets.ps1` 生成，由 `scripts/install-developer-assets.ps1` 安装。
- R2 发布清单只接受通过 `assets:full-pack:verify` 的图集。

## 分支与提交

- 新分支建议使用 `codex/<topic>`、`feat/<topic>` 或 `fix/<topic>`。
- 一个提交只解决一个清晰问题。
- 不要修改或回滚不属于本任务的并发改动。
- Crystal 子模块改动必须先推送到配置的 handoff 分支，再更新根仓库的子模块指针。
- 不要把生成证据、临时日志、浏览器配置和本地账户数据混入功能提交。

## 开发验证

快速验证：

```powershell
cd mir2-web3
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

提交前完整验证：

```powershell
.\scripts\verify-developer-setup.ps1 -RunCoreTests
```

依赖已就绪但无网络时，可用 `-Offline -RunCoreTests`；它不会跳过本地构建和测试。

R2 素材变更还应执行：

```powershell
.\scripts\verify-developer-setup.ps1 -AssetBaseUrl "https://assets.example.com/mir2/v/<version>"
```

文档提交至少应检查：

```powershell
git diff --check
git status --short
```

如果完整验证因为机器资源或外部服务无法运行，请在 PR 中明确写出未运行的命令、原因和剩余风险，不要写成“全部通过”。

## Pull Request 清单

- 说明修改解决了什么玩家或开发者问题。
- 列出实际执行的测试和结果。
- 标注是否需要 Starter、私有素材包或 R2 才能复现。
- 涉及视觉变化时附上相同场景、相同分辨率的前后证据。
- 涉及 Gateway/Simulation 时说明协议、持久化和共享 Zone 影响。
- 涉及素材发布时给出不可变版本 URL、内容哈希和远程 `full/index.json` 验证结果。
- 确认没有密钥、账号数据或受限原始素材进入提交。

## 高冲突区域

`apps/simulation/src/runtime.rs` 及大型前端场景文件同一轮只允许一个代码写入者。多人并行时，探索者保持只读，由主协调者分配最终写入范围；具体规则以 `docs/AGENT-ORCHESTRATION.md` 为准。
