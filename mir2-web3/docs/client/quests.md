# 任务系统 (Quest log) — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

The quest log: the player's accepted/available/completed quests, their per-task
objectives, structured rewards, and the three log-window actions (track / share /
abandon). 客户端**不拥有任务逻辑** — the simulation is authoritative: it computes each
quest's `stage` (`available → inProgress → readyToTurnIn → completed`), tracks objective
counts, and grants rewards. The web layer's job is to **merge** the inbound quest packets
into `world.questLog` and present them. Accept and turn-in flow through the **NPC dialog**,
not the quest-log window; the window only does share/abandon.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/page.tsx` | `QuestEntry` type (window-facing, enriched) | `type QuestEntry` :583; `type QuestStage` :229; `GatewayQuestEntry` :310 |
| `apps/web/app/page.tsx` | inbound packet handlers | `case "CompleteQuest"` :8095; `case "ChangeQuest"` :8114; `case "ShareQuest"` :8143; `case "NewQuestInfo"` :8391 |
| `apps/web/app/page.tsx` | payload → objectives/rewards parsers | `parseQuestObjectives` :11428; `parseQuestRewards` :11442; `questNumber` :11423 |
| `apps/web/app/page.tsx` | snapshot → `world.questLog` projection | `const questLog = snapshot.questLog.map(...)` :9707 |
| `apps/web/app/page.tsx` | outbound actions + window mount | `shareQuest` :5774; `abandonQuest` :5778; `<ExtraWindows … questLog={…}>` :11242; `setShowQuestLog`/`Q` toggle :1482 / :1590 |
| `apps/web/app/components/original-client-quest-log-window.tsx` | the log window (presentation-only) | `QuestLogWindow` :103; `QuestLogEntry` :54; `QuestRewardView` :342; `stageLabel`/`stageColor` :416 / :430 |
| `apps/gateway/src/web.rs` | inbound JSON projection (snake→camel) | `ServerPacket::ChangeQuest` arm :5836; `CompleteQuest` :5863; `ShareQuest` :5870; `NewQuestInfo` :5891 |
| `apps/gateway/src/web.rs` | outbound `BrowserCommand` → `ClientPacket` | `BrowserCommand::{AcceptQuest,FinishQuest,AbandonQuest,ShareQuest}` defs :1078; arms :3177 |
| `packages/protocol/src/packets.rs` | the wire enums | `ClientPacket::{AcceptQuest,FinishQuest,AbandonQuest,ShareQuest}` :394; `ServerPacket::{ChangeQuest,CompleteQuest,ShareQuest,NewQuestInfo}` :2946 |
| `apps/simulation/src/config.rs` | authoritative stage + snapshot shape | `enum QuestStage` :3319; `struct QuestSnapshot` :3456 (`stage` field :3463); `quest_log: Vec<QuestSnapshot>` :4264 |
| `apps/simulation/src/runtime/quests.rs` | stage transitions + camelCase serialize | `begin_quest` :628; `quest.stage = ReadyToTurnIn` :414; stage→string map `quest_stage_key` :482 |

## 数据流 (How it threads the layers)

Quest data reaches `world.questLog` by **two complementary inbound routes**, plus the
periodic full snapshot:

**1. The full worldSnapshot (the steady-state source of truth).** Sim emits
`QuestSnapshot { questId, title, summary, objective, progressLabel, tracker, stage, current,
required, rewardPreview }` per quest (config.rs:3456). The gateway carries it as the
camelCase `snapshot.questLog`; page.tsx projects it 1:1 at **:9707** into `world.questLog`.
**This is the only place `stage:"readyToTurnIn"` ever enters the client** — the sim sets it
(quests.rs:414, serialized via quests.rs:486), the snapshot carries it; the per-packet
handlers below never produce it.

**2. Incremental quest packets (live deltas between snapshots):**

- `NewQuestInfo` — the **static** definition (Crystal `ClientQuestInfo`). Gateway arm
  (web.rs:5891) hoists `id`, `name`, `descriptionLines`, `objectives` (from
  `task_description` via `quest_objective_json`), `rewards` (gold/exp/credit/items), and a
  `timeLimit` label alongside the raw `info`. page.tsx `case "NewQuestInfo"` (:8391)
  **appends a new `available` `QuestEntry`** — but only if `questId` isn't already in
  `questLog` (dedupe guard at :8405).
- `ChangeQuest` — the **dynamic** progress (Crystal `ClientQuestProgress`). Gateway arm
  (web.rs:5836) emits `questId`, `objectives` (parsed from the `task_list` progress lines),
  `descriptionLines` (raw `task_list`), plus the `taken`/`completed`/`new`/`questState`
  flags. page.tsx `case "ChangeQuest"` (:8114) finds the matching entry and updates its
  `stage` (→ `completed` if `completed`, → `inProgress` if `taken === true`, else
  unchanged) and merges `objectives` / `descriptionLines`.
- `CompleteQuest` — a list of finished quest ids. page.tsx `case "CompleteQuest"` (:8095)
  flips each matching entry's `stage` to `"completed"` and logs `ui.questCompleted`.
- `ShareQuest` — a party member shared a quest. page.tsx `case "ShareQuest"` (:8143) is
  **log-only** (`server.QuestShared` into the `group` channel); it does NOT add a quest.

**Outbound (the three window actions + the dialog-driven accept/finish):**

- **Share / Abandon** (the wired window buttons): the window's `onShareQuest`/`onTrackQuest`/
  `onAbandonQuest` props are both/all bound to `shareQuest` / `abandonQuest` (page.tsx
  :11242). `shareQuest` (:5774) → `send({type:"shareQuest", questIndex})`; `abandonQuest`
  (:5778) → `send({type:"abandonQuest", questIndex})` → `send` (:4026) → gateway
  `BrowserCommand::ShareQuest`/`AbandonQuest` (web.rs:3196 / :3191) → `ClientPacket::ShareQuest`
  / `AbandonQuest` (packets.rs:405 / :402) → sim. **Note:** the log window's "Track" button is
  also wired to `shareQuest` — track and share are the same command client-side.
- **Accept / Turn-in** (NOT in the quest-log window): the real-player path is an **NPC
  dialog link** `@quest:accept:<id>` / `@quest:finish:<id>`. Clicking it →
  `onSelectNpcDialogTarget` → `send({type:"selectNpcDialog", target})` (page.tsx:11233) →
  gateway `BrowserCommand::SelectNpcDialog` (web.rs:2701) → `SessionAction::SelectNpcDialog`
  → the sim NPC script runs `begin_quest`/finish (quests.rs:628). The protocol *also* has
  direct `ClientPacket::AcceptQuest` / `FinishQuest` (packets.rs:394) with gateway
  `BrowserCommand` arms (web.rs:3177) — but **no production page.tsx code calls
  `acceptQuest`/`finishQuest`**; only QA harnesses use that deterministic spine
  (`apps/web/scripts/qa-quests.mjs`).

## 状态形状 (State shape)

- **`world.questLog: QuestEntry[]`** (page.tsx :716, init `[]` at :860) — the single client
  store for quests. `QuestEntry` (:583):
  - flat fields (always present, snapshot-sourced): `questId:number`, `title`, `summary`,
    `objective`, `progressLabel`, `tracker`, `stage:QuestStage`, `current:number`,
    `required:number`, `rewardPreview`.
  - enriched **optional** fields (added by `NewQuestInfo`/`ChangeQuest`, NOT in the snapshot
    projection): `descriptionLines?:string[]`, `objectives?:{label,current?,required?,done?}[]`,
    `rewards?:{gold?,experience?,credit?,items?,selectItems?}`, `timeLimit?:string`.
- **`GatewayQuestEntry`** (:310) — the snapshot's quest shape (only the flat fields); read by
  the `:9707` projection. The enriched fields are deliberately absent here.
- **`QuestStage`** = `"available" | "inProgress" | "readyToTurnIn" | "completed"` (page.tsx
  :229, mirrored in the window :14 and `original-client-types.ts` :27).
- **Local React state (page.tsx `HomePage`):** `showQuestLog:boolean` (:1482) — drives the
  window's `open`; toggled by `Q` (:1590) and by `setShowQuestLog(false)` on close (:11242).
  The HUD quest button instead opens the inventory's `quest` tab (`activeInventoryTab ===
  "quest"`, :11274) — there are **two** ways to see quests.
- **Window-local UI state** (`QuestLogWindow`, not in `world`): `stageFilter`, `page`,
  `selectedId` — purely presentational filter/pagination/selection (:111–113).

## 坑 & 不变量 (Invariants & gotchas)

- **Reward auto-belts → it lands in `beltItems`, not the bag; React state lags the
  snapshot.** Crystal `AddItem` puts a stackable consumable reward (e.g. a small HP potion)
  into the **belt** if a matching/free belt slot exists, only spilling to the bag otherwise.
  So a quest that "gives a potion" delivers it to `world.beltItems` (key like
  `crystal-item-658`, qty 1), and the very next React render may not show it yet because
  `updateWorld` rAF-batches `setWorld`. **When verifying a reward, check belt + bag + the WS
  snapshot truth (`worldRef.current` / the raw `worldSnapshot` frame), never just
  `inventoryItems` or the React `world`.** This was a confirmed QA false-negative, resolved
  in PR #143 — see `docs/client/inventory.md` and the memory note `quest-qa-arc-mechanics`.
- **`stage:"readyToTurnIn"` only ever arrives via the full snapshot (:9707), never from a
  per-packet handler.** `CompleteQuest`/`ChangeQuest` can set `completed` or `inProgress`,
  but the "ready to turn in" transition is computed server-side (quests.rs:414). If you make
  a UI decision based on `readyToTurnIn`, remember it can lag until the next snapshot.
- **`NewQuestInfo` is idempotent by `questId` (dedupe at :8405); `ChangeQuest`/`CompleteQuest`
  only patch existing entries.** If a `ChangeQuest` arrives for a quest the client never saw a
  `NewQuestInfo` (or snapshot) for, it is silently dropped (the `.map` matches nothing). The
  snapshot is the backstop that reconciles this.
- **Track == Share, client-side.** Both the window's "Track" and "Share" buttons call
  `shareQuest` (:11242). There is no separate client-side "track for the on-screen compass"
  command; the sim's `TrackQuest` flag rides `ChangeQuest` inbound (web.rs:5860). Don't assume
  "Track" is a no-op or a distinct packet.
- **Accept/Finish are NOT window actions.** The quest-log window has no accept/turn-in button;
  those happen at the NPC via `selectNpcDialog` `@quest:…` links. Adding an "Accept" button to
  the window would need wiring the unused `acceptQuest` command (see below) — and Crystal's
  semantics require the NPC interaction, so prefer the dialog path.
- **The window is presentation-only and defensively typed.** It accepts any `QuestLogEntry[]`;
  the enriched fields are optional and it falls back to the flat string fields when absent
  (e.g. `objectives` → `objective` + a progress bar, :253). Never put `send(...)` or business
  logic in the component — go through the `on*` props.
- **`parseQuestObjectives` reads either `text` or `label` and drops empty-label rows** (:11434);
  `parseQuestRewards` returns `undefined` unless at least one of gold/exp/credit/items is
  present (:11457). A `rewards:{}` from the gateway therefore degrades to the flat
  `rewardPreview` string, which is correct.
- **All `world.questLog` writes go through `updateWorld`** (page.tsx :1436) — it writes
  `worldRef.current` synchronously then rAF-batches `setWorld`. A raw `setWorld` would desync
  `worldRef` and the next packet handler (which reads `worldRef.current`) would clobber the
  update.

## 如何扩展 (How to extend / add to this area)

**Surface a new quest datum in the log window (inbound, e.g. add a "suggested level"
chip):** follow the additive/optional-field rule end-to-end:

1. `packages/protocol/src/packets.rs` — add the field to `ServerPacket::NewQuestInfo`'s
   `ClientQuestInfo` (or `ChangeQuest`), optional/back-compat (:2946).
2. `apps/simulation/src/runtime/quests.rs` — populate it; cite the Crystal `file:line` for the
   semantic.
3. `apps/gateway/src/web.rs` — extend the `NewQuestInfo` arm (:5891) (or `ChangeQuest` :5836)
   to hoist the **camelCase** key into `payload`.
4. `apps/web/app/page.tsx` — read `payload.<camelKey>` inside `case "NewQuestInfo"` (:8391)
   (or `ChangeQuest` :8114); add it to the `QuestEntry` literal **as an optional field**.
   Also widen `type QuestEntry` (:583).
5. `apps/web/app/components/original-client-quest-log-window.tsx` — widen `QuestLogEntry`
   (:54) with the optional field and render it in the detail panel (:228+).
   (If it must also survive a reconnect/snapshot, also add it to `GatewayQuestEntry` :310 +
   the `:9707` projection + the sim `QuestSnapshot` config.rs:3456 — otherwise it's
   delta-only and resets on the next snapshot.)

**Add a new quest action button (outbound, e.g. wire a real "Accept" in the window):**

1. `apps/web/app/page.tsx` — add a handler like `acceptQuest(npcIndex, questId)` that calls
   `send({type:"acceptQuest", npcIndex, questIndex:questId})` (mirroring `shareQuest` :5774),
   and pass it as a new `on*` prop on the `questLog={…}` mount (:11242). **Never `send` inside
   the component.**
2. `apps/gateway/src/web.rs` — the `BrowserCommand::AcceptQuest` variant + arm **already exist**
   (:1078 / :3177); just confirm the JS field names match (`#[serde(alias = "questIndex")]`,
   `#[serde(alias = "npcIndex")]`). For a brand-new command, add the variant + arm there.
3. `packages/protocol/src/packets.rs` — `ClientPacket::AcceptQuest` already exists (:394); a
   genuinely new command needs the variant + `packet_id`.
4. `apps/web/app/components/original-client-quest-log-window.tsx` — add the button + its
   `onAcceptQuest?` prop (mirror the `actions` block :309).

## 相关 (Related)

- `docs/client/inventory.md` — the reward auto-belt gotcha (potion → `beltItems`) + item state.
- `docs/client/stage5-social.md` — quest **share** requires a party; party rosters live in
  `stage5Systems.group`.
- `docs/client/protocol-cross-layer.md` — the full 5-layer add-a-feature recipe (both
  directions).
- `docs/client/page-tsx-map.md` — navigating the ~12.7k-line page.tsx; the `ServerPacket`
  switch grouped by domain.
- Source anchors: `apps/web/app/page.tsx` (handlers :8095–8156 / :8391, parsers :11423–11459,
  projection :9707, actions :5774); `apps/web/app/components/original-client-quest-log-window.tsx`;
  `apps/gateway/src/web.rs` (:3177 outbound, :5836–5891 inbound); `apps/simulation/src/runtime/quests.rs`;
  `apps/web/scripts/qa-quests.mjs` (the CDP quest-arc harness).
