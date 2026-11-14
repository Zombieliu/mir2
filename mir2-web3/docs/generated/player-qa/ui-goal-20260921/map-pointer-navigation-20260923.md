# Map pointer coordinates and Mini Map navigation — 2026-09-23

## User-visible correction

The Big Map footer previously displayed `BigMapModel.player_location`, so moving
the mouse did not change XY. It now displays the tile under the cursor, using
the same `BigMapImageGeometry` crop/scale and floor operation as click routing.
The player marker remains independent. MouseLeave retains the last valid tile;
closing, focus loss, scene/session changes and World Map clear stale coordinates.
Preview maps use their own dimensions. Frame, list, controls and letterboxing
never produce map tiles.

A separate coordinate label and Big Map render cache avoid rebuilding the
whole dialog while hovering. The cache includes map/NPC/player/selection/scroll,
search/hunt state, fallback title, root identity and asset-server availability.
Same-tile cursor movement does not mutate Text or recreate its entity.

Mini Map left/right clicks now start the existing ordinary Walk/Run route.
The last rendered image profile/crop is inverted, including clamped map edges
and stretched small images. The snapshot is bound to current map index, epoch,
image index and dimensions, and checked against the native collision map.
It works before opening Big Map or receiving its `NewMapInfo`; no teleport or
debug position command is introduced. Existing collision, occupancy, replan,
server ACK, modal/death/drag/focus and cancellation rules remain in force.

Both maps may stay open. Old displayed Mini Map images and frames consume
clicks during a map change even when new metadata is absent, preventing world
click-through. Collapsed Mail/BigMap buttons use their actual footer position.
Successful clicks emit `mapClickRoute` movement telemetry with source/map/start/
target/step count. Unreachable or occupied destinations retain error feedback.

## Original Crystal comparison

Locally inspected `E:/mir2/Crystal/Client/MirScenes/Dialogs/BigMapDialog.cs`
MouseMove/MouseDown (approximately lines 594–626) derives the pointer tile from
image offsets and scale; both mouse buttons route, and leaving retains XY.
`MainDialogs.cs` MiniMapDialog (approximately lines 1764–2100) only draws and
has Mail/BigMap/Toggle controls. Mini Map click navigation is a requested
convenience extension, not a claim of original Crystal 1:1 behavior.

## Validation

- Shared client `cargo +1.95.0 test --offline --features native-ui --lib`:
  **1,074 passed**, including 8 new Big Map coordinate/cache cases and 2 new
  Mini Map geometry/identity cases.
- Windows native suite: **728 passed, 2 explicitly ignored GPU soak tests**.
  Six new Mini Map cases cover both buttons, three stage scales, no Big Map
  metadata, ACK pacing/release/Escape, nine stale/loading states, modal/death/
  focus/inventory overlap, an occupied target, Big Map coexistence and old/
  collapsed frame input shielding. Existing navigation/quest/movement tests pass.
- Independent read-only review caught map-open and map-transition input
  pass-through cases; implementation and regression fixtures were corrected.
- Changed-file whitespace checks pass. Original player store and running
  Gateway remain untouched during tests. No native gameplay input was issued.

The default native test EXE was locked by another application (Restart Manager
reported DeltaForce). It was not terminated. The final tests were instead
compiled with `cargo +1.95.0 rustc --offline --tests -- -C extra-filename=-map-ui-20260923-final`
and that exact EXE was run with `--test-threads=1`. This changes only the test
artifact name, preserving the dependency/profile configuration.

Logs under `C:/mir2-ui-repair-20260921/`:

- `map-hover-tests-20260923.log`
- `map-ui-shared-tests-20260923.log`
- `map-ui-native-build-20260923.log`
- `map-ui-native-tests-20260923.log`

## Handoff

Code source `d26e0d73ec43febb704a3923118c9c9215431532` is committed and pushed.
The offline Windows release build passes (1m 50s). Hash-verified package:
`C:/numeron-legend-of-rebirth-20260923-map-ui`.

- Client SHA256: `C514129FCF2935138D7BB42219994D50766B2D662F1E18C7564C8E2D742C5FF3`.
- Unchanged Gateway SHA256: `355E8461074A130C9E9B7CE2AA6039DC3DFBE25B2F58F8677EA29A74E7FD65D6`.
- Unchanged config SHA256: `01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.
- Assets share the existing project public-directory junction. Fixed daylight,
  newcomer V2 and the previous font/NPC/turn-in fixes are retained.
- Release build log: `C:/mir2-ui-repair-20260921/map-ui-release-build-20260923.log`.

At preparation, old client PID 25920 was still responsive and the unchanged
Gateway PID 39152 remained live. Requested ordinary logout and client close
before replacement to preserve the player's active progress. No forced stop,
store modification or second interactive game process was issued.
After the user confirmed normal exit, the old client was absent. The original
store was copied and hash-verified to the package manifest's `saveBackup` path.
New client PID **43284** is responsive with title `numeron-legend of rebirth`
and logged a successful WebSocket connection; unchanged Gateway PID **39152**
continues serving the same store. Movement/render/font metrics remain enabled.
Launch record: `C:/mir2-ui-repair-20260921/render-live/map-ui-client-launch.json`.
Log prefix: `20260923-214030-876`. Package manifest now records `deployed=true`,
`visualAccepted=false` and `wholeGameStabilityAccepted=false`.
Source tests establish controller behavior; physical player acceptance and
full-game visual/stability acceptance remain separate and pending.
