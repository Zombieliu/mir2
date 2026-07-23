# 物品 / 背包 / 装备 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

背包窗口（两个 bag tab + quest/storage tab）、腰带（belt）、人物装备槽位的 UI、
状态、以及全部物品动作（use / equip / drop / split / sell / move / merge / 仓库存取）。
This area owns the inventory window (two bag tabs + a quest/storage tab), the belt
dialog, the equipment slots inside the character window, and every item action. All
item state lives in `WorldState` on `page.tsx` as four parallel `WorldItem[]`/
`EquipmentItem[]` arrays (`inventoryItems` / `beltItems` / `storageItems` /
`equipmentItems`); the windows are presentation-only and read those arrays via the
`DisplayWorld` snapshot. There is **no `stage5Systems` record for items** — items are a
first-class top-level slice of world state, not a loosely-typed stage5 adapter record.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/components/original-client-inventory-window.tsx` | 背包+仓库主窗口、所有「pending mode」交互（move/sell/delete/split/store/takeBack）、点击派发 | `InventoryWindow` (:68), `activateInventoryItem` (:322), `runContextAction` (:503), `confirmSellItem`/`confirmDeleteItem`/`confirmSplitItem` (:415/:439/:460) |
| `apps/web/app/components/original-client-inventory-action-panels.tsx` | 删除/出售/拆分/丢金 弹窗 + 类型过滤器 + 右键菜单 + 工具条 | `InventoryContextMenu` (:475), `InventoryToolbar` (:331), `inventoryItemFilterFor` (:304), `InventorySplitPanel` (:63) |
| `apps/web/app/components/original-client-inventory-utils.ts` | 图标路径、key→装备槽 启发式、BinaryDateTime 解析 | `originalItemIconPath` (:3), `equipmentSlotForItemKey` (:37), `formatBinaryDateTimeLabel` (:7) |
| `apps/web/app/components/original-client-item-tooltip.tsx` | 物品悬浮提示（grade 配色 + 属性行 + 需求 + 绑定/封印） | `OriginalItemTooltip` (:96), `buildStatRows` (:267), `GRADE_COLOUR` (:80) |
| `apps/web/app/components/original-client-panels.tsx` | 腰带窗口（横/竖两种布局，仅 use 动作） | `BeltDialog` (:366) |
| `apps/web/app/components/original-client-character-window.tsx` | 人物装备槽渲染 + 点击卸下 | equipment slot map (:253), `onRemoveItem` call (:270), `equipmentSlotFromLabel` (:483) |
| `apps/web/app/page.tsx` | 状态真源 + 出站 BrowserCommand + 入站 packet 合并 + snapshot→DisplayWorld | `useItem`/`equipItem`/`moveItem`/`mergeItem`/`storeItem`/`removeItem` (:5258–:5440), snapshot map (:9660–:9706), packet cases (:6975–:7491) |
| `apps/web/lib/original-ui.ts` | 槽位坐标 + sprite 路径 | `inventory.slots` (:322), `storage.slots` (:327), `game.belt.slots` (:120), `character.equipmentSlots` (:355) |

类型集中在 `apps/web/app/components/original-client-types.ts`：`DisplayItem` (:155),
`DisplayEquipmentItem` (:173), `ItemContainer` (:4), `EquipmentSlot` (:12),
`ItemActionRef`/`MoveItemRef`/`MergeItemRef` (:168–:171)。
回调签名在 `original-client-shell-types.ts:186–201`（`onUseItem`…`onSpecialRepairItem`）。

## 数据流 (How it threads the 5 layers)

### 出站 (player action → server) — e.g. 使用一个物品

1. UI 点击：`InventoryWindow.activateInventoryItem` (inventory-window.tsx:322)。
   先按当前 pending-mode 分流（store/takeBack/delete/sell/move）；正常模式下用
   `equipmentSlotForItemKey(item.key)` (utils.ts:37) 判定 **装备还是使用**，调
   `onEquipItem(ref, slot)` 或 `onUseItem(ref)`。腰带物品走 `BeltDialog` (panels.tsx:366)
   的 `useBeltItem` → `onUseItem`。
2. `page.tsx` 回调 `useItem` (page.tsx:5258) 发 `send({ type: "useItem", uniqueId, slot,
   grid })`。**grid 由 container 推导**：`belt`→`"belt"`、`quest`→`"questInventory"`、
   其余→`"inventory"`（page.tsx:5271；move/merge/split 多一个 `storage`→`"storage"`）。
3. 网关 `apps/gateway/src/web.rs` `BrowserCommand::UseItem` (web.rs:2705) →
   `ClientPacket::UseItem { unique_id, grid }`，grid 字符串经 `parse_grid` (web.rs:2714)
   转成协议枚举。其它命令同段：`MoveItem`/`MergeItem`/`EquipItem`/`RemoveItem`/
   `SplitItem`/`StoreItem`/`TakeBackItem` (web.rs:2727–:2792)。
4. simulation 处理后回 `ServerPacket`（UseItem/DuraChanged/DeleteItem/StoreItem…）。

### 入站 (ServerPacket → UI)

1. 网关 `server_packet_to_event` 把每个 `ServerPacket` 序列化成 camelCase JSON
   （如 `ServerPacket::StoreItem` → web.rs:4533）。
2. `page.tsx` 的 packet 大 switch 按 `case "X"` 合并进 `WorldState`：
   - `case "UseItem"` (page.tsx:6975)：`success===true` 时对 inventory/belt/storage 三个
     数组各跑 `consumePacketItem(arr, grid, uniqueId, 1)` 扣 1。
   - `case "DeleteItem"`/`"SellItem"`/`"DropItem"` (page.tsx:7430/:7444/:6988)：
     `removeItemByUniqueId`。
   - `case "DuraChanged"` (page.tsx:6999)：按 uniqueId 改耐久；**注意它把
     uniqueId 同时当作装备槽 index**（`equipmentSlotFromIndex(uniqueId)`，page.tsx:7003）。
   - `case "RefreshItem"`/`"ItemUpgraded"`/`"ItemRepaired"` (page.tsx:7402/:7466)：
     `patchItemsByUniqueId` 改数量/耐久。
   - `case "UserStorage"` (page.tsx:7049)：整页仓库重建（按 slot 合并旧 key/name）。
3. **整张世界快照**（`worldSnapshot` 事件）走另一条路：`page.tsx:9660–9706` 把
   `snapshot.{inventoryItems,beltItems,storageItems,equipmentItems}` 整体 map 成
   `WorldItem[]`，**直接覆盖**这四个数组。增量 packet 只是在两次快照之间做乐观更新。
4. 渲染：`InventoryWindow`/`BeltDialog`/character window 从 `DisplayWorld`
   （= `WorldState`，经 page.tsx:4356 透传）读 `world.inventoryItems` 等。
   **没有 stage5 adapter 这一跳** —— 物品窗口直接吃 `world.*Items`。

## 状态形状 (State shape)

`WorldState`（page.tsx:673；以 `DisplayWorld` 别名透传给窗口）的物品相关键：

- `inventoryItems: WorldItem[]` — 两个背包合一，按 `container: "bag1" | "bag2"` 区分。
  窗口用 `world.inventoryItems.filter(i => i.container === activeTab)` 取当前 tab
  （inventory-window.tsx:107）。
- `beltItems: WorldItem[]` — `container: "belt"`，slot 0..5 对应腰带 6 格
  （original-ui.ts:120）。
- `storageItems: WorldItem[]` — `container: "storage"`，slot 是 0..159 的绝对槽位；
  窗口按 `storagePageIndex*80` 分两页（inventory-window.tsx:108–113）。
- `equipmentItems: EquipmentItem[]` — 形状不同：`{ slot: EquipmentSlot, name, icon,
  shape?, description, durabilityCurrent, durabilityMax, attack, defence }`
  （types.ts:173 `DisplayEquipmentItem`）。character window 建 `equipmentBySlot` Map
  按 `EquipmentSlot` 取（character-window.tsx:162）。
- 容量/重量标量：`currentWeight` `maxWeight` `freeBagSlots` `maxBagSlots`
  `storageSize` `hasExpandedStorage`；仓库密码：`hasStoragePassword`
  `requireStoragePassword` `storageSessionUnlocked` `storagePasswordLastSetBinaryDatetime`
  `expandedStorageExpiryTimeBinaryDatetime`（page.tsx:690–700）。
- `gold: number` —— 背包窗口右下角金币 + 丢金弹窗读它。

`WorldItem`（page.tsx:535 / world-model/types.ts:160）：
`{ key, name, icon, uniqueId, slot, container, quantity, description, durabilityCurrent?, durabilityMax? }`。

`InventoryWindow` 内部本地 React state（**纯 UI 模式，不进 world**）：
`deleteMode` `sellMode` `storageMode("store"|"takeBack"|null)` `storagePageIndex`
`pendingMoveItem` `pendingSplitItem` `pendingDeleteItem` `pendingSellItem`
`pendingGoldDrop` `itemFilter` `contextMenu` `dragOverSlot` `deleteFeedback`
（inventory-window.tsx:93–122）。这些是「先选一个，再点目标槽」的两步交互的暂存。

## 坑 & 不变量 (Invariants & gotchas)

- **auto-belt 坑（最容易踩）**：Crystal 的 `AddItem` 会把药水类自动放进腰带，所以一个
  奖励（如 quest 给的小红药）会落到 `world.beltItems`（key 如 `crystal-item-658`，
  qty 1）而**不是** `inventoryItems`/`React state`。验收物品到账时必须查 belt+bag+WS
  真值，别只看 `inventoryItems`。（来源：PR #143，纠正过一次 QA false-negative。）
- **template id ≠ unique id**：`icon`（int）是模板/外观 id，用来取
  `/original-ui/Items/{icon}.png`（utils.ts:3）；`uniqueId` 是该物品实例的服务器引用，
  所有动作（use/drop/sell/equip/move/merge）发的是 **uniqueId**。两者不要混。
- **uniqueId 可能缺**：snapshot 里 `uniqueId` 是可选的；缺失时客户端按
  `itemClientReference` (page.tsx:11960) **合成**一个：bag2 用 `40 + slot`，其余用
  `slot`。snapshot map 也内联了同样的 fallback（page.tsx:9664/:9676）。增量 packet 的
  `itemMatchesPacketGrid` (page.tsx:11972) 必须用同一个合成规则，否则匹配不上。
- **grid 字符串是协议契约**：`belt`/`questInventory`/`storage`/`inventory` 四个值由
  container 推导（page.tsx:5271 等）。bag1+bag2 都映射成 `"inventory"`——服务器侧背包
  是一个连续网格，bag2 只是客户端的视觉第二页（slot ≥ 某偏移）。
- **装备判定是 key 正则启发式**，不是服务器 `ItemType`：`equipmentSlotForItemKey`
  (utils.ts:37) 靠物品 key 里的英文词（Sword/Armour/Ring…）猜槽位。改了物品命名或加新
  装备类别要同步这张正则表，否则新装备会被当成「可使用」走 `onUseItem`。同理类型过滤器
  `inventoryItemFilterFor` (action-panels.tsx:304) 也是 key+name 正则。
- **`DuraChanged` 复用 uniqueId 当装备槽 index**：page.tsx:7003 把 `uniqueId` 喂给
  `equipmentSlotFromIndex`（0=weapon…13=mount，page.tsx:11882）。装备耐久变化时服务器
  发的是槽位 index 而非物品 uniqueId——这是 Crystal 协议本身的双语义，别「修正」成统一 id。
- **卸装备 = removeItem 到一个空背包格**：character window 点装备直接
  `onRemoveItem({slot})`（character-window.tsx:270），`page.tsx:removeItem` (5299) 自己
  算第一个空 bag1 槽当 `to`，发 `removeItem{ uniqueId: equipmentSlotIndex(slot), grid:"inventory", to }`。
  注意这里 `uniqueId` 字段塞的是**装备槽 index**，不是物品实例 id。
- **storage 第二页 / 仓库锁**：page2（slot 80..159）在 `!hasExpandedStorage` 时锁住
  （`storagePageLocked`，inventory-window.tsx:110）；整个仓库在
  `storageProtectionEnabled && !storageSessionUnlocked` 时锁（:106）。锁态下点击 no-op。
- **乐观更新 vs 快照**：增量 packet 做即时扣减，但任何 `worldSnapshot` 会整体覆盖四个
  数组（page.tsx:9660+）。如果某个 packet 没被处理，UI 会在下一次快照「自愈」——所以
  调试「物品不更新」要同时看 packet case 是否命中 **和** 快照里有没有该物品。
- **belt 只能 use**：`BeltDialog`（panels.tsx:366）只暴露 `onUseItem`；拖动/装备/拆分腰带
  物品要回到背包窗口。

## 如何扩展 (How to extend / add to this area)

加一个典型的新物品动作（以「拆分到指定槽」之类的新按钮为例），按此顺序、遵守
additive/optional 规则：

1. **协议层** `packages/protocol/src/packets.rs`：给 `ClientPacket`（出站）/`ServerPacket`
   （入站结果）加新变体，字段尽量与 Crystal 1:1（引 `Crystal/` 的 `file:line`）。
2. **simulation** `apps/simulation/`：实现处理逻辑（容量/绑定/封印校验等），回发结果
   ServerPacket。
3. **网关出站** `apps/gateway/src/web.rs`：在 `BrowserCommand` 枚举加变体（~line 761 区域
   是 StoreItem 等的样板），在 `browser_command_to_action` 的 match（~line 2705，函数定义
   :2570）映射成 `ClientPacket`（包成 `SessionAction::Packet`）；grid 字符串用现成的 `parse_grid`。
4. **网关入站** `apps/gateway/src/web.rs` `server_packet_to_event`（~line 4524 区域）：把新
   `ServerPacket` 序列化成 camelCase JSON。
5. **page.tsx 出站回调**：在物品命令区（page.tsx:5258–5440）加一个 `function fooItem(...)
   { send({ type: "foo", ... }) }`，grid 用现有 container→grid 推导写法。
6. **page.tsx 入站 case**：在 packet switch（~page.tsx:6975 起）加 `case "Foo":`，用
   `consumePacketItem` / `patchItemsByUniqueId` / `removeItemByUniqueId` 之一更新四个数组。
   **务必也确认 `snapshot.*Items` 会带上结果**，让下次快照能自愈。
7. **回调透传**：在 `original-client-shell-types.ts`（item 回调段 :186–201）给 `OriginalClientShellProps`
   加可选回调 `onFoo?`，经 shell/`original-client-game-ui-scene.tsx` 传到窗口。
8. **窗口 UI**：在 `InventoryWindow`（或 `InventoryContextMenu` action-panels.tsx:473 的
   `InventoryContextAction` 联合类型 + rows）接上按钮，调新回调。若是新交互模式，加一个
   本地 `pendingFooItem` state，模仿现有 `activateInventoryItem` 的 pending-mode 分流。
9. 若涉及新装备类别：同步 `equipmentSlotForItemKey`（utils.ts:37）正则、`EquipmentSlot`
   联合（types.ts:12）、以及 `equipmentSlotIndex`/`equipmentSlotFromIndex`（page.tsx:11840+）
   的双向映射，三处要一致。
10. 类型检查：`cd apps/web && npx tsc --noEmit` 必须 0 错误（新字段一律 optional，别破
    `DisplayWorld`/`WorldItem` 的现有消费者）。

## 相关 (Related)

- 兄弟文档：`docs/client/` 下的其它「前置铺垫」文档（背包动作常与商店/仓库/邮件/交易
  /行情交叉——那些走 `stage5Systems` adapter，与本区不同）。
- 关键源文件：
  - `apps/web/app/components/original-client-inventory-window.tsx`（主窗口 + 仓库）
  - `apps/web/app/components/original-client-inventory-action-panels.tsx`（弹窗/菜单/过滤/工具条）
  - `apps/web/app/components/original-client-inventory-utils.ts`（图标/槽位启发式/日期）
  - `apps/web/app/components/original-client-item-tooltip.tsx`（悬浮提示）
  - `apps/web/app/components/original-client-panels.tsx` → `BeltDialog`（腰带）
  - `apps/web/app/components/original-client-character-window.tsx`（装备槽）
  - `apps/web/app/page.tsx`：命令 `:5258–5440`、入站 packet `:6975–7491`、
    snapshot→DisplayWorld `:9660–9706`、grid/槽位映射 `:11840–11984`
  - `apps/gateway/src/web.rs`：`BrowserCommand`→`ClientPacket` `:2705–2792`、
    `server_packet_to_event`（StoreItem/TakeBackItem 等）`:4524+`
  - 类型：`apps/web/app/components/original-client-types.ts`、
    `apps/web/lib/world-model/types.ts`、`apps/web/lib/original-ui.ts`（槽位坐标）
