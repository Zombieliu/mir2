# Mir2 Web / Crystal 1:1

这是一个以 Crystal / Legend of Mir 2 为行为和视觉基准的现代化实现：浏览器端由 Next.js 与 Bevy WASM 负责呈现，Rust Gateway 和 Simulation 负责权威游戏状态，`Crystal` 子模块用于源码比对和验收。

> 新开发者请从本页开始。跨平台交付架构见 [`mir2-web3/docs/DEVELOPER-DELIVERY.md`](mir2-web3/docs/DEVELOPER-DELIVERY.md)，macOS 说明见 [`mir2-web3/docs/LOCAL-DEVELOPMENT-MACOS.md`](mir2-web3/docs/LOCAL-DEVELOPMENT-MACOS.md)，Windows 说明见 [`mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md`](mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md)。授权前必须阅读 [`mir2-web3/docs/LEGAL-AND-ASSET-RIGHTS.md`](mir2-web3/docs/LEGAL-AND-ASSET-RIGHTS.md)。

## Windows / macOS 一键启动

新开发者运行 Starter 默认只需安装 Git 和 Docker Desktop。Node `22.18.0`、npm `11.13.0`、Rust `1.89.0`、WASM target 与 Linux 构建依赖全部由仓库的锁定开发镜像提供；安装私有完整素材时还需要宿主机 GitHub CLI。

Windows：

```powershell
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
.\scripts\dev.cmd up -Build -OpenBrowser
```

macOS（Intel 与 Apple Silicon）：

```bash
git clone --filter=blob:none --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2/mir2-web3
./scripts/dev.sh up --build --open
```

默认使用仓库自带 Starter 素材，可直接完成注册、登录、创建角色和进入游戏。获授权开发者也可从 clone 后用一条命令安装固定版本完整视觉素材并启动；首次运行由宿主机官方 `gh` 打开 GitHub 设备授权。启动器核验远端 witness、镜像摘要和 revision，使用临时 Docker 配置拉取镜像，再经标准输入把凭据交给精确的不可变 fetcher，不写入仓库或容器配置：

```text
Windows: .\scripts\dev.cmd up -FullAssets -OpenBrowser
macOS:   ./scripts/dev.sh up --full-assets --open
```

日常协作只记住同一组动词；两端执行的是同一个 Compose 定义和同一套锁定工具链：

| 工作 | Windows | macOS |
| --- | --- | --- |
| 环境诊断 | `.\scripts\dev.cmd doctor` | `./scripts/dev.sh doctor` |
| 启动 | `.\scripts\dev.cmd up -OpenBrowser` | `./scripts/dev.sh up --open` |
| 提交前验证 | `.\scripts\dev.cmd verify` | `./scripts/dev.sh verify` |
| 查看日志 | `.\scripts\dev.cmd logs` | `./scripts/dev.sh logs` |
| 停止 | `.\scripts\dev.cmd down` | `./scripts/dev.sh down` |

版本锁统一来自 `mir2-web3/config/developer-release.json`：Node `22.18.0`、npm `11.13.0`、Rust `1.89.0`、Crystal gitlink、开发镜像与素材哈希都不能由某台机器私自漂移。文本格式由根目录 `.editorconfig` 与 `.gitattributes` 共同约束。

空目录自动验收：

```text
Windows: .\scripts\accept-clean-room.cmd
macOS:   ./scripts/accept-clean-room.sh
```

Crystal 原生 `Client.exe` 只能在 Windows 运行；Mac 开发者使用相同 Web/Gateway 代码与素材，并通过共享 Windows 验收机、截图/录像/trace 做原生对照。

## 统一浏览器操作

Player Web 使用同一个链接自动适配桌面、手机横屏和支持 Web Gamepad API 的
主机浏览器，不需要维护单独的移动版地址。客户端会根据最近使用的输入方式显示
键鼠、触控或手柄教程；触控模式提供摇杆、Walk/Run、Attack、Approach、Pick、
技能/物品快捷键及 Char/Bag 入口说明，手柄模式会按 Xbox、PlayStation 或
通用手柄显示对应按键标签。教程完成后仍可从游戏菜单的 `Help` 重新播放。

## 素材模式

启动前先明确本次使用哪一种素材模式：

| 模式 | 获取方式 | 适用场景 | 完整度 |
| --- | --- | --- | --- |
| Starter | Git 仓库自带 | 首次启动、协议/玩法开发、登录与新手流程 | 可进入游戏，但不是全角色/怪物/装备素材验收 |
| GitHub 私有开发素材包 | 私有 GitHub Release 分卷下载并安装到本地 | 核心开发者、离线开发、完整素材调试 | 完整图集；默认缓存加安装约 18.2 GiB，安装前需至少 40 GiB 空闲 |
| R2 CDN | 启动时传入版本化素材 URL | 未来的远程验收、低端设备和 CDN 测试 | 维护者模板，当前尚未发布可用 URL |

全量素材不会提交进 Git。Starter、私有包和 R2 的详细边界见 [`mir2-web3/docs/ASSET-CONSUMER-SETUP.md`](mir2-web3/docs/ASSET-CONSUMER-SETUP.md)。

## Windows 原生工具链（备用）

以下路径不依赖 Docker，适合需要直接调试 Rust/Node 进程或运行 Crystal 原生客户端的 Windows 开发者。

### 1. 安装前置工具

- Git for Windows
- Node.js 22 或更高版本（包含 npm）
- Rustup / Cargo
- Visual Studio C++ Build Tools 与 Windows SDK
- Chrome 或 Edge

主仓库和素材 Release 是私有的。接手者还需要 GitHub CLI，并由仓库所有者授予代码和私有 Release 的读取权限。`Zombieliu/Crystal` 当前公开可读，但公开可见不等于具有开源许可或再分发权；项目所有者应先按权利说明确认该镜像的保留方式。

先验证身份和 Release 权限：

```powershell
gh auth login
gh auth status
gh auth setup-git
gh release view developer-assets-f71b89aa3850 --repo Zombieliu/mir2
git ls-remote https://github.com/Zombieliu/Crystal.git `
  refs/heads/codex/handoff-parity-tools
```

不要通过聊天或配置文件共享个人 token。技术访问权也不等于素材再分发权，边界见权利说明文档。

### 2. 克隆代码和子模块

```powershell
git clone --filter=blob:none --recurse-submodules --also-filter-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
```

过滤克隆仍会完整检出当前代码和固定 Crystal 提交，只避免预先下载历史中的旧 WASM 与 QA 大文件。旧版 Git 不支持 `--also-filter-submodules` 时，去掉两个过滤参数即可使用普通完整克隆。

如果已经克隆但缺少 `Crystal`：

```powershell
git submodule sync --recursive
git submodule update --init --recursive
```

### 3. 初始化开发环境

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap-developer.ps1
```

脚本会检查 Git、Node.js 22+、npm、Rustup/Cargo、Crystal 子模块提交，安装 Rust `1.89.0`、Player Web 和 Admin Web 的锁定依赖，并检查 Gateway。它默认使用仓库已提交的 WebGPU/WebGL2 Bevy WASM，不要求首次接手者编译 WASM。

### 4. 启动 Starter 模式

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

默认地址：

| 服务 | 地址 |
| --- | --- |
| Player Web | `http://127.0.0.1:3002/` |
| Gateway WebSocket | `ws://127.0.0.1:7110/ws` |
| Gateway health | `http://127.0.0.1:7110/health` |
| Crystal TCP gateway | `127.0.0.1:7000` |

脚本在当前窗口运行 Web，并按需构建、后台启动 Gateway。按 `Ctrl+C` 会停止 Web，以及由该脚本启动的 Gateway。

### 5. 注册并进入游戏

1. 在登录页输入一个新的账号和密码。
2. 点击 `New Account`。
3. 创建成功后点击 `Login`；注册不会自动登录。
4. 点击 `New Character`，填写角色名并选择职业、性别。
5. 选中角色并点击 `Start Game`。

本地默认使用文件账户库，数据保存在 `mir2-web3/.mir2-data/accounts.json`。基础开发不需要 Docker、Postgres 或 Redis。

## 使用完整素材

### GitHub 私有开发素材包

仓库已经跟踪当前完整素材包的校验清单。确认你的 GitHub 账号有私有仓库访问权，然后执行：

```powershell
.\scripts\install-developer-assets.ps1 -Download

.\scripts\start-developer.ps1 -OpenBrowser
```

当前包为 [`developer-assets-f71b89aa3850`](https://github.com/Zombieliu/mir2/releases/tag/developer-assets-f71b89aa3850)，包含 7 个分卷、1,440 个 library shards 和 4,446 张唯一 PNG pages。安装器会校验每个分卷和总归档的 SHA-256，再解压到 `apps/web/public/generated/crystal-packs/full`；中断后重新运行会验证缓存并自动重下损坏分片。该目录被 Git 忽略。

这个私有包是完整的**转换后视觉图集**，不是原生客户端备份，也不包含完整 Crystal 声音库。Git 只携带 4 个 Starter WAV；需要全量音频时，必须由项目所有者另行提供合法授权的数据源或未来经过验收的私有/CDN 音频发布，具体边界见 `mir2-web3/docs/ASSET-CONSUMER-SETUP.md`。

### R2 CDN

当前没有已发布并通过全对象验收的 R2 URL。以下命令只供维护者完成未来发布后使用，不能把示例域名用于验收：

```powershell
$AssetBaseUrl = "https://assets.example.com/mir2/v/<version>"
.\scripts\start-developer.ps1 -AssetBaseUrl $AssetBaseUrl -OpenBrowser
```

不要把 `latest` 或未版本化目录当作正式素材地址。远程完整索引必须返回 HTTP 200：

```powershell
Invoke-WebRequest -Method Head `
  "$AssetBaseUrl/generated/crystal-packs/full/index.json"
```

## 验证开发环境

Starter 或已安装本地私有包：

```powershell
.\scripts\verify-developer-setup.ps1
```

R2 模式：

```powershell
.\scripts\verify-developer-setup.ps1 -AssetBaseUrl $AssetBaseUrl
```

完整验证会检查 Crystal handoff 分支可达性、关键 Starter 素材、Gateway、素材发布安全测试、Player/Admin TypeScript 和两个 Web 应用的生产构建。日常快速检查可临时加 `-SkipBuild`，但提交前应至少完成一次不带该参数的验证。

交接或核心代码提交还应运行完整 Rust 回归：

```powershell
.\scripts\verify-developer-setup.ps1 -RunCoreTests
```

已准备好依赖和素材、但当前无网络时使用 `-Offline`；它只跳过 Crystal 远程分支可达性，不跳过本地提交、素材和构建校验。

## 首次启动预期

- 第一次 Rust Gateway 编译可能需要数分钟。
- 第一次 Web 启动会生成本地地图 Atlas；终端仍有输出时不要提前结束。
- 干净环境首次从 `Start Game` 到可玩画面可能需要约 35-60 秒。
- Starter 模式下 `/generated/crystal-packs/full/index.json` 的 404 表示未安装全量包，客户端会回退；如需纯 Starter 调试，可打开 `http://127.0.0.1:3002/?crystalFullPack=0`。

## 项目目录

| 路径 | 作用 |
| --- | --- |
| `Crystal` | Crystal 参考客户端/服务端子模块与比对工具 |
| `mir2-web3/apps/web` | Player Web、资源缓存、浏览器 QA |
| `mir2-web3/apps/admin-web` | 运营管理 Web 界面 |
| `mir2-web3/apps/game-client/runtime` | Bevy WebGPU/WebGL2 WASM Runtime |
| `mir2-web3/apps/gateway` | Rust TCP/HTTP/WebSocket Gateway |
| `mir2-web3/apps/admin-api` | 运营管理 API、审计与管理查询 |
| `mir2-web3/apps/simulation` | 权威玩法与共享 Zone Simulation |
| `mir2-web3/packages` | 协议、游戏数据和转换工具 |
| `mir2-web3/docs` | 架构、1:1 路线图、QA 证据与交接文档 |

## 参与开发

开始修改前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 和 [`mir2-web3/docs/AGENT-ORCHESTRATION.md`](mir2-web3/docs/AGENT-ORCHESTRATION.md)。不要提交账号库、`.env.local`、R2 凭据、Crystal 原始客户端文件或 `generated/crystal-packs/full`。
