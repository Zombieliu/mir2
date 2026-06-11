# M4 端到端冒烟 Runbook（本地 3 进程 + Sui testnet）

> **没有任何组件部署在服务器上。** Indexer / Relayer / Gateway 全是**本地进程**；唯一的远端
> 是 Sui testnet（链）。部署上（staging）服务器是 **M7（★go/no-go）** 的事。
> Indexer 甚至不在关键路径上——Relayer 直连 Sui `queryEvents`（M2 架构微调），跑 e2e 不用开它。

```
浏览器(本地 next dev) ── mine_batch(钱包签名) ──▶ Sui testnet
        ▲                                            │ mine_settled / ore_redeemed
        │ WS :7110                                   ▼
本地 gateway ◀── POST /onchain/inject ── 本地 relayer(轮询 queryEvents)
        │ execute_with_outcome(权威落库)
        ▼
   GainedItem + MineNodeState ──▶ 浏览器(背包 + 矿脉分档 + HUD 对账)
```

## 0. 前置

- Sui CLI ≥ 1.73.0；testnet 钱包有 SUI（faucet）。**浏览器侧**需要一个 Wallet-Standard 的
  Sui 钱包扩展（连接后用它签 `mine_batch`/`redeem`），其地址需 faucet 充值。
- 合约已部署：`onchain/deployments/testnet.json`（packageId `0xe6c3…dbe5`，mine_id=1，
  smoke 矿 hit/drop=100/100、max_stones=10、regen 300s、per_swing_fee=0 占位）。
- 各 `.env` 全部本地、**绝不入库**。

## 1. Gateway（终端 1）

```bash
cd mir2-web3
# 操作者令牌：relayer 与 gateway 必须一致（生产 fail-closed；本地显式给值）
export MIR2_GATEWAY_OPERATOR_TOKEN="$(openssl rand -hex 24)"
echo "OPERATOR_TOKEN=$MIR2_GATEWAY_OPERATOR_TOKEN"   # 抄给 relayer 用
# 链上矿脉映射是 env 门控的（默认关——没有 on-chain 栈的部署不会出"幽灵矿脉"）。
# 格式 mine_id:map:x:y:max_stones[,...]；testnet mine 1 放在 Bichon 出生点东侧：
export MIR2_ONCHAIN_MINE_NODES="1:0:335:270:10"
# 钱包/passkey 登录的 token 验签：本地必须显式开 dev 密钥（生产 fail-closed 设计）
export MIR2_ALLOW_DEV_PASSKEY_SECRET=1
# 网关默认 web 端口是 7010，而 web 客户端本地默认连 ws://127.0.0.1:7110/ws —— 对齐到 7110。
# crate 有多个 bin（smoke/packet_trace/...），必须指明 --bin：
MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7110 cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway
# （注入端点即 http://127.0.0.1:7110/onchain/inject）
```

> 注意：网关默认的 CrystalWorld 模式下 **starter 地图没有任何 P0 矿点**（P0 starter 矿区
> x∈[331,335) 只在 starter-region 测试模式由 `with_crystal_map_runtime` 播种；Crystal 把
> 矿区存在 Map DB 而非 manifest，所以 CrystalWorld 不自带矿区）——别把"看不到 P0 矿脉"当
> 回归。(335, 270) 无论哪种模式都**不是** P0 矿点：服务端不会因挥击产矿，矿只能由链确认
> 事件落账（不会双发）。

## 2. Relayer（终端 2）

```bash
cd mir2-web3/onchain
cp relayer/.env.example .env   # 然后填：
#   MINE_PACKAGE_ID=0xe6c3602e4055b76afd82a48745f9cd34daa4a6dce1f420747f0779732640dbe5
#   GATEWAY_INJECT_URL=http://127.0.0.1:7110/onchain/inject
#   OPERATOR_TOKEN=<终端 1 的值>
pnpm relayer
```

## 3. Web（终端 3）

```bash
cd mir2-web3/apps/web
# MIR2_ALLOW_DEV_PASSKEY_SECRET 同样要给 web（/api/passkey/login 用它签登录 token，
# 与网关验签共用同一个 dev 密钥开关；只给一边会报 "MIR2_PASSKEY_AUTH_SECRET is not set"）
NEXT_PUBLIC_ONCHAIN_MINE=1 MIR2_ALLOW_DEV_PASSKEY_SECRET=1 npm run dev
# 可选微调：NEXT_PUBLIC_ONCHAIN_MINE_BATCH=5（攒挥阈值，M5 拍板前的占位）
#          NEXT_PUBLIC_ONCHAIN_MINE_FEE_PER_SWING_MIST=0（链上 per_swing_fee 同步占位 0）
#          NEXT_PUBLIC_ONCHAIN_MINE_NODE_X/Y=335/270（须与 sim 配置一致）
```

> 真机踩坑（M4 live e2e 实录）：
> - **旧 service-worker 缓存会喂旧代码**：本应用带 serwist 预缓存（`pages`/`assets-cache`…），
>   改代码后浏览器里务必 `Cmd+Shift+R` 硬刷新；怀疑不对就 DevTools → Application →
>   Clear storage。已打开的旧标签页不会自己更新。
> - **Slush/Suiet 这类钱包只应答 `app-ready`、不主动广播注册**，注入晚于页面初始化就会
>   永久缺席钱包列表——客户端已在每次枚举前重新广播 `wallet-standard:app-ready` 兜底
>   （`passkey-auth.ts` `rescanWalletStandard`），列表打开即可见。

## 4. 冒烟步骤

1. 浏览器开 `http://localhost:3000` →（推荐）**钱包登录**（签名钱包自动保留给挖矿用）；
   passkey 登录也行，HUD 里再点"连接 Sui 钱包"。**签名钱包地址必须 == 登录账号地址**
   （结算按链上 `miner` 地址投递到 `sui:0x..` 会话；不一致时奖励会投给签名地址的会话，
   该地址离线则等 M6 离线持久化——链上 `ore_balance` 仍在，可后续 redeem，不会烧掉）。
2. 进游戏后右下角出现 **On-chain Mine (testnet)** 面板；矿脉 (335,270) 应显示 `full vein 满`
   （进图即播种）。走到矿脉旁。
3. 点 **挥镐 Swing** ×5（或点"立即结算"提前出批）→ 攒满后自动弹钱包签名 **1 笔
   `mine_batch`** → 面板出现在途 digest。
4. 等链确认 + relayer 轮询（默认 4s）注入：日志出现
   `[on-chain mine] batch settled: +N ore confirmed on-chain`，背包多出 BlackIronOre
   （dura = N×1000），矿脉分档随链上 `stones_left` 变化。
   **注意分档算法与 P0 1:1**（恰好一半仍算"满"）：默认 5 挥/批、max_stones=10 时
   stones 走 10→5→0，看到的是 **满→满→空**；想看"裂"档（stones×2 < max，即 1..4），
   用 **立即结算** 在非整批挥数（如先挥 6 下）时出批。
   **对账**：面板"上次对账"显示 一致 ✓ / 幻影（多退）/ 补差（少补）。
5. **redeem**：面板里填数量 → **兑换金币 Redeem** → 钱包签名 → `ore_redeemed` → relayer →
   `CreditGoldFromOre` → 日志 GainedGold、金币增加（M4 占位兑率 1:1，M5 拍板）。
6. **三处幂等抽查**：
   - 链 nonce：重发同 nonce 的 `mine_batch` → `BadNonce` abort（链上拒绝）。
   - relayer：重启 relayer → 旧事件全部 dedup，不再注入。
   - sim：手动重 POST 同 `idempotencyKey` 到 `/onchain/inject` → `packetCount: 0`（no-op）。

## 5. 已知 M4 占位/边界（不要当 bug 报）

| 事项 | 现状 | 归宿 |
|---|---|---|
| ore→gold 兑率 | 1:1 占位（gateway inject 处) | **M5 拍板** |
| per_swing_fee / 批大小 / 排放封顶 | 0 / 5 / 大额占位 | **M5 拍板** |
| 玩家离线时的链奖励 | 200 accepted / `connected:false`，**不持久化** | M6 |
| nonce 恢复 | localStorage 持久 + 面板可手改；不读链 | M6 |
| 多笔在途批次 | 一次只允许 1 笔在途 | M6 |
| 链矿脉跨会话初始分档 | 进程内记忆（重启回满档） | M6 |
