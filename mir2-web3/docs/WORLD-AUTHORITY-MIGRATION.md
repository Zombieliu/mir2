# World Authority Migration — shared zone simulation

Tracks promoting per-session map/door/hazard/combat/AI state to the shared
`ZoneRuntime` so co-located players see one authoritative world.

## Authority model (as found)
Movement is **dual-mode**, decided by presence:
- Solo (no `presence_key`): `ClientPacket::Walk/Run/Turn` → per-session
  `SimulationSession::move_player_by_direction`, validated against the
  per-session collision.
- In a shared zone (`presence_key = Some`): routed to `ZoneRuntime`, which
  validates movement against the shared `ZoneCollision`
  (`is_player_movement_blocked`) and broadcasts to co-located players. **The
  zone is authoritative while a player is in it.**

Already shared/authoritative in `ZoneRuntime` before this work:
- Players (position, hp, buffs, movement queue, attack/cast cooldowns).
- Native monsters (`ZoneNativeMonster`): AI think/attack timers, shared HP,
  poison/control, ground spells, summons — `tick_native_monsters`,
  `player_attack_native_object`, `resolve_pending_native_*`. Sessions sync their
  monsters into the zone via `SpawnMonster`; the zone simulates them and applies
  authoritative damage/kills/awards. **Combat + monster AI are shared.**

## Gaps closed by this work

### Shared doors ✅
- `ZoneMapCollisionData.doors` (index → cells) + `ZoneCollision.{doors, open_door,
  close_door}`: door cells start in the shared blocked set.
- `ZoneRuntime.open_doors` (index → auto-close ms) + `ZoneCommand::OpenDoor`:
  opening unblocks the door's cells in the **shared** collision (so every
  co-located player can walk through), broadcasts `OpenDoor{close:false}` to all,
  and auto-closes after 5 s (`tick_doors`) re-blocking + broadcasting close.
- Gateway routes `ClientPacket::OpenDoor` to the zone when the player is in a
  shared zone (`execute_zone_player_packet`); solo players still use the
  per-session door.
- Tests: open→unblock→auto-close→re-block; unknown index no-op.

### Shared hazards ✅
- `ZoneCommand::ConfigureHazards` + `ZoneRuntime.hazard` (`ZoneHazardState`):
  per-map lightning/fire flags, fed from the session's `map_hazards` config on
  zone join.
- `tick_hazards`: every 3–15 s each hazard strikes every player on the map; a
  1-in-4 strike lands on a player for authoritative damage (zone HP reduced +
  `ObjectHealth` to observers + `PlayerDamaged` applied to the session), the rest
  hit a nearby cell (broadcast `ObjectSpell{MapLightning/MapLava}`).
- Tests: strikes + authoritative damage on a configured map; silent without
  configuration.

## Status
With doors + hazards now shared (and combat/AI already shared), the
multiplayer-authoritative surface covers movement, collision, doors, hazards,
monsters, monster AI, and combat resolution — the real path to multiplayer
parity. Remaining hardening (cross-process sharding, persistence of zone monster
state across restarts) is operations/scaling work tracked in
`PRODUCTION-GAP-ASSESSMENT.md`.
