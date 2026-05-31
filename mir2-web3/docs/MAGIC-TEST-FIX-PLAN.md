# Magic / combat test failures → green: diagnosis & plan

Status: **root cause proven, fix template in place, campaign in progress** on
branch `claude/optimistic-mayer-gswKV` (PR #8).

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
