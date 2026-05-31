# Magic / combat / persistence test failures → green: diagnosis & plan

Status: **lib failures 70 → 3** on branch `claude/optimistic-mayer-gswKV`
(PR #8), with **zero regressions** at every step (baseline failure set stays a
strict superset; verified by `comm` on each engine change). Fixed this effort:
all 50 magic_packet_* tests, all 9 combat damage-branch tests, all 5
persistence/save/reconnect tests, and the soak test (via safe-zone immunity).

The **3 remaining** are map-data (manifest) drift, not code bugs:
- `crystal_current_map_spawn_table_uses_representative_map_rosters` — expected
  spawn roster rules absent from the current respawn manifest.
- `crystal_manifest_movements_skip_crystal_invalid_direct_transfers` and
  `walk_onto_blocked_crystal_manifest_movement_source_transfers_map` — reference
  map movements (e.g. 322,248→0104/Library) not present in the current
  `crystal_respawn_manifest.json`. These need the manifest regenerated from the
  Crystal submodule (empty in this container) or the test expectations re-pinned
  to the current data — a data task, not an engine fix.

## Safe-zone player damage immunity (engine, Crystal parity)
Added `combat::current_player_in_safe_zone`; `resolve_pending_combat_actions`
skips incoming monster/PvP combat damage (no Struck) while the player stands in
a safe zone. Poison/bleeding DOT still ticks (Crystal-accurate). The combat
tests that fought at/near the town spawn (a safe zone) were relocated to the
in-bounds, confirmed-non-safe tile `(322,277)` so they still exercise real
damage. NOTE: the live runtime's safe zones (config + Crystal manifest) are
larger than a naive bounds estimate, and the map playBounds is x318–342 /
y261–279 — probe `is_combat_position` rather than guessing tiles. `(343,281)`
is out of bounds; `(322,277)` is valid.

`bomb_spider_explodes_when_adjacent_and_damages_player` was a genuine *test*
bug surfaced (not caused) by the immunity: it spawned an owner-*summoned* bomb
spider, which the engine correctly treats as friendly (detonates on hostile
monsters, never the player), so it never damaged the player. The test only
passed at baseline because the idle town player was being hit by ambient
Royal_Archers — masking the bug — which safe-zone immunity removed. Fixed by
spawning a hostile bomb spider (owner `None`, summoned `None`, hostile
override `Some(true)`).

Verified engine+tests together: **70 → 3** single-threaded lib failures, zero
regressions (each remaining failure was already failing at baseline), zero
warnings, full workspace builds.

## (historical) magic-test status
The 19 remaining (at that earlier checkpoint) were unrelated to magic: combat
damage-branch tests, storage/persistence, soak, and manifest-drift transfers.

## Root causes fixed (all engine-correct, no behaviour hacks)

1. **Safe-zone origins (dominant).** Offensive-magic preflight forbids casting
   from/onto a safe-zone tile; tests used coordinates (notably `(333,267)` and
   open-tile searches) that the Bichon manifest safe zones now cover. Fixed with
   `find_combat_origin_box` / `find_combat_origin_line` / `is_combat_position`
   test helpers and `is_combat_position` guards on search idioms.
2. **Required items.** PoisonShot/CrippleShot need a poison amulet; binding_shot
   / delayed_explosion / explosive_trap / trap need an Amulet. Added equips.
3. **Duplicate object ids.** Two tests cast at `target_id 3002`, which resolved
   to the starter-scene monster inside the safe zone (out of range from the
   relocated caster). Spawn an own target with a unique id.
4. **Cast-kind / offensive misclassification (engine).** Several spells defaulted
   to single-target `Target` (or `offensive`) and bailed:
   - SelfOnly buffs: `SoulShield`, `BlessedArmour`, `UltimateEnhancer`,
     `MoonMist`.
   - Non-offensive cleanse: `Purification` (Target-kind, may target an ally, but
     must not require a hostile target).
   - Directional/self-centred AoE: `IceThrust`, `HeavenlySword`, `ThunderStorm`
     (cast with `target_id 0` + a facing) → `Direction`.

These were genuine engine bugs (the spells were uncastable as designed), fixed
in `skills.rs::crystal_spell_cast_kind` / `crystal_spell_is_offensive`.

## Combat damage-branch: ALL 9 fixed

Root causes (test-setup, no engine behaviour changes beyond the cast-kind
corrections already noted):
- **Player died mid-combo.** armadillo (3 half-DC hits) / snow_yeti (double hit)
  schedule all hits in one tick; a hit applied to an already-dead player emits no
  Struck, so the default-HP player died and fewer hits registered. Boost HP.
- **Damage capped at starting HP.** general_meow triple-DC slam is huge; the
  observed loss capped at the player's HP rather than the computed damage. Boost
  HP so the full hit is observed.
- **Green-poison DOT contaminated the measure.** water_dragon's ranged hit also
  applies green poison whose first tick (5) lands in the same tick; add it to the
  test's expected damage.
- **Summon/trap cast bailed.** SummonShinsu needs an amulet (equip it); Stonetrap
  is a friendly self-placed trap → reclassified `SelfOnly` (it defaulted to
  Ground+offensive, whose context-free `cast_skill` preflight bails).

## Remaining 10 (separate subsystems — not magic/combat-branch)
- Persistence/save (4): `file_account_store_survives_fresh_config_reload`
  (`Login{result:4}` — looks env/file-store dependent), `storage_items_persist`,
  `storage_password...`, `item_roll_fields_persist` (missing packets after reload).
- Manifest drift (2): `crystal_manifest_movements_skip_crystal_invalid_direct_transfers`,
  `walk_onto_blocked_crystal_manifest_movement_source_transfers_map` — reference
  map movements (322,248→0104) absent from the current respawn manifest.
- Soak/reconnect (2): `long_running_tick_soak_preserves_player_state_without_panic`,
  `stage3_playable_pve_loop_persists_after_reconnect`.
- Misc (2): `crystal_current_map_spawn_table_uses_representative_map_rosters`,
  `mental_state_trickshot_reduces_crystal_archer_shot_damage`.

Session total: lib failures 70 → 10, **zero regressions** (baseline failure set
is a strict superset, verified by `comm`). All 50 magic + all 9 combat-branch
fixed; engine fixes were genuine cast-kind/offensive corrections.

---

## Historical diagnosis (kept for reference)

## Remaining 10 (need per-test work, not the safe-zone template)

These now pass preflight (cast resolves) but fail on spell-shape / fixture
assertions, or depend on starter-scene geometry inside the safe zone:

1. `..._halfmoon_crosshalfmoon_and_heavenly_sword_hit_shapes` — multi-session;
   constant origin (310,275); a sub-target takes no damage (arc-shape geometry).
2. `..._ice_thrust_hits_three_column_path_and_freezes` — `near` (origin.x+2)
   untouched; the front tile (origin.x+1) is occupied by `outside` — likely a
   path/occupancy interaction in the spell, possibly an engine shape bug.
3. `..._moon_mist_hides_and_hits_nearby_targets` — missing AddBuff packet.
4. `..._purification_removes_player_curse_debuff` — missing expected packet.
5. `..._shoulder_dash_moves_pushes_and_reports_blocked_failures` —
   `dash_locations` empty (push/dash movement geometry).
6. `..._skill_gain_multiplier_scales_practice_experience` — relies on starter
   monster 3002 near spawn (inside safe zone); needs own spawned target.
7. `..._special_arrow_shots_queue_damage_and_apply_visible_buffs` — missing
   packet (StraightShot/DoubleShot may need a bow or different setup).
8. `..._thunder_storm_hits_current_location_square_and_reduces_living_damage` —
   `undead` at candidate.Left not struck; ThunderStorm square geometry.
9. `..._ultimate_enhancer_consumes_amulet_and_scales_target_class_stat` —
   already equips Amulet; missing packet (target class/stat scaling).
10. `..._progresses_user_magic_and_emits_level_packets` — relies on starter
    monster 3002 near spawn; needs own spawned target outside the safe zone.
    (An attempt to spawn 3002 + combat origin still missed MagicDelay — FireBall
    projectile resolution from the relocated origin needs a closer look.)

Approach for the tail: read each spell's hit/shape implementation in `skills.rs`
and align the test's target offsets / facing with it; for the two starter-scene
tests, spawn an own target outside the safe zone. A few may be genuine engine
shape bugs — verify against Crystal before changing engine code.

## The failures

A single-threaded `cargo test -p mir2-simulation --lib` shows ~70 deterministic
failures. ~50 are `magic_packet_crystal_*` (offensive-spell behaviour), the rest
are a varied tail (storage persistence, stage3 reconnect, a few combat-branch
and manifest-drift cases). They fail on the branch **baseline** — they are NOT
caused by the movement work; the movement commits add zero regressions (verified
by a byte-identical failing-set diff).

## Root cause (proven end-to-end)

These are **test-setup bugs, not engine bugs.** Offensive-magic preflight
correctly forbids casting from/onto a safe-zone tile:

- `skills.rs::crystal_skill_context_preflight` →
  `if metadata.offensive && crystal_magic_point_in_safe_zone(world, &player_position) { return false; }`
  (and the same check on the target tile).
- `crystal_magic_point_in_safe_zone` → `map::is_safe_zone_point`, which unions
  the config safe zone **and the Crystal manifest safe zones** for the map.

The Bichon (map "0") manifest safe zones are large and overlap the coordinates
the tests use:

| centre | radius | covers |
|--------|--------|--------|
| (304,256) | 10 | x 294–314, y 246–266 |
| (328,264) | 10 | x 318–338, y 254–274 |
| (267,256) | 10 | x 257–277, y 246–266 |
| (331,330) | 10 | x 321–341, y 320–340 |
| (288,616),(650,629) | 10 | (far) |

Plus the config zone x 324–332, y 268–273.

Two failing patterns:
1. **Hardcoded origin `Point { x: 333, y: 267 }`** — this tile is inside the
   (328,264) manifest zone, so every offensive cast from it silently bails
   (`cast_skill_with_context` returns `Vec::new()` at the preflight guard → no
   damage / no poison / no packets). Confirmed via a temporary `eprintln` probe:
   `PROBE: lightning bailed at preflight`.
2. **Searched origin** (e.g. `(300..380).flat_map(... (240..310) ...).find_map`)
   — the search only checks `can_occupy` (collision), not safe zones, so it
   picks the first open tile, which lands inside a manifest safe zone.

A handful of the ~50 are genuinely different (e.g. ThunderBolt asserts an
undead-vs-living damage *comparison*; some assert push/buff/tracking) and need
per-test inspection after the safe-zone issue is cleared.

## Fix template (proven: Lightning is green)

Added a shared test helper in `runtime/tests.rs`:

```rust
fn is_combat_position(session: &SimulationSession, point: &Point) -> bool {
    let world = session.app.world();
    let config = &world.resource::<RuntimeConfigResource>().config;
    let map = world.resource::<MapRuntimeResource>();
    !is_safe_zone_point(config, map, point)
}
```

- **Searched-origin tests**: add `&& is_combat_position(&session, &tile)` to the
  `find_map` predicate for the origin and every target/AoE tile. (Done for
  `magic_packet_crystal_lightning_scans_six_tiles_in_facing_line` → passes.)
- **Hardcoded-origin tests**: replace the fixed `(333,267)` with a searched
  combat-clear origin, or a constant verified clear of every safe zone and
  walkable (e.g. around (340,275)+ — confirmed outside safe zones; still must
  pass `can_occupy`). Prefer searching so the test is robust to map data.

After moving the origin, re-verify each test's target geometry
(`origin.x + N`, AoE squares) still lands on walkable, in-bounds, non-safe tiles
and that the assertion's expected hit pattern holds.

## Remaining work (campaign)

- ~48 magic tests still red: apply the template, validating geometry per test.
  Many share `(333,267)` and can move together; batch by spawn pattern.
- Re-examine the non-safe-zone tail (ThunderBolt undead comparison, push/buff/
  tracking asserts, `magic_packet_progresses_user_magic`, projectile-family).
- Storage / stage3-reconnect / manifest-drift failures are separate from magic
  and out of this plan's scope.

## Verify discipline (this environment)
- One `cargo` command at a time, pipe to `tail`. No background agents, no stacked
  sleeps (a runaway background agent stalled an earlier session ~40 min).
- `cargo test -p mir2-simulation --lib <test_name> 2>&1 | tail -8` per test.
- Confirm no engine files change: the fixes belong in `runtime/tests.rs` only.
  If an engine change seems needed, re-check against Crystal — the safe-zone
  block is correct behaviour.
