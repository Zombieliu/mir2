# Native Android locomotion animation evidence (2026-09-11)

## Version and scope

- Branch: `codex/android-player-journey`
- Implementation commit: `843f9459339d85c8088815f93f6ac856c8d7ee19`
- `ObjectWalk` and `ObjectRun` now use the same bounded Android-private action
  sidecar and monotonic frame clock as the earlier packet action baseline.
- The packet coordinate remains authoritative. This change adds no prediction,
  pathfinding, collision decision, replay or client-owned movement rule.

## Verification

- Android Rust suite: 116 passed, 0 failed.
  - The deterministic packet test verifies a direction-less Walk uses the last
    authoritative facing, applies the new server position, advances frames and
    returns to standing without another snapshot.
  - The packaged-atlas fixture verifies exact Down walking frames 56-61 and
    running frames 104-109 alongside the existing attack frames.
- Configured real shared-atlas test: 1 passed, 0 failed.
- API 31 `aarch64-linux-android` check: passed.
- Java MockWebServer/TLS suite: 11 passed, 0 failed.
- Normal debug APK package, streamed install and cold launch: passed on the API
  31 ARM64 `sdk_gphone64_arm64` emulator; the Activity was resumed and the log
  contained no fatal exception, native signal/panic, ANR, link error or OOM.
- APK path (ignored local output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- APK size: 330,238,325 bytes.
- APK SHA-256:
  `21bb63449af1e454d38e67b68fc01c6df1105591db07c3fbac439522017e6a39`.

## Acceptance boundary

- The normal APK has no approved Gateway URL. Its visible login surface proves
  package launch only, not real login, StartGame or live locomotion rendering.
- Generic unmounted non-Archer/non-Assassin default catalogs are covered.
  Generated per-library/mounted variants remain open.
- Position changes are packet-first tile updates; continuous interpolation and
  `ObjectBackStep` animation remain open.
- No physical Android device was attached. Touch/network/background/thermal
  journey acceptance remains a device gate.
- APKs, assets, caches, credentials and signing material were not committed.
