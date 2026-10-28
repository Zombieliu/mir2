# Dubhe Wallet 扫码登录

Windows 原生版的入口、客户端配置和验证记录见 [Windows 扫码登录](DUBHE-WALLET-WINDOWS-LOGIN.zh-CN.md)。本页记录共用服务端协议与浏览器端验证。

实现日期：2026-10-28（Asia/Shanghai）。这是新增登录方式，不代表游戏整体或前端最终验收完成。

## 玩家流程

登录页点击 **Wallet → Dubhe Wallet 扫码登录**，用手机扫描二维码，在 Dubhe Wallet
确认登录签名。游戏随后沿用已有身份登录流程，进入角色选择；首次登录按现有流程创建角色。
同设备也可点击二维码下方的确认链接，返回游戏页后继续登录。

二维码有效期为两分钟。界面提供倒计时、取消、拒绝/过期/失败后的刷新；二维码在本地生成，
不发送到第三方图片服务。玩家无需在游戏所在浏览器安装钱包扩展。

## 服务配置

Next Web 与 Gateway 必须继续共享既有的 `MIR2_PASSKEY_AUTH_SECRET`（至少 32 字符），
并配置既有的 `MIR2_PASSKEY_ALLOWED_ORIGINS` 或 `MIR2_ALLOWED_WEB_ORIGINS`。
生产来源必须使用 HTTPS；开发仅允许 localhost/127.0.0.1/::1 的 HTTP 来源。
渠道凭证兑换沿用 `/api/channels/session/exchange` 和既有 Gateway 地址配置。

新增服务端可选配置如下，均不需要暴露给浏览器：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `MIR2_DUBHE_CONNECT_RELAY_URL` | `https://dubhe-connect-relay.zombiesliu.workers.dev` | Dubhe Connect relay origin |
| `MIR2_DUBHE_WALLET_ORIGIN` | `https://wallet.dubhe.obelisk.build` | 钱包确认页面 origin |
| `MIR2_DUBHE_CONNECT_NETWORK` | `testnet` | 仅接受 `testnet` 或 `mainnet` |

URL 配置必须是 origin，不能包含路径、查询或凭据。自建 relay/wallet 可使用 HTTPS；
仅本地开发可用 HTTP。扫码需要手机能访问钱包与 relay，游戏来源也应为可识别的正式地址。

## 协议与认证边界

适配官方 **Dubhe Connect v2**，核对版本：
[`0xobelisk/dubhe-wallet` 的 `c318c458490bd0115f4df35827d6c418d7c1227b`](https://github.com/0xobelisk/dubhe-wallet/tree/c318c458490bd0115f4df35827d6c418d7c1227b)。
参考 `docs/DUBHE_CONNECT_DAPP_INTEGRATION.md`、`packages/shared/src/connect.ts`、
`connectDapp.ts` 和 `connectVerify.ts`。其 `@dubhe-hub/shared` 是私有 workspace 包，
本项目实现了所需的小型 HTTP 协议适配器。

1. `POST /api/wallet/dubhe/session`：检查 Origin，服务端向固定 relay 创建请求，
   校验响应中的来源、网络、URL、声明和有效期，返回二维码 URL 与签名 intent。
2. 浏览器只把 `approvalUrl` 编入二维码；私有 `intentToken` 仅留在本次请求的内存中。
3. `POST /api/wallet/dubhe/session/status`：使用 `Authorization: Bearer <intentToken>`，
   服务端向固定 relay 轮询。校验不可变请求、v2 完整消息、字节、地址、公钥、网络和时间，
   使用 Sui SDK 验证 personal-message 签名。
4. 返回既有 `suiWallet` proof，浏览器再经过渠道兑换取得 canonical 游戏账号，
   最后走现有 `passkeyLogin` 的已认证入口。二维码登录不会直接信任客户端地址。

所有签发使用确定性的 `jti = dubhe-connect:<sessionId>` 和固定的 intent 到期时间。
Gateway 的现有共享缓存原子消费 proof，重复轮询不会延长凭证有效期或允许第二次兑换。
Next 无需进程内待登录状态，适用于多个实例。接口响应禁用缓存，relay 请求限制大小、
超时与重定向；浏览器取消、登录 epoch、socket 或身份变更会使迟到结果失效。

扫码授权只用于游戏登录，不在电脑上保留远程钱包的交易签名连接。
需要链上交易时，仍使用现有的钱包连接/签名流程。

## 验证证据

可复现命令（`apps/web` 目录）：

```text
npm run test:dubhe-wallet
node node_modules/typescript/bin/tsc --noEmit --pretty false --incremental false
node scripts/test-gateway-protocol-handshake.mjs
```

新增鉴权测试使用真实 Sui Ed25519 签名，覆盖请求绑定、签名篡改、账号/公钥/网络不一致、
过期、固定 proof 到期时间、来源限制、空 POST 流及请求/relay 大小限制。
客户端测试覆盖拒绝、过期、取消、所有异步边界的迟到结果、重试、隐藏页签返回，
并执行实际页面登录函数的身份租约与 busy 状态保护。

2026-10-28 本轮结果：

- `npm run test:dubhe-wallet`：**21/21 通过**（9 项服务端、12 项客户端/实际页面函数）。
- 全 Web `tsc --noEmit --pretty false --incremental false`：通过。
- 本地真实 Next 页面 → 官方 relay：创建接口与状态轮询均返回 HTTP 200；二维码和倒计时
  正常显示，Escape 取消后恢复普通登录并把焦点还给 Wallet 按钮。
  [等待确认截图](generated/player-qa/dubhe-wallet-qr-20261028/01-pending.jpg)、
  [取消后截图](generated/player-qa/dubhe-wallet-qr-20261028/02-cancelled.jpg)。
- 这台开发机使用代理上网，Node 22 原生 fetch 默认不读取代理环境变量；本地验证进程
  使用独立 QA preload 连接现有代理，产品依赖与实现未加入机器特定的代理配置。
- 两项相邻旧测试存在独立阻挡：`test-storage-page-gate.mjs` 在租赁模块载入时因
  `unexpected rental dependency: ./npc-purchase-client` 失败；
  `test-gateway-protocol-handshake.mjs` 期待至少 4 处特定源码形式的旧 socket guard，
  实际只有 2 处。已核对 HEAD 的依赖及 guard 数量相同，本轮不修改这些无关逻辑。
- `git diff --check`：通过。

尚未部署。真实用户在手机钱包确认并进入角色选择，仍需使用目标部署及实际钱包账号完成
验收；自动化签名测试与真实二维码创建验证不替代该人工环节。
