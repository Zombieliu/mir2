# 协议跨层接线 + 加功能配方 (Protocol cross-layer wiring) — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

A player action and a server state-change cross **five layers**. Outbound: the browser sends
a small JSON `{type:"…", …}` over a single WebSocket; the **gateway** (`web.rs`) parses it into
a `BrowserCommand`, lowers it to a `ClientPacket` (or a higher-level `SessionAction`), and feeds
it to the **simulation**. Inbound: the simulation returns `Vec<ServerPacket>`; the gateway turns
each into a JSON **event** `{type:"packet", packet:"<Name>", payload:{…camelCase…}}`; `page.tsx`
switches on `event.packet`, merges into world state / `stage5Systems`; an **adapter** in
`stage5-window-adapters.ts` defensively reshapes that loose record into a typed summary; a
presentation-only **window component** renders it. This doc traces one feature (party/group) in
both directions and gives the canonical recipe for adding a new one.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `packages/protocol/src/packets.rs` | The two wire enums (Rust↔Rust). | `enum ClientPacket` (:20), `enum ServerPacket`, `ClientPacket::packet_id` (:557) |
| `packages/protocol/src/types.rs` | Shared structs (`UserItem`, `ClientFriend`, `Point`, buff/stat types). | — |
| `apps/gateway/src/web.rs` | WS bridge: both translation tables + the loop. | `enum BrowserCommand` (:585), `fn browser_command_to_action` (:2570), `enum SessionAction` (:1167), `fn execute_session_action` (:2328), `fn flush_session_updates` (:2056), `fn send_server_packet` (:3347), `fn server_packet_to_event` (:3610) |
| `apps/simulation/src/runtime/packets.rs` | Simulation entry: dispatches one `ClientPacket`. | `fn handle_packet` (:6821) → `fn handle_packet_impl` (:6830); arms e.g. `ClientPacket::AddMember` (:7121), `ClientPacket::MoveItem` (:7458) |
| `apps/web/app/page.tsx` | Outbound `send()`, inbound `handleGatewayEvent` switch, world state. | `type GatewayEvent` (:205), `function send` (:4026), `function handleGatewayEvent` (:6346) `switch (event.packet)` (:6422), `type Stage5SystemsState` (:351), `updateWorld` (:1436) |
| `apps/web/lib/stage5-window-adapters.ts` | Defensive loose-record → typed-summary adapters. | `asRecord` (:90), `readString` (:94), `readNumber` (:108), `adaptGroup` (:317), `adaptFriends` (:398), `adaptTrade` (:728), `adaptBuffs` (:842) |
| `apps/web/lib/world-model/types.ts` | Canonical `Stage5SystemsState` type. | `export type Stage5SystemsState` (:285) |
| `apps/web/app/components/original-client-*-window.tsx` | Presentation-only windows (e.g. `GroupWindow`). | rendered by `<ExtraWindows>` (`original-client-extra-windows.tsx`), whose `group=`/`friends=`/`trade=`/`buffs=` props are built at `page.tsx` ~:11245 |

## 数据流 (How it threads the 5 layers)

### Outbound — "invite a player to my party" (one `BrowserCommand` end-to-end)

1. **page.tsx** — a window's `onInviteMember` calls a handler that calls `send(...)`. The
   "add a party member" path emits two JSON commands: `send({ type: "switchGroup", allowGroup: true }, { quiet: true })` then `send({ type: "addMember", name })` (page.tsx ~:5735-5736). `send` (page.tsx:4026) does
   `socketRef.current.send(JSON.stringify(command))` (:4084) on the open WS.
2. **gateway parse** — the WS read loop pulls `Message::Text` (web.rs:1712) and does
   `serde_json::from_str::<BrowserCommand>(&message)` (:1718). `BrowserCommand` is
   `#[serde(tag = "type", rename_all = "camelCase")]` (:584), so `{"type":"addMember","name":…}`
   deserializes to `BrowserCommand::AddMember { name }`.
3. **lower to packet** — `browser_command_to_action(command)` (web.rs:2570) matches
   `BrowserCommand::AddMember { name } => Ok(SessionAction::Packet(ClientPacket::AddMember { name }))`
   (:3206). Most arms are a 1:1 wrap; some lower to richer `SessionAction` variants
   (`MoveTo`, `Attack`, `Interact`, `UseItem`, `CastSkill`, `Stage5Command`, …; enum at :1167).
4. **simulation** — `execute_session_action` (web.rs:2328) → `SessionAction::Packet(packet) => session.handle_packet(packet)` (:2340; the production-safety path is the parallel `execute_production_session_action` at :2368). That `session` is the gateway's `GatewaySession`, whose `handle_packet` (session.rs:247) wraps the packet as `WorldCommand::ClientPacket` and runs it; that lands in the simulation's `SimulationSession::handle_packet` (`runtime/packets.rs:6821`)
   → `handle_packet_impl` (:6830), which matches `ClientPacket::AddMember { name }` (:7121) and returns
   `Vec<ServerPacket>`.
5. **echo back** — those responses are drained by `flush_session_updates` (web.rs:2056): `for response in responses { send_server_packet(sender, &response) }` (:2070) — i.e. they re-enter the inbound path below.

### Inbound — server pushes the party roster change (one `ServerPacket` end-to-end)

1. **gateway encode** — `send_server_packet` (web.rs:3347) calls
   `server_packet_to_event(packet)` (:3610) and ships its JSON over the WS (:3352). Every arm
   returns the same envelope: `{type:"packet", packet:"<Name>", payload:{…}}`. Example:
   `ServerPacket::DeleteMember { name } => json!({ "type":"packet", "packet":"DeleteMember", "payload": { "name": name } })` (web.rs:4233). **This is where snake_case→camelCase happens** — each
   arm hand-writes camelCase keys (e.g. `AddBuff` at :5019 emits `buffType`, `objectId`,
   `expireTime`). There is no automatic rename on the *inbound* side.
2. **page.tsx receive** — the WS `onmessage` does
   `handleGatewayEvent(JSON.parse(event.data) as GatewayEvent)` (page.tsx:4605). `handleGatewayEvent`
   (:6346) short-circuits `error` / `worldSnapshot`, then `if (event.type !== "packet") return`
   (:6401) and `switch (event.packet)` (:6422).
3. **merge into state** — `case "AddMember": case "DeleteMember":` (page.tsx:7546) reads
   `payload.name` and calls `updateWorld(current => ({ …current, stage5Systems: { …current.stage5Systems, group: { …(current.stage5Systems.group ?? {}), members: groupMembersAfterChange(...) } } }))` (:7550-7562). The enriched roster arrives separately as `case "GroupMemberInfo"` (:7566) → `stage5Systems.group.memberInfos`.
4. **adapter** — at render, the group window is mounted with
   `group: adaptGroup(world.stage5Systems.group)` (page.tsx:11245). `adaptGroup`
   (stage5-window-adapters.ts:317) prefers `memberInfos` over the bare `members` name list (:333),
   pulls fields via `readString(record, ["name"])` / `readNumber(record, ["level"])` (:342-358) and
   returns a typed `GroupSummary { members, lootMode }` (:366).
5. **component** — `GroupWindow` (`app/components/original-client-group-window.tsx`, rendered by
   `<ExtraWindows>` whose `group={{…}}` prop is built at page.tsx:11245) receives the typed
   `group` prop plus action callbacks (`onInviteMember`, `onKickMember`, …) and only renders +
   calls them; it holds no protocol knowledge.

The same shape governs every feature: `adaptFriends`←`stage5Systems.social`, `adaptTrade`←`stage5Systems.trade`,
`adaptMarketListings`←`stage5Systems.auction`, `adaptBuffs`←`world.activeBuffs` (buffs are a top-level
field, NOT under `stage5Systems`).

## 状态形状 (State shape)

- `Stage5SystemsState` — `apps/web/lib/world-model/types.ts:285` (mirrored inline at page.tsx:351).
  Loosely-typed per-window slices, all optional:
  - `group?: { members?: string[]; memberInfos?: Array<{name; level?; class?; hp?; maxHp?; online?}>; lootMode?; leaderName? }`
  - `social?: { friends?; blocked?; friendInfos?; blockedInfos? }`, `trade?: Record|null`,
    `auction?: Array<Record>`, `guild?`, `mentor?`, `relationship?`, `mail?`, `conquest?`,
    `guildTerritory?`, `hero?`, `itemRental?`, `profession?`, `intelligentCreatures?`.
- Buffs live OUTSIDE stage5: `world.activeBuffs: ActiveBuff[]`, written by `applyAddBuffPacket`
  (page.tsx:9424) / `applyRemoveBuffPacket` (:9462), read by `adaptBuffs(world.activeBuffs)` (~:11252).
- Inventory/equipment/gold are top-level world fields (`inventoryItems`, `beltItems`,
  `storageItems`, `equipmentItems`, `gold`) — see e.g. `case "UseItem"` (page.tsx:6975).
- All writes go through `updateWorld` (page.tsx:1436), a `useCallback` setter that takes a
  `current => next` reducer.

## 坑 & 不变量 (Invariants & gotchas)

- **Two rename directions, two mechanisms.** Outbound JSON→`BrowserCommand` is auto-camelCased by
  `#[serde(tag="type", rename_all="camelCase")]` (web.rs:584); fields whose JS name differs use an
  explicit `#[serde(alias = "…")]` (e.g. `accountId`→`account_id` at :590, `objectId`→`object_id`).
  Inbound `ServerPacket`→JSON is **hand-written** camelCase per arm in `server_packet_to_event` —
  there is no derive. If you add an inbound field, you type the camelCase key string yourself.
- **The browser is never trusted to send a `ServerPacket`.** It sends a `BrowserCommand`; the
  gateway decides which `ClientPacket`/`SessionAction` it maps to. Adding a wire field is not
  enough — you must add the `BrowserCommand` arm AND the `browser_command_to_action` arm.
- **Adapters are defensive on purpose.** `stage5Systems.*` records are `Record<string,unknown>`;
  always read via `asRecord` / `readString` / `readNumber` / `readBool` (which coerce string↔number
  and trim), never via `record.foo as string`. A missing field must degrade to `undefined`, not throw.
- **Many handlers re-key by Crystal semantics, not by a 1:1 field.** Buffs are keyed
  `crystalBuffKey(objectId, buffType)` and `applyAddBuffPacket` drops the packet entirely if
  `visible === false` or if it targets another player (page.tsx:9427-9432). Don't assume a packet
  always mutates state.
- **A command can fan out to several packets.** "Add member" sends `switchGroup` *and* `addMember`
  (page.tsx:5735-5736); the enriched roster comes back as a *separate* `GroupMemberInfo` packet, so
  `adaptGroup` must merge `members` + `memberInfos`. Inbound, one action's `Vec<ServerPacket>` is
  flushed one-event-per-frame (web.rs:2070).
- **Some data is genuinely absent from Crystal's protocol**, not unimplemented — e.g. only the
  partner's side of a trade is pushed (own offer is client-tracked); `friend_entry_json`
  (web.rs:3388) deliberately omits `level`/`mapName` because `ClientFriend` doesn't carry them.
  Don't "fix" a missing field by inventing one.
- **`SessionAction` is the gateway's own enum**, broader than `ClientPacket` (web.rs:1167): movement
  (`MoveTo`), interaction, `Stage5Command`, `SetLanguage`, `Tick`. Not every browser command becomes
  a single packet; check whether your feature wants a packet wrap or a richer action.

## 如何扩展 (How to extend / add to this area)

Follow the additive/optional-field rule at every hop: **new fields are optional + backward
compatible**; never break `DisplayWorld` or an existing adapter/consumer.

**To add an INBOUND server→client field/packet** (e.g. surface a new datum in a window):
1. `packages/protocol/src/packets.rs` (and `types.rs` if a struct) — add the `ServerPacket` variant
   or field (Rust). Keep existing fields; new ones optional where the protocol allows.
2. `apps/simulation/…` — make the sim emit it (out of scope for this doc; cite Crystal `file:line`
   for the semantic).
3. `apps/gateway/src/web.rs` `server_packet_to_event` (:3610) — add/extend the arm; hand-write the
   **camelCase** payload key(s). This is the snake_case→camelCase boundary.
4. `apps/web/app/page.tsx` — add a `case "<PacketName>":` in `switch (event.packet)` (:6422) that
   reads `payload.<camelKey>` and merges via `updateWorld` into `world.*` or
   `world.stage5Systems.<slice>` (spread the prior slice so you don't drop sibling keys).
5. `apps/web/lib/world-model/types.ts` `Stage5SystemsState` (:285) — widen the slice type (optional).
6. `apps/web/lib/stage5-window-adapters.ts` — read the new field in the relevant `adapt*` via
   `readString`/`readNumber`; extend the typed summary type it returns (optional field).
7. `apps/web/app/components/original-client-*-window.tsx` — render it from the typed prop.

**To add an OUTBOUND client→server action** (e.g. a new button):
1. `apps/web/app/page.tsx` — write a handler that calls `send({ type: "<camelType>", …camelArgs })`
   and wire it as a window callback prop (e.g. `onFoo`).
2. `apps/gateway/src/web.rs` `enum BrowserCommand` (:585) — add the variant; use `#[serde(alias=…)]`
   for any field whose JS name isn't the snake_case of the Rust name.
3. `apps/gateway/src/web.rs` `browser_command_to_action` (:2570) — add the arm mapping it to
   `SessionAction::Packet(ClientPacket::Foo{…})` (or a richer `SessionAction`).
4. `packages/protocol/src/packets.rs` `enum ClientPacket` (:20) + `packet_id` (:557) — add the
   variant if new.
5. `apps/simulation/src/runtime/packets.rs` `handle_packet_impl` (:6830) — add the `ClientPacket::Foo`
   arm that mutates the world and returns the `Vec<ServerPacket>` the client should observe.

**Type-check before every push** (from `mir2-web3/`):
- web: `cd apps/web && npx tsc --noEmit` (MUST be 0) + `npm run test:frontend-logic` (adapters).
- backend: `cargo check -p mir2-gateway`, `cargo check -p mir2-protocol`,
  `cargo test --locked -p mir2-simulation -- --test-threads=1`, and **`cargo fmt --all --check`**
  (the CI `local-candidate-gate`; a red gate is almost always missing `cargo fmt`).

## 相关 (Related)

- `docs/client/page-tsx-map.md` — navigating the ~12.7k-line `page.tsx`.
- `docs/client/stage5-social.md` — the group/friends/trade window stack in depth.
- `docs/client/inventory.md` — inventory/equipment item flow (sibling outbound example).
- `docs/client/audio-vfx.md`, `docs/client/onchain-mine.md` — other client subsystems.
- Source: `apps/gateway/src/web.rs` (`browser_command_to_action`, `server_packet_to_event`),
  `packages/protocol/src/packets.rs`, `apps/web/app/page.tsx` (`send`, `handleGatewayEvent`),
  `apps/web/lib/stage5-window-adapters.ts`.
