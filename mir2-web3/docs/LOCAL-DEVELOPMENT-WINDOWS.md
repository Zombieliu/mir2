# Windows Local Development

本文是 Windows 上运行 Player Web + Rust Gateway 的可复制步骤。默认使用仓库预编译的 Bevy WebGPU/WebGL2 Runtime，普通接手者不需要先安装 `wasm-bindgen-cli`。

## 前置条件

| 工具 | 要求 |
| --- | --- |
| Windows | Windows 10/11 x64 |
| Git | Git for Windows，支持子模块 |
| Node.js | 22 或更高版本 |
| npm | 随 Node.js 安装；本项目使用 `package-lock.json` 和 `npm ci` |
| Rust | Rustup，脚本安装/使用 `1.89.0` |
| MSVC | Visual Studio C++ Build Tools + Windows SDK |
| 浏览器 | Chrome 或 Edge，启用硬件加速；WebGPU 不可用时可回退 WebGL2 |

检查：

```powershell
git --version
node --version
npm --version
rustup --version
cargo --version
```

## 克隆

```powershell
git clone --recurse-submodules https://github.com/Zombieliu/mir2.git
cd mir2\mir2-web3
```

已有 checkout：

```powershell
git pull --ff-only
git submodule sync --recursive
git submodule update --init --recursive
```

Crystal 必须与根仓库记录的提交一致。不要在子模块中随意切回上游 `master`。

## 初始化

只对当前 PowerShell 进程放宽脚本执行策略：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap-developer.ps1
```

脚本执行：

1. 检查 Git、Node.js 22+、npm、Rustup 和 Cargo。
2. 初始化并校验 Crystal 子模块。
3. 安装 Rust `1.89.0`（如缺失）。
4. 在 `apps/web` 执行 `npm ci`。
5. 执行 `cargo +1.89.0 check --locked -p mir2-gateway`。
6. 确认预编译 WebGPU WASM 存在。

仅在明确知道缓存已准备好时使用跳过参数：

```powershell
.\scripts\bootstrap-developer.ps1 -SkipWebInstall -SkipRustCheck
```

## 启动

Starter 模式：

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

默认端口：

| 名称 | 默认值 |
| --- | --- |
| `WebPort` | `3002` |
| `GatewayWebPort` | `7110` |
| `GatewayTcpPort` | `7000` |

自定义端口：

```powershell
.\scripts\start-developer.ps1 `
  -WebPort 3010 `
  -GatewayWebPort 7210 `
  -GatewayTcpPort 7100 `
  -OpenBrowser
```

脚本会：

- 拒绝已被占用的端口。
- 增量构建 `mir2-gateway.exe`，除非存在可用二进制并传入 `-SkipGatewayBuild`。
- 在隐藏窗口中启动 Gateway，并等待 `/health` 最多 60 秒。
- 注入正确的 `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`。
- 设置 `MIR2_USE_PREBUILT_BEVY_RUNTIME=1`。
- 在前台启动 Next dev server。
- `Ctrl+C` 后清理由脚本启动的 Gateway。

浏览器可能先于 Next dev server 完成编译而打开；看到暂时无法访问时，等待 Web 终端显示 ready 后刷新。

## 完整素材模式

本地已经安装 GitHub 私有素材包时，仍使用普通启动命令：

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

使用 R2：

```powershell
$AssetBaseUrl = "https://assets.example.com/mir2/v/<version>"
.\scripts\start-developer.ps1 -AssetBaseUrl $AssetBaseUrl -OpenBrowser
```

素材模式说明和私有包安装命令见 `docs/ASSET-CONSUMER-SETUP.md`。

## 创建测试账号

1. 输入未使用过的账号和密码。
2. 点击 `New Account`。
3. 看到创建成功后点击 `Login`。
4. 点击 `New Character`。
5. 输入不超过 12 个字符的角色名，选择职业和性别。
6. 创建后选中角色，点击 `Start Game`。

账户默认写到：

```text
.mir2-data/accounts.json
```

如需独立测试账户库，可在启动脚本之前为当前 shell 设置：

```powershell
$env:MIR2_ACCOUNT_STORE_PATH = ".mir2-data/accounts-local-dev.json"
```

## 验证

完整验证：

```powershell
.\scripts\verify-developer-setup.ps1
```

快速验证，不执行生产 Web build：

```powershell
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

R2：

```powershell
.\scripts\verify-developer-setup.ps1 `
  -AssetBaseUrl "https://assets.example.com/mir2/v/<version>"
```

## 首次启动性能

- `npm ci` 和第一次 Gateway 编译受网络、CPU 与磁盘影响，可能需要数分钟。
- 首次 Web 启动会生成被 Git 忽略的 `public/generated/map-atlas`。
- 干净账户首次进入世界可能需要 35-60 秒。
- 自动化脚本若只等待 15 秒可能误报；负载脚本可设置：

```powershell
$env:MIR2_WS_LOAD_READY_TIMEOUT_MS = "120000"
```

## 排障

### Gateway 未连接

```powershell
Invoke-RestMethod http://127.0.0.1:7110/health
Get-NetTCPConnection -State Listen -LocalPort 7110
Get-Content .\.mir2-data\developer-logs\gateway.err.log -Tail 80
```

如果手工启动 Gateway 使用了 `7010`，它不会匹配标准 Web 的 `7110`。优先改用 `start-developer.ps1`，或用 `-GatewayWebPort` 统一端口。

### 端口占用

```powershell
Get-NetTCPConnection -State Listen | `
  Where-Object LocalPort -in 3002,7000,7110 | `
  Select-Object LocalAddress,LocalPort,OwningProcess
```

不要直接结束未知进程；换端口或确认所有者后再处理。

### Crystal 子模块不一致

```powershell
git -C .. submodule sync --recursive
git -C .. submodule update --init --recursive
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

### `full/index.json` 返回 404

- Starter 模式：这是未安装全量包的预期回退，可用 `?crystalFullPack=0` 禁用完整图集探测。
- 私有包模式：重新运行 `install-developer-assets.ps1`，检查最终 `index.json`。
- R2 模式：确认 URL 包含正确不可变版本，并运行 `verify-developer-setup.ps1 -AssetBaseUrl ...`。

### 素材缓存或旧版本残留

打开以下 URL 做无 Service Worker 诊断：

```text
http://127.0.0.1:3002/?assetCache=0
```

也可在浏览器控制台调用：

```javascript
await window.__mir2AssetCacheReset({ reload: true });
```

### Bevy Runtime 构建错误

普通开发不要执行源码 WASM 构建，标准启动会设置 `MIR2_USE_PREBUILT_BEVY_RUNTIME=1`。只有修改 `apps/game-client/runtime` 时才安装与锁文件完全匹配的 CLI：

```powershell
cargo +1.89.0 install wasm-bindgen-cli --version 0.2.118 --locked
```

### 注册显示“禁止创建账号”

当前 UI 会把重复账号等非成功结果映射为通用文案。先尝试用该账号登录，或换一个唯一账号。

## 手工启动（仅用于诊断）

标准流程应使用 `start-developer.ps1`。确需拆分终端时：

Gateway 终端：

```powershell
$env:MIR2_GATEWAY_TCP_ADDR = "127.0.0.1:7000"
$env:MIR2_GATEWAY_WEB_ADDR = "127.0.0.1:7110"
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

Web 终端：

```powershell
cd apps\web
$env:MIR2_USE_PREBUILT_BEVY_RUNTIME = "1"
$env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL = "ws://127.0.0.1:7110/ws"
npm run dev -- --hostname 127.0.0.1 --port 3002
```
