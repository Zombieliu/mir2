# Native fallback font identity and OOM repair — 2026-09-23

## Failure and attribution

The player's old `215ea95b7` client (PID 49816) recorded font atlases growing
from 2 MiB/two font Blob IDs at 10 seconds to 15,125 MiB/8,866 IDs at 620
seconds, followed by `OutOfMemory RenderError` at 2026-09-22 21:32:27 UTC.
Those byte counts are CPU atlas image data, not a direct GPU memory reading.
Source log: `C:/mir2-ui-repair-20260921/render-live/20260923-052201-894-client.stderr.log`.

The prior named Arial/YaHei pinning covered explicit family selection but
not all system-selected Chinese and symbol fallback faces. Bevy 0.19's atlas
key contains `run.font().data.id()`. Fontique 0.9's ordinary source cache can
expire at Bevy's `Last` prune (age 2), or during Parley layout (age 128).
Reloading the same file creates a new Blob ID; Bevy retains the old atlas.
Repeated label/panel recreation therefore keeps allocating new atlas pages.

A new test using the unchanged seven pinned faces and actual game-like
Latin, Chinese and symbol strings fails before the fix: eight complete
hide/expiry/rebuild cycles change 6 IDs/9 MiB to 30 IDs/49 MiB. This is a
reproduced cache lifecycle defect, rather than an inference from the total
asset lookup counter. It does not identify every individual font in the old
live recording, which did not record font bytes/family names.

## Repair

`platform-windows/src/native_fonts.rs` enables Fontique's shared source
cache before first text layout. A native resource retains a single shared
byte reference per actually used font Blob ID, collected from changed
`ComputedTextBlock` layouts after UI PostLayout and Text2d layout. The shared
cache can recover that same Blob through its weak reference after ordinary
cache expiry or complete text-entity teardown.

Both parts are necessary: shared lookup alone loses the Blob when all owners
disappear; retaining a Blob alone does not reconnect an unshared file cache.
The resource lifetime matches the existing font atlas lifetime. It does not
register fallback faces as new font assets, change family precedence, change
TTC face indices, remove glyphs, disable fallback, or clear live atlas images.
Missing named font files do not disable identity retention.

Current game input fields use ordinary Text/ComputedTextBlock. Bevy's separate
built-in EditableText editor is not used by production code; if it is added,
its separate editor layout also needs retention. No dependency, server,
character save, or gameplay operation changes in this patch.

## Verification

- Before-fix reproduction: fails as expected, 9 to 49 MiB in eight cycles.
- Seven focused font tests pass, including 2,000 hide/rebuild cycles and
  10,005 layouts at a constant 6 IDs/9 MiB; 51 dense 200-label frames and
  10,200 layouts also remain at 6 IDs/9 MiB.
- Independent negative controls remove either shared lookup or strong
  retention: both reproduce 9 to 49 MiB growth.
- The selected source bytes, TTC face indices, raster image bytes, glyph
  positions, and line indices match the pre-fix path for all mixed samples.
- An ECS scheduling test covers post-layout collection, deferred entity
  creation, complete teardown, and unavailable named-font asset pinning.
- Full Windows unit regression: 722 passed, zero failed; two explicit GPU
  tests are ignored by the ordinary suite and are run separately.
- Real offscreen GPU negative control, using the full Bevy UI and rendering
  pipeline: 52 to 84 MiB font image data, 56 to 88 CPU/GPU images in the
  bounded run. The test exits after demonstrating growth, well below OOM.
- Fixed GPU soak passes for 660.009 seconds and 39,375 rendered frames. It rebuilds 200
  labels per visible frame, changes HP values, and periodically destroys
  every label for four frames. Counts remain exactly 5 IDs/8 MiB,
  12 main-world images and 12 uploaded GPU images from baseline to finish.
  Separate process sampling during the later part of this run remains about
  576 MiB private memory, with no monotonic allocation growth. It is not a
  whole-game process-memory measurement.
- Targeted formatting and `git diff --check` pass. Native release build passes.
- Independent read-only code review found no blocking issue in the repair
  or tests. Physical gameplay/whole-client stability acceptance is separate.

Logs under `C:/mir2-ui-repair-20260921/`:

- `font-oom-reproduction.log`
- `font-oom-fixed-tests.log`
- `font-oom-native-regression.log`
- `font-oom-gpu-before.log`
- `font-oom-gpu-after.log`
- `font-oom-gpu-process.jsonl`
- `font-oom-release-build.log`

The tests create no game connection or desktop window and do not touch player
state. CPU and GPU atlas tests cover the demonstrated font-cache mechanism;
they do not establish that every possible gameplay OOM or crash is fixed.

## Deployment

The offline release build passes in 21.51 seconds. Prepared package:
`C:/numeron-legend-of-rebirth-20260923-font-memory`.

- Client SHA256: `DAC021DEE90E44591199E9EE3E2119CC98A16005F6D2BE0445EEBF216A01ACCC`.
- Retained Gateway source `e65bdc4ebcd99b0c80658c087a1bf2f26c4cbc33`, SHA256:
  `355E8461074A130C9E9B7CE2AA6039DC3DFBE25B2F58F8677EA29A74E7FD65D6`.
- Unchanged client config SHA256:
  `01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.

Both ordinary game processes and ports were absent before handoff preparation.
The original account file was copied and hash-verified at
`C:/mir2-ui-repair-20260921/player-save-backups/20260923-201539-559-font-memory-accounts.json`.
Launch retains the original account store/profile/identity keys, fixed-daylight
setting and prior NPC/quest fixes. Physical gameplay acceptance remains pending.

Source `5c0b381c77cbe7ae3c96a1147457391087541072` was committed and pushed.
The unchanged Gateway was launched as PID 39152; client PID 25920 is responsive
with title `numeron-legend of rebirth` and a confirmed WebSocket connection.
Exact render/font diagnostics remain enabled. No native login/gameplay input
or forced process close was issued. Launch record:
`C:/mir2-ui-repair-20260921/render-live/font-memory-client-launch.json`.
Client log prefix: `20260923-202103-948`. The package explicitly records
`fontCacheLeakResolved=true`, `wholeGameStabilityAccepted=false`, and
`visualAccepted=false`.
