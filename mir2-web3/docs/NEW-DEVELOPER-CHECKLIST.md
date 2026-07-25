# New Developer Checklist

这是一页式接手清单。跨平台交付见 `docs/DEVELOPER-DELIVERY.md`，Windows 细节见 `docs/LOCAL-DEVELOPMENT-WINDOWS.md`，macOS 细节见 `docs/LOCAL-DEVELOPMENT-MACOS.md`，素材边界见 `docs/ASSET-CONSUMER-SETUP.md`。

## 项目所有者先完成

- 确认开发者具有书面的代码和素材使用范围，参见 `docs/LEGAL-AND-ASSET-RIGHTS.md`。
- 邀请开发者读取私有的 `Zombieliu/mir2` 及其 Release。`Zombieliu/Crystal` 当前公开可读，但所有者必须先确认公开镜像依据；没有依据时应在 GitHub 中收紧可见性后再交接。
- 不发送个人 GitHub token、R2 Secret、生产账号或现有玩家账号文件。
- 告知开发者本次使用 Starter、GitHub 私有全量包还是未来的 R2 模式。
- 明确私有包是完整视觉图集，不含另外 316 个声音文件；全音频验收需要所有者另行提供合法数据源或受控发布。

## 开发者验证权限

```powershell
gh auth login
gh auth status
gh auth setup-git
gh release view developer-assets-f71b89aa3850 --repo Zombieliu/mir2
git ls-remote https://github.com/Zombieliu/Crystal.git `
  refs/heads/codex/handoff-parity-tools
```

四条命令均成功后再克隆。Release 无权访问时应由仓库所有者调整成员权限，不要互传 token。`git ls-remote` 成功只证明代码可读，不证明具有修改、复制或再分发许可。

## 从零启动 Starter

Windows：

```powershell
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
.\scripts\dev.cmd up -Build -OpenBrowser
```

macOS：

```bash
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2/mir2-web3
./scripts/dev.sh up --build --open
```

打开 `http://127.0.0.1:3002/`，执行 `New Account -> Login -> New Character -> Start Game`。本地账号保存在 `.mir2-data/accounts.json`，不要提交。

## 安装完整素材

安装前建议同一磁盘至少保留 40 GiB 空闲，完整开发环境建议 50 GiB 以上：

```powershell
.\scripts\dev.cmd up -FullAssets -OpenBrowser
```

```bash
./scripts/dev.sh up --full-assets --open
```

首次运行会在隔离授权容器中要求 GitHub 设备授权；凭据仅保存在本机 Docker 命名卷，默认工作区和项目进程不可读取。需要提前授权或更换账号时，仍可单独运行 `dev.cmd auth` / `dev.sh auth`。

默认缓存位于 `.mir2-data/developer-assets/developer-assets-f71b89aa3850`。安装完成后，已校验分片约 9.08 GiB；本地图集约 9.08 GiB。需要释放空间时可在确认安装验证通过后删除该 tag 的缓存目录，未来重装则需要重新下载。

## 提交前门禁

```powershell
.\scripts\verify-developer-setup.ps1 -RunCoreTests
git diff --check
git status --short
```

无网络但依赖和素材已准备好时：

```powershell
.\scripts\verify-developer-setup.ps1 -Offline -RunCoreTests
```

PR 必须列出实际运行的命令、素材模式、玩家可见影响和未验证风险。不要声称 Starter 结果代表全量素材视觉验收。

## 代码入口

| 任务 | 首选路径 |
| --- | --- |
| 登录、HUD、浏览器输入、资源加载 | `apps/web` |
| 运营后台页面、审批和审计界面 | `apps/admin-web` |
| 地图、角色、特效和 WebGPU/WebGL2 渲染 | `apps/game-client/runtime` |
| WebSocket/TCP、认证、会话与 Zone 路由 | `apps/gateway` |
| 运营管理 API、管理查询和审计事件 | `apps/admin-api` |
| 权威移动、战斗、共享世界和持久化 | `apps/simulation` |
| Crystal 协议 | `packages/protocol` |
| 转换后的游戏数据和导入工具 | `packages/game-data`, `packages/tooling` |

多人并行前阅读 `docs/AGENT-ORCHESTRATION.md`。Session 是个人状态，Zone 才是共享世界；不要把多人同步重新实现成每个 Session 内的伪远端玩家。
