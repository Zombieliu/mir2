# Movement System → 90%+ Parity: Plan & Progress

Status: **in progress** on branch `claude/optimistic-mayer-gswKV`.

This document captures the full implementation plan to bring the **movement
system** to production (Crystal/Mir2 1:1) parity, plus the exact edit sites, so
work can continue without re-deriving the analysis.

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

### 1. Server-side A* pathfinding — DONE (module written), needs wiring
- File added: `apps/simulation/src/runtime/pathfind.rs` (bounded 8-dir A*,
  Chebyshev heuristic, uses `movement::can_occupy`). **Currently NOT declared in
  `mod.rs`, so it is inert / build stays green.**
- TODO wiring:
  - `runtime/mod.rs`: add `mod pathfind;` (near the other `mod` lines).
  - `runtime/movement.rs::move_to_with_mode_impl` (lines ~557-608): replace the
    straight-line `step_point_toward_by` + `can_traverse_between` candidate with
    `pathfind::next_step_toward(world, &next_position, &target,
    move_distance_for_mode(running), Some(player_entity))`, falling back to the
    existing straight-line logic when it returns `None` (graceful degradation).
  - Optionally use it in `follow_player_with_stage5_hero` for hero routing.
  - Test: place a wall between player and click target; assert the player reaches
    a tile that requires a detour (straight line would be blocked).

### 2. Conquest-gated map transfers — `map.rs` + `config.rs`
- `config.rs` `MapTransferRecord` (struct @1741): add `pub conquest_index: i32`.
  Update ALL constructors (grep `MapTransferRecord {`): map.rs:310 (set from
  `movement.conquest_index`), config.rs defaults / any tests (set `0`).
- `map.rs::crystal_movement_transfer_records_for_map` (line 300): change filter
  from `|| movement.conquest_index > 0` (drop) to KEEP conquest movements, store
  `conquest_index: movement.conquest_index`. Keep dropping `need_hole`/`need_move`.
- Add gate helper (has `world`):
  ```rust
  fn conquest_transfer_allowed(world: &World, conquest_index: i32) -> bool {
      conquest_index <= 0
          || !world.resource::<MapRuntimeResource>()
              .conquest_wars.get(&conquest_index).copied().unwrap_or(false)
  }
  ```
- Apply gate in `transfer_for_current_player_position` (line 264 `.find`) and
  `is_current_map_transfer_source` (line 277 crystal branch `.any`): only treat
  the cell as an active transfer when `conquest_transfer_allowed`. During an
  active war the cell is still walkable (don't add it as a transfer source).
- Test: war active (conquest_wars[idx]=true) ⇒ no transfer; war off ⇒ transfer.

### 3. Slow-debuff cadence + Frozen blocks (zone) — `zone/runtime.rs`
- `zone_player_status_blocks_movement` (7249): also block on
  `CRYSTAL_POISON_FROZEN` (8) while unexpired (frozen = cannot move, matches
  Crystal).
- Add `zone_player_move_delay_ms(player, effective_running, now_ms)`:
  base = `movement_delay_ms(effective_running)`; if `native_status_poison &
  CRYSTAL_POISON_SLOW (4)` and unexpired ⇒ `base = base * 2`; if mounted (see #4)
  ⇒ `base = base * 2 / 3`. Use it in `consume_step_action` line 964-965 in place
  of `movement_delay_ms(effective_running)`.
- Test: slowed player's `movement_ready_at_ms` advances 2× a normal player's.

### 4. Mount speed + restriction (zone) — types/runtime/manager/join + protocol
- Carry mount into the zone: add `mount_type: i16`, `riding_mount: bool` to
  `ZoneJoin` (zone/types.rs) and `ZonePlayer::from_join`; populate from the
  session's `MountResource` where `active_zone_join_snapshot` is built.
- Speed: in `zone_player_move_delay_ms` apply the mounted factor (see #3). Mounts
  also let you move at run distance — optional: treat mounted walk as run.
- Restriction (`no_mount` maps): plumb the map's `no_mount` flag into
  `ZoneRuntime` (alongside `collision`); when a player joins / the zone is for a
  `no_mount` map, force `riding_mount=false` and emit `MountUpdate`.
- Add `ZoneCommand::SetMount { session_id, mount_type, riding_mount }` →
  updates player + broadcasts `ServerPacket::MountUpdate` to AOI observers.
- Dismount-on-hit is combat-adjacent; do it where the zone reduces player HP
  (native monster attack application) — set `riding_mount=false` + MountUpdate.
- Test: mounted player faster than unmounted; no_mount map forces dismount.

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
