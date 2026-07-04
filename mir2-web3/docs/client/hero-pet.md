# 英雄 / 宠物 / 坐骑 / 灵物 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

The companion-systems surface: the single **battle hero** (战士英雄 — a recruitable AI
fighter with its own level/HP/MP/loyalty + AI behaviour), the player's **intelligent
creatures** (灵物/宠物 — tameable pets with a lifespan and a pickup mode), and **mounts**
(坐骑 — a rideable appearance layer on the player avatar). All three are read out of
`world.stage5Systems` and rendered by one window, `HeroPetWindow`. This is a **thin,
read-mostly** subsystem: the inbound side merges a handful of `ServerPacket`s into a
loose `stage5Systems.hero` record + `stage5Systems.intelligentCreatures` array; the
outbound side is just `changeHero` / `setHeroBehaviour` / `updateIntelligentCreature`.
Several window buttons (hero **dismiss/recall**) have **no Crystal packet** and are
deliberately unwired — see 坑 & 不变量.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/page.tsx` | Inbound hero/creature/mount `ServerPacket` case handlers (merge into `stage5Systems`) | `case "MountUpdate"` :7234 · `case "GainHeroExperience"/"HeroLevelChanged"/"HeroHealthChanged"` :7297 · `case "NewHero"` :7786 · `case "ManageHeroes"` :7789 · `case "ChangeHero"/"SetHeroBehaviour"/"UpdateHeroSpawnState"` :7803 · `case "NewIntelligentCreature"/"UpdateIntelligentCreatureList"` :7827 · `case "HeroInformation"/"NewHeroInfo"` :8335 · `case "IntelligentCreaturePickup"` :8370 · `case "HeroBaseStatsInfo"` :8468 · `case "HeroCreateRequest"` :8658 · `case "UnlockHeroAutoPot"` :8670 · `case "IntelligentCreatureEnableRename"` :8679 · `case "ObjectHero"` :6804 |
| `apps/web/app/page.tsx` | Outbound action handlers (window `on*` callbacks → `send`) | `summonHero` :5786 · `intelligentCreatureRecord` :5796 · `summonCreature` :5807 · `releaseCreature` :5813 · `cycleCreaturePickupMode` :5822 · `setHeroBehaviour` :5856 · `recallHero` :5863 |
| `apps/web/app/page.tsx` | Window mount + open state | `showHeroPet` state :1485 · toggle hotkey **H** :1591 · `heroPet={{…}}` prop bag :11243 |
| `apps/web/lib/stage5-window-adapters.ts` | Defensive record → typed summary | `adaptHero` :191 · `pickupModeLabel` :228 · `adaptCreatures` :254 |
| `apps/web/app/components/original-client-hero-pet-window.tsx` | Presentation-only window (Hero / Creatures tabs) | `HeroPetWindow` :105 · `HeroSummary` :23 · `CreatureSummary` :55 · `HERO_BEHAVIOURS` :76 |
| `apps/web/lib/world-model/types.ts` | State slice types | `Stage5SystemsState.hero` :334 · `.intelligentCreatures` :343 · entity `mountLibrary` :67 |
| `apps/gateway/src/web.rs` | Cross-layer bridge (snake↔camel + `BrowserCommand`→`ClientPacket`) | `BrowserCommand::SetHeroBehaviour` :2906 · `::ChangeHero` :2911 · `::UpdateIntelligentCreature` :3086 · inbound `ServerPacket::MountUpdate` :5381 · `::UpdateIntelligentCreatureList` :5707 |
| `apps/simulation/src/runtime/packets.rs` | Sim handlers | `ClientPacket::UpdateIntelligentCreature` :7190 · `::NewHero` :7310 · `::SetHeroBehaviour` :7704 · `::ChangeHero` :7715 |

## 数据流 (How it threads the layers)

### Inbound — hero info end-to-end (`HeroInformation`)

```
sim → ServerPacket::HeroInformation { info: HeroUserInformation }  (protocol packets.rs:1937)
  → gateway server_packet_to_event arm                            (web.rs ~ "NewHeroInfo"/"HeroInformation")
       JSON: { packet:"HeroInformation", payload:{ info:{…camelCase…} } }
  → page.tsx case "HeroInformation" / "NewHeroInfo"               (page.tsx:8335)
       updateWorld → stage5Systems.hero.info = payload.info       (spreads prior hero slice; only sets `info`)
  → adaptHero(world.stage5Systems.hero)                           (adapters:191)
       reads record.currentHero.name / hp / level / class → HeroSummary
  → HeroPetWindow hero={…}                                        (window:105, Hero tab gauges + stat grid)
```

Other inbound merges follow the same shape, each writing ONE sub-key onto `stage5Systems.hero`:
`ManageHeroes` → `{ maximumCount, currentHero, heroes:count }` (page.tsx:7789); `ChangeHero` →
`{ fromIndex }`, `SetHeroBehaviour` → `{ behaviour }`, `UpdateHeroSpawnState` → `{ spawnState }`
(all three share one arm, page.tsx:7803); `HeroBaseStatsInfo` → `{ baseStats }` (8468);
`HeroCreateRequest` → `{ canCreateClass }` (8658); `UnlockHeroAutoPot` → `{ autoPotUnlocked:true }`
(8670). The live-stat trio `GainHeroExperience` / `HeroLevelChanged` / `HeroHealthChanged` share
one arm (page.tsx:7297) that mirrors hp/mp/level/experience onto the hero slice. `NewHero` (7786)
only logs a localized create-result via `heroCreateResultMessage` (`lib/extended-server-packets.ts:246`).

Creatures: `NewIntelligentCreature` / `UpdateIntelligentCreatureList` share one arm (page.tsx:7827)
that **replaces** `stage5Systems.intelligentCreatures` with `payload.creatureList` (falling back to
the prior list when absent). `IntelligentCreaturePickup` (8370) and `IntelligentCreatureEnableRename`
(8679) only append a system log line.

### Inbound — mount

`MountUpdate` (page.tsx:7234) does **not** carry the mount appearance. It patches the affected
entity to clear `movementAnimation`/`movementStartedAt`/`movementUntil` so walk/run speed
re-syncs; the actual mount sprite layer (`entity.mountLibrary` / `mountFrameOffset`, types.ts:67)
is re-derived client-side from the entity's `mountType` in the **next world snapshot** (Crystal
`Libraries.Mounts[MountType]`, see comment page.tsx:246) and rendered by the scene layer
(`original-client-scene-rendering.tsx`), not by this window.

### Outbound — summon / behaviour / creature

```
Hero tab "Summon"  → onSummonHero  → summonHero()      → send({type:"changeHero", listIndex:0})   (page.tsx:5786)
Hero tab "Recall"  → onRecallHero  → recallHero()      → send({type:"changeHero", listIndex:0})   (page.tsx:5863)  ← SAME packet as summon
Hero AI buttons    → onSetHeroBehaviour → setHeroBehaviour(k) → send({type:"setHeroBehaviour", behaviour:ordinal}) (page.tsx:5856)
Creature "Summon"  → onSummonCreature  → send({type:"updateIntelligentCreature", creature, summonMe:true,  …})  (page.tsx:5807)
Creature "Release" → onReleaseCreature → send({type:"updateIntelligentCreature", creature, releaseMe:true, …})  (page.tsx:5813)
Creature "Pickup"  → onCyclePickupMode → send({…, creature:{…, petMode:(n+1)%6}, all flags false})            (page.tsx:5822)
        │
        ▼ page.tsx send() (:4026)
  gateway browser_command_to_action:  BrowserCommand::ChangeHero (:2911) / ::SetHeroBehaviour (:2906) / ::UpdateIntelligentCreature (:3086)
        → ClientPacket::ChangeHero{list_index} / ::SetHeroBehaviour{behaviour} / ::UpdateIntelligentCreature{creature,summon_me,unsummon_me,release_me}
        ▼ sim runtime/packets.rs
  ChangeHero (:7715): spawns the stage5 hero (if map allows) and echoes ChangeHero + ManageHeroes + UpdateHeroSpawnState + HeroInformation
  SetHeroBehaviour (:7704): stores hero.behaviour, echoes SetHeroBehaviour
  UpdateIntelligentCreature (:7190): summonMe sets petMode, releaseMe removes the creature, else updates the stored record
```

The creature outbound path looks up the **full creature record** via `intelligentCreatureRecord`
(page.tsx:5796), keyed by the `creature-<slotIndex>` id the window uses — the packet echoes the
whole `ClientIntelligentCreature`, so the handler re-sends the stored record with the desired flags.

## 状态形状 (State shape)

- **`world.stage5Systems.hero?: Record<string,unknown> | null`** (types.ts:334) — a loose bag merged
  arm-by-arm. Keys observed across the handlers: `currentHero` (object `{name,level,class,…}`),
  `info` (HeroUserInformation), `baseStats`, `maximumCount`, `heroes` (a **count**, not the array),
  `fromIndex`, `behaviour` (number), `spawnState` (number), `canCreateClass` (array), `autoPotUnlocked`,
  and the mirrored live stats `hp`/`mp`/`level`/`experience`/`maxExperience`. `adaptHero` reads it
  defensively (`currentHero.name`→`name`, `class`→`classKey`, `hp`/`maxHp`/`level`/`mp`/`loyalty`/
  `attack`/`defence`), deriving `active` from `spawnState > 0`.
- **`world.stage5Systems.intelligentCreatures?: Array<Record<string,unknown>>`** (types.ts:343) — the
  raw `ClientIntelligentCreature[]`. `adaptCreatures` (adapters:254) maps each by `slotIndex` to
  `id:"creature-<slotIndex>"`, reads `customName`/`petName`→`name`, `icon`, `petLevel`/`level`, `hp`,
  `petMode`→`pickupMode` label (`pickupModeLabel`: 0 Both · 1 Group · 2 Guild · 3 None · 4 Attack ·
  5 Move), and derives `lifespan`/`maxLifespan` from `fullness` (0–1000).
- **Window-local React state** (window:118-119): `tab: "hero" | "creatures"` (defaults to the hero tab
  only when a hero exists) and `selectedCreatureId`.
- **`showHeroPet` boolean** (page.tsx:1485) — open/close, toggled by **H** (:1591).
- **Entity render fields** `mountLibrary` / `mountFrameOffset` (types.ts:67) — client-derived avatar
  layer, NOT part of `stage5Systems`.

## 坑 & 不变量 (Invariants & gotchas)

- **Hero dismiss / recall have NO Crystal client packet — this is a protocol gap, not a TODO.**
  Crystal `Shared/ClientPackets.cs` defines exactly three hero verbs: `NewHero` (:1200),
  `SetHeroBehaviour` (:1259), `ChangeHero` (:1275) — there is **no `DismissHero`/`RecallHero`/
  `UnsummonHero`** (`grep -rni dismiss\|recall packages/protocol/src/` returns nothing). In Crystal the
  HeroManage dialog "make active" button sends `C.ChangeHero { ListIndex = index+1 }`
  (`Crystal/Client/MirScenes/Dialogs/HeroDialogs.cs:829`); toggling the hero in/out beside the player
  rides that same packet. So:
  - `recallHero()` (page.tsx:5863) sends the **same** `changeHero listIndex:0` as `summonHero()`
    (5786) — it is a faithful no-better-wire mapping, not a bug.
  - There is **no `dismissHero` function** and the mount at page.tsx:11243 **does not pass
    `onDismissHero`**. The window's "Dismiss" button (window:272) therefore renders disabled
    (`disabled={!onDismissHero || …}`). Do not "fix" this by inventing a packet.
- **`ChangeHero` is a TOGGLE/SPAWN on the sim side** (packets.rs:7715): it spawns the stage5 hero if
  the current map allows (`current_map_disallows_hero`, map.rs:250) and emits a burst of echoes
  (`ChangeHero` + `ManageHeroes` + `UpdateHeroSpawnState` + `HeroInformation`); on a hero-disallowed
  map it emits a `server.CannotSummonHeroOnMap` system message instead. `SetHeroBehaviour` is a no-op
  if there is no hero or the behaviour is unchanged (packets.rs:7704).
- **`MountUpdate` does not change appearance** — it only invalidates the entity's movement animation
  (page.tsx:7234). The mount sprite comes from the next world snapshot's `mountType`. If a mount looks
  wrong, look at the snapshot/scene layer, not this packet.
- **`stage5Systems.hero.heroes` is a count, not the roster.** `ManageHeroes` stores
  `heroes: payload.heroes.length` (page.tsx:7798) and keeps the active one in `currentHero`. The
  full roster array is not retained client-side.
- **Behaviour ordinals are positional, round-trip-safe.** `setHeroBehaviour` maps
  `{attack:0,counterAttack:1,follow:2,custom:3}` (page.tsx:5856) onto Crystal's `HeroBehaviour` enum;
  the sim stores+echoes the raw number, so even if an ordinal drifts the round-trip stays consistent.
- **Adapter returns `null` when there's nothing to show.** `adaptHero` (adapters:202) bails to `null`
  if there is no name AND no hp AND no level — the window then shows the empty state and the tab
  auto-switches to Creatures (window:129).
- **Creature ids are slot-keyed.** Outbound creature actions resolve the record by
  `Number(slotIndex) === slot` parsed from `creature-<slot>` (page.tsx:5799). If the sim ever omits
  `slotIndex`, `adaptCreatures` falls back to the array index (`creature-<index>`) and the lookup in
  `intelligentCreatureRecord` (which matches on `slotIndex`) will miss — keep `slotIndex` populated.
- **`hero.info` vs `hero.currentHero`.** `HeroInformation` writes `info` (page.tsx:8343) but `adaptHero`
  reads `currentHero`/top-level fields, not `info`. `info`/`baseStats` are stored but not yet surfaced
  by the window — read them if you extend the stat sheet.

## 如何扩展 (How to extend / add to this area)

**Surface a new hero/creature datum in the window (inbound):**
1. `packages/protocol/src/{packets.rs,types.rs}` — add/extend the `ServerPacket` (or a field on
   `HeroUserInformation` / `ClientIntelligentCreature`), keeping it **optional**.
2. `apps/simulation/src/runtime/…` — emit it (cite Crystal `file:line` for the semantic).
3. `apps/gateway/src/web.rs` `server_packet_to_event` — add/extend the arm with **camelCase** keys
   (model on the `MountUpdate` arm :5381 or `UpdateIntelligentCreatureList` :5707).
4. `apps/web/app/page.tsx` — in the `switch` add/extend `case "<Packet>":`, then `updateWorld` to
   spread the prior `stage5Systems.hero` (or `intelligentCreatures`) and set your **one** new key.
   Do NOT replace the whole hero record.
5. `apps/web/lib/stage5-window-adapters.ts` — read the new key in `adaptHero` / `adaptCreatures`
   via `readString`/`readNumber`/`readBool` and add it to `HeroSummary`/`CreatureSummary`.
6. `apps/web/app/components/original-client-hero-pet-window.tsx` — extend `HeroSummary`/
   `CreatureSummary` (window:23/55) and render the new field; keep the prop optional.

**Wire a new button (outbound) — only if a Crystal `ClientPacket` exists for it:**
1. `apps/web/app/components/…hero-pet-window.tsx` — add an `on*?` prop and a button that calls it
   (presentation only; never `send` here).
2. `apps/web/app/page.tsx` — write the handler (`send({type:"<camelType>", …}`) and pass it into the
   `heroPet={{…}}` prop bag at :11243.
3. `apps/gateway/src/web.rs` — add the `BrowserCommand` variant (:585 region) + its arm in
   `browser_command_to_action` (:2570 region) → `ClientPacket::Foo`. Use `#[serde(alias=…)]` for any
   JS field name that isn't the snake_case of the Rust name.
4. `packages/protocol/src/packets.rs` — add the `ClientPacket` variant + `packet_id` mapping.
5. `apps/simulation/src/runtime/packets.rs` `handle_packet_impl` — add the arm that mutates the world
   and returns the `Vec<ServerPacket>` the client should observe.

> If there is **no** matching Crystal `ClientPacket` (the dismiss/recall case), stop at step 1 and
> leave the `on*` prop unwired — passing it would silently send the wrong packet. Document it as a gap.

## 相关 (Related)

- [`stage5-social.md`](./stage5-social.md) — sibling `stage5Systems` slices (group/friends/trade) read the same way.
- [`shell-rendering.md`](./shell-rendering.md) — where `heroPet={{…}}` is threaded into the window mounts.
- [`world-scene-render.md`](./world-scene-render.md) — how `mountType`/`mountLibrary` and `ObjectHero` entities become on-screen sprites.
- [`protocol-cross-layer.md`](./protocol-cross-layer.md) — the 5-layer recipe + the "some UI actions have no ClientPacket" rule.
- [`page-tsx-map.md`](./page-tsx-map.md) — block map of the `ServerPacket` switch in `page.tsx`.
- Source: `apps/web/app/components/original-client-hero-pet-window.tsx` · `apps/web/lib/stage5-window-adapters.ts` (`adaptHero`/`adaptCreatures`) · gateway `apps/gateway/src/web.rs` · sim `apps/simulation/src/runtime/packets.rs` (:7190–:7763) · Crystal `Shared/ClientPackets.cs` (hero :1200/:1259/:1275) + `Client/MirScenes/Dialogs/HeroDialogs.cs`.
