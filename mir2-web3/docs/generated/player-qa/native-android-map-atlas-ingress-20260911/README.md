# Native Android map-atlas ingress — 2026-09-11

## Source

- Branch: `codex/android-player-journey`
- Implementation commit: `31946256b340eea0628fc7df387c7127e3d3559e`
- Parent: Windows-sync merge `61b4da3e096cac4c5cc3e16b6f142b8aa5923fbb`
- Changed implementation files:
  - `apps/game-client/runtime/src/native_ingest.rs`
  - `apps/game-client/runtime/src/native_ingest_wasm.rs`
  - `apps/game-client/runtime/src/lib.rs`

## Closed boundary

The native runtime queue now carries validated raw RGBA map-atlas pages keyed
by atlas page. Repeated pages coalesce by key, queue byte/count limits apply,
and scene/session reset classification drops stale page generations. The Bevy
consumer drains those pages into the existing `RuntimeMapRenderAtlases` image
registry and advances its revision, so the existing render-readiness retry can
rebind a waiting map state after pixels arrive.

This is the native equivalent of the existing WASM
`setMir2MapRenderAtlas(key, width, height, pixels)` byte path. The WASM adapter
keeps a no-I/O matching variant so both compile targets share the same ingest
system shape.

## Verification

| Gate | Result |
| --- | --- |
| Runtime serial suite | 228 passed, 0 failed |
| Android host suite | 94 passed, 0 failed |
| Runtime WASM compile | `wasm32-unknown-unknown` passed |
| Android cross-compile | arm64, API 31, NDK `26.1.10909125` passed |

The new regressions reject malformed pixel lengths, prove per-page
coalescing, and prove that a valid native upload becomes a real Bevy `Image`
with the expected dimensions and bytes.

## Open acceptance

No APK was packaged or installed in this slice. No emulator, physical device,
approved Gateway, login, StartGame, movement, or online player-loop result is
claimed. Android still needs an asset-pack loader, atlas integrity/decode step,
real map draw-list and entity-render producers, and a render-ready bootstrap
gate before the shared shell may enter `InGame`.
