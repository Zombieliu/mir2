# Native Android death lifecycle animation evidence (2026-09-11)

## Version and scope

- Branch: `codex/android-player-journey`
- Implementation commit: `c17bd7e6d27551bcff45bda245ca50eb390b7764`
- `Death` and `ObjectDied` now start the exact packaged Crystal `die` sequence
  and settle into the terminal `dead` pose at the authoritative position and
  last known facing.
- `ObjectHarvested` selects and holds the exact `skeleton` pose.
- `Revived` and `ObjectRevived` clear dead presentation, play the exact
  packaged `revive` sequence and return to standing.
- Missing exact action catalogs retain the explicit dead-opacity or standing
  fallback. Packets cannot select arbitrary atlas names and the live path does
  not decode new images.

## Verification

- Android Rust suite: 117 passed, 0 failed.
  - The deterministic lifecycle test verifies `die-0 -> die-1 -> dead`,
    terminal skeleton hold, opacity reset, `revive-0 -> revive-1 -> standing`,
    authoritative position retention and dead/live model state.
- Configured real shared-atlas test: 1 passed, 0 failed.
- API 31 `aarch64-linux-android` check: passed.
- Java MockWebServer/TLS suite: 11 passed, 0 failed.
- Normal debug APK package, streamed install and cold launch: passed on the API
  31 ARM64 `sdk_gphone64_arm64` emulator. `com.mir2.web3/.MainActivity` was the
  resumed Activity and the launch log contained no fatal exception, Rust
  panic, ANR, native link error or OOM.
- APK path (ignored local output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- APK size: 330,239,717 bytes.
- APK SHA-256:
  `3698111654ce2d0cb77085906c38be0546a4d407ad25bd8a9711b2904693d565`.

## Acceptance boundary

- The normal APK has no approved Gateway URL or test account. Emulator launch
  does not prove real login, StartGame or live lifecycle packet rendering.
- Exact lifecycle sequences cover generic unmounted non-Archer/non-Assassin
  default catalogs. Generated per-library/mounted variants remain open.
- Spell/impact effects, health feedback, ground drops, continuous locomotion
  interpolation and `ObjectBackStep` remain separate object-layer work.
- No physical Android device was attached. Touch, IME, network/background,
  thermal and extended player-journey acceptance remain a device gate.
- APKs, assets, caches, credentials and signing material were not committed.
