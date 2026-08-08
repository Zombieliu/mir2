# 基础设施 / 服务器 / Token / R2 一览

> 项目部署所需的**服务器、各类 token/密钥权限、R2 资产情况**的单页速查。
> 数值来源于仓库内已提交的配置与 runbook（截至 2026-06）。**真实密钥值不写入本文件**，
> 这里只记录密钥的*名字*和它们*存放的位置*。
>
> 详细操作流程见：
> - 资产/R2 发布 → [`ASSET-RELEASE-RUNBOOK.md`](ASSET-RELEASE-RUNBOOK.md)
> - Gateway 服务器发布 → [`GATEWAY-RELEASE-RUNBOOK.md`](GATEWAY-RELEASE-RUNBOOK.md)
> - Vercel 前端部署 → [`VERCEL-PLAYER-WEB-DEPLOYMENT.md`](VERCEL-PLAYER-WEB-DEPLOYMENT.md)
> - 链上挖矿（Sui/Dubhe）→ [`ONCHAIN-M4-E2E-RUNBOOK.md`](ONCHAIN-M4-E2E-RUNBOOK.md)

---

## 1. 系统拓扑（谁在哪）

```
浏览器
  │  https / wss
  ▼
Cloudflare Worker  mir2.obelisk.build/*  (mir2-web3-domain-proxy)
  ├── HTML/JS ────────────────► Vercel  (Player Web, apps/web)
  ├── /original-ui /original-map /generated/* (同源资产) ─► R2 bucket  mir2-web3-assets
  └── /ws  /health ───────────► Gateway 主机  (165.154.65.136:7110)

Cloudflare Worker  assets.mir2.obelisk.build/*  (mir2-r2-asset-cache)
  └── 不可变 Crystal UI/地图/音频 ─► R2 bucket  mir2-web3-assets  (前缀 mir2/v/)

Gateway 主机 (UCloud 香港 VPS, 4H8G)
  ├── mir2-gateway (systemd)  :7110 web/ws  · :7000 tcp(私网)
  ├── Postgres  :5432   账号/角色权威存储
  ├── Redis     :6379   session/route 缓存
  └── /var/lib/mir2/crystal-client/current  (Map/*.map 碰撞数据)

私有控制面 (Tailscale/内网，staging 才完整)
  Admin API :7420 · NATS :4222 · Redpanda :8082 · ClickHouse :8123 · Admin Web
```

**职责切分**：Player Web（浏览器客户端）放 Vercel；Gateway 这种长连 WebSocket 游戏服务
**不能**放 Vercel Functions，必须独立主机；不可变资产走 Cloudflare R2/CDN。

---

## 2. 服务器 / 服务清单

| 服务 | 地址 / 主机 | 端口 / 路由 | 说明 |
|---|---|---|---|
| **Player Web** | Vercel 项目 `obelisk-labs/mir2-web3-web`，别名 `mir2-web3-web.vercel.app` | — | Next.js (`apps/web`)，Root Dir `mir2-web3/apps/web`，Node ≥22 |
| **玩家域名代理** | Cloudflare Worker `mir2-web3-domain-proxy` | `mir2.obelisk.build/*` | 转发到 Vercel + 同源代理 R2 资产 + 代理 `/ws` `/health` 到 Gateway |
| **资产 CDN** | Cloudflare Worker `mir2-r2-asset-cache` | `assets.mir2.obelisk.build/*` | 边缘缓存 R2 不可变对象（前缀 `mir2/v/`，`max-age=1y immutable`） |
| **批量上传 Worker** | Cloudflare Worker `mir2-r2-bulk-upload` | `assets.mir2.obelisk.build/upload*` | 鉴权批量上传小文件（需 `MIR2_R2_UPLOAD_SECRET`） |
| **Gateway** | UCloud 香港 VPS `165.154.65.136`（公网 `https://165.154.65.136.sslip.io`） | web/ws `0.0.0.0:7110`；tcp `127.0.0.1:7000`（私网） | systemd `mir2-gateway`；SSH `ubuntu@165.154.65.136`；release 在 `/opt/mir2/gateway/` |
| **Postgres** | Gateway 主机 / 内网 `postgres:5432` | 5432 | 账号·角色权威库 `mir2`（用户 `mir2`） |
| **Redis** | Gateway 主机 / 内网 `redis:6379` | 6379 | session/route 缓存 + StartGame 路由租约 |
| **Admin API** | 内网 `mir2-admin-api` | `0.0.0.0:7420` | 运维/GM；不对公网暴露 |
| **NATS** | 内网 `nats:4222` | 4222 | Admin outbox（JetStream，stream `MIR2_ADMIN`）— 仅 staging |
| **Redpanda** | 内网 `redpanda:8082` | 8082 | gameplay 事件流（Kafka 兼容）— 可选 |
| **ClickHouse** | 内网 `clickhouse:8123` | 8123 | 分析库 `mir2_events`（用户 `mir2`）— 可选 |
| **Sui 全节点** | `https://fullnode.testnet.sui.io:443` | 443 | 链上挖矿（仅 testnet，M8 前不上主网） |
| **Dubhe Indexer** | 本地 `localhost:4000/graphql` | 4000 | 链上事件 GraphQL（HTTP + WS） |

**Cloudflare 账号 ID**：`85bf64d86ea9221e172d26feba9fd47e`，Zone `obelisk.build`。

> ⚠️ Vercel 对 `obelisk.build` 没有域名所有权，所以 `mir2.obelisk.build` 由 Worker 代理，
> Worker 服务端注入 Vercel 自动化绕过密钥 `VERCEL_BYPASS_SECRET`。**不要删除该 Worker 路由**，
> 除非已在 Vercel 团队里完成自定义域名认领。

---

## 3. R2 资产情况

- **Bucket**：`mir2-web3-assets`（Cloudflare 账号 `85bf64d86ea9221e172d26feba9fd47e`）。
- **公网原始 dev 域名**：`https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/`
  （裸 R2 自定义域名可能返回 `cf-cache-status: DYNAMIC`，所以走 Worker 而非直连）。
- **生产自定义域名**：`https://assets.mir2.obelisk.build`（经 `mir2-r2-asset-cache` Worker 缓存）。

### 对象布局
| 前缀 | 内容 |
|---|---|
| `mir2/v/<version>/original-ui/**` | Crystal UI/精灵 PNG、`meta.json`、音频 WAV/CUR |
| `mir2/v/<version>/original-map/**` | 地图瓦片/对象 PNG |
| `mir2/v/<version>/generated/original-map-blend/**` | 生成的地图混合帧 |
| `mir2/v/<version>/remote-asset-release.json` | 该版本的远程资产清单 |
| `gateway/releases/<tag>/mir2-gateway-linux-x64.tar.gz(.sha256)` | Gateway 二进制发布包 |
| `gateway/map-assets/<tag>/mir2-crystal-map-assets.tar.gz(.sha256)` | 服务端 `Map/*.map` 碰撞数据包 |

### 当前生效版本
| 类型 | 版本 / tag | 来源 |
|---|---|---|
| Web 资产 | `mir2/v/20260601-fullcrystal-a2f10be0` | `infra/cloudflare/mir2-domain-proxy/wrangler.jsonc`（active，1:1 完整，0 缺失） |
| 地图数据包 | `20260518T050053Z-eeb0b443`（468 个 `Map/*.map`，~46MB 压缩） | 两份 runbook |

### 发布方式（关键）
- R2 发布是**手动** `workflow_dispatch`：GitHub Actions → **`Mir2 Web Assets R2 Release`**
  （`.github/workflows/web-assets-r2-release.yml`）。push 到 main 只是 no-op 闸门。
- 真正发布要带：`publish_r2=true deploy_worker=true deploy_vercel=true`。
  顺序强制：先传 R2 → 验证 → 部署 Worker → 再部署 Vercel。
- 上传驱动 `MIR2_R2_UPLOAD_DRIVER`：`r2-s3`（默认）/ `wrangler` / `api` / `worker`。

> ⚠️ **资产在 git 里但不从 Vercel 提供**。`.vercelignore` + `prune-vercel-output-assets.mjs`
> 会把 `public/original-ui/**`、`public/original-map/**` 从 Vercel 构建里剥掉 → 这些 PNG
> **只由 R2 提供**。若某帧 git 里有却在游戏里 `net::ERR_FAILED`，那是 **R2 发布过期/不完整**，不是代码 bug。

---

## 4. Token / 密钥权限清单

> 命名约定：`✅ 已提交配置里能看到的非敏感值` / `🔒 真实值不入库，只列名字`。

### 4a. GitHub Actions Secrets（发布链用）
| Secret 名 | 用途 | 备注 |
|---|---|---|
| 🔒 `CLOUDFLARE_API_TOKEN` | 写 R2 对象 + 部署 Worker | 需 R2 写权限 |
| ✅ `CLOUDFLARE_ACCOUNT_ID` | Cloudflare 账号 | `85bf64d86ea9221e172d26feba9fd47e` |
| ✅ `MIR2_R2_ASSET_BUCKET` / `MIR2_R2_BUCKET` | Web 资产 bucket | `mir2-web3-assets` |
| ✅ `MIR2_R2_RELEASE_BUCKET` | Gateway 发布包 bucket | 同账号下的 release bucket |
| 🔒 `MIR2_R2_S3_ENDPOINT` | R2 S3 兼容端点 | `r2-s3` 上传驱动用 |
| 🔒 `MIR2_R2_ACCESS_KEY_ID` / `MIR2_R2_SECRET_ACCESS_KEY` / `MIR2_R2_SESSION_TOKEN` | R2 S3 凭据 | `r2-s3` 上传驱动用 |
| 🔒 `MIR2_PUBLIC_R2_ASSET_BASE_URL` | 公网资产 base（可选） | 默认 `https://assets.mir2.obelisk.build/mir2/v/{version}` |
| 🔒 `VERCEL_TOKEN` / `VERCEL_ORG_ID` / `VERCEL_PROJECT_ID` | Vercel 部署 | deploy 步骤用 |
| 🔒 `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` | 注入到 Vercel 构建的 WS URL | 默认 `wss://mir2.obelisk.build/ws` |

### 4b. Vercel 项目环境变量（`obelisk-labs/mir2-web3-web`）
| 变量 | scope | 说明 |
|---|---|---|
| ✅ `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` | browser/route | 指向当前 R2 release，例 `https://assets.mir2.obelisk.build/mir2/v/<version>` |
| ✅ `MIR2_ASSET_OBJECT_PREFIX` | route | `mir2/v/<version>`，要和上面同版本 |
| ✅ `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` | browser | `wss://mir2.obelisk.build/ws`（公开，设计如此） |
| 🔒 `MIR2_PASSKEY_AUTH_SECRET` | server | **必须与 Gateway 同值**，passkey/钱包登录 token 才能校验 |
| ✅ `MIR2_ENV` / `MIR2_DEPLOYMENT_ENV` | server | `staging`：缺 auth secret 时 fail-closed |
| ✅ `NEXT_PUBLIC_MIR2_RUNTIME_VERSION` | browser | `/bevy-runtime` 缓存破坏（可选，现走生成的 json） |

> 不要把数据库 URL、Redis URL、operator token、Admin 密钥放进 Player Web 的 Vercel 项目——
> 玩家端只通过 WebSocket 跟 Gateway 通信。

### 4c. Gateway 主机环境（`/etc/mir2/gateway.env`，模板 `infra/systemd/mir2-gateway.env.example`）
| 变量 | 说明 |
|---|---|
| 🔒 `MIR2_PASSKEY_AUTH_SECRET` | 32-byte 随机；与 Vercel 同值 |
| 🔒 `MIR2_ACCOUNT_STORE_DATABASE_URL` | `postgres://mir2:<pw>@127.0.0.1:5432/mir2` |
| ✅ `MIR2_GATEWAY_REDIS_CACHE_URL` | `redis://127.0.0.1:6379` |
| ✅ `MIR2_GATEWAY_WEB_ADDR` / `MIR2_GATEWAY_TCP_ADDR` | `0.0.0.0:7110` / `127.0.0.1:7000` |
| ✅ `CRYSTAL_CLIENT_ROOT` | `/var/lib/mir2/crystal-client/current`（`Map/*.map`） |
| 🔒 `MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN` | 可选，接 Admin API 时用 |
| ✅ 容量护栏 | `MAX_WS_CONNECTIONS` / `MAX_ACTIVE_SESSIONS` / `MAX_RECONNECT_LEASES`（当前生产 `30/15/15`） |

### 4d. Admin / staging 密钥（模板 `infra/staging.env.example`）
| Secret 名 | 用途 |
|---|---|
| 🔒 `POSTGRES_PASSWORD` | Postgres 账号库密码 |
| 🔒 `CLICKHOUSE_PASSWORD` | ClickHouse 分析库密码 |
| 🔒 `GATEWAY_OPERATOR_TOKEN` | Gateway↔Admin 心跳/server-action |
| 🔒 `LEAD_OPERATOR_TOKEN` / `PEER_OPERATOR_TOKEN` | 运维操作员 token |

### 4e. Cloudflare Worker 密钥
| Secret 名 | Worker | 用途 |
|---|---|---|
| 🔒 `MIR2_R2_UPLOAD_SECRET` | `mir2-r2-bulk-upload` | 鉴权批量上传 |
| 🔒 `MIR2_R2_UPLOAD_WORKER_URL`（GitHub Actions） | `mir2-r2-bulk-upload` | CI 上传入口；生产值为 `https://assets.mir2.obelisk.build` |
| 🔒 `VERCEL_BYPASS_SECRET` | `mir2-web3-domain-proxy` | 注入 Vercel SSO 绕过 |

### 4f. 链上挖矿（`onchain/.env`、`onchain/relayer/.env`，均 gitignored）
| 变量 | 用途 |
|---|---|
| 🔒 `PRIVATE_KEY` | Sui deployer/admin/miner key（`dubhe publish` + smoke）；suiprivkey/base64/hex |
| 🔒 `OPERATOR_TOKEN` | relayer → Gateway `/onchain/inject` 注入授权；须等于 Gateway operator token |
| ✅ `SUI_NETWORK` / `SUI_FULLNODE_URL` | `testnet` / `https://fullnode.testnet.sui.io:443` |
| ✅ `MINE_PACKAGE_ID` / `FRAMEWORK_PACKAGE_ID` / `DAPP_HUB_ID` / `DAPP_STORAGE_ID` | 部署产物（公开 id，可入库） |
| ✅ `GATEWAY_INJECT_URL` | 默认 `http://127.0.0.1:7110/onchain/inject` |
| ✅ `INDEXER_GRAPHQL_URL` / `_WS` | Dubhe indexer GraphQL |

---

## 5. 一键对照：要发一次完整生产，需要什么

1. **R2 上有当前版本资产**（`mir2/v/<version>`，0 缺失）。
2. **Cloudflare 凭据**：托管 CI 默认使用 `MIR2_R2_UPLOAD_WORKER_URL` +
   `MIR2_R2_UPLOAD_SECRET` 写 R2；`CLOUDFLARE_API_TOKEN` 只负责 Worker 控制面。
   本地发布 10 GB 级完整包时仍优先使用独立 R2 S3 凭据。
3. **Vercel 凭据**：`VERCEL_TOKEN` / `VERCEL_ORG_ID` / `VERCEL_PROJECT_ID`，且项目环境变量里
   `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` / `MIR2_ASSET_OBJECT_PREFIX` 指向同一版本。
4. **Gateway 主机**在线（`https://165.154.65.136.sslip.io/health` OK），且
   `MIR2_PASSKEY_AUTH_SECRET` 与 Vercel 一致。
5. 触发 `Mir2 Web Assets R2 Release`，参数 `publish_r2=true deploy_worker=true deploy_vercel=true`。

健康检查：
```bash
curl https://165.154.65.136.sslip.io/health          # Gateway
curl https://mir2.obelisk.build/health               # 经 Worker 代理的 Gateway
curl https://mir2.obelisk.build/api/asset-manifest    # 确认 remoteAssets.assetBaseUrl 非 null 且版本正确
mir2-remote-status                                    # Mac 上的 SSH 包装（默认 ubuntu@165.154.65.136）
```
