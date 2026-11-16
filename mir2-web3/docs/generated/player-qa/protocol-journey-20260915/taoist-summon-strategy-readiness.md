# Taoist SummonSkeleton strategy readiness

Read-only source audit for the ordinary Taoist route (`JT3f4a4e1f`, Lv25,
trace `C:/mir2-protocol-journey-20260911/Taoist.2026-09-15T20-40-27-269Z.trace.jsonl`).
No gameplay, store, QA, or code mutation was performed.

## Source facts

- The authoritative item manifest names the book exactly `SummonSkeleton`
  (`item_index: 1020`, `item_type: 20` book, `shape: 65`), with `required_class: 4`
  (Taoist), `required_gender: 3`, `required_amount: 19`, and price 9000:
  `packages/game-data/data/generated/crystal_item_manifest.json:46463-46484`.
- Crystal NPC help text calls it `SummonSkeleton (Level 19)` and says it consumes
  MP and an Amulet; it also tells the player to obtain a level-appropriate book,
  double-click it in inventory, then activate it in the skill screen:
  `packages/game-data/data/generated/crystal_npc_manifest.json:17436-17456`
  and `:17870-17881`.
- The ordinary BookStore scripts expose only the first ten names in `[Trade]`
  (through `StraightShot`); `SummonSkeleton` appears in the help text but is not
  in that trade list. The parser builds public NPC goods only from `[Trade]`:
  `packages/game-data/data/generated/crystal_npc_manifest.json:5211-5310`,
  `apps/simulation/src/runtime/npc.rs:656-731`. Therefore no source-backed
  public NPC purchase path is proven for this book in the current data.
- Crystal drop tables do contain the book through normal monster drops. Examples
  include `AncientCaves/OmaCavern/Ancient_Skeleton` and Bone/Axe/Fighter/Warrior
  tables at `1/100`, elite at `1/50`, and their `0` variants at `1/10`; DeadMine
  zombie tables are `1/200` (normal) and `1/20` (`0` variant). The generated
  manifest contains 139 matching table entries, all in `Books` or Taoist sections:
  `packages/game-data/data/generated/crystal_drop_manifest.json`.
- Normal use is implemented. `UseItem` requires the book to pass class/gender/
  level checks, learns the skill only once, emits `NewMagic`, and consumes the
  book: `apps/simulation/src/runtime/items.rs:3065-3090`, `:3218-3335`,
  `:3436-3449`. At Lv25, the trace character satisfies the level gate.
- Cast requirements and shared-world handling are implemented. SummonSkeleton
  requires one equipped amulet (`apps/simulation/src/runtime/skills.rs:900-918`),
  consumes one, queues a `BoneFamiliar` after 500 ms, caps two, recalls an
  existing owned summon, and keeps it within the summoner range
  (`apps/simulation/src/runtime/skills.rs:6040-6095`). Gateway routes the spell
  as a summon and includes it in item-consumption preflight:
  `apps/gateway/src/routing.rs:7046-7110`.
- Public pet mode is protocol-supported: browser `ChangePMode` maps to Crystal
  `C.ChangePMode`, and the gateway exposes the `ChangePMode` acknowledgement:
  `apps/gateway/src/web.rs:1918-1930`, `:9590-9610`.

## Trace snapshot evidence

The last `worldSnapshot` in the supplied trace reports `knownSkills` only
`Healing` and `SoulFireBall`; there is no `SummonSkeleton`. Equipment includes
49 Amulets (and belt stacks of 2 and 24 earlier in the same snapshot), so the
cast reagent is present. The snapshot has `stage5Systems.petMode: 0`,
`summonedIntelligentCreatureType: 99`, `intelligentCreatures: []`, and the
only entity is the self player; there is no active monster pet. This is an
ordinary progression state, not evidence of a failed summon backend.

## Readiness conclusion

The concrete missing strategy is book acquisition/learning: route the Taoist to
normal Skeleton/OmaCavern or DeadMine hunting and harvest the ordinary
`SummonSkeleton` drop, then use the book at Lv25 with an equipped Amulet. Do not
assume a bookstore purchase unless a separate source table is added; the current
BookStore `[Trade]` data does not list it. Once learned, the existing normal
`Magic` packet path and Zone summon path are source-backed for spawn/recall and
Amulet consumption. Pet-mode toggling is available, but there is no active pet
in this snapshot to toggle. The evidence therefore points to player strategy
(book not yet acquired/used), not a confirmed gateway pet-spawn or AI bug.
