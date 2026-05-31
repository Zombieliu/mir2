# Movement System → 90%+ Parity: Plan & Progress

Status: **in progress** on branch `claude/optimistic-mayer-gswKV` (PR #8).

This document captures the full implementation plan to bring the **movement
system** to production (Crystal/Mir2 1:1) parity, plus the exact edit sites, so
work can continue without re-deriving the analysis.

## Progress summary

| # | Work item | Status |
|---|-----------|--------|
| 1 | Server-side A* pathfinding (click-to-move routes around blockers) | ✅ done + tests |
| 2 | Conquest-gated map transfers (open in peace, seal at war) | ✅ done + tests |
| 3 | Frozen blocks movement + Slow doubles cadence (zone) | ✅ done + tests |
| 4 | Mount speed boost (zone, via MountUpdate state) | ✅ done + tests |
| 5 | AOI interest management w/ hysteresis | ⬜ next |
| 6 | Roadmap doc refresh | ⬜ pending |

Commits: `01844886` (A* module + plan), `b6f1c956` (wire A*, conquest, cadence),
`d9e10ce5` (mount cadence zone test). New unit tests: 6 pathfind + 5 zone
cadence/status + 3 conquest gate; new integration test: mount cadence. Full
130-test `shared_zone` suite stays green; whole workspace builds.

Note on #4 scope: rather than adding `ZoneJoin` mount fields + a dedicated
`SetMount` command, the mount state rides the existing observer-action path —
a client `MountUpdate` broadcast through `ZoneCommand::BroadcastPackets` flips
`ZonePlayer.riding_mount`/`mount_type` via `apply_observer_action_state`
(`zone/packets.rs`), which `zone_player_move_delay_ms` then honours. `no_mount`
map enforcement + dismount-on-hit remain follow-ups (see item 4 notes below).

> NOTE on environment: this session repeatedly hit very long tool-result
> delivery stalls (a runaway background research agent ran ~40 min before dying;
> stacked `cargo build` + wait calls created further multi-minute stalls). Do NOT
> spawn background agents or stack long-running foreground calls here. Run one
> `cargo` command at a time, pipe to `tail`, and avoid `run_in_background`.

## Verified current state (corrects the earlier PRODUCTION-GAP assessment)

The movement system is split into two paths:

1. **Single-session** (`runtime/movement.rs`, `resources.rs`, `map.rs`,
   `combat.rs`) — personal session / click-to-move / QA path.
2. **Shared Zone** (`runtime/zone/*`) — authoritative multiplayer path. This is
   the one that matters for real online play; the gateway routes Walk/Run/Turn
   here.

Already implemented (better than the assessment claimed):
- 8-direction walk(1)/run(2), occupancy + static collision, push/knockback
  (`movement.rs::push_player_in_direction`, used by monster_ai).
- Walk/Run/Turn cadence in zone: `ZONE_WALK_DELAY_MS=600`, `ZONE_RUN_DELAY_MS=300`,
  `ZONE_TURN_DELAY_MS=350`, `ZONE_RUN_GRACE_MS=1200`, input buffering 300ms,
  seq-dedup, blocked→correction, AOI diff, SaveTransform persistence.
- Movement blocked by status: single-session
  `combat.rs::crystal_player_movement_blocked_by_status` (paralysis/dazed/stun/
  frozen buffs); zone `zone/runtime.rs::zone_player_status_blocks_movement`
  (poison bitflags PARALYSIS=256 | STUN=16).
- Run gating via step counter (`resources.rs::crystal_player_can_run` /
  `mark_crystal_player_move`).
- Conquest war state already stored: `MapRuntimeResource.conquest_wars:
  BTreeMap<i32,bool>` (resources.rs:499) and `config.conquest_wars` (config.rs:1856).
- Mount fields already exist but are inert: `ZonePlayer.mount_type/riding_mount`
  (zone/types.rs:458-459), `resources.rs::MountResource`, protocol
  `ServerPacket::MountUpdate` (id 198), `config` map flag `no_mount` (config.rs:1768).

Real remaining gaps → the work items below.

## Work items (each independently committable + testable)

### 1. Server-side A* pathfinding — ✅ DONE
- `apps/simulation/src/runtime/pathfind.rs`: bounded 8-dir A*, Chebyshev
  heuristic. Refactored into a pure closure-based core `find_path_with` (unit
  testable, no `World`) + a `World`-backed wrapper `find_path` using
  `movement::can_occupy`. Declared in `mod.rs`.
- Wired into `movement.rs::move_to_with_mode_impl`: a new `pathfind_next_step`
  helper picks the next routed tile (run extends to the 2nd tile only when the
  route keeps the same direction, else degrades to a walk step); falls back to
  the original straight-line `step_point_toward_by`/`can_traverse_between` when
  no full route exists (e.g. clicking onto an occupied/monster tile), preserving
  the legacy "approach then stop" feel.
- Tests (`pathfind::tests`): open-ground diagonal, wall detour, walled-off goal,
  blocked goal tile, range cap, already-on-goal.
- Follow-up (optional): also route `follow_player_with_stage5_hero` and monster
  AI through `find_path` so heroes/monsters path around obstacles too.

### 2. Conquest-gated map transfers — ✅ DONE
- `config.rs` `MapTransferRecord`: added `pub conquest_index: i32` (0 =
  unconditional). All constructors updated (map.rs from `movement.conquest_index`,
  config.rs starter transfer = 0).
- `map.rs::crystal_movement_transfer_records_for_map`: no longer drops conquest
  movements — keeps them and carries `conquest_index`; still drops
  `need_hole`/`need_move`.
- Gate helpers: `conquest_transfer_allowed(&MapRuntimeResource, i32)` delegating
  to a pure, testable `conquest_transfer_allowed_with(&BTreeMap<i32,bool>, i32)`.
  Applied in `transfer_for_current_player_position` and
  `is_current_map_transfer_source` (both manifest + config branches). During an
  active war the cell stays walkable but does not transfer.
- Tests (`conquest_gate_tests`): unconditional always allowed; opens in peace /
  when unknown; seals during active war (and a different conquest is unaffected).
  Existing `walk_onto_crystal_manifest_movement_transfers_map` (peacetime
  transfer) still passes.

### 3. Slow-debuff cadence + Frozen blocks (zone) — ✅ DONE
- `zone_player_status_blocks_movement`: now also blocks on `CRYSTAL_POISON_FROZEN`
  (8) while unexpired (frozen = cannot move, matching paralysis/stun).
- Added `zone_player_move_delay_ms(player, running, now_ms)`: base =
  `movement_delay_ms(running)`; mounted (`riding_mount && mount_type>=0`) ⇒
  `base*2/3`; `zone_player_slowed` (CRYSTAL_POISON_SLOW=4, unexpired) ⇒ `base*2`.
  Wired into `consume_step_action` (replaces the bare `movement_delay_ms`).
- Tests (`movement_status_tests`): frozen blocks until expiry; green poison alone
  does not block; slow doubles the delay (and reverts on expiry).

### 4. Mount speed + restriction (zone) — ✅ DONE (speed); restrictions = follow-up
- Mount state reaches the zone via the existing observer-action path: a client
  `MountUpdate` sent through `ZoneCommand::BroadcastPackets` flips
  `ZonePlayer.riding_mount`/`mount_type` in `apply_observer_action_state`
  (`zone/packets.rs`). No new `ZoneJoin` fields or `SetMount` command needed.
- Speed: `zone_player_move_delay_ms` applies the mounted `*2/3` factor (item 3).
- Tests: `movement_status_tests` (mounted faster; riding flag without a mount
  type does not speed up) + integration `mounted_player_walks_a_step_sooner_than_
  an_unmounted_player` in `tests/shared_zone.rs` (full client→zone→cadence path).
- Follow-ups (not yet done):
  - `no_mount` map enforcement inside the zone: plumb the map's `no_mount` flag
    into `ZoneRuntime` and force `riding_mount=false` + emit `MountUpdate` on join.
    (Single-session toggle already respects `no_mount` in `equipment.rs`.)
  - Dismount-on-hit: clear `riding_mount` + `MountUpdate` where the zone reduces
    player HP from a native monster attack.

### 5. AOI interest management w/ hysteresis — `zone/aoi.rs` + 2 diff fns
- `aoi.rs`: add `AOI_HYSTERESIS_MARGIN` and `points_stay_visible` /
  `players_stay_visible` (range + margin). Keep `points_visible` (entry) as-is.
- `zone/runtime.rs::diff_visibility_for` (6699): for the `(visible_now,
  was_visible)` match, compute `visible_now` with entry range BUT keep an object
  visible while within stay-range — i.e. remove only when NOT
  `players_stay_visible`. Same for `diff_zone_object_visibility_for` (6176).
- ⚠️ Re-run `tests/shared_zone.rs`: boundary visibility tests may need the margin
  accounted for; keep margin small (1–2) and adjust assertions if needed.

### 6. Tests + docs
- New `apps/simulation/tests/movement_parity.rs` covering #1–#5.
- Update `docs/CRYSTAL-1TO1-ROADMAP.md` movement section and this file.

## Build/verify discipline for this environment
- `cd /home/user/mir2/mir2-web3 && cargo build -p mir2-simulation 2>&1 | tail -25`
- `cargo test -p mir2-simulation 2>&1 | tail -40`
- One command at a time. No background agents. No stacked sleeps.
