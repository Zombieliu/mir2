# Mir2 / Dubhe 游戏发行渠道接入与统一账号

## 结论

角色不归属于钱包地址，也不归属于 itch 或 CrazyGames。角色归属于服务端生成的稳定
`Obelisk Player ID`（格式为 `obl_<128-bit random>`）。

- **Sui Passkey**：推荐的主归属凭证。普通玩家不需要安装钱包，设备可用 Face ID、
  Touch ID、Windows Hello 等完成签名。
- **Dubhe Wallet / 其他 Sui Wallet**：可选绑定，用于链上资产、交易和高级用户恢复；
  不应成为普通玩家进入游戏的强制门槛。
- **CrazyGames、itch、Steam 等渠道账号**：渠道登录凭证。它们可以绑定同一个
  Obelisk Player ID，但不能直接成为角色表主键。
- **角色与进度**：只引用 Obelisk Player ID，因此新增发行渠道不需要迁移角色数据。

```mermaid
flowchart LR
  A["官网 / itch / CrazyGames / 后续 Steam"] --> B["Channel Bridge"]
  B --> C["Gateway 凭证验真"]
  C --> D["Identity Registry (PostgreSQL)"]
  D --> E["Obelisk Player ID"]
  E --> F["账号 / 角色 / 背包 / 公会"]
  E --> G["Gateway Session"]
  G --> H["Zone / 家庭节点 / Commonware Finality"]
  P["Sui Passkey"] --> D
  W["Dubhe / Sui Wallet"] --> D
```

## 已实现的渠道能力

| 渠道 | 首次进入 | 已登录身份 | 角色归属/恢复 | SDK 生命周期 | 状态 |
|---|---|---|---|---|---|
| 官网 | 原密码、Passkey 或 Wallet | Sui 签名证明 | Passkey 推荐，Wallet 可选 | 不需要渠道 SDK | 已实现 |
| itch | 服务端签名游客 | itch 游客命名空间 | 选角页绑定 Passkey/Wallet | 渠道标记、归因 | 已实现首发适配器 |
| CrazyGames | 未登录也可游客进入 | `getUserToken()` JWT | CrazyGames `userId` 自动映射；登录变化自动绑定 | SDK v3 init、loading、gameplay、rewarded ad | 已实现首发适配器 |
| Steam | Steam Session Ticket | SteamID | 映射到同一 Player ID | Steamworks | 接口模型可复用，尚未实现适配器 |
| Epic | EOS Auth/Connect token | Product User ID | 映射到同一 Player ID | EOS | 接口模型可复用，尚未实现适配器 |

itch 的 HTML5 项目运行在 iframe 中；仓库已提供
[`distribution/itch/index.html`](../distribution/itch/index.html) 轻量 launcher，加载
`https://mir2.obelisk.build/?channel=itch`，并为限制第三方 Cookie 或 WebAuthn 的浏览器
提供新标签页入口。游戏本体、Gateway 和动态 API 仍由 Obelisk 基础设施提供。若改为
上传完整游戏 ZIP，必须把 Next 服务端 API 独立成公网 Identity API。

CrazyGames Full Integration 的规则要求游客可直接玩、已登录用户自动登录、服务端
验证 `getUserToken()`、游戏中监听账号变化，并且不能把外部登录作为 CrazyGames
内的普通登录入口。因此本实现会在 CrazyGames 渠道隐藏 Passkey/Wallet 绑定面板，
使用 CrazyGames Auth Listener 自动绑定；官网和 itch 才展示 Passkey/Wallet 绑定。

参考：

- [CrazyGames Account integration](https://docs.crazygames.com/requirements/account-integration/)
- [CrazyGames HTML5 SDK v3](https://docs.crazygames.com/sdk/intro/)
- [CrazyGames User token 与服务端验签](https://docs.crazygames.com/sdk/html5-v2/user/)
- [itch HTML5 上传规范](https://itch.io/docs/creators/html5)
- [Steam 用户认证与所有权](https://partner.steamgames.com/doc/features/auth)

## 身份与安全模型

1. Web 端完成 Passkey/Wallet 签名，或从渠道 SDK 获取短期凭证。
2. Next 同源 API 只负责安全代理和签发 HttpOnly-cookie 游客证明。
3. Gateway 验证凭证：
   - Passkey/Wallet：验证 Sui personal-message 签名产生的短期证明；
   - CrazyGames：使用官方 RSA 公钥验证 RS256 JWT，检查 `exp` 和预期 `gameId`；
   - 游客：验证服务端 HMAC、渠道和过期时间。
4. Identity Registry 对 `provider + providerSubject` 做 SHA-256 后持久化；原始渠道
   user ID 不写入数据库。
5. Registry 解析或创建稳定 Player ID，并签发内存使用的短期 Gateway Session
   Token。
6. 角色、账号存档和 Zone 路由继续只看到 Player ID。

游客首次进入时，`primaryProvider` 可以暂时是 itch 或 CrazyGames Guest；绑定真实
Sui Passkey 后会自动晋升为 `suiPasskey`。优先级固定为
`Sui Passkey > Sui Wallet > CrazyGames > guest`，低等级凭证不会覆盖更强的恢复凭证。
服务端还会校验 Sui 签名方案，普通钱包签名不能仅靠篡改 `provider` 字段伪装成
Passkey。

绑定接口必须同时具备：

- 当前 Player ID 的未过期 Gateway Session Token；
- 新身份的实时签名或渠道 JWT；
- Registry 中新身份未属于其他 Player ID。

因此仅知道 Player ID、钱包地址或渠道 user ID 都不能抢走角色。冲突返回 HTTP 409，
不会自动合并两个已有进度账号；账号合并必须进入单独的人工审核流程。

## 生产配置

Gateway：

```bash
MIR2_CHANNEL_IDENTITY_DATABASE_URL=postgresql://USER:PASSWORD@PRIMARY:5432/mir2
MIR2_CHANNEL_IDENTITY_PG_POOL_MAX_SIZE=16
MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES=1
MIR2_REQUIRE_CHANNEL_IDENTITY_STORE=1
MIR2_CRAZYGAMES_GAME_ID=<CrazyGames gameId>
MIR2_CHANNEL_SESSION_TOKEN_TTL_SECONDS=3600
MIR2_PASSKEY_AUTH_SECRET=<32-byte-or-longer-random-secret>
MIR2_GATEWAY_OPERATOR_TOKEN=<operator-secret>
```

Player Web（Vercel/服务器端变量，不能使用 `NEXT_PUBLIC_`）：

```bash
MIR2_GATEWAY_HTTP_URL=https://gateway.example.com
MIR2_PASSKEY_AUTH_SECRET=<same-as-gateway>
```

生产必须设置 `MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES=1`。单机 JSON 仅用于开发，多个
Gateway 使用 JSON 会产生不同步风险。Gateway 健康检查的 `channel_identity` 应为：

```json
{"backend":"postgres","durable":true,"accountCount":1,"identityCount":2}
```

## API

### 渠道凭证换游戏会话

```http
POST /v1/channels/session/exchange
Content-Type: application/json

{"provider":"crazyGames","credential":"<CrazyGames JWT>"}
```

返回 `accountId/playerId/token/expiresAt/provider/created`。客户端随后沿用现有
WebSocket `passkeyLogin` 命令；协议名为兼容旧客户端保留，token 已是通用 Obelisk
Identity Token。

### 给当前角色绑定新身份

```http
POST /v1/channels/identity/link
Content-Type: application/json

{
  "accountId":"obl_...",
  "sessionToken":"...",
  "provider":"suiPasskey",
  "credential":"<fresh Sui proof>"
}
```

### 运维查询角色归属

```bash
curl -H "Authorization: Bearer $MIR2_GATEWAY_OPERATOR_TOKEN" \
  "https://gateway.example.com/admin/channel-identities/obl_..."
```

结果只包含哈希后的渠道 subject、绑定类型、创建/最后使用时间以及最近认证渠道，不返回
原始家庭 IP、CrazyGames user ID、钱包签名或私钥。

## 本地验收

1. 启动 PostgreSQL：

   ```bash
   docker compose -f infra/docker-compose.dev.yml up -d postgres
   ```

2. 启动 Gateway：

   ```bash
   MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7110 \
   MIR2_GATEWAY_TCP_ADDR=127.0.0.1:7000 \
   MIR2_CHANNEL_IDENTITY_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
   MIR2_PASSKEY_AUTH_SECRET=local-channel-test-secret-change-me \
   MIR2_GATEWAY_OPERATOR_TOKEN=local-operator-test-token-change-me-2026 \
   cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway
   ```

3. 启动 Player Web：

   ```bash
   cd apps/web
   MIR2_GATEWAY_HTTP_URL=http://127.0.0.1:7110 \
   NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7110/ws \
   MIR2_PASSKEY_AUTH_SECRET=local-channel-test-secret-change-me \
   npm run dev
   ```

4. 打开 `http://127.0.0.1:3000/?channel=itch`：
   - 不输入账号密码也应自动进入选角；
   - account ID 应为 `obl_...`；
   - 点击左上角 `Obelisk ID` 可绑定 Passkey 或检测到的 Dubhe/Sui Wallet；
   - 刷新后仍解析到同一 Player ID。
5. 打开 `http://127.0.0.1:3000/?isCrazyGames=true`：
   - SDK v3 初始化；
   - 未登录时以游客进入且不显示外部登录；
   - 已登录时服务端验 JWT 并自动进入；
   - 登录变化通过 Auth Listener 自动绑定；
   - 进入游戏触发 `gameplayStart`，离开触发 `gameplayStop`。
6. 查询 `GET /health`，确认 `channel_identity.backend=postgres` 和
   `channel_identity.durable=true`。

自动 PostgreSQL 双实例验收：

```bash
MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 test --locked -p mir2-gateway --lib \
  channel_identity::tests::postgres_store_is_shared_across_gateway_instances \
  -- --ignored --test-threads=1
```

## 下一批渠道优先级

1. **Steam**：适合原生 Windows/Linux 客户端和付费发行；使用 Web API Session Ticket
   在服务端验真，再把 SteamID 作为新的 provider subject。
2. **Epic Games Store / EOS**：用于跨平台账号和好友体系，接入成本高于 Steam。
3. **Poki / GameDistribution**：等 CrazyGames 数据证明 Web 渠道留存后再接，避免同时
   维护多套广告 SDK。
4. **App Store / Google Play**：属于移动支付、审核、包体与原生登录的独立里程碑，
   不与首轮 HTML5 渠道混做。

渠道扩展只需要新增 `ChannelAdapter + Gateway verifier`；Player ID、角色、Zone、
Commonware 和家庭节点架构不需要按渠道复制。
