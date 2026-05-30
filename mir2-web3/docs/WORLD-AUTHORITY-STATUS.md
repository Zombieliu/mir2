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
| 10 | Monster → player damage **computed** in the zone | 🟡 | computed in `launch_native_monster_player_*`, but melee uses a fixed placeholder (`1`) while ranged already rolls Crystal stats; player base AC/MAC not yet subtracted (only buff AC). See "Remaining work". |
| 11 | **Magic/skill damage value** rolled in the zone | 🟡 | spell power is still computed in the attacker's session and passed as a scalar; the zone applies it. Moving the spell-power formulas into the zone overlaps the combat/skills numbers workstream. |
| 12 | NPC state / quest mutation authority | ❌ | NPCs remain session-local (`npc_script.rs`) |
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

## Remaining work to reach a true production-grade 90%

These are the honest gaps. Items 10/11 are partly the **combat-numbers** and
**skills** workstreams, but they also gate "authoritative":

1. **Monster → player melee damage** — replace the `1` placeholder in
   `launch_native_monster_player_attack` with the Crystal-sourced roll already
   used by the ranged path, and subtract the player's authoritative
   `Min/MaxAC` / `Min/MaxMAC` (now carried on `ZonePlayerCombatStats`) in
   `zone_player_native_incoming_damage`. Coupled change — must be tuned and
   re-baselined against several mitigation tests (defence buff, magic shield),
   which need the Crystal reference for the correct shield/absorb semantics.
2. **Magic/skill damage value authority** — move the per-spell power formulas
   (`crystal_magic_damage_from_base` and friends) into the zone so the zone,
   not the session, produces the magic damage number, and subtract monster MAC.
3. **NPC / quest authority** — promote NPC state mutation into the zone.
4. **Spawn-source authority** — let the zone own respawn timers from the map
   manifest instead of mirroring per-session spawns.
5. **Cross-process distribution** — complete the `ZoneOwner` RPC handoff so a
   single owner process holds the write lock per zone, then soak/load test.

Items 1–4 are individually bounded; item 5 is the large distributed-systems
effort. The combat-resolution authority (#5/#6 in the scorecard) was the
single highest-leverage gap and is now closed and tested.
