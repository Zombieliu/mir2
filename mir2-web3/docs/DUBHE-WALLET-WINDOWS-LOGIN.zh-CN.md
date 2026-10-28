# Windows 原生版 Dubhe Wallet 扫码登录

更新日期：2026-10-28（Asia/Shanghai）。本次需求目标为 Windows 游戏 `.exe`。
此前完成的浏览器 Web 登录界面是另一端的实现；本次补上 Windows 原生 Bevy 登录入口，
复用已有服务端鉴权接口。此记录不代表游戏整体、安装包或最终玩家验收通过。

## 玩家操作

在 Windows 登录界面点击 **Dubhe Wallet 扫码登录**，或按 **F8**。
原生 Bevy 弹窗显示本地生成的二维码，使用手机扫码，在 Dubhe Wallet 确认登录签名。
验证通过后沿用现有开门动画进入角色选择，随后按现有流程选择或创建角色、进入游戏。

弹窗显示剩余有效秒数；有效期以服务端返回时间为准。可点击取消或按 Escape 关闭；
拒绝、过期或失败后可刷新二维码，按 Enter 也可刷新。扫码弹窗打开时，键盘及鼠标输入由
弹窗接收，避免同时提交原有账号密码表单。扫码不需要在电脑浏览器安装钱包扩展。

界面代码见 [登录入口](../apps/game-client/client-bevy/src/crystal_ui/login.rs)、
[原生弹窗与快捷键](../apps/game-client/client-bevy/src/native_shell_ui.rs)；
网络请求与轮询由 [Windows 后台线程](../apps/game-client/platform-windows/src/wallet_login.rs)
负责，不阻塞游戏帧。

## 本机人工测试

在仓库根目录打开 PowerShell，先运行[本机测试服务脚本](../scripts/start-native-wallet-test-services.ps1)：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-test-services.ps1
```

看到“本机测试服务已就绪”后保持窗口运行，另开一个 PowerShell，在仓库根目录执行
[客户端启动器](../scripts/start-native-wallet-client.ps1)：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-client.ps1
```

游戏打开后按 **F8**，用手机 Dubhe Wallet 扫描游戏中的实时二维码并确认登录签名；
确认后应进入角色选择，再选择或创建角色并进入游戏。
电脑需要能访问本地服务、Dubhe relay；手机需要能访问 Dubhe Wallet 及 relay，
不需要把电脑本机登录服务暴露到公网。不要扫描本文离屏预览图中的演示二维码。

测试时依次检查：扫码确认后进入角色选择；取消或 Escape 能关闭；取消或刷新后，
旧二维码的确认不应触发游戏登录；手机拒绝后能重试。若尚未进入角色选择，
不能视为真实登录验收通过。当前使用 testnet，测试服务每次启动都创建一套独立测试数据，
不会读取正式服角色。需测试同一钱包再次登录能否找回角色时，保持本轮服务运行，
关闭并重新打开游戏，再用同一钱包登录。

默认网关为 `ws://127.0.0.1:7110/ws`，登录服务为 `http://127.0.0.1:3192`。
服务只监听电脑本机；每轮数据在 `.mir2-data/dubhe-wallet-local-test/<run-id>/`。
鉴权密钥在本轮进程中随机生成，不写入文件。退出测试时先关闭游戏，再在服务窗口按
Ctrl+C。脚本仅停止自身启动的子进程，不负责构建网关或客户端。

只检查环境、不启动游戏：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-client.ps1 -CheckOnly
```

服务地址不同可传 `-GatewayWsUrl` 和 `-AuthHttpUrl`。启动器检查 EXE、网关 `/health`
及登录服务 TCP 端口；端口可连接不代表钱包接口配置或手机签名已经验证。
环境变量仅传入新启动的游戏进程；启动器会移除该进程继承的自动登录、QA 截图和
`MIR2_` 密钥/token 环境变量，不修改当前 PowerShell 的环境。

2026-10-28 16:21（Asia/Shanghai）本机服务实测：网关 `/health` 返回 200，
使用原生客户端 Origin 的 WebSocket 成功连接；登录服务创建真实 Dubhe relay 会话成功，
轮询返回 `pending`，无效凭证兑换返回 401，客户端启动器 `-CheckOnly` 通过。
详见[本机服务检查记录](generated/player-qa/dubhe-wallet-windows-20261028/local-services-readiness.json)。
这些检查尚未包含手机真实签名、Windows 可见窗口操作及进入游戏。

## 客户端与服务部署配置

2026-10-28 远程测试服及独立钱包登录服务已更新；实际连接地址、数据保留核验和
Windows 启动脚本见[测试服更新记录](DUBHE-WALLET-PLAYTEST-ROLLOUT-20261028.zh-CN.md)。
手机钱包实际签名与进入游戏仍需人工验收。

Windows 客户端必须明确配置自己的第一方登录服务 origin，不从网关地址猜测登录网站。
使用环境变量 `MIR2_NATIVE_AUTH_HTTP_URL`，或在 `mir2-client.toml` 中配置：

```toml
[server]
gateway_ws_url = "wss://game.example.com/playtest/ws"
auth_http_url = "https://login.example.com"
```

上述域名仅为示例，交付客户端前需换成实际部署地址。环境变量优先于 TOML。
配置文件先查找 `.exe` 所在目录，再查找启动工作目录。
`auth_http_url` 必须是规范的 origin，不能含路径、查询、片段或凭据，也不写结尾 `/`；
生产使用 HTTPS，仅本机 localhost/127.0.0.1/::1 开发可使用 HTTP。
`gateway_ws_url` 继续配置实际游戏 WebSocket 端点，远程连接使用 WSS。

缺少登录服务配置时，普通原生登录界面仍能打开，扫码入口会显示服务未配置或不可用。
客户端配置只保存公开服务地址，不保存钱包 intent、签名、token、密码或服务端密钥，
也没有默认 demo 账号。配置校验见
[session_config.rs](../apps/game-client/platform-windows/src/session_config.rs)。

实际玩家使用前，必须先部署到 `auth_http_url` 对应的第一方服务，并配置以下现有端点：

1. `POST /api/wallet/dubhe/session`：创建 Dubhe Connect v2 扫码请求。
2. `POST /api/wallet/dubhe/session/status`：验证手机钱包签名并返回受验证的钱包 proof。
3. `POST /api/channels/session/exchange`：兑换为 canonical `obl_` 游戏账号及网关 token。

这三个接口复用此前 Web 实现的服务端能力，Windows 游戏画面由原生 Bevy 渲染。
服务端仍需配置共享的 `MIR2_PASSKEY_AUTH_SECRET`、允许的来源，以及 Dubhe relay/wallet
相关配置；详见 [服务端协议与配置说明](DUBHE-WALLET-QR-LOGIN.zh-CN.md)。
扫码成功所用的身份系统及网关必须属于同一部署环境。

## 鉴权与取消边界

[HTTP 适配器](../apps/game-client/platform-windows/src/wallet_login_http.rs) 限制超时、响应大小
和重定向，并校验 approval URL、有效期、provider 及兑换后的 canonical 身份。
只有服务端验证签名并完成渠道兑换后，客户端才通过
[鉴权桥接](../apps/game-client/platform-windows/src/shell_bridge.rs) 发送
`ClientVersion` 和带 token 的 `passkeyLogin`。网关继续验证签名 token，不接受仅凭账号的登录。
token 不进入界面模型或配置文件，应用层 Debug 输出对 token 脱敏，扫码失败不会自动提交密码登录。

一次扫码绑定当前操作编号及准确的网关连接状态。取消、刷新、断线、登录界面切换或
其他登录开始后，旧结果失效；已提交的 proof 不重放。倒计时同时受本机单调时钟和
服务端到期时间约束；取消、过期、刷新和登录提交后会清理生成的二维码图片资源。
扫码授权仅用于登录，不在电脑保留手机钱包交易签名连接。

## 登录语言与发布分支对齐（2026-10-28）

后续用户要求完整合并分支；本轮继续集成了远端 `e11c32fb1` 的完整历史，
并保留本地玩法与钱包改动。[完整集成记录及新构建验证](BRANCH-INTEGRATION-20261028.zh-CN.md)
更新了下面最初仅恢复语言时的范围边界。

本轮确认了实际的分支差异：当前工作目录 HEAD `bacb56f46` 与已有 Windows R23
`faefb69671272843958f68781b4f8ca2f4014522` 互非祖先。R23 已有原生九语菜单、
字体及用户偏好机制，而当前开发源码缺少这些模块。此前新增的钱包入口、弹窗和运行提示
又直接写成简体中文，因此没有跟随原有 Windows 语言选择。这里描述的是本地 Git 对象的
核对结果，不代表已核对远端最新分支，也不代表整批功能分支已经合并。

已选择性恢复原有 `native_i18n`、九语目录、字体及 `native_locale` 偏好机制，
钱包文案使用同一目录新增的 `wallet.json`，没有新增独立的四语设置。
沿用的语言为繁中、英文、巴葡、俄语、印地语、印尼语、越南语、泰语和阿语。
语言偏好仍存放在 `%APPDATA%/mir2-web3/locale.json`；启动时优先读取该文件，
再读取已有 seed，最后按系统语言回退。本机已有偏好为 `zh-TW`。

登录页右下角的原有语言菜单可选 English 等语言；扫码弹窗打开时也可切换。
登录标题、按钮及扫码入口、说明、阶段、倒计时、错误和操作按钮均使用当前原生语言。
切换只重绘文案和字体，不重新创建扫码请求，不替换二维码图片、不延长有效期，
也不提交或取消登录。手机钱包页面的语言由钱包自身设置控制。
Web 的浏览器偏好与 Windows 的本机偏好分别保存，本次没有修改 Web 支持语言。

本轮验证（各筛选器有覆盖重叠，不累计为用例总数）：

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 共享 UI `wallet` | 17 通过，含九语 Text、语言选择及保留当前 QR 回归 | [最终日志](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-nine-locale-shared-tests-final-02.log) |
| 原生语言目录与选择机制 | 12 通过 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-nine-locale-catalog-tests.log) |
| 原用户偏好机制 | 4 通过 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-native_locale-tests.log) |
| 字体 | 10 通过；2 项额外显式测试未在此筛选器运行 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-native_fonts-tests.log) |
| Windows 钱包、取消及 HTTP 边界 | 27 通过；1 项显式 GPU 测试在下行运行 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-nine-locale-host-tests.log) |
| 九语实际 GPU 离屏渲染 | 1 通过，13 张图片，0 个窗口；英繁切换 QR 范围和像素相同，九语无文字溢出 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-nine-locale-gpu.log)、[记录](generated/player-qa/dubhe-wallet-windows-20261028/nine-locale/wallet-offscreen-fixture.json) |
| 登录桥接相邻回归 | 17 通过；受验证身份登录筛选器 5 通过 | [桥接](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-shell_bridge-tests.log)、[身份](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-verified_identity_login-tests.log) |
| 九语开发 EXE 构建与启动 | 构建成功，已实际打开响应正常的 Windows 窗口并连接原本机网关 | [构建](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-nine-locale-build.log)、[产物记录](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-build-result.json) |

GPU 图片仍使用 `.invalid` 演示二维码，仅验证显示和语言切换，不是手机钱包实签证据。
九语文字能够显示不代表各语言语义、复杂文字排版及整个游戏九语功能均已通过人工验收。

## 本轮验证证据

统一证据目录：
[dubhe-wallet-windows-20261028](generated/player-qa/dubhe-wallet-windows-20261028/)。
以下是不同筛选器的运行结果，存在覆盖重叠，不相加为独立用例总数。

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| Windows `verified_identity_login` | 5 通过 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/native-test-build-02.log) |
| Windows `wallet` | 最终 27 通过；1 项显式 GPU 测试在下行单独执行并通过 | [最终日志](generated/player-qa/dubhe-wallet-windows-20261028/native-wallet-tests-final.log) |
| 共享原生 UI `wallet` | 14 通过 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/shared-wallet-tests.log) |
| 共享 UI `verified_identity_login` | 2 通过 | [日志](generated/player-qa/dubhe-wallet-windows-20261028/shared-verified-login-tests.log) |
| Windows `shell_bridge` 相邻回归 | 修正合法手工登录测试夹具后，17 通过 | [复跑日志](generated/player-qa/dubhe-wallet-windows-20261028/native-adjacent-auth-tests-02.log) |
| Windows `.exe` 构建 | dev profile，optimized + debuginfo，成功 | [首次构建](generated/player-qa/dubhe-wallet-windows-20261028/native-build.log)、[最终构建](generated/player-qa/dubhe-wallet-windows-20261028/native-build-final.log) |
| 无窗口离屏 GPU 界面夹具 | 1 通过；1024×768，0 个窗口，未进行钱包认证 | [运行日志](generated/player-qa/dubhe-wallet-windows-20261028/offscreen-native-ui.log)、[夹具说明](generated/player-qa/dubhe-wallet-windows-20261028/wallet-offscreen-fixture.json) |

实际原生组件的离屏渲染：[扫码入口](generated/player-qa/dubhe-wallet-windows-20261028/01-wallet-entry-offscreen-fixture.png)、
[等待扫码弹层](generated/player-qa/dubhe-wallet-windows-20261028/02-wallet-pending-offscreen-fixture.png)。
图片含明确的演示标识，二维码指向保留的 `.invalid` 域名，不能用于钱包认证。

已构建开发产物为
[`target/halfmoon-client/debug/mir2-platform-windows.exe`](../target/halfmoon-client/debug/mir2-platform-windows.exe)，
首次扫码实现构建大小 **106,968,064 字节**，其 SHA-256 保存在
[首次构建结果](generated/player-qa/dubhe-wallet-windows-20261028/native-build-result.json)。
本次九语更新后的开发 EXE 为 **129,748,480 字节**，已覆盖上述开发路径并重新启动；
最新 SHA-256、构建时间及工作目录修订见
[九语构建结果](generated/player-qa/dubhe-wallet-windows-20261028/native-nine-locale-build-result.json)。
没有生成 release、签名 Candidate 或完整安装包。

从 `mir2-web3` 项目目录复现主要检查：

```text
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client verified_identity_login
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client wallet
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client shell_bridge
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client -p mir2-client-bevy --lib wallet
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client -p mir2-client-bevy --lib verified_identity_login
cargo +1.95.0 test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client native_wallet_visual_tests::native_wallet_login_offscreen_fixture -- --ignored --exact
cargo +1.95.0 build --locked --manifest-path apps/game-client/platform-windows/Cargo.toml --target-dir target/halfmoon-client --bin mir2-platform-windows
```

## 尚待完成的验收

早期两次通过 `Start-Process` 启动真实 Windows 游戏窗口被自动审批策略以
`blocked by policy` 拒绝。随后在用户明确要求启动后，已通过本机启动器实际打开 Windows
游戏，进程有可见窗口句柄且响应正常，并已连接本机网关；尚无真实 Windows 窗口操作截图。
离屏 GPU 夹具只验证渲染，不能替代手机钱包确认。尚未完成手机钱包实签进入角色选择，
也未部署生产。

后续需在已部署并配置好的登录服务与网关上，使用实际 Windows 客户端及手机钱包完成
扫码、签名、角色选择和进入游戏的人工验收。QA 自动打开扫码界面仅在
`MIR2_NATIVE_WALLET_QA_START=1` 且设置 `MIR2_NATIVE_CAPTURE_DIR` 时启用；
普通玩家启动默认不执行该自动操作。
