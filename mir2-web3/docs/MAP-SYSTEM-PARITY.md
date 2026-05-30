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
