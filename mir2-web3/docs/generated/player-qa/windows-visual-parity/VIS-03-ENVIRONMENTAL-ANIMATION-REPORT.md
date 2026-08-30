# VIS-03 Windows environmental animation report

Date: 2026-08-29

Status: bounded automated checkpoint only; exact Candidate timed capture and human acceptance remain open.

## Claim state

```text
implementationRevision: b7367fd72cebbf7d3a3bc9095a269555fbda9466
branch: codex/windows-visual-parity
type100TileAnimationFieldsParsed: true
tileAnimationLayerResolved: true
middleFrontTickDwellResolved: true
incompleteAnimationFamiliesFailClosedToBaseFrame: true
runtimeAnimationClockCheckpointPassed: true
mapParserAnimationCheckpointPassed: true
sameExeTimedCaptureProduced: false
fullVisibleAnimationFamilyAuditComplete: false
additiveBlendParityAccepted: false
nativeThirtyMinuteSoakProduced: false
humanVisualAccepted: false
accepted: false
```

## Closed bounded leaf

- The Windows map parser now reads Crystal type-100 tile-animation fields from
  the authoritative map bytes:
  - `tile_animation_image`
  - `tile_animation_offset`
  - `tile_animation_frames`
- Map draw resolution now preserves three distinct animation families instead
  of flattening them into one static frame:
  - dedicated Shanda tile-animation layer from `MapLibs[190]`
  - middle-layer animation count and tick dwell
  - front-layer animation count and tick dwell
- The runtime now preloads every resolved phase of an animation family and
  toggles visibility with a Crystal-compatible 100 ms global clock. This keeps
  the family atomic once resident and avoids rebinding textures every frame.
- If a packaged family is incomplete, the renderer holds the source base frame
  as one stable draw instead of flashing transparent or alternating between
  present and missing phases.

## Why this remains bounded

- This checkpoint proves parser/runtime intent and atomic-family behavior, not
  end-user visual closure.
- It does not yet prove that every visible animated lamp, brazier, fire, tile,
  or additive environmental effect in the audited maps has its full packaged
  family present.
- It does not yet prove same-EXE native window timing, GPU blend parity,
  sustained map-transfer stability, or human-visible equivalence to Crystal.

## Automated evidence on the current head

| Gate | Result |
| --- | --- |
| `map_parser::tests::resolve_map_draws_preserves_crystal_animation_family_parameters` | PASS |
| `map_parser::tests::render_state_emits_complete_animation_family_with_phase_metadata` | PASS |
| `map_animation_tests::crystal_animation_clock_advances_once_per_hundred_milliseconds` | PASS |
| `map_animation_tests::crystal_animation_phase_matches_frame_and_tick_dwell_formula` | PASS |
| `map_animation_tests::animation_family_exposes_exactly_one_phase` | PASS |

Commands run on `2026-08-29` against the exact implementation committed as
`b7367fd72cebbf7d3a3bc9095a269555fbda9466`:

- `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml resolve_map_draws_preserves_crystal_animation_family_parameters -- --nocapture`
- `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml render_state_emits_complete_animation_family_with_phase_metadata -- --nocapture`
- `cargo +1.95.0 test --manifest-path apps/game-client/runtime/Cargo.toml map_animation_tests -- --nocapture`

## Explicitly open gates

- Exact Candidate timed capture showing at least two correct visible phases for
  a real in-scene environmental animation.
- Full residency audit for the visible animation families used by the audited
  map route.
- Additive and foreground ordering comparison against Crystal in the same
  scene.
- Map-transfer retest on the exact Candidate, including the user's earlier
  GroceryStore failure route.
- Native 30-minute soak, real DPI, authenticated same-EXE live WSS, and human
  visual/feel acceptance.

This report closes one bounded automation numerator for environmental
animation. It does not claim full world-render parity or overall Windows
acceptance.
