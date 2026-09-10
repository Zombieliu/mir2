# Native Android initial Bichon map-frame evidence (2026-09-11)

## Result

Code commit: `529189a6e7a5fab1a9797e12932525ffdc8341c7`

This slice connects an authoritative Android world snapshot to the first real
Bichon map frame. The build pack contains the approved compact-v2 atlas and a
bounded raw `0.map`. Android validates and decodes the atlas, strictly parses
Crystal type-100 map cells, projects the server scene center, and queues the
matching shared `MapRenderState` only after all inputs are accepted.

The producer currently handles atlas-backed, floor-safe, non-additive pieces.
It does not silently approximate keyed/additive object layers. At the real
Bichon `(302,634)` baseline it produces:

- map dimensions: `700 x 700`
- queued floor tiles: `607`
- referenced atlases: `7`
- unresolved keyed/additive object draws: `215`

Disconnect, map change, and reconnect advance a generation token, so stale
background map/atlas work cannot publish into a newer session.

## Source pack provenance

The generated pack is local build input and is ignored by Git:

`apps/game-client/platform-android/target/local-world-20260911-raw`

- source asset root:
  `/Users/henryliu/obelisk/numeron/mir2-web3/apps/web/public/original-map`
- pack helper SHA-256:
  `0b3a10fd557f4300530ed9fc2521954a51be47882d5469434a5f8aa2908e98c1`
- compact-v2 manifest SHA-256:
  `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce`
- atlas libraries/pages/source rects: `10 / 49 / 2,111`
- atlas PNG bytes: `14,785,921`
- source compressed `0.map.gz`: `512,762` bytes,
  SHA-256 `bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5`
- packaged raw `0.map`: `12,740,008` bytes,
  SHA-256 `ed4783215ffa989658f79892c2dd6720753fb111e1102cb14da8e6182d88d7f6`

The helper explicitly expands `0.map.gz` into `0.map` before Android resource
packaging. This avoids relying on AAPT's implicit compression/renaming behavior
and makes the Rust asset path deterministic.

## Verification

- Android Rust library with the real pack: `102 passed; 0 failed`.
- Java host tests, forced rerun: `8 tests; 0 skipped; 0 failures; 0 errors`.
- Android `aarch64-linux-android` API 31 target check: passed.
- Focused shared-runtime test
  `native_data_path_tests::native_map_atlas_upload_reaches_the_render_registry`:
  `1 passed; 227 filtered out`.
- Final release Rust build and debug APK package gate: passed.
- `git diff --check`: passed before the code commit.
- Repository-wide `cargo fmt --check` remains blocked by pre-existing formatting
  drift in unrelated shared files; those files were not changed in this slice.

## APK evidence

APK:

`apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`

- size: `329,006,324` bytes
- SHA-256:
  `1ec1c9ac1acdcd15924e5e770993c4b01fe78e8909d4382ec2c903b0f3c38a62`
- packaged atlas PNG pages: `49`
- packaged raw maps: `1` (`assets/generated/crystal-map-pack/0.map`)
- packaged native library: `lib/arm64-v8a/libmir2_platform_android.so`
- `MIR2_GATEWAY_URL`: empty
- `UI_PREVIEW`: `false`
- version name: `0.1.0-shared-ui`

## Acceptance boundary

This is source/build/data-path evidence, not a visual or online player-loop
acceptance:

- the map state reaches the bounded runtime queue, but this APK was not
  installed or launched and no visible frame was observed;
- the shared shell still lacks a render-ready `PlayerBootstrapped` transition;
- character/entity atlases and keyed/additive object rendering are absent;
- the 215 unresolved object draws mean this is not full Bichon parity;
- no Gateway URL was configured, so real authentication, roster, StartGame,
  network recovery, and online state restoration were not exercised;
- there is no emulator/device identity, screenshot, recording, or physical
  device acceptance for this APK.

## Next bounded slice

Add faithful keyed object plus character/entity atlas producers, make runtime
application receipt gate the render-ready bootstrap, and only then install the
result on the API 31 emulator for visible map/player verification. Physical
device input, keyboard, backgrounding, and reconnect acceptance remain a later
independent gate.
