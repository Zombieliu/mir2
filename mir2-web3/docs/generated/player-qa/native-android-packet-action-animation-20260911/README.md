# Native Android packet action animation evidence (2026-09-11)

## Version and scope

- Branch: `codex/android-player-journey`
- Implementation commit: `73119525ef539ef7d559513348f8a4798a2b0c76`
- Scope: authoritative post-`IN_GAME` actor action packets select and advance
  exact immutable packaged Crystal atlas frames, then restore standing.
- Supported baseline: generic unmounted non-Archer/non-Assassin actors for
  harvest, melee attack variants, range attack, dash attack and struck.
- No production deployment, real stored character mutation, credential use,
  APK/signing material commit, or Windows-branch modification was performed.

## Automated evidence

- `cargo +1.95.0 test --manifest-path apps/game-client/platform-android/Cargo.toml -- --test-threads=1`
  - 116 passed, 0 failed.
  - Includes deterministic first/next/settled action-frame projection without
    a second world snapshot and a packet-without-direction fallback to the
    actor's last authoritative facing.
- Configured real shared atlas test:
  `MIR2_ANDROID_ENTITY_ASSET_ROOT=$PWD/apps/web/public cargo +1.95.0 test --manifest-path apps/game-client/platform-android/Cargo.toml entity_render::tests::configured_shared_atlas_resolves_real_player_frame_when_present -- --exact --test-threads=1`
  - 1 passed, 0 failed; exact real player attack frames resolve.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31 ./apps/game-client/platform-android/build-android.sh`
  - `aarch64-linux-android` target check passed.
- `JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home ANDROID_HOME=/Users/henryliu/Library/Android/sdk ./gradlew testDebugUnitTest`
  - MockWebServer/JVM suite: 11 passed, 0 failed.

## APK and emulator evidence

- APK (ignored local build output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- Size: 330,238,349 bytes.
- SHA-256:
  `d4289ec0597c4b663eef3c9f5daef3cc921eb8ad9c7213dfd35672a2a305b269`.
- Device: Android API 31 ARM64 `sdk_gphone64_arm64` emulator,
  2340 x 1080 landscape application window.
- `adb install -r` succeeded. The normal package cold-launched to the actual
  shared Crystal login screen, stayed resumed/fullscreen with a live process,
  and logged no fatal exception, native panic/signal, ANR, link error or OOM.
- Home/resume retained the same process id.
- Local ignored screenshot:
  `apps/game-client/platform-android/target/evidence/android-native-packet-actions-20260911/launch.png`.

The screenshot proves only the normal APK's visible shared login surface. The
APK was intentionally built without an approved Gateway URL and displays
`Test server not configured`; it does not prove real login, StartGame, a live
action packet, or online gameplay.

## Remaining acceptance gaps

- No approved WSS endpoint/test account was available, so real login, roster,
  StartGame, online player position and live packet-driven animation remain
  unverified.
- Walk/run, generated per-library class/mount catalogs, die/revive and skeleton
  motion, effects, ground drops and remaining gameplay reducers are not closed.
- No physical Android device was attached. Touch, IME, background recovery,
  network loss/reconnect, thermal behavior and the full player journey remain
  physical-device acceptance gates.
- This evidence does not establish a complete object layer, a complete native
  client, human visual parity, signing/store readiness or production safety.
