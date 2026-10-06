# Native Android sharded player atlas evidence — 2026-09-12

## Result

This Android-private slice started from
`codex/android-player-journey@97476eaad9f8bbbdf75801c0d07e92f1f58e1307`.
The native entity loader now accepts a bounded group of immutable atlases
instead of requiring one monolithic atlas. Atlas keys, page files, render page
keys, rect keys and canonical source paths must be unique across the complete
group. Manifest, atlas, page, rect, PNG and decoded-memory limits remain
fail-closed. Final partial pages are accepted only when every rect fits that
page's exact dimensions.

Only pages referenced by the current authoritative scene are decoded. The
local licensed proof pack has 35 one-root atlases, 102 PNG pages, 23,224 rects
and 48 fully unindexed rects. It covers the available `C*`, `A*`, `AR*` player
body/hair/weapon roots, `Mount/00` through `Mount/11`, and `Monster/003`.
Its manifest is 7,028,198 bytes and its PNG pages total 40,312,262 bytes. The
pack was staged only into generated APK assets through
`MIR2_ANDROID_ENTITY_ASSET_ROOT`; it and the APK remain ignored and are not
committed.

## Verification

- Rust 1.95.0 formatting passes and the Android `ui-preview` suite is 152/152
  (`rust-fmt.txt`, `rust-tests.txt`).
- The forced sharded-pack test requires the exact 35-root inventory, 102 pages
  and 48 unindexed rects, resolves representative Archer/mount layers, and
  passes 1/1 (`sharded-atlas-test.txt`).
- The ARM64 API 31 `uiPreview` APK contains the manifest and all 102 entity
  pages. It is 375,653,485 bytes with SHA-256
  `c14c47fd543943e6c15868e5770ab64b40b0d98352fae74cd8ad18e9af4a248c`
  (`apk-assets.txt`, `apk-package.txt`, `apk-sha256.txt`).
- Streamed install passed on the `sdk_gphone64_arm64` API 31 emulator at
  physical size 1080 x 2340 (`install.txt`, `device-*.txt`).
- The labelled offline specimen reached full-screen landscape Bichon
  render-ready at `(302,634)` with 849 map draws, seven entities, twelve entity
  layers and zero unresolved map/entity entries. The selected scene decoded 25
  of the 102 entity pages: 11,175,056 compressed bytes and 90,177,536 RGBA
  bytes, below the 128 MiB selected-page cap. All 48 unindexed occupants remain
  observable and unusable (`runtime-metrics.txt`, `ready-markers.txt`).
- The packet-presentation specimen reached remote backstep active and settled
  markers. `world-render-ready.png` and `world-render-action.png` are the
  visually inspected 2340 x 1080 frames; their hashes are recorded in
  `screenshot-sha256.txt`.

## Emulator renderer observation

The old local Emulator 31.3.10 software renderer produced the render-ready and
action evidence above, then reported a wgpu `DeviceLost` about 43 seconds after
render-ready. A repeat with `-gpu host` remained software-rendered and lost the
device before world-ready. This is recorded in
`emulator-renderer-observation.txt`; it means this run is not a stability or
soak pass. It does not invalidate the earlier bounded ready/action frames, but
the emulator image/renderer must be refreshed or the result repeated on a
physical device before sustained-render acceptance.

## Acceptance boundary

This is an explicitly labelled offline UI Preview using a local proof pack. It
does not approve that pack as the shared Web/Android release and does not prove
every player/equipment combination. No approved WSS endpoint or account was
provided, so this is not real login, roster, StartGame, live map transition or
online packet evidence. No physical Android device was attached, so touch,
IME, background/reconnect, thermals, sustained rendering and human visual
acceptance remain open. The tracked shared Web atlas was not modified.
