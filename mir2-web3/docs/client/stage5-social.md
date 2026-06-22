# stage5 社交系统(组队/好友/交易/拍卖/羁绊)— client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

`world.stage5Systems` 是一组**松类型记录**(`Record<string, unknown>` / 数组),由 `page.tsx` 的
packet case 处理器**增量合并**而来——组队、好友/拉黑、交易、拍卖行/集市、羁绊(婚姻+师徒)、英雄/宠物、行会等。
这些记录故意不强类型,因为它们直接来自 gateway 的 camelCase JSON,且分多个 packet 拼装。
`lib/stage5-window-adapters.ts` 里的 `adapt*` 函数把这些记录**防御式**地翻译成各 Crystal UI 窗口要求的严格 prop 形状
(`readString/readNumber/readBool/asRecord` 逐字段校验、缺字段优雅降级),让窗口组件保持纯展示(presentation-only)。

The adapters are the seam: page state stays loose & merge-friendly, windows stay strict & dumb.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/lib/stage5-window-adapters.ts` | 全部 `adapt*` 适配器 + 防御读取原语 | `asRecord` :90, `readString` :94, `readNumber` :108, `readBool` :123, `classKeyFromUnknown` :149 |
| ↑ same | 组队/好友/交易/集市/羁绊适配器 | `adaptGroup` :317, `adaptFriends` :398, `adaptTrade` :728, `adaptMarketListings` :594, `adaptRelationship` :447, `adaptMentor` :462, `adaptBuffs` :842 |
| ↑ same | 货币标签(金币+城市声望币) | `CITY_CURRENCY_LABELS` :47, `currencyLabel` :54 |
| ↑ same | 入参类型(松→严的 between 形状) | `Stage5SystemsLike` :66, `RawGroupMember` :298, `RawFriendEntry` :380, `ActiveBuffLike` :767 |
| `apps/web/app/page.tsx` | 入站 packet→state 合并 (case 处理器) | `case "GroupMemberInfo"` :7566, `"FriendUpdate"` :7623, `"TradeItem"` :7727, `"NPCMarket"`/`"NPCMarketPage"` :8593, `"MentorUpdate"` :7650, `"LoverUpdate"` :7665 |
| ↑ same | 出站回调 (调 `send()` 发 BrowserCommand) | `addFriend` :5611, `groupInviteMember` :5732, `acceptTrade` :5839, `confirmTrade` :5843, `setTradeGold` :5934, `marketBuyListing` :5633, `proposeMarriage` :5656 |
| ↑ same | 窗口挂载点(把 adapter 结果喂给组件) | JSX props 区 :11243-11251 (`group=`, `friends=`, `bonds=`, `market=`, `trade=`) |
| ↑ same | BrowserCommand 发射器 | `send(command, { quiet })` :4026 |
| `apps/web/app/components/original-client-group-window.tsx` | 组队窗口(纯展示) | `GroupMember` :15, `GroupSummary` :30 |
| `apps/web/app/components/original-client-friends-window.tsx` | 好友/拉黑窗口 | `FriendEntry` :15, `FriendsSummary` :31 |
| `apps/web/app/components/original-client-trade-window.tsx` | 交易窗口 | `TradeItemSlot` :15, `TradeSummary` :37 |
| `apps/web/app/components/original-client-market-window.tsx` | 拍卖行/集市窗口 | `MarketListing` :19, `MarketMode` :~57 |
| `apps/web/app/components/original-client-bonds-window.tsx` | 羁绊(婚姻+师徒)窗口 | `RelationshipSummary` :15, `MentorSummary` :43 |

## 数据流 (How it threads the 5 layers)

### 入站 (server → screen),以好友为例
```
ServerPacket::FriendUpdate
  → gateway server_packet_to_event  (JSON, camelCase: { friends:[...], blocked:[...] })
  → page.tsx  case "FriendUpdate" (:7623)
        normalizeFriendList → 写入 stage5Systems.social.{friends, blocked, friendInfos, blockedInfos}
  → adaptFriends(world.stage5Systems.social) (:398)   ← 优先读 *Infos 富对象,退回裸名数组
  → <FriendsWindow social={…}/>  (page.tsx :11246)     ← 纯展示
```
其它系统同构,只是 case/adapter/slice 不同:

| 系统 | 入站 packet case (page.tsx) | 写入 slice | 适配器 |
|---|---|---|---|
| 组队 | `SwitchGroup` :7534 / `AddMember`+`DeleteMember` :7547 / `GroupMemberInfo` :7566 / `DeleteGroup` :7601 | `group.{members, memberInfos, lootMode, leaderName}` | `adaptGroup` |
| 好友 | `FriendUpdate` :7623 | `social.{friends, blocked, friendInfos, blockedInfos}` | `adaptFriends` |
| 羁绊-婚 | `LoverUpdate` :7665 | `relationship.{name, mapName, marriedDays}` | `adaptRelationship` |
| 羁绊-师 | `MentorUpdate` :7650 | `mentor.{name, level, online, menteeExp}` | `adaptMentor` |
| 交易 | `TradeRequest`/`TradeAccept` :7694 / `TradeGold` :7715 / `TradeItem` :7727 / `TradeConfirm` :7745 / `TradeCancel` :7754 | `trade.{partner, state, partnerGold, partnerItemCount, partnerItems, confirmed}` | `adaptTrade` |
| 集市 | `NPCMarket`/`NPCMarketPage` :8593 | `auction`(整列覆盖,非 merge) | `adaptMarketListings` |
| Buff | `AddBuff`/`RemoveBuff`/`PauseBuff` :6950 | `world.activeBuffs`(注意:**不在** stage5Systems) | `adaptBuffs` |

`MarriageRequest`/`DivorceRequest`/`MentorRequest`(:7679)与 `GroupInvite`(:7610)**只记日志**(`appendLog`),不写 state——它们是通知,不是状态。

### 出站 (click → server),以「确认交易」为例
```
TradeWindow onConfirm  → page.tsx confirmTrade() (:5843)
  → send({ type:"tradeConfirm", locked:true })  (:4026, WS JSON)
  → gateway web.rs: BrowserCommand::TradeConfirm → ClientPacket::TradeConfirm
  → simulation 处理
```
出站回调全是 `page.tsx` 里的具名 `function`,统一经 `send()` 发一个 `{ type: "<camelCase>" }` 对象。
注意几条**绕路**(没有专用 packet 时复用现成的):
- `groupInviteMember` (:5732):先 `switchGroup{allowGroup:true}`(quiet)再 `addMember`——AddMember 要求先开启组队。
- `groupToggleLootMode` (:5761) / `groupLeave` 的 fallback / `conquestStartWar`:走 `stage5Command{ action, args }` 通道(无专用 ClientPacket)。
- `whisperPlayer` (:5723):**不发包**,只 `setChatMessage("/<name> ")` 预填聊天框,正文由玩家敲(Chat 包的 `/name body` 路由成私聊)。
- `sendGuildChat`:走 Chat 包,`!~` 前缀服务端路由到行会频道。
- `proposeMarriage`/`toggleAllowMarriage`:服务端以**面朝的玩家**为目标,所以窗口传的 name 不进包(`_name` 被忽略)。

## 状态形状 (State shape)

`Stage5SystemsLike`(adapters.ts :66)是适配器读取的 slice 子集。所有 slice 可 `null`/缺省。

```ts
world.stage5Systems = {
  group?:        { members?: Array<string|RawGroupMember>; memberInfos?: ...; lootMode?: string; leaderName?: string } | null
  social?:       { friends?: Array<string|RawFriendEntry>; blocked?: ...; friendInfos?: ...; blockedInfos?: ... } | null
  relationship?: UnknownRecord | null   // { name|partnerName, mapName|partnerMap, marriedDays, allowMarriage, pendingRequestFrom }
  mentor?:       UnknownRecord | null   // { name, level, online, menteeExp, allowMentor, pendingRequestFrom }
  auction?:      Array<UnknownRecord> | null   // 集市/拍卖整列;NPCMarket 覆盖式写入
  trade?:        UnknownRecord | null   // { partner, state, partnerGold, partnerItemCount, partnerItems, confirmed } —— 见交易坑
  conquest?:     UnknownRecord | null
  guildTerritory?: UnknownRecord | null
  hero?:         UnknownRecord | null   // 见 hero-pet adapter
  intelligentCreatures?: Array<UnknownRecord> | null
  // 另有 guild(GuildStatus 写)/rankings 等不在 Stage5SystemsLike 里
}
```
- **双形状字段**:`group.members` / `social.friends|blocked` 同时接受 `string[]`(legacy 裸名)和富对象数组。后端富载荷落地前后都兼容。富对象进 `memberInfos`/`*Infos`,适配器**优先**读它(`adaptGroup` :333,`adaptFriends` :437),裸名为退路。
- 其它 React state(adapter 之外):`showGroup/showFriends/showTrade/showMarket/showBonds`(窗口开关)、`world.gold`、`world.cityCurrencies`(集市/交易货币钱包)、`setChatMessage`(whisper 预填)。
- 窗口的「己方」交易数据(`myGold`/`myItems`/`myConfirmed`)**不在** `trade` slice——由 page 从自身 `world.gold`/库存单独喂(见交易坑)。

## 坑 & 不变量 (Invariants & gotchas)

- **交易:Crystal 只推「对方」那侧,己方是客户端自己记的。** 这是协议本身的语义,不是没实现。`TradeGold`/`TradeItem` 只携带 partner 的金额/物品,服务端**无从得知**你自己摆了什么。证据见 gateway `web.rs` :5338-5362 注释,引 `Crystal/Server/MirObjects/PlayerObject.cs:10759,10776` 与 `Crystal/Client/MirScenes/GameScene.cs:6319,6325`。因此 `adaptTrade`(:728)**故意只映射 partner 字段**;`myGold`/`myLocked`/`myItems` 是 `TradeWindow` 的独立 props,page 从自身状态供给(`adaptTrade` 文档注释 :719-726 明说不映射己方)。改交易窗口时别去 trade slice 找己方数据——找不到。
- **`confirmed` 是对方按下确认,不是你。** `adaptTrade` 把 `partnerLocked`/`partnerConfirmed`/`confirmed` 都读成 `summary.confirmed`(:742),语义是 *partner* 锁定。己方确认在 `myConfirmed` prop。
- **`auction` 是覆盖写,不是 merge。** `NPCMarket`/`NPCMarketPage`(:8593)直接 `auction: listings` 整列替换;别期望增量累加。富载荷优先读 `payload.auctions`,退回 `payload.listings`。
- **Buff 不在 stage5Systems。** `adaptBuffs` 读的是 `world.activeBuffs`(顶层),case 是 `AddBuff/RemoveBuff/PauseBuff`(:6950)。别去 `stage5Systems.buffs` 找——没有。
- **`remainingMs` vs `remainingTicks` 单位不一致。** 富载荷发毫秒,legacy 发 server tick;`adaptBuffs`(:856)按 `BUFF_TICKS_PER_SECOND=10` 把 ms 换算成 tick。
- **好友删除靠下标,不靠名字。** stage5 social 没带 character index,`friendCharacterIndex`(:5596)按显示顺序(先 friends 后 blocked)推算下标,对齐 gateway 的 `stage5_friend_entries` 枚举。`RemoveFriend` 同时覆盖「删好友」与「解除拉黑」。顺序错位 = 删错人。
- **`leader` 是计算出来的,不是 server 字段。** `adaptGroup`(:346):有 `leaderName` 就按名字匹配,否则**默认第一行**是队长。
- **防御读取会吞坏数据。** `readString` 跳过空串、`readNumber` 跳过 NaN、`asRecord` 拒绝数组——缺名的条目被 `flatMap` 直接丢弃(组员/好友/listing/buff 皆然)。窗口看不到某条目时,先查它有没有合法 `name`。
- **`enrich` 回调不会覆盖规范字段。** `adaptGroup`/`adaptFriends` 的 `options.enrich` 只增补,`name` 和计算出的 `leader` 永远以 base 为准(:364)。
- **加字段必须 optional + additive。** slice 是松类型增量合并的;新字段一律可选、读不到就降级,绝不能破坏现有 `DisplayWorld`/窗口 props。每次改完跑 `npx tsc --noEmit`(必须 0)。

## 如何扩展 (How to extend / add to this area)

典型场景:让某社交窗口多显示一个后端新加的字段(例:好友的 `guildName`)。按序改:

1. **协议层** `packages/protocol/src/types.rs`:给对应 struct 加 optional 字段(`#[serde(default)]`),保持 camelCase 序列化。
2. **gateway** `apps/gateway/src/web.rs` `server_packet_to_event`:在对应 `ServerPacket::X` 分支把新字段塞进 `payload`(camelCase)。若是 Crystal 语义,引 `Crystal/...:line`。
3. **page.tsx case 处理器**:在写 slice 的 case(如 `FriendUpdate` :7623)里把新字段并进 `*Infos` 富对象。**别**动裸名数组(那是 legacy 兼容路径)。
4. **窗口 prop 类型** `original-client-<x>-window.tsx`:给 `FriendEntry`/`MarketListing`/… 加 optional 字段(`guildName?: string`)。
5. **adapter** `stage5-window-adapters.ts`:在对应 `adapt*` 里用 `readString/readNumber/readBool` 读出,**仅当存在时**赋值(`if (v !== undefined) base.x = v;`),保持 legacy 行像素级不变(参考 `adaptMarketListings` 富字段段 :621-640)。
6. **组件渲染**:在窗口里展示新字段(纯展示,不写逻辑)。
7. `npx tsc --noEmit`(0)+ `npm run test:frontend-logic`(覆盖 adapters)。

新增**出站动作**:在 page.tsx 写一个 `function fooBar()` 调 `send({ type:"fooBar", … })`(:4026 风格),在 gateway `web.rs` 加 `BrowserCommand::FooBar → ClientPacket::…` 映射,再把回调经 JSX props 传给窗口(:11243-11251 区)。无专用 ClientPacket 时用 `stage5Command{ action, args }` 兜底通道。

## 相关 (Related)

- 适配器:`apps/web/lib/stage5-window-adapters.ts`
- 中央客户端:`apps/web/app/page.tsx`(case 处理器 + 出站回调 + 窗口挂载)
- 窗口:`apps/web/app/components/original-client-{group,friends,trade,market,bonds}-window.tsx`
- gateway 桥:`apps/gateway/src/web.rs`(`server_packet_to_event` 入站、`BrowserCommand→ClientPacket` 出站)
- 协议:`packages/protocol/src/{packets.rs,types.rs}`
- Crystal 权威(交易语义):`Crystal/Server/MirObjects/PlayerObject.cs:10705,10741-10742,10759,10776`、`Crystal/Client/MirScenes/GameScene.cs:6314,6319,6325`
- 测试:`apps/web/scripts/test-stage5-adapters.mjs`(`npm run test:frontend-logic`)
- 架构总览:`docs/ARCHITECTURE-CURRENT.md`;前端完成度:`docs/FRONTEND-COMPLETENESS-AUDIT.md`
