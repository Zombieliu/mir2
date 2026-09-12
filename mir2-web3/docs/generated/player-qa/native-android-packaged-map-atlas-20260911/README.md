# Native Android packaged map-atlas evidence — 2026-09-11

## Result

Source implementation commit:
`15ec8e36685ebc36a28810ab2edca923715dfb6a` on
`codex/android-player-journey`.

The native Android host now has a real APK-assets path for the generated map
atlas pack:

- Gradle accepts `MIR2_ANDROID_WORLD_ASSET_ROOT` and stages only
  `generated/map-atlas/manifest.json` plus its PNG pages.
- Rust validates compact manifest schema 2, page/rect uniqueness and geometry,
  safe asset paths, declared statistics, and compressed/decoded memory limits.
- Every page must decode successfully with the declared dimensions before any
  page is published.
- After a validated and queued authoritative world snapshot, decoding runs on
  a background thread and raw RGBA pages enter the bounded/coalescing Bevy
  native ingress.
- Missing or invalid resources remain an explicit loading error. No blank or
  synthetic map is accepted as gameplay readiness.

This closes packaged map-page loading only. Android still lacks the
authoritative viewport `MapRenderState` producer and character/entity atlas
producer, so this is not visible-map, StartGame-bootstrap, online gameplay,
emulator, or physical-device acceptance.

## Local generated input

The generated input remained under ignored build output and was not added to
Git:

- pack directory: `apps/game-client/platform-android/target/local-world-20260910`
- manifest kind/schema: `mir2-map-atlas-manifest` / `2`
- manifest SHA-256:
  `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce`
- libraries: 10 tile/small-tile libraries
- pages: 49
- source rects: 2,111
- PNG bytes: 14,785,921
- decoded RGBA bytes: 47,185,920

The input pack is limited to the atlas sources listed by its manifest. It does
not establish complete Bichon object coverage or character rendering.

## Verification

Toolchain used:

- Rust `1.95.0`
- `cargo-ndk 4.1.2`
- Android API 31
- NDK `26.1.10909125`
- OpenJDK `17.0.18`
- target/ABI: `aarch64-linux-android` / `arm64-v8a`

Checks completed:

1. `MIR2_ANDROID_WORLD_ASSET_ROOT=... cargo +1.95.0 test --offline --lib`
   - 99 passed, 0 failed.
   - Five new tests include the real 49-page local pack plus compact fixture,
     traversal rejection, statistics drift rejection, and PNG dimension
     rejection.
2. `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31 ./apps/game-client/platform-android/build-android.sh`
   - Android target check passed.
3. Runtime focused native atlas ingestion test
   - 1 passed, 0 failed (227 filtered).
4. `./gradlew --no-daemon testDebugUnitTest`
   - 8 passed, 0 failed.
5. Release Rust library plus debug APK package gate
   - Gradle `assembleDebug` passed.
   - The first packaging attempt exposed the shell's Java 8 default; the final
     command used the already-installed Homebrew JDK 17 explicitly. No global
     Java selection was changed.

`git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains
blocked by pre-existing formatting drift in shared client/runtime files outside
this Android write set; those unrelated files were not reformatted here.

## APK evidence

Generated artifact (ignored, not committed):

- path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- bytes: 329,001,359
- SHA-256:
  `e4d95fb52d18adb110378f6c5b20fa3a4d62e728fcaa1584c3fffd6d1b285e38`
- packaged map PNG pages: 49
- packaged manifest SHA-256:
  `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce`
- native library present:
  `lib/arm64-v8a/libmir2_platform_android.so`

The debug APK was built without `MIR2_GATEWAY_WS_URL`, so it is deliberately
not an online test candidate. It was not installed or launched in this round.
No emulator screenshot, physical-device run, authentication, StartGame, map
frame, gameplay loop, background recovery, or disconnect/reconnect claim is
made.

## Next bounded slice

Build an Android/shared authoritative viewport draw-list producer from the
validated manifest rect index plus the actual map layout, then publish
`MapRenderState`. Acceptance for that slice is a deterministic Bevy render
registry/frame test using the packaged page keys. Character/entity atlas and
render-ready bootstrap remain separate subsequent slices.
