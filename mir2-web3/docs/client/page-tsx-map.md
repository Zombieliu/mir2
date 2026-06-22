# page.tsx 主控分区地图 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

`apps/web/app/page.tsx` (~12.7k 行) 是整个 web 客户端的**单文件中枢**：一个
`export default function HomePage()` (起始 1319) 容纳了 WebSocket 生命周期、入站
`ServerPacket` 的约 280-case 大 switch（switch 体 6422–8990 内 `case "…"` ≈279；全文件
`grep -c 'case "'` ≈347，多出的在 `packetMovementAnimation` 等模块级纯函数里）、所有
world / stage5 / questLog 状态、移动预测
引擎、以及 JSX 渲染（登录/选角/游戏三屏 + 所有窗口挂载）。

它是 5 层数据流的中间两层：把 gateway 发来的 camelCase JSON 事件 merge 进 world
状态，再把状态喂给 `lib/stage5-window-adapters.ts` adapter + `components/original-client-*-window.tsx`
组件渲染；反方向把玩家操作打成 `BrowserCommand` (`send({type:...})`) 发回 gateway。
This file is a presentation host + packet reducer; almost all heavy rendering is delegated
out to `OriginalClientShell` and the window components — **never** add rendering markup here.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `app/page.tsx` | 主组件 + 全部状态/refs | `HomePage()` page.tsx:1319 |
| `app/page.tsx` | 入站事件分派器（switch 外壳） | `handleGatewayEvent(event)` page.tsx:6346 |
| `app/page.tsx` | **~280-case ServerPacket 大 switch**（switch 体内 ≈279;全文件 `grep` ≈347,见下） | `switch (event.packet)` page.tsx:6422 |
| `app/page.tsx` | 出站命令发送器 | `send(command, options?)` page.tsx:4026 |
| `app/page.tsx` | WS 连接 + open/close/error/onmessage | `connectGateway()` page.tsx:4518；`new WebSocket(...)` page.tsx:4531；onmessage → `handleGatewayEvent(JSON.parse(...))` page.tsx:4605 |
| `app/page.tsx` | world 状态合并（rAF 批量 setWorld） | `updateWorld` (useCallback) page.tsx:1436 |
| `app/page.tsx` | 主渲染（三屏 + 窗口挂载） | `return (` page.tsx:11104；`<OriginalClientShell>` 11107；`<ExtraWindows>` 11240 |

> 第二个 `switch` (`packetMovementAnimation`, page.tsx:11525) **不是** 协议分派——它是
> 模块级纯函数，按移动包名返回 walking/running 动画，给 `withPacketMovementAnimation`
> (11501) 用。别和 6422 的大 switch 混淆。

## 文件块地图 (Block map — jump straight here)

行号为近似，已逐一打开确认。

| 行段 Lines | 块 Block | 委托给 Delegates to |
|---|---|---|
| 1–138 | imports（lib/* 与 components/* 全在此） | 见下「相关」 |
| 140–146 | `OriginalClientShell` 动态 import（`ssr:false`） | `app/original-client-shell.tsx` |
| 148–812 | 模块级 type/const：`WorldState`(673)、`GatewayWorldSnapshot`(417)、`WorldEntity`(465)、`Stage5SystemsState`(351)、`VIEWPORT_*` / `CRYSTAL_*_MS` 移动常量(873–905) | — |
| 814–872 | `DEFAULT_WORLD_STATE` (818) | — |
| 1319–1435 | **state/refs**（见「状态形状」）：socket/world/runtime refs (1320–1421)，移动预测 refs (1359–1421)，React `useState` (1423–1644) | — |
| 1436–1452 | `updateWorld`：写 `worldRef`+`worldStoreRef`，rAF 合批 `setWorld` | `lib/world-model` |
| 1453–1704 | 更多 useState（窗口开关 `show*` 1480–1503；`onchain*`；inventory/character tab 1635–1637） | — |
| 1504–1816 | 早期 useEffect：语言/runtime boot 同步、**热键监听** `onExtraWindowHotkey` (1586，`window.addEventListener("keydown")` 1608) | — |
| 1765–1815 | 派生 selector：`self` (1765)、`predictedSelf` (1783)、`displayEntities` (1802)、`selectedEntity` (1813) | — |
| 1816–3401 | 渲染相关 useEffect + scene/sprite 派生（runtime 喂数据、Bevy atlas 上传、scene 资产就绪） | `original-client-scene-map-rendering`, runtime WASM |
| 2745–3354 | scene/视口构建 helpers（含多处 `return null` 守卫） | `lib/scene-types` |
| 3355–3401 | 视口 selector：`sortedEntities` (3355)、`viewportEntities` (3371)、`viewportTiles` (3387) | — |
| 3402–4025 | 资产/重连/runtime useEffect + `appendLog` (3986) 等日志 helper | `lib/debug-snapshot` |
| 4026–4530 | **出站**：`send` (4026)、`sendGatewayTick` (4092)、重连序列 `sendGatewayReconnectSequence` (4436) | `lib/client-login-runtime` |
| 4518–4612 | `connectGateway` + socket 四事件（open 4535 / close 4575 / error 4595 / message 4605） | — |
| 4612–4798 | 登录/账号 useEffect + `createAccount` (4618)、`submitLogin` (4632)、`startSelectedCharacter` (4694) | `lib/client-login-runtime` |
| 4799–5238 | 移动/朝向出站：`sendCrystalTurn` (4799)、`moveToTile` (4948)、`attackTarget` (4959)、`harvestToward` (4965) | `original-client-movement-controller` |
| 5239–5468 | 角色/物品命令：`createCharacter` (5239)、`useItem`(5258)/`dropItem`(5275)/`equipItem`(5285)/`moveItem`(5315)/`storeItem`(5375)/`sellItem`(5420) | inventory/character windows |
| 5453–5548 | 技能施放：`sendMagicSkill` (5453)、`castSkillAtTile` (5470)、`castSkill` (5476)、`sendClientCommand` (5549) | — |
| 5516–5973 | 社交/系统命令：`transferMap`(5516)、`claimMail`(5522)、`runStage5Command`(5545)、`requestRanking`(5587)、`addFriend`(5611)、`marketBuyListing`(5633)、`proposeMarriage`(5656)、`inviteGuildMember`(5706)、`groupInviteMember`(5732)、`summonHero`(5786)、`acceptTrade`(5839)、`sendMailMessage`(5949) | stage5 windows + adapters |
| 5974–6345 | 视口交互入口：`pickGroundDrop`(5974)、`selectEntity`(5992)、`activateEntity`(5999)、`handleViewportTileAction`(6045)、`handleViewportTileStepAction`(6090)、`handleViewportDirectionStep`(6141)、`handleViewportDirectionIntent`(6156)、`handleViewportDirectionStop`(6174) | — |
| **6346–6421** | `handleGatewayEvent` 外壳：去重日志、movement-packet 记录、`recordDebugEvent` | `lib/debug-snapshot` |
| **6422–8990** | **ServerPacket 大 switch**（按域分组见下表） | per-domain helpers + adapters |
| 9089–9610 | 每包 world 变异器：`updateWorldEntityFromLocationPacket`(9089)、`markWorldEntityAttack/Magic/Struck/Dead/Revived`(9122–9374)、`pushDamageFloater`(9192)、buff/health/mana appliers (9424–9565)、`applyRankingPacket`(9565) | `lib/game-events`(VFX/sound) |
| 9610–10025 | `applyGatewayWorldSnapshot` (9610) — 全量快照重建 world | — |
| 10026–11103 | **移动预测/和解引擎**：`reconcileMovementPlanWithServer`(10026)、direction-step 队列与校正、`crystalMovement*` 寻路 (10531–10934)、`clearPredictedPlayer*` (10980+) | `original-client-movement-controller` |
| **11104–11329** | **主 JSX**：`<OriginalClientShell>`(11107) + `<ExtraWindows>`(11240) + `DeathReviveOverlay` + tutorial + `OnchainMinePanel`(11280) | shell / ExtraWindows / overlays |
| 11331–11403 | `DeathReviveOverlay` 组件 | — |
| 11404–12694 | 模块级纯 helper：quest 解析(`adaptMailMessages` 11404 / `parseQuestObjectives` 11428…)、entity list upsert(`upsertEntityInList` 11461)、`packetMovementAnimation`(11525)、log/chat helper(`gatewayChatChannel` 11756)、`equipmentSlotIndex/FromIndex`(11849/11882)、sprite-from-packet 工厂 (`spriteFromPacket` 12021…12247)、`crystalMovement*` 寻路 helper(12452+)、character 解析(`parseCharacters` 12599) | window components |

## ServerPacket switch — 按域分组 (the 6422–8990 switch, grouped)

`grep -n 'case "' page.tsx` 在 6423–8990 段约 280+ case。按域：

| 域 Domain | 大致行段 | 代表 case | 落点 Lands in |
|---|---|---|---|
| 连接/登录/选角/系统 | 6423–6578, 7127–7186, 8245–8252, 8488–8508, 8982 | Connected, Login, LoginSuccess(6532), StartGame(6557), LogOutSuccess(7127), ReturnToLogin(8245), ChangePassword | `screen` 切换、`characters`、重连 refs |
| 世界/场景 | 6579–6674, 8156–8211, 8748–8770, 8877 | MapInformation(6579), UserInformation(6614), MapChanged(8156), SetCompass(8198), TimeOfDay(8757), NewMapInfo(8877) | `world.map*`、scene 重载、`applyGatewayWorldSnapshot` |
| 移动 | 6674–6800 | UserLocation/Pushed/ObjectWalk/ObjectRun/ObjectDash…(共一个大 case 体 6688) | `updateWorldEntityFromLocationPacket`(9089) + 预测和解 |
| 实体生命周期 | 6801–6946 | ObjectPlayer(6801), ObjectMonster(6810), ObjectNpc(6816), ObjectRemove(6819), ObjectItem(6825), ObjectGold(6828), MineNodeState(6899) | upsert/patch entity list, ground drops |
| 战斗/魔法/VFX | 6851–6946, 7220–7335, 8077–8095 | ObjectAttack(6851), ObjectStruck(6859), Magic(6873), ObjectMagic(6885), RangeAttack(7220), Poisoned(7332), DamageIndicator(8077), ObjectEffect(8089) | `markWorldEntity*`、`pushDamageFloater`、`spawnRangeProjectile`、`lib/game-events` |
| 状态/buff/数值 | 6950–6975, 7258–7332, 8812–8837 | AddBuff(6950), ObjectHealth(6969), GainExperience(7258), LevelChanged(7268), HealthChanged(7281), SetConcentration(8812), SetElemental(8824) | buff appliers (9424+)、health/mana appliers (9489+) |
| 物品/背包 | 6975–7024, 7370–7533, 8212–8335, 8624–8658 | UseItem(6975), DuraChanged(6999), GainedItem(7370), RefreshItem(7402), DeleteItem(7430), EquipItem/MoveItem/…(8212), SplitItem(8281), Awakening(8325), MergeItem(8783) | `consumePacketItem`、`patchItemsByUniqueId`(extended-server-packets)、inventory state |
| 仓库/NPC 商店 | 7038–7127, 7866–7905, 8576–8623, 8892–8932 | UserStorage(7049), NPCGoods(7866), NPCResponse(7877), NPCMarket(8593), GameShopStock(8892), NPCRequestInput(8922) | `world.stage5Systems`、storage/NPC dialog state |
| 聊天/系统消息 | 7024–7037, 7898–7922 | Chat(7024), ObjectChat(7031), SendOutputMessage(7898), Roll(7905), OpenBrowser(7916) | `appendLog` + `gatewayChatChannel/Tone` (11756/11795) |
| 社交/stage5 | 7534–7864, 8003–8027, 8443–8576, 8610–8720, 8937–8971 | SwitchGroup(7534), GroupMemberInfo(7566), FriendUpdate(7623), LoverUpdate(7665), TradeRequest(7694), TradeItem(7727), ReceiveMail(7763), GuildStatus(7953), GuildMemberChange(7966), GuildBuffList(8707), GuildStorageList(8937) | `world.stage5Systems.{group,social,trade,auction,mail,guild,…}` → adapters |
| 英雄/宠物/灵物 | 7234–7258, 7786–7841, 8325–8390, 8468–8482, 8658–8684 | MountUpdate(7234), NewHero(7786), ChangeHero(7803), NewIntelligentCreature(7827), HeroInformation(8335), HeroBaseStatsInfo(8468) | `stage5Systems.{hero,intelligentCreatures}` |
| 任务 | 8095–8156, 8391–8437 | CompleteQuest(8095), ChangeQuest(8114), ShareQuest(8143), NewQuestInfo(8391) | `world.questLog` |
| 颜色/名字/外观/杂项 | 7905–7945, 8027–8077, 8804–8867 | ChangeAMode(7923), ColourChanged(8027), ObjectName(8051), ObjectLeveled(8066), PlayerUpdate(8804), Opendoor(8848), UpdateNotice(8857) | entity 字段、`appendLog` |

> 同一行多个 `case ...:` fall-through 共享一个 body，分组里只列锚点 case。改单个 handler
> 时**先 `grep -n 'case "X"'` 定位**，别按行号猜——switch 很长、行号会随编辑漂移。

## 数据流 (How it threads the 5 layers)

**入站 (一个 ObjectStruck 飘血字为例):**
1. sim 发 `ServerPacket::ObjectStruck` → gateway `server_packet_to_event`（`apps/gateway/src/web.rs`）转成 camelCase JSON `{type:"packet", packet:"ObjectStruck", ...}`。
2. socket onmessage (page.tsx:4605) → `handleGatewayEvent` (6346) → `switch` 命中 `case "ObjectStruck"` (6859)。
3. body 调 `markWorldEntityStruck(payload)` (9270) → `markEntityStruckFlash` + 经 `gameBusRef`/`pushDamageFloater` 把飘字推进 `world` + VFX/音效订阅（`lib/game-events`）。
4. `updateWorld` 写 `worldRef`，rAF 合批 `setWorld` → 触发渲染。
5. 渲染由 `OriginalClientShell` / overlay 层消费 `world.damageFloaters`。

**入站 (一个 stage5 社交包为例):** `case "GroupMemberInfo"` (7566) merge 进
`world.stage5Systems.group` → JSX 里 `adaptGroup(world.stage5Systems.group)` (page.tsx:11245,
来自 `lib/stage5-window-adapters.ts`) → `<ExtraWindows group={...}>` → `original-client-group-window.tsx` 渲染。

**出站 (玩家点格子走路):** `onViewportTileClick` (11208) → `handleViewportTileAction` (6045)
→ `moveToTile` (4948) → `queueCrystalMoveIntent({kind:"target",...})` (4790)（**不直接发包**，
入移动预测队列）→ 后续 `trySendQueuedCrystalMove` 把目标格转成单步朝向 →
`send({type: mode==="run"?"run":"walk", direction})` (4937 via `send` 4026) → gateway `web.rs`
`BrowserCommand → ClientPacket` (`Walk`/`Run`) → sim。出站命令对象一律是松散的 `{type, ...}`
record（走路用 `walk`/`run`+direction，没有 `moveTo` 包），gateway 负责翻成 `ClientPacket`。

## 状态形状 (State shape)

**`world: WorldState`**（type 定义 page.tsx:673，默认 818）——单一真相源，存在
`worldRef.current`，经 `updateWorld` 改、rAF 后镜像到 `world` React state：
- `entities: WorldEntity[]`、`playerObjectId`、`groundDrops`、`worldItems`、`equipment`
- `gold`、`cityCurrencies`、`mapTitle`/`mapFileName`、`mapTransfers`、`activeBuffs`
- `knownSkills: KnownSkill[]`、`questLog: QuestEntry[]`、`damageFloaters`、`projectiles`
- `rankings` / `rankingCurrentKey`
- **`stage5Systems: Stage5SystemsState`**（type page.tsx:351）：`{group, social, trade, auction, mail, guild, guildTerritory, conquest, relationship, mentor, hero, intelligentCreatures, …}` —— 松散 record，由 adapter 防御式读取。

**关键本地 React state:** `screen`(1428 `"login"|"select"|"game"`)、`characters`、
`selectedCharacterIndex`、`wsState`、`reconnectStatus`、窗口开关 `show*`(1480–1503)、
`activeInventoryTab`(1635)、`activeCharacterTab`(1636)、`predictedPlayerPosition`(1638)、
`onchain*`(1462–1476)、`logs`(1453)。

**移动预测 refs**（不进 React state、避免重渲染）：`movementPlanRef`、`pendingSelfMoveRef`、
`predictedPlayerPositionRef`、`directionStepPending*Ref`、`movementBlockedStepsRef` 等
(1359–1421)。详见 docs/client 移动文档（若有）+ `original-client-movement-controller.tsx`。

## 坑 & 不变量 (Invariants & gotchas)

- **`world` 改动必须走 `updateWorld` (1436)**，它同时写 `worldRef.current`（同步真相）+ `worldStoreRef` + rAF 合批 `setWorld`。直接 `setWorld(...)` 会让 `worldRef` 与 React state 脱节，下一个 packet handler 读到旧 `worldRef` 就丢更新。
- **packet handler 读 `worldRef.current` 不读 `world`**：同一 microtask 内多个 packet 连续到达，React `world` 还没 flush；务必用 ref 读最新态。
- **case fall-through 共享 body**：很多行是 `case "A":\n case "B": {`，改 A 会影响 B。先 grep 确认锚点。
- **新增字段一律 optional + 向后兼容**（CLAUDE.md 约定）：别改 `WorldState`/`GatewayWorldSnapshot` 既有字段的类型/必填性，否则 break `DisplayWorld` 与现有 consumer。
- **窗口组件是 presentation-only**：业务逻辑留在 page.tsx 的 action handler + adapter；窗口里别直接 `send(...)`，要经 props 回调（见 `<ExtraWindows>` 11240 的 `on*` 回调）。
- **隐藏 tab 暂停 rAF**：`updateWorld` 靠 `requestAnimationFrame` flush；后台标签页 rAF 不跑 → `setWorld` 永不触发（QA 验证时会看到「Loading map…」/黑底）。见 MEMORY 的 mir2-chrome-mcp-verify-gotchas。
- **出站走 gateway 桥，不是 1:1 ClientPacket**：`send({type:"X"})` 的 `type` 是 `BrowserCommand`，由 `apps/gateway/src/web.rs` 翻成 `ClientPacket`。有的命令没有对应 ClientPacket（如 conquest gate/tax、hero dismiss）——属协议缺口，不是没接。
- **gateway 只推交易对家的一侧**：自己 offer 由客户端本地记（CLAUDE.md）——`adaptTrade` 里 partner 字段才有数据。
- **二号 switch 是动画纯函数**：`packetMovementAnimation` (11525) 模块级、无副作用；别往里塞协议处理。

## 如何扩展 (How to extend / add to this area)

**新增一个 ServerPacket handler:**
1. 协议层：`packages/protocol/src/packets.rs` 加 `ServerPacket` 变体（+ `src/types.rs` 结构体）。
2. gateway：`apps/gateway/src/web.rs` `server_packet_to_event` 加分支，emit camelCase JSON。
3. page.tsx：在 `switch (event.packet)` (6422) 按域插 `case "NewPacket":`（fall-through 复用就并到锚点 case）。body 调 `updateWorld` 或一个新的 `apply*/mark*` 变异器（仿 9089–9610 区那批）。
4. 若进 stage5：merge 进 `world.stage5Systems.<system>`，再在 `lib/stage5-window-adapters.ts` 写 `adaptX`，在窗口组件里渲染。

**新增一个出站玩家操作:**
1. 写 action handler（仿 5258–5973 区），内部 `send({type:"newCmd", ...})`。
2. gateway `web.rs` 加 `BrowserCommand→ClientPacket` 映射；sim 处理该 ClientPacket。
3. 把 handler 经 props 传给窗口/shell（`<OriginalClientShell on...>` 11160+ 或 `<ExtraWindows>` 11240），UI 里调回调——别在组件内直接发包。

**新增一个窗口:**
1. `useState` 加 `showXxx`(仿 1480–1503)；热键在 `onExtraWindowHotkey` (1586) 注册。
2. 组件放 `components/original-client-xxx-window.tsx`（presentation-only）。
3. 在 `<ExtraWindows>` (11240) 挂载，props 用 `adaptXxx(world.stage5Systems.xxx)` + `on*` 回调。

## 相关 (Related)

- `apps/web/CLAUDE.md` — 客户端总览 + 加功能配方索引。
- `apps/web/lib/stage5-window-adapters.ts` — 防御式 adapter（`adaptGroup/Friends/Trade/Market/Mail/Guild/…`）。
- `apps/web/app/original-client-shell.tsx` — 三屏 + 视口 + inventory/character 渲染宿主。
- `apps/web/app/components/original-client-movement-controller.tsx` — 移动预测/和解纯逻辑。
- `apps/web/lib/world-model.ts` — `createWorldStore` / `createSnapshotEmitter`。
- `apps/web/lib/game-events.ts` — VFX/音效事件总线（`createGameEventBus`）。
- `apps/web/lib/extended-server-packets.ts` — 物品/社交 packet 归一化 helper。
- `apps/gateway/src/web.rs` — `server_packet_to_event`（入站）+ `BrowserCommand→ClientPacket`（出站）。
- `packages/protocol/src/packets.rs` / `src/types.rs` — `ServerPacket`/`ClientPacket` 权威定义。
