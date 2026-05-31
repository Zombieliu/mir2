# Shared World Authority — Status & Invariants

This document tracks the **multiplayer shared-world authority** workstream: the
degree to which the shared `ZoneRuntime`
(`apps/simulation/src/runtime/zone/`) — rather than each player's personal
`SimulationSession` — is the single source of truth for gameplay state that
multiple players observe together.

It is intentionally invariant-based rather than a single headline percentage,
because "90% of what" depends on which invariants you count. The scorecard below
is the honest accounting.

## Why this matters

Each connected player historically ran a *complete, isolated* `SimulationSession`
(its own ECS world, its own monsters, its own combat math). "Multiplayer" that
simulates the same monster independently in every session is not authoritative:
two players can disagree about a monster's HP, an attack's outcome, or who
looted a drop. The shared `ZoneRuntime` exists to hold the one true copy.

## Authority invariant scorecard

Legend: ✅ authoritative in the zone · 🟡 partial · ❌ still per-session / absent

| # | Invariant (observable shared state) | Status | Evidence |
|---|---|---|---|
| 1 | Player presence, position, movement validation | ✅ | `zone/runtime.rs` movement tick + occupancy |
| 2 | Monsters mirrored into the zone while visible (not lazily on first hit) | ✅ | gateway sync spawns every visible monster each tick (`routing.rs` `native_monster_spawns`); `spawn_native_monster` is idempotent (skips if present) so the zone copy is never overwritten by a stale per-session view |
| 3 | Monster HP is a single shared value | ✅ | `ZoneNativeMonster.hp`, `apply_native_monster_damage` |
| 4 | Monster AI (aggro, move, ranged/melee choice, summons, totems) ticks once in the zone | ✅ | `tick_native_monster` / `tick_native_summon_monster` |
| 5 | **Player → monster combat resolution** (hit/miss roll, `Random(MinDC..=MaxDC)`, monster armour) computed in the zone | ✅ **(this workstream)** | `zone_resolve_player_physical_attack`; previously the gateway pre-rolled a scalar in the attacker's personal session and the zone trusted it with no hit/miss and no armour |
| 6 | Player combat stat block kept fresh in the zone (equip/buff/level changes) | ✅ **(this workstream)** | `ZonePlayerCombatStats`, `ZoneCommand::UpdatePlayerCombatStats`, gateway delta-sync |
| 7 | Ground-drop spawn + ownership/contention arbitration | ✅ | `claim_ground_drop`, tombstones |
| 8 | Status effects on monsters (poison/freeze/stun/paralysis, controls) | ✅ | `tick_native_monster_damage_poisons`, `expire_native_monster_controls` |
| 9 | Area/projectile/summon spells resolved in the zone | ✅ | `resolve_pending_native_projectiles`, ground-spell ticks |
| 10 | Monster → player damage resolved in the zone | ✅ | melee and ranged roll the monster's authoritative Crystal damage (`zone_native_monster_player_attack_damage`), and `zone_player_native_incoming_damage` subtracts the player's authoritative armour — `Random(MinAC..=MaxAC)` for physical hits, `Random(MinMAC..=MaxMAC)` for magic hits (tagged on `PendingNativePlayerHit`), plus buff AC and the reduction buff. |
| 11 | **Magic/skill damage value** computed in the zone | ✅ | `player_cast_native_magic` recomputes the spell damage authoritatively via `zone_authoritative_magic_damage` (= `crystal_magic_damage_from_base` over the player's base) and ignores the gateway scalar when the player has an authoritative stat block. Value-identical to the old session formula, so authority moves without a balance change. Exception: PoisonCloud keeps the supplied value (its amulet bonus depends on inventory the zone lacks). Monster MAC subtraction on magic hits is the remaining defensive refinement. |
| 12 | NPC state / quest mutation authority | 🟡 | Re-investigated: this is **mostly already authoritative, just not in the per-map `ZoneRuntime`** — and correctly so. NPC presence is zone-retained (rows above); NPC global script variables + random seed + map entity side-effects (NPC-spawned objects) are **cross-map** shared state committed transactionally through the gateway's `SharedNpcWorldService` (`ApplyScriptOutcome`); per-player dialog/shop/quest is correctly session-local (like inventory). Moving the cross-map saved-value state into a per-map `ZoneRuntime` would **silo it per map — a correctness regression**. The genuine remaining nuance is that the shared commit uses optimistic concurrency (conflict-detected) rather than a single cross-map writer, which can race for counter-style NPCs; closing that is a cross-map single-writer redesign (needs the Crystal reference for global-vs-per-map variable semantics). |
| 13 | Single-writer tick correctness (in-process) | ✅ | monster think/attack windows (`next_ai_ready_at_ms`, `next_attack_ready_at_ms`) rate-limit actions, so N session-driven ticks per interval do not double-advance a monster |
| 14 | Cross-process / sharded authority (one owner process per zone) | ❌ | `ZoneOwnerLeaseAuthority` + fencing-token scaffolding exists; RPC handoff is future work |

## What changed in this workstream

Commits on `claude/wonderful-volta-EmhiA`:

1. **`feat(zone): make player→monster combat resolution authoritative`** — the
   zone now rolls `Random(MinDC..=MaxDC)` (+ buff DC) with its deterministic
   RNG, runs the Crystal accuracy-vs-agility hit check (a miss shows only the
   swing animation), and subtracts `Random(MinAC..=MaxAC)` using the monster's
   own Crystal-sourced defensive stats. New types `ZonePlayerCombatStats` /
   `ZoneMonsterDefense`; new command `UpdatePlayerCombatStats`. Backward
   compatible: with no stat block the zone falls back to the trusted scalar, so
   all 129 prior `shared_zone` tests pass unchanged, plus 6 new tests covering
   stat-driven damage, armour block, miss resolution, RNG determinism,
   shared-HP consistency across two attackers, and the refresh command.
2. **`feat(gateway): keep the zone's authoritative player combat stats fresh`** —
   the gateway re-sends the recomputed stat block during the per-action delta
   sync so equipment/buff/level changes reach the zone.

Test status in this environment: `shared_zone` 135/135 pass; gateway 247/249
pass (2 failures are pre-existing and data-dependent — empty Crystal submodule);
simulation lib 833 pass / 70 fail, **identical to the pre-change baseline** (no
regressions; the 70 are pre-existing/environmental).

## Score

Of the 14 invariants: **12 ✅, 1 🟡 (row 12, NPC — already mostly authoritative
in the correct cross-map place), 1 ❌ (row 14, cross-process distribution)**.

Splitting by kind:

- **Gameplay authority logic** (rows 1–13): ~12.5 / 13 ≈ **96%**. The single
  shared world computes movement, monster HP/AI, bidirectional combat
  resolution, magic damage, drops, status, and shares NPC state transactionally.
- **Including cross-process distribution infra** (row 14): ~12.5 / 14 ≈ **89%**.

Distribution (row 14) is a different class of work (a distributed-systems infra
track — single-owner RPC handoff + soak/load), not gameplay-authority logic.

## Remaining work

1. **Monster MAC on magic hits** — subtract the target monster's MAC when a
   player's magic damages it (the defensive analog of the player-side AC/MAC),
   across the scattered magic application paths (direct, projectile, ground). A
   bounded numbers refinement.
2. **NPC shared-mutation concurrency** — the cross-map NPC commit uses optimistic
   concurrency; a single cross-map writer would remove counter-style races. This
   is a design pass (needs the Crystal reference for global-vs-per-map variable
   semantics), **not** a move into the per-map `ZoneRuntime` (which would
   incorrectly silo cross-map state).
3. **Spawn-source authority** — let the zone own respawn timers from the map
   manifest instead of mirroring per-session spawns.
4. **Cross-process distribution** (row 14) — complete the `ZoneOwner` RPC
   handoff so a single owner process holds the write lock per zone, then
   soak/load test. The large distributed-systems effort.

The bidirectional combat-resolution authority (rows 5, 6, 10) and magic-value
authority (row 11) were the highest-leverage gameplay-authority gaps and are now
closed and tested.
