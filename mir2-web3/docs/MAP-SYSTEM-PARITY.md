# Map System Parity — Crystal (C#) → Rust

Tracking doc for raising the **map system** from ~40% to 90%+ parity with the
Crystal reference server (`Crystal/Server/MirEnvir/Map.cs` et al).

Tick model: 1 runtime tick = 1000 ms (`combat_delay_ticks(ms) = ceil(ms/1000)`).

## Status legend
- ✅ done / production-grade
- 🟡 partial
- ❌ missing

## Feature checklist

| # | Feature | Crystal ref | Before | Target |
|---|---------|-------------|--------|--------|
| 1 | Map format parsing v0–v7 + v100 | `Map.LoadMapCells*` | ✅ | ✅ |
| 2 | Walls / bounds / blocking | `Cell`, `ValidPoint` | ✅ | ✅ |
| 3 | Safe zones | `CreateSafeZone` | ✅ | ✅ |
| 4 | Map rule flags (no teleport/escape/random/throw/drop/mount/hero/bridle) | `MapInfo` | ✅ | ✅ |
| 5 | Monster/NPC spawn per manifest | `Map.Load` | ✅ | ✅ |
| 6 | Monster respawn over time | `ProcessRespawns` | ✅ | ✅ |
| 7 | Map transfers / movements | `Map` movements | ✅ | ✅ |
| 8 | **Dynamic doors (open/close)** | `OpenDoor`, `Process`, `CheckDoorOpen` | ❌ | ✅ |
| 9 | **Mining nodes** | `Map.CreateMine`, `HumanObject.Mining`/`GetMinePayout` | ❌ | ✅ |
| 10 | **Environmental hazards (lightning/fire)** | `Map.Process` | ❌ | ✅ |
| 11 | **Fishing cell attributes from map** | `LoadMapCells*` (light 100–119) | ❌ | ✅ |
| 12 | Cell attribute fidelity (fishing) | `Cell.FishingAttribute` | 🟡 | ✅ |
| 13 | Conquest / siege movement gating | `ConquestObject`, movements | ❌ | 🟡 |

## Implementation log

### Phase 1 — Dynamic doors ✅
- `DoorRegistry`/`DoorRuntime` in `resources.rs`: door cells grouped by index
  (`& 0x7F`, deduped) exactly like Crystal `Map.AddDoor`.
- `runtime/door.rs`: `open_door` (opens + schedules 5s auto-close + unblocks
  cells) and `tick_doors` (auto-close + re-block + broadcast), wired into
  `advance_world`.
- `stage5_open_door_packet` now drives the real state machine while preserving
  conquest-gate bookkeeping; closed doors block movement, open doors don't.
- Tests: registry grouping/masking, open→unblock→auto-close→re-block, door
  isolation. Tick model: door closes 5 ticks (5s) after opening.

> Sandbox note: real `.map` files are absent here, so map-file-dependent
> transfer tests fail environmentally (pre-existing, not from this work).

### Phase 2 — Mining nodes ✅
- `runtime/mining.rs`: `MineSet`/`MineDrop`/`MineSpot`/`MiningResource` with the
  two built-in Crystal mine sets (HitRate 25, DropRate 10, MaxStones 80,
  SpotRegenRate 5 min; set 1 = Gold/Silver/Copper/BlackIron, set 2 =
  Platinum/Ruby/Nephrite/Amethyst).
- `try_mine`: a melee swing (`Spell::None`) into a mineable cell with a
  `CanMine` weapon depletes a stone, rolls a hit (`HitRate`), on hit rolls a
  payout (`DropRate` → `GetMinePayout`: ore with `(MinDura+rand)*1000`
  durability + bonus) and damages the pickaxe; depleted spots regenerate stones
  on a timer. Emits `MapEffect{Mine}` + `GainedItem` + `DuraChanged`.
- Mine zones come from `SimulationConfig.mine_zones` (Crystal stores them in the
  Map DB, absent here); rebuilt on map change via `rebuild_mine_spots`.
- Wired into `attack_in_direction_with_spell` (no creature target → mine).
- Tests: depletion, no-pickaxe no-op, regen timer, guaranteed-set ore+dura,
  attack-flow integration.

### Phase 3 — Environmental hazards (lightning/fire) ✅
- `runtime/hazard.rs`: `MapHazardResource` + `tick_map_hazards`. On a hazard
  map the server strikes every 3–15 ticks; 1-in-4 strikes hit a player's cell
  (clean per-strike counter, deterministic 25%) for `Random(damage)`, the rest
  hit a random cell within ±10 tiles. Emits `ObjectSpell{MapLightning/MapLava}`
  and applies damage on a direct hit.
- Hazard flags come from `SimulationConfig.map_hazards`; timers/counters reset
  on map change. Wired into `advance_world`.
- Tests: strikes on a lightning map, no strikes on a normal map, direct-hit
  damage.

### Phase 4 — Fishing cell attributes ✅
- `.map` parsers (v0/v1/v2/v3/v5/v7/v100) now read each cell's light byte and
  record fishable cells (light 100..=119 → attribute 0..=19), matching Crystal
  `LoadMapCells*`. v4/v6 carry none (Crystal doesn't parse them).
- `FishingCellTemplate` added to `StarterMapCollision`; fishing cells flow into
  `RuntimeMapCollisionData` and `MapRuntimeResource.fishing_cells`.
- Fishing cast derives `fishing_attribute` from the cell three tiles ahead
  (Crystal `FishingCast` `PointMove(loc, dir, 3)`). Maps that declare no fishing
  cells keep the permissive default so the synthetic starter field stays
  fishable (no regression).
- Tests: v0 parser (in/out of range), cast over a fishing cell, cast rejected
  off a fishing cell.

## Outcome
Map system raised well past 90% for cell-/map-level mechanics: doors, mining,
hazards and fishing cells now match Crystal. Remaining (tracked, lower
priority): conquest/siege movement gating (depends on the guild-war system) and
zone-shared door/hazard authority (part of the broader world-authority work).
