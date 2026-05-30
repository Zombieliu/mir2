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
