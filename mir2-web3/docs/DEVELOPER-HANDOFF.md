# Developer Handoff

## 2026-07-23 最终 Candidate 基线

当前维护分支已经达到自动化 `100% Candidate`。最后一轮补齐了三项容易被
截图误判的问题：Windows GDI 固定文本、真实四行聊天状态、逐对象 NPC/怪物
动画相位。r40 固定场景是 Bichon `0 @ 328,275`、light 1、
`bevy-e9d354eada933661`，运行时/布局/实体/像素趋势门禁均为 100%，无关键
控制台错误或非 favicon 404。WebGPU、WebGL2、四步移动时序和全量 1,440 个
素材库哈希校验均通过。

这里的 `Candidate` 不是伪造的“像素完全相等”：两个独立客户端的游走角色、
随机待机帧、粒子相位和 GDI/浏览器合成器不会天然锁步。当前原始指标仍为
world 89%、HUD UI 91%、chat 84%、MiniMap 87%。代码层已无该固定场景的已知
P0/P1 缺口，接手者下一步只需进行最终人工 `Accepted` 观感/手感签字；若发现
可重复缺陷，应先固定同地图、坐标、光照、账号状态和动作时间线，再作为新
回归处理，不要用一次随机帧差异重开移动管线。

本文面向接手 `mir2-web3` 的开发者和主协调者，描述当前可运行边界、代码入口、素材交付方式及交接验收。首次安装请先执行 `scripts/bootstrap-developer.ps1`，不要从历史 QA 命令反推本地端口和环境变量。

一页式执行清单见 `docs/NEW-DEVELOPER-CHECKLIST.md`。邀请外部开发者或交付素材前必须阅读 `docs/LEGAL-AND-ASSET-RIGHTS.md`；GitHub 访问权限和素材再分发授权是两件不同的事。

## 当前产品边界

`mir2-web3` 是 Crystal / Legend of Mir 2 的现代 Web 实现：

- Player Web 使用 Next.js 承载登录、选角、HUD 和浏览器输入层。
- Admin Web 与 Admin API 提供运营界面、审计、审批和管理查询，不参与玩家实时渲染。
- Bevy WASM 提供 WebGPU 与 WebGL2 地图、角色和特效呈现。
- Rust Gateway 提供 Crystal TCP、HTTP health、浏览器 WebSocket 和会话路由。
- Rust Simulation 与共享 Zone Runtime 负责权威玩法、移动、AOI、战斗和持久化。
- `../Crystal` 子模块是行为、数据格式和视觉比对基准，不是 Web 的生产运行时。

自动化 Candidate 不等于最终视觉验收。当前路线图、进度和证据以以下文件为准：

- `docs/AGENT-TASK-QUEUE.md`
- `docs/CRYSTAL-1TO1-ROADMAP.md`
- `docs/BACKEND-1TO1-PROGRESS.md`
- `docs/CRYSTAL-SERVER-PARITY.md`

## 仓库结构

| 路径 | 所有权 |
| --- | --- |
| `apps/web` | Player Web、素材 URL、Service Worker、前端 QA |
| `apps/admin-web` | 运营管理 Web 界面 |
| `apps/game-client/runtime` | Bevy WebGPU/WebGL2 WASM Runtime |
| `apps/gateway` | 网络入口、认证、WebSocket/TCP 和 Zone 路由 |
| `apps/admin-api` | 运营管理 API、审计事件和管理查询 |
| `apps/simulation` | 权威会话、共享 Zone、战斗和持久化 |
| `packages/protocol` | Crystal 兼容协议与编解码 |
| `packages/game-data` | 已转换并允许进入仓库的游戏数据 |
| `packages/tooling` | Crystal 数据导入和生成工具 |
| `scripts` | 新开发者初始化、启动、验证和素材包脚本 |
| `docs/generated` | 自动化证据；不要把它当手写源文件 |
| `../Crystal` | 配置在 `codex/handoff-parity-tools` 的参考子模块 |

## 标准脚本

| 脚本 | 作用 | 关键参数 |
| --- | --- | --- |
| `scripts/bootstrap-developer.ps1` | 检查工具、初始化子模块、安装 Rust 1.89 与 Player/Admin Web 锁定依赖、检查 Gateway | `-SkipRustCheck`, `-SkipWebInstall` |
| `scripts/start-developer.ps1` | 对齐端口、构建/启动 Gateway、使用预编译 Bevy Runtime 并启动 Web | `-WebPort`, `-GatewayWebPort`, `-GatewayTcpPort`, `-AssetBaseUrl`, `-OpenBrowser`, `-SkipGatewayBuild`, `-ReuseGateway` |
| `scripts/verify-developer-setup.ps1` | 校验子模块、Starter/本地 full-pack 闭包、Rust、素材安全测试、Player/Admin TypeScript 和生产 build | `-AssetBaseUrl`, `-SkipBuild`, `-Offline`, `-RunCoreTests` |
| `scripts/install-developer-assets.ps1` | 下载或读取私有 Release 分卷，校验 SHA-256 并安装本地全量图集 | `-ManifestPath`, `-PartsDirectory`, `-CacheDirectory`, `-Download`, `-Force`, `-KeepArchive` |
| `scripts/package-developer-assets.ps1` | 验证并确定性打包本地全量图集，生成 GitHub Release 分卷和 manifest | `-OutputDirectory`, `-PartSizeBytes`, `-KeepArchive` |

这些脚本是新开发者命令的 source of truth。调整默认端口、依赖、素材目录或验证门禁时，必须同时更新脚本和本组文档。

## 首次接手流程

私有主仓库接手者先确认邀请已接受，并完成：

```powershell
gh auth login
gh auth status
gh auth setup-git
gh release view developer-assets-f71b89aa3850 --repo Zombieliu/mir2
git ls-remote https://github.com/Zombieliu/Crystal.git `
  refs/heads/codex/handoff-parity-tools
```

截至 2026-07-22，GitHub 元数据显示 `Zombieliu/mir2` 为私有、`Zombieliu/Crystal` 为公开。后者的公开可读性不构成许可证；项目所有者必须按 `docs/LEGAL-AND-ASSET-RIGHTS.md` 决定保留公开镜像还是收紧可见性，接手开发者不能自行推定再分发权。

随后克隆和初始化：

```powershell
git clone --filter=blob:none --recurse-submodules --also-filter-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap-developer.ps1
.\scripts\start-developer.ps1 -OpenBrowser
```

过滤克隆仍会完整检出当前工作树和固定 Crystal 提交，只避免预取历史中的旧 WASM/QA blobs。Git 版本过旧、不支持 `--also-filter-submodules` 时再退回普通 `--recurse-submodules` 克隆。

默认端口：

| Surface | 地址 |
| --- | --- |
| Player Web | `http://127.0.0.1:3002/` |
| Gateway HTTP/WS | `127.0.0.1:7110` |
| Gateway WebSocket | `ws://127.0.0.1:7110/ws` |
| Gateway health | `http://127.0.0.1:7110/health` |
| Gateway Crystal TCP | `127.0.0.1:7000` |

`start-developer.ps1` 显式设置 WebSocket URL，因此不依赖开发者个人的、被 Git 忽略的 `.env.local`。如果目标端口已占用，脚本会在启动前失败；只有确认已有进程就是兼容的本项目 Gateway 时才显式传 `-ReuseGateway`。

## 素材交付模型

### Starter

仓库跟踪登录、选角、HUD、关键 NPC/Monster/角色帧、原始地图图块、Starter Entity Atlas 以及预编译 WebGPU/WebGL2 Runtime。它支持首次启动和新手流程，但不能代表全职业、全怪物、全装备和全地图的完整视觉验收。

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

### GitHub 私有开发素材包

完整图集保存在私有 GitHub Release，仓库跟踪的 `config/developer-assets.json` 固定 repository、releaseTag、分卷大小和 SHA-256。拥有仓库权限的开发者直接下载并安装：

```powershell
.\scripts\install-developer-assets.ps1 -Download
```

当前固定版本为 [`developer-assets-f71b89aa3850`](https://github.com/Zombieliu/mir2/releases/tag/developer-assets-f71b89aa3850)。目标目录固定为 `apps/web/public/generated/crystal-packs/full`，该目录被 Git 忽略。默认分片缓存和已安装图集各约 9.08 GiB；安装期间还会创建总 tar 与 staging，所以开始前至少准备 40 GiB 空闲，完整开发环境建议 50 GiB 以上。安装后正常运行 `start-developer.ps1`，无需传 `-AssetBaseUrl`。

这个 Release 是完整转换视觉图集，不是原生客户端或全声音库。Git 只提供 4 个 Starter WAV；另外 316 个发布态声音文件必须由所有者按授权边界单独交付本地源，或等待受控音频发布。没有这些音频不会阻止 Gateway、Player Web 和图集开发启动，但不能签署全音频验收。

### R2 CDN

当前 R2 只保留发布工具和维护者模板，尚无可供开发者使用的已验收 URL。未来发布必须使用不可变版本目录，由 Service Worker 按需回源，并提供已通过发布清单校验以及全部 5,887 个 full-pack 对象并发 HEAD 探测的 URL：

```powershell
$AssetBaseUrl = "https://assets.example.com/mir2/v/<version>"
.\scripts\verify-developer-setup.ps1 -AssetBaseUrl $AssetBaseUrl -SkipBuild
.\scripts\start-developer.ps1 -AssetBaseUrl $AssetBaseUrl -OpenBrowser
```

完整素材消费和发布步骤见 `docs/ASSET-CONSUMER-SETUP.md`。

## 原生 Crystal 对照客户端

普通 Web、Gateway 和 Simulation 开发不需要原生 `Client.exe`。需要做 Crystal-vs-Web 视觉、时序或手感验收的开发者，必须由项目所有者在 Git 和私有素材 Release 之外另行提供合法取得的原生运行环境与数据。当前 Windows 工作区惯例路径是：

```text
<repo>\Crystal\Build\Client\Debug\Client.exe
```

该路径只是本地约定，不是 `bootstrap-developer.ps1` 的必需输入，也不保证源代码克隆后自动存在。不要提交原生可执行文件、完整 `Build/Client/Debug/Data`、账号数据库或原始 `.Lib`；对照证据应记录客户端提交、分辨率、地图、坐标和测试账号状态，而不是复制整个客户端目录。

## 账号与数据

本地默认使用文件账户库：

```text
mir2-web3/.mir2-data/accounts.json
```

新账号流程为 `New Account -> Login -> New Character -> Start Game`。注册成功不会自动登录。重复账号目前可能显示通用“禁止创建账号”文案，应先尝试直接登录或使用新账号。

本地基础运行使用文件账户库和进程内缓存，不需要 Docker。只有显式启用 Postgres、Redis、Admin 或生产/预发布策略时才需要 `infra/docker-compose.dev.yml`。

## 首次运行时序

- `bootstrap-developer.ps1` 的 Rust check 以及 Player/Admin npm install 取决于网络与本机缓存。
- `start-developer.ps1` 首次会增量构建 Gateway，最长等待 health 60 秒。
- Web `npm run dev` 会确保地图 Atlas 存在，并验证预编译 Bevy Runtime。
- Player/Admin 的 `npm run typecheck` 会先执行 `next typegen`；Next 管理的
  `next-env.d.ts` 已被 Git 忽略，不要手工编辑或提交。
- 干净环境首次 `Start Game` 到可玩画面通常需要 35-60 秒；不要使用 15 秒测试超时判断启动失败。
- Gateway 日志写入 `.mir2-data/developer-logs/gateway.out.log` 和 `gateway.err.log`。

## 验收门禁

日常快速检查：

```powershell
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

交接或提交前：

```powershell
.\scripts\verify-developer-setup.ps1 -RunCoreTests
git diff --check
git status --short
```

已经完成依赖和素材安装但暂时无网络时，可用 `-Offline -RunCoreTests`。离线模式只跳过远程 handoff 分支可达性，仍校验本地子模块提交、Rust、素材、Player/Admin TypeScript 和 Web build。

R2 交接还必须提供：

- 不可变素材根 URL。
- `/generated/crystal-packs/full/index.json` 的 HTTP 200 结果。
- full pack `contentHash`。
- R2 上传报告和素材发布清单。

## 继续开发时的规则

- Session 是个人登录/角色状态，Zone 才是共享世界；不要把远端玩家重新塞回每个个人 Session。
- 不要向普通客户端暴露 `MoveTo`、Stage5/debug teleport、裸 passkey account id 或 QA/admin 命令。
- 不要回滚其他开发者或 Agent 的并发改动。
- `apps/simulation/src/runtime.rs` 等高冲突文件同一轮只允许一个写入者。
- 后端改动在测试通过后更新 roadmap/progress/parity 文档；前端改动更新 Player QA/视觉差距证据。
- 发生上下文丢失或重启时，先读 `docs/AGENT-RESUME-HANDOFF.md`。

## 交接完成清单

- 根仓库目标提交已推送。
- Crystal 子模块提交可从配置的 handoff 分支获取。
- `bootstrap-developer.ps1` 在干净 Windows 环境通过。
- Starter 能注册、创建角色并进入游戏。
- 私有素材 Release 的 manifest 和所有分卷可下载并通过 SHA-256。
- 如果本次交付包含 R2，则使用不可变版本 URL，且发布清单中的全部 5,887 个 full-pack 对象返回成功状态；当前 R2 未发布，不能把示例 URL 当作完成项。
- `verify-developer-setup.ps1` 完整通过。
- README、已知限制、测试结果和剩余任务与代码一致。
- 外部开发者的代码与素材使用范围已经由项目所有者书面确认。
