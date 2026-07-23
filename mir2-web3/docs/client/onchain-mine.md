# 链上矿 (on-chain mine, web3 子系统) — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

可选的「链上智能矿」垂直切片 (M4 / WF-6, DESIGN §3/§4)：玩家走到一个固定矿脉格攻击它，客户端把每次挥镐攒成 N 次一批 (`mine_batch`)，由 Sui 钱包或临时会话密钥签名上链。**链是产出权威，但金币/矿石不在客户端铸造** —— 上链确认后由 Relayer 把事件注入 Sim (apps/gateway/src/inject.rs → apps/simulation/src/runtime/onchain.rs)，矿石再以普通 `GainedItem` 包回流。客户端期间只显示「乐观 VFX」，settle 时与链上确认值对账 (多退少补)。

整块由 build flag `NEXT_PUBLIC_ONCHAIN_MINE=1` 控制；不开 flag 时面板既不渲染也不被引用，`@mysten/sui` SDK 也不进主 bundle（PTB builder / 钱包会话全部 **动态 import**）。

This is OPTIONAL and OFF by default. It does NOT touch the normal P0 mining path — the on-chain vein cell is deliberately not a P0 mine spot, so no server payout can double-credit the chain grant (page.tsx onchainSwing comment ~line 5061).

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/lib/onchain-mine-config.ts` | 纯配置 + 无依赖 helper（无 SDK，page.tsx 可静态 import）。部署 id、矿种枚举、nonce tracker、对账/矿脉阶段 helper | `TESTNET_MINE_DEPLOYMENT` :35, `ORE_KINDS` :48, `createNonceTracker` :92, `reconcileOptimisticOre` :132, `stonesLeftToVeinStage` :154 |
| `apps/web/lib/onchain-mine.ts` | PTB builders（静态 import `@mysten/sui` — **只能动态 import**）。re-export 整个 config | `buildMineBatchTransaction` :80, `buildRedeemTransaction` :127, `oreKindConstructorTarget` :50 |
| `apps/web/lib/onchain-mine-session.ts` | Wallet-Standard 签名+提交（钱包每批弹窗）。`MineSessionContext` 定义在此 | `MineSessionContext` :56, `executeMineBatch` :81, `executeRedeem` :93, `walletCanSignAndExecute` :41 |
| `apps/web/lib/onchain-mine-sessionkey.ts` | SK2 临时会话密钥（弹窗杀手）+ `ensureUserStorage` + localStorage 持久化 | `ensureUserStorage` :222, `activateSession` :250, `deactivateSession` :288, `executeMineBatchWithSession` :309, `getActiveSession` :133 |
| `apps/web/lib/onchain-mine-state.ts` | 纯状态机（headless 单测）：攒挥→在途→settle→对账 + 结算检测 helper | `OnchainMineState` :17, `recordSwing` :69, `beginBatch` :85, `confirmSettlement` :135, `oreUnitsFromGainedItem` :204, `loadPersistedNextNonce` :245 |
| `apps/web/app/components/onchain-mine-panel.tsx` | HUD 面板（presentation-only，可拖拽/收起，props 进、callback 出） | `OnchainMinePanel` :178, `OnchainMinePanelProps` :31 |
| `apps/web/app/page.tsx` | 5 层接线：config 常量、state、effects、callback、WS 结算检测、面板挂载 | config block :932-967, state :1461-1475, effects :1682-1763, callbacks :4980-5236, panel mount :11279 |
| `onchain/deployments/testnet.json` | 链上 id 真值（packageId/dappStorageId/frameworkPackageId/dappHubId/seededMines）。Web 通过 `NEXT_PUBLIC_*` 注入 | (SK1 fresh publish, Dubhe 1.2.x) |

## 数据流 (How it threads the 5 layers)

这块**双向**穿过 5 层，但出站不是普通 BrowserCommand → 而是直接上链：

**出站 (client → chain，绕过 gateway):**
1. 世界手势：点/踏到矿脉格 → page.tsx `handleTileClick`/移动落点判定 `x === ONCHAIN_MINE_VEIN.x` (page.tsx :6076, :6129) → `mineOnchainVein()` :5122 → 相邻则 `startOnchainMiningLoop()` :5091，否则 `pendingOnchainMineRef=true` 走到旁边、到达 effect (:3958) 再启动。
2. 每次挥镐 `onchainSwing()` :5066 → `send({type:"attackDirection"})`（**这一步走正常 gateway**，只是让服务端播放挥镐动作，不产出）+ `recordOnchainSwing()` 累加 pending。
3. 攒满 `ONCHAIN_MINE_BATCH_SIZE` → auto-begin effect (:1714) 调 `beginBatch()` → submit effect (:1725) 调 `submitOnchainBatch()` :5146。
4. `submitOnchainBatch` 选签名路径：有 active session → `executeMineBatchWithSession`（session 密钥本地签，**无弹窗**）；否则 `ensureUserStorage(ctx)`（testnet RPC 查/建 UserStorage，可能一次弹窗）→ `executeMineBatch`（钱包签）。成功后 `tracker.next()` + `persistNextNonce()` + `batchSubmitted(digest)`。

**入站 (chain → Relayer → Sim → ServerPacket → client，正常 5 层):**
1. 链确认 → Relayer 读事件 → gateway inject → Sim 授予矿石 + 重广播矿脉。
2. **`GainedItem`** 包 → gateway `server_packet_to_event` → page.tsx `case "GainedItem"` (:7370)：若在途，用 `oreUnitsFromGainedItem(item, swings - alreadyStashed)` 从 `dura/1000` 读出链授予单位，暂存进 `onchainConfirmedUnitsRef`。
3. **`MineNodeState`** 包（结算信号，**跟在 GainedItem 之后**同一 flush）→ page.tsx `case "MineNodeState"` (:6899)：写 `world.mineNodes`；若是配置矿脉且在途 → `confirmSettlement(current, onchainConfirmedUnitsRef.current)` 关闭该批并对账。
4. 状态变化 → 面板 (:11279) 重渲染（**无 stage5-window-adapter 这层** —— 本子系统直接 props 喂 `OnchainMinePanel`，不经 `stage5-window-adapters.ts`）。

赎回路径：`redeemOnchainOre()` :5200 → `ensureUserStorage` → `executeRedeem`（burn ore）→ 链 emit `OreRedeemedEvent` → Relayer → `CreditGoldFromOre` → Sim 权威加金币（金币**不**客户端铸造）。

## 状态形状 (State shape)

**`world.*` (DisplayWorld):**
- `world.mineNodes: { x: number; y: number; stage: number }[]` (page.tsx :711, 初值 [] :855) —— 唯一与本子系统相关的 world 字段；`MineNodeState` 包写它，面板 `veinStage` 从里查。`stage`: 2=满 / 1=裂 / 0=空。

**Local React state (page.tsx :1461-1475)，全部仅在 `ONCHAIN_MINE_ENABLED` 时有意义:**
- `onchainMine: OnchainMineState` (:1462) —— 状态机本体（见 onchain-mine-state.ts :17）：`pendingSwings`/`pendingOptimisticUnits`/`inFlightSwings`/`inFlightOptimisticUnits`/`inFlightDigest`/`inFlightNonce`/`confirmedUnits`/`settledBatches`/`lastReconcile`/`lastError`。
- `onchainWallet: ActiveSuiWalletSession | null` (:1463) —— 复用登录流连过的钱包（type 来自 `lib/client-login-runtime`）。
- `onchainWalletBusy` / `onchainSubmitBusy: boolean` (:1464-65) —— 钱包操作 / 上链中。
- `onchainRedeemAmount: string` (:1466)。
- `onchainSession: { address; expiresAt } | null` (:1468) —— active 会话密钥的展示态。
- `onchainNextNonce: number` (:1469) —— 面板显示/可编辑的下一个 nonce。

**Refs (非渲染态):**
- `onchainNonceRef: NonceTracker | null` (:1470) —— 权威 nonce 计数器；连接时由 `loadPersistedNextNonce` 恢复。
- `onchainSubmitGuardRef` (:1472) —— 同步重入锁（effect 双触发 / StrictMode）。
- `onchainConfirmedUnitsRef: number | null` (:1475) —— 暂存从 GainedItem 读出的链授予单位，供随后的 MineNodeState 消费。
- `onchainInFlightSwingsRef` (:1704)、`pendingOnchainMineRef` (:1354)、`onchainMiningTimerRef` (:1356)。

**localStorage keys:** `mir2.onchainMine.nextNonce.{acct}.{pkg}` (state.ts :240)、`mir2.onchainMine.userStorage.{acct}.{pkg}` 与 `mir2.onchainMine.session.{acct}.{pkg}` (sessionkey.ts :48-53)、`mir2.onchainMinePanel.pos.v1`/`.collapsed.v1` (panel :74-75)。

## 坑 & 不变量 (Invariants & gotchas)

- **build flag 是硬门。** `ONCHAIN_MINE_ENABLED = process.env.NEXT_PUBLIC_ONCHAIN_MINE === "1"` (page.tsx :935)。面板挂载条件是 `ONCHAIN_MINE_ENABLED && screen === "game"` (:11279) —— **独立于 Bevy/canvas**，不开 flag 的生产构建连 import 都没有。
- **`@mysten/sui` 绝不能静态 import 进 page.tsx。** `onchain-mine.ts` / `-session.ts` / `-sessionkey.ts` 都静态拉了 SDK；page.tsx 只静态 import `onchain-mine-config.ts`（纯）和 `onchain-mine-state.ts`（纯），其余全部 `await import(...)` 在 handler 内（见 :5164, :5022, :4997, :5213）。破坏这条会把整个 SDK 灌进主 chunk。
- **nonce 必须严格递增且持久化。** 合约拒绝 replay / 乱序。`tracker.next()` 只在链**接受** tx 后才推进 (:5179)，且立刻 `persistNextNonce`。失败走 `batchFailed`（**恢复** swings，不推进 nonce）。
- **`batchFailed` vs `abandonInFlightBatch` 语义相反 —— 别混。** `batchFailed` (state.ts :116) 把在途 swings 退回 pending（tx 从未上链：钱包拒签/RPC 错/合约 abort，不重试，`lastError` 门控）。`abandonInFlightBatch` (state.ts :176, watchdog :1741 触发) **不**退 swings —— tx 已上链 nonce 已花，重发会双挖；矿石仍会以 GainedItem 到。
- **结算信号是 `MineNodeState` 不是 `GainedItem`。** Sim 在授予物品**之后**重广播矿脉，client 靠这个矿脉更新关批 (:6926 注释)。`GainedItem` 只是先把单位暂存进 ref。
- **链授予单位编码：`dura = units * 1000`，仅 FRESH item 可读** (state.ts :204 `oreUnitsFromGainedItem`)：`count<=1 && dura==maxDura && dura%1000==0`。形状不唯一（商店/合成的整耐久也满足），所以**必须**用在途 swing 数上界（合约每挥至多 1 矿）排除误判 —— page.tsx 传 `inFlightSwings - alreadyStashed` (:7393)。叠加到已有 stack 读不出 → null（settle 但单位未知，不主张对账 delta）。
- **乐观显示 ≠ 真实产出。** `pendingOptimisticUnits` / `confirmedUnits` 全是 display-only；真实矿石只走链→Relayer→Sim→GainedItem。`reconcileOptimisticOre` / `confirmSettlement` 做「多退少补」。
- **矿脉阶段 tier 与 P0 sim 1:1：** `stonesLeftToVeinStage` (config.ts :154) —— 0 空；`left*2 < capacity` 裂（严格小于一半，恰好一半仍满）；否则满。
- **session 密钥是低权委托键，不是主钱包键。** 持久在 localStorage，reload 保留；过期/清除/deactivate 结束；session 钱包自付 gas（用户先转一点 SUI，gasless 是 SK3）。effective miner = UserStorage 的 `canonical_owner`，谁签都记给 owner (sessionkey.ts :7 注释)。
- **`ensureUserStorage` 是矿/赎回的链上前置 (sessionkey.ts :222)：** 先查 localStorage cache → testnet RPC `findExistingUserStorageId`（跨设备扫历史）→ 都无则 `init_user_storage`（一次钱包弹窗）。**这就是「mine batch 边界」上 ensureUserStorage testnet RPC then walletSign 的那一步** —— 仅 fallback（无 session）路径走它 (:5174)；session 路径的 UserStorage 来自 session 自带 (:5170)。
- **连续挖矿循环 (`startOnchainMiningLoop` :5091) 需要已连签名钱包**，否则只挥一下就停（防无人值守攒下无法结算的 swings）。循环在丢失相邻 / 矿脉空 / 离开 game / 签名中自停 (:5100-5112)。
- **dev Swing 按钮只是捷径；真正挖矿是世界里攻击矿脉。** 面板注释与 `onSwing` (panel :447) 都标了这点。
- **auto-retry 被 `lastError` 门控** (:1716)：失败批不自动重发（否则无限弹窗），玩家点「立即结算」清错重试 (`flushOnchainBatch` :5139)。

## 如何扩展 (How to extend / add to this area)

加一个新的链上操作（例：新增一个 `upgrade_vein` 交易），按顺序改：

1. **PTB builder** —— 在 `apps/web/lib/onchain-mine.ts` 加 `buildUpgradeVeinTransaction(deployment, params)`，用 `tx.moveCall({ target: \`${deployment.packageId}::xxx_system::upgrade_vein\` })`。若需新 system/module id，先看 `onchain/deployments/testnet.json` 是否已部署。
2. **（如需新部署 id）** 在 `onchain-mine-config.ts` 的 `OnchainMineDeployment` type + `TESTNET_MINE_DEPLOYMENT` 加字段（**additive**，从 `process.env.NEXT_PUBLIC_*` 读，`?? ""` 兜底），并在 `onchain/deployments/testnet.json` 记真值。
3. **签名包装** —— 在 `onchain-mine-session.ts` 加 `executeUpgradeVein(ctx, params)`（钱包路径）；若要免弹窗，在 `onchain-mine-sessionkey.ts` 加 `executeUpgradeVeinWithSession(deployment, session, params)`。
4. **（如有新状态）** 在 `onchain-mine-state.ts` 扩 `OnchainMineState` + 写**纯** transition（旧态进、新态出，便于 headless 单测），别在这里碰 DOM/钱包。
5. **page.tsx 接线**（4 处）：(a) 在 callback 区 (~:4980-5236) 加 `async function upgradeOnchainVein()`，**动态 import** 签名模块；(b) 如需新 React state/ref，加在 :1461-1475 一带；(c) 若由 server 包驱动结算，在对应 `case "X"` handler (WS switch) 里加检测（参照 `case "MineNodeState"` :6899 的 `ONCHAIN_MINE_ENABLED && ...Ref.current > 0 && isOnchainVeinNode(...)` 门控）；(d) 把新 prop/callback 传给面板 :11280。
6. **面板 UI** —— 在 `onchain-mine-panel.tsx` 的 `OnchainMinePanelProps` (:31) 加**可选/additive** prop + callback，渲染一行/一个按钮。保持 presentation-only：值进、callback 出，逻辑留在 page.tsx / state.ts。
7. **测试** —— 纯 helper 与状态机走 headless 单测（state/config 模块本就如此）；type-check `npx tsc --noEmit` 必须 0。注意 **不破坏** `DisplayWorld` / 现有 consumer：world 只追加 optional 字段。

> 调试入口（无须真链）：本地起带 flag 的 dev server + 连一个 Wallet-Standard 钱包；面板「挥镐(调试)」按钮触发同一条 `onchainSwing`。CDP 验收要点见 MEMORY「web3 QA automation mechanics」(qa-web3.mjs)：面板 gated on `NEXT_PUBLIC_ONCHAIN_MINE=1` 构建、`screen=game` 挂载、独立于 bevy、**无** `__mir2Stage5.onchain` 状态（读 DOM）、error div = `[style*=break-all]`。

## 相关 (Related)

- 源码：本文「入口在哪」表中 6 个 `lib/onchain-mine*` + `app/components/onchain-mine-panel.tsx` + page.tsx 区段。
- 链上侧：`onchain/deployments/testnet.json`（id 真值）、`apps/gateway/src/inject.rs`（Relayer 注入）、`apps/simulation/src/runtime/onchain.rs`（Sim 授予）。
- 登录/钱包复用：`apps/web/lib/client-login-runtime.ts`（`ActiveSuiWalletSession` / `connectSuiWalletForSigning` / `getActiveSuiWalletSession`）。
- 设计：DESIGN §3/§4（多退少补、攒满 N 次发 1 笔）、`onchain-sk/SK0-FINDINGS.md`（session key 模型）。
- 兄弟文档：`docs/client/*.md`（本目录其余「前置铺垫」文档）；索引见 `apps/web/CLAUDE.md`。
