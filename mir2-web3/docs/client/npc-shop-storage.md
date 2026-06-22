# NPC 对话 / 商店 / 仓库 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

The NPC interaction loop: clicking an NPC opens a **dialog** (a page of body text +
clickable `@target` links + an optional text input), and the links route to NPC
**services** — buy/sell shop, repair, refine, and **personal storage** (deposit /
withdraw, password-protected). 一次点击 NPC → `interact` → 服务器回 `NPCResponse`(对话文字)
+ 一个 `worldSnapshot`(里面带 `activeNpcDialog` 结构化对话);点对话里的链接(@Buy/@Sell/@Exit/
@Storage…)→ `selectNpcDialog` → 服务器换页或回一个 service 包(`NPCGoods` / `NPCSell` /
`NPCStorage` / `UserStorage`)。

**Critical layout fact:** the dialog itself is **snapshot-driven** (`activeNpcDialog`), not
built from the `NPCResponse` packet (that one only echoes lines into the chat log). The
shop/storage **service** panels reuse the **inventory window** surface, not a standalone shop
window. `NpcShopWindow` exists in `original-client-game-shop.tsx` but is **not mounted** — see
the buy gap in 坑 & 不变量.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/page.tsx` | Inbound: dialog text echo | `case "NPCResponse"` :7877 |
| `apps/web/app/page.tsx` | Inbound: open a service panel (reuse inventory) | `case "NPCGoods"/"NPCSell"/"NPCRepair"/…` :7866; `case "NPCStorage"` :7094; `case "NPCConsign"` :8926 |
| `apps/web/app/page.tsx` | Inbound: storage contents | `case "UserStorage"` :7049 |
| `apps/web/app/page.tsx` | Inbound: storage password / lock / size | `case "StorageUnlockResult"` :7038; `"StoragePasswordResult"` :7099; `"ResizeStorage"` :7113 |
| `apps/web/app/page.tsx` | Inbound: dialog text-input prompt | `case "NPCRequestInput"` :8922 (→ `restoreObjectSelection` :9079) |
| `apps/web/app/page.tsx` | Inbound: cash-shop stock echo | `case "GameShopStock"` :8892 |
| `apps/web/app/page.tsx` | Snapshot → `activeNpcDialog` shaping | inside the snapshot handler :9754–9775 |
| `apps/web/app/page.tsx` | Outbound: click NPC | `interactTarget` :5970 (`send({type:"interact",objectId})`) |
| `apps/web/app/page.tsx` | Outbound: storage deposit / withdraw | `storeItem` :5375; `takeBackItem` :5383 |
| `apps/web/app/page.tsx` | Outbound: storage password | `unlockStorage` :5391; `setStoragePassword` :5398; `removeStoragePassword` :5406; `rentExpandedStorage` :5413 (`@ADDSTORAGE`) |
| `apps/web/app/page.tsx` | Outbound: sell / cash-shop buy | `sellItem` :5420; `buyGameShopItem` :5537 (stage5 `gameShop.buyGold`/`buyCredit`) |
| `apps/web/app/page.tsx` | Outbound: dialog link / input (mount-site closures) | `onSelectNpcDialogTarget` :11233 (`selectNpcDialog`); `onSubmitNpcInput` :11234 (`submitNpcInput`) |
| `apps/web/app/components/original-client-dialogs.tsx` | The dialog window (presentation) | `NpcDialogPanel` :299; `NpcDialogPanelProps` :247; `npcLinkKind`/`npcLinkGlyph` :260/272 |
| `apps/web/app/components/original-client-game-ui-scene.tsx` | Mounts the dialog from `world.activeNpcDialog` | `dialogKey`/`visibleDialog` :159–163; `<NpcDialogPanel>` mount :352 |
| `apps/web/app/components/original-client-game-shop.tsx` | Cash GameShop (mounted) + NpcShopWindow (NOT mounted) | `GameShopWindow` `onBuy` :52/207; `NpcShopWindow` :505; `NpcShopGood` :418 |
| `apps/web/app/components/original-client-storage-password-panel.tsx` | Storage-password sub-panel | (rendered from inventory window) |
| `apps/gateway/src/web.rs` | Outbound BrowserCommand → ClientPacket | `SelectNpcDialog` :2701; `StoreItem` :2785; `TakeBackItem` :2788; `SellItem` :2839; `BuyItem` :2845 |
| `apps/gateway/src/web.rs` | Inbound ServerPacket → JSON | `NPCGoods` :3980; `NPCSell` :3995; `NPCStorage` :4041; `UserStorage` :4046; `StoreItem`/`TakeBackItem` :4533/:4524; `SellItem` :4296 |
| `apps/simulation/src/runtime/npc.rs` | Buy / sell semantics | `buy_item_impl` :747; `sell_item_impl` :977 |
| `apps/simulation/src/runtime/npc_script.rs` | Dialog target routing | `select_npc_dialog_target` :3211; `submit_npc_input` :3216 |
| `apps/simulation/src/runtime/stage5.rs` | QA storage seed hook | `stage5_qa_open_storage` :1987 |

## 数据流 (How it threads the layers)

### Inbound — open dialog (click NPC)

```
interactTarget(objectId)  page.tsx:5970
  send({type:"interact", objectId})
   → gateway browser_command_to_action  → SessionAction (Interact)
   → sim NpcSession::interact_impl  npc_script.rs:3231  (sets active_npc_dialog, builds page)
   → emits ServerPacket::NPCResponse (page lines)  +  the world snapshot carries activeNpcDialog
gateway server_packet_to_event → page.tsx
  case "NPCResponse" :7877  →  appendLog(line) for each page line (chat log only)
  snapshot handler :9754   →  shapes snapshot.activeNpcDialog into world.activeNpcDialog (NpcDialog)
   → original-client-game-ui-scene.tsx :159  →  visibleDialog gate (dialogKey vs dismissedDialogKey)
   → <NpcDialogPanel dialog=…> :352   (renders title/body/links/input)
```

### Outbound — click a dialog link / submit input

```
NpcDialogPanel link <button onClick=onSelectTarget(link.target)>  dialogs.tsx:345
  → onSelectNpcDialogTarget(target)  page.tsx:11233  →  send({type:"selectNpcDialog", target})
   → gateway BrowserCommand::SelectNpcDialog :2701  →  SessionAction::SelectNpcDialog
   → sim select_npc_dialog_target  npc_script.rs:3211
       @target routes: a new page → NPCResponse + snapshot; @Buy → NPCGoods; @Sell → NPCSell;
       @Storage → NPCStorage + UserStorage; @Exit → clears active_npc_dialog
input form onSubmit → onSubmitNpcInput(value)  page.tsx:11234  →  send({type:"submitNpcInput", value})
   → sim submit_npc_input  npc_script.rs:3216
```

### Outbound — sell (buy is NOT wired; see gotchas)

```
inventory window sell-mode → confirmSellItem(item)  original-client-inventory-window.tsx:415
  → onSellItem(ref, count) → sellItem  page.tsx:5420  →  send({type:"sellItem", uniqueId, count})
   → gateway BrowserCommand::SellItem :2839 (#[serde(alias="uniqueId")] → unique_id)
   → ClientPacket::SellItem  →  sim sell_item_impl  npc.rs:977
```

### Outbound — storage deposit / withdraw

```
NPCStorage / qa.openStorage → page.tsx case "NPCStorage" :7094
  setShowInventory(true); setActiveInventoryTab("bag1"); setStorageServiceOpenVersion(v+1)
inventory window effect on storageServiceOpenVersion  inventory-window.tsx:124
  → enters "takeBack" storage mode; if locked → mounts storage-password sub-panel
deposit:  onStoreItem(ref, toSlot) → storeItem  page.tsx:5375  → send({type:"storeItem", from, to})
withdraw: onTakeBackItem(ref, toSlot) → takeBackItem  :5383  → send({type:"takeBackItem", from, to})
   → gateway :2785 / :2788  →  ClientPacket::StoreItem / TakeBackItem
   → sim mutates inventory↔storage, re-emits UserStorage  →  page.tsx case "UserStorage" :7049
```

## 状态形状 (State shape)

World state (`apps/web/lib/world-model/types.ts`):
- `world.activeNpcDialog: NpcDialog | null` (type def :212, field :401, default :463). Shape:
  `{ npcObjectId, npcName, title, body: string[], footer, links: {text,target}[], input?: {target,prompt}|null }`.
  **Built only from the snapshot** (page.tsx :9754). Cleared on map change (:6598, :8183) and when the
  selected NPC's object leaves (:9070).
- `world.storageItems: WorldItem[]` — withdrawable items, indexed by `slot`. Merged in `case "UserStorage"`
  (:7049): the sim sends a sparse `storage[]` of `{count,current_dura,max_dura,unique_id}`; the handler
  **preserves the prior slot's `key`/`name`/`icon`/`description`** (server omits them) and re-derives
  `uniqueId`/`quantity`/durability per entry.
- `world.hasStoragePassword`, `world.storageSessionUnlocked`, `world.requireStoragePassword`,
  `world.storagePasswordLastSetBinaryDatetime` — driven by `StorageUnlockResult` (:7038) /
  `StoragePasswordResult` (:7099).
- `world.storageSize`, `world.hasExpandedStorage`, `world.expandedStorageExpiryTimeBinaryDatetime` —
  driven by `ResizeStorage` (:7113).
- `world.gold` — read by the cash-shop and shown in shop footers; debited server-side.

Local React state:
- `storageServiceOpenVersion` (page.tsx :1637) — a **monotonic counter**, not a boolean. Each NPCStorage
  packet bumps it; the inventory window's `useEffect` keys off the change (:124) to re-enter storage mode.
  Using a counter (not `showStorage=true`) lets the same open re-fire after the user toggled tabs.
- `dismissedDialogKey` (game-ui-scene.tsx :157) + `dialogKey = npcObjectId:title:worldTick` (:159) — the
  dialog stays dismissed until a **new** packet changes the key (different NPC / title / tick).
- `NpcDialogPanel` holds `inputValue` locally; cleared on submit (dialogs.tsx :369).

Note: `world.stage5Systems` is **not** used by NPC dialog/shop/storage — the only stage5 touch is the
**cash GameShop** buy (`buyGameShopItem`, page.tsx :5537, sends `{type:"stage5Command", action:
"gameShop.buyGold"/"gameShop.buyCredit"}`).

## 坑 & 不变量 (Invariants & gotchas)

- **The NPC merchant BUY path is not wired in the client.** `NPCGoods` carries the goods `list`
  (gateway :3980) but page.tsx `case "NPCGoods"` (:7866) **only opens the inventory** — it never reads
  `payload.list`, and there is **no `send({type:"buyItem"})` anywhere** in page.tsx. `NpcShopWindow`
  (`original-client-game-shop.tsx` :505) is a complete, exported component but is **never mounted**.
  The full backend buy path *does* exist end-to-end (gateway `BuyItem` :2845 → sim `buy_item_impl`
  npc.rs:747), so wiring buy is "mount the window + add a `buyItem` handler", not new protocol.
- **`BuyItem.itemIndex` = the goods entry's `uniqueId`, NOT the item template index.** Gateway aliases
  `itemIndex`→`item_index` (web.rs :807); `buy_item_impl` looks the offer up via
  `crystal_npc_service_item_for_purchase(world, &service, item_index)` (npc.rs:770), matching Crystal
  `PlayerObject.BuyItem(ulong index, …)` which passes `index` to `script.Buy(this, index, count)`
  (`Crystal/Server/MirObjects/PlayerObject.cs:7944`). Sending a template id silently no-ops (offer not
  found → empty `Vec`). The gateway test pins the JSON shape: `{"type":"buyItem","itemIndex":…,"count":…,"panelType":0}` (web.rs :6966).
- **`panelType` must be `PanelType.Buy` (0).** `buy_item_impl` early-returns unless `panel_type ==
  CRYSTAL_PANEL_BUY` (npc.rs:753). Mirrors Crystal's `if (type == PanelType.Buy)` guard
  (`PlayerObject.cs:7963`).
- **Buy/sell require an in-range, buy-capable NPC service.** `buy_item_impl` filters
  `current_crystal_npc_service_in_range(world).filter(active_crystal_buy_service)` (npc.rs:760).
  This is the **GTMerchant gotcha**: `GTMerchant_Jamie` is a *guild-hall* merchant, not a general store —
  its page isn't a buy page, so buys against it no-op. Verify against a real general-goods NPC. (Crystal
  gate: the `NPCPage.Key` must be one of `BuySellKey/BuyKey/BuyBackKey/…`, `PlayerObject.cs:7949`.)
- **The dialog is snapshot-only.** `NPCResponse` (page.tsx :7877) just echoes lines to chat — if you try
  to *render* the dialog from that packet you'll get a blank panel. The structured dialog (title/links/
  input) arrives via the **next `worldSnapshot`'s `activeNpcDialog`** (shaped at :9754). A handler that
  needs the dialog must read `worldRef.current.activeNpcDialog`, not the packet.
- **`NPCRequestInput` doesn't open anything new** (page.tsx :8922) — it just `restoreObjectSelection`s the
  NPC; the actual text-input box is part of `activeNpcDialog.input` surfaced through the snapshot.
- **Storage is the only password-gated surface.** Opening storage while locked mounts the password panel
  (inventory-window.tsx :139); deposits/withdraws before unlock will be server-rejected. `storageSessionUnlocked`
  is reset to `false` on a fresh QA open (`stage5_qa_open_storage`, stage5.rs:2027) so QA must unlock first
  (or use a no-password character).
- **`UserStorage` is a partial refresh — preserve prior slot metadata.** The sim sends only counts/dura/
  ids; the handler (:7049) **must** fall back to the existing slot's `key`/`name`/`icon` or the items show
  as `"Storage Item N"`. Don't "simplify" it into a blind overwrite.
- **`rentExpandedStorage` is a chat command, not a real packet** — it sends `@ADDSTORAGE` (page.tsx :5413).
  Don't look for a `BrowserCommand` arm; there isn't one.
- **QA seeding:** `qa.openStorage` (sim stage5.rs:1987) spawns/repurposes object-id 21 as
  `InnKeeper_Brittney` with script `BichonProvince/BichonWall/Warehouse1` and emits the open packets; it is
  **not** test-server-gated, so it's reliable for storage QA (see the economy-QA memory note).

## 如何扩展 (How to extend / add to this area)

**Wire the missing NPC merchant buy window (the highest-value gap):**
1. `apps/web/app/page.tsx` `case "NPCGoods"` (:7866) — read `payload.list` (+ `rate`, `panelType`,
   `hideAddedStats`) and merge into a **new optional** world field (e.g. `world.npcShopGoods`), spreading
   prior state via `updateWorld` (:1436). Add the field to `lib/world-model/types.ts` `WorldState` as
   optional. Don't drop the existing `setShowInventory` behaviour unless you mount a dedicated window.
2. `apps/web/app/page.tsx` — add a `buyItem` handler: `send({ type:"buyItem", itemIndex: <goods uniqueId>,
   count, panelType: 0 })`. **`itemIndex` is the goods entry id, not the template** (see gotchas).
3. Mount `NpcShopWindow` (`original-client-game-shop.tsx` :505) from the snapshot data and wire
   `onBuy={(id, qty) => buyItem(id, qty)}`, `onSell={(id, qty) => sellItem({uniqueId:id,…}, qty)}`,
   `onRepair`/`onSpecialRepair` to the existing `repairItem`/`specialRepairItem` handlers (page.tsx :5435/:5442).
   No gateway/sim/protocol change needed — `BrowserCommand::BuyItem` (web.rs :2845) and `buy_item_impl`
   (npc.rs:747) already exist.

**Add a new dialog `@target` action (e.g. a new service the sim already supports):** the client side is
already generic — `onSelectNpcDialogTarget` (page.tsx :11233) forwards any string to `selectNpcDialog`.
Just teach the **sim** `select_npc_dialog_target_impl` (npc_script.rs :3389) to recognise the new
`@target` and emit the right `ServerPacket`; if that packet is new, follow the inbound recipe in
`protocol-cross-layer.md`.

**Surface a new storage datum (e.g. an extra password flag):** follow the standard inbound recipe —
protocol field (optional) → sim emit (cite Crystal) → gateway `server_packet_to_event` (camelCase, e.g.
extend the `StoragePasswordResult` arm) → page.tsx `case` merge into `world.*` → render in the
inventory/storage-password panel. Keep every new `WorldState` field optional + backward-compatible.

## 相关 (Related)

- [`inventory.md`](./inventory.md) — the inventory window that hosts the sell/storage surface; item refs, slots, auto-belt gotcha.
- [`protocol-cross-layer.md`](./protocol-cross-layer.md) — the full 5-layer add-a-feature recipe (both directions).
- [`page-tsx-map.md`](./page-tsx-map.md) — navigating the ~12.7k-line `page.tsx` ServerPacket switch.
- [`stage5-social.md`](./stage5-social.md) — the auction/market surface (the *other* buy/sell economy, stage5-backed).
- Source: `apps/web/app/page.tsx` (cases above) · `apps/web/app/components/original-client-dialogs.tsx` ·
  `apps/web/app/components/original-client-game-shop.tsx` · `apps/gateway/src/web.rs` ·
  `apps/simulation/src/runtime/npc.rs` + `npc_script.rs` · Crystal `Server/MirObjects/PlayerObject.cs:7944`.
