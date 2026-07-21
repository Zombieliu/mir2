# Mir2 Web / Crystal 1:1

这是一个以 Crystal / Legend of Mir 2 为行为和视觉基准的现代化实现：浏览器端由 Next.js 与 Bevy WASM 负责呈现，Rust Gateway 和 Simulation 负责权威游戏状态，`Crystal` 子模块用于源码比对和验收。

> 新开发者请从本页开始。Windows 的完整说明见 [`mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md`](mir2-web3/docs/LOCAL-DEVELOPMENT-WINDOWS.md)，项目交接说明见 [`mir2-web3/docs/DEVELOPER-HANDOFF.md`](mir2-web3/docs/DEVELOPER-HANDOFF.md)。

## 素材模式

启动前先明确本次使用哪一种素材模式：

| 模式 | 获取方式 | 适用场景 | 完整度 |
| --- | --- | --- | --- |
| Starter | Git 仓库自带 | 首次启动、协议/玩法开发、登录与新手流程 | 可进入游戏，但不是全角色/怪物/装备素材验收 |
| GitHub 私有开发素材包 | 私有 GitHub Release 分卷下载并安装到本地 | 核心开发者、离线开发、完整素材调试 | 完整图集，本机占用约 10GB |
| R2 CDN | 启动时传入版本化素材 URL | 验收、远程协作、无需本地保存全包的开发者 | 按需加载完整图集，依赖网络与已发布版本 |

全量素材不会提交进 Git。Starter、私有包和 R2 的详细边界见 [`mir2-web3/docs/ASSET-CONSUMER-SETUP.md`](mir2-web3/docs/ASSET-CONSUMER-SETUP.md)。

## Windows 快速开始

### 1. 安装前置工具

- Git for Windows
- Node.js 22 或更高版本（包含 npm）
- Rustup / Cargo
- Visual Studio C++ Build Tools 与 Windows SDK
- Chrome 或 Edge

使用私有 GitHub 素材包时还需要 GitHub CLI，并完成 `gh auth login`。

### 2. 克隆代码和子模块

```powershell
git clone --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
```

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

脚本会检查 Git、Node.js 22+、npm、Rustup/Cargo、Crystal 子模块提交，安装 Rust `1.89.0` 和 Web 依赖，并检查 Gateway。它默认使用仓库已提交的 WebGPU/WebGL2 Bevy WASM，不要求首次接手者编译 WASM。

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
gh auth login
.\scripts\install-developer-assets.ps1 -Download

.\scripts\start-developer.ps1 -OpenBrowser
```

安装器会校验每个分卷和总归档的 SHA-256，再解压到 `apps/web/public/generated/crystal-packs/full`。该目录被 Git 忽略。

### R2 CDN

从维护者处取得已经验证、以版本号结尾的素材根 URL：

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

完整验证会检查 Crystal handoff 分支可达性、关键 Starter 素材、Gateway、素材发布安全测试、TypeScript 和 Web 生产构建。日常快速检查可临时加 `-SkipBuild`，但提交前应至少完成一次不带该参数的验证。

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
| `mir2-web3/apps/game-client/runtime` | Bevy WebGPU/WebGL2 WASM Runtime |
| `mir2-web3/apps/gateway` | Rust TCP/HTTP/WebSocket Gateway |
| `mir2-web3/apps/simulation` | 权威玩法与共享 Zone Simulation |
| `mir2-web3/packages` | 协议、游戏数据和转换工具 |
| `mir2-web3/docs` | 架构、1:1 路线图、QA 证据与交接文档 |

## 参与开发

开始修改前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 和 [`mir2-web3/docs/AGENT-ORCHESTRATION.md`](mir2-web3/docs/AGENT-ORCHESTRATION.md)。不要提交账号库、`.env.local`、R2 凭据、Crystal 原始客户端文件或 `generated/crystal-packs/full`。
