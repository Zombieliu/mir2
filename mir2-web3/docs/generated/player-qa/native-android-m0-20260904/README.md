# Native Android Bevy M0 evidence — 2026-09-04

## Acceptance boundary

This pack validates only the first native Android milestone:

- Gradle `GameActivity` loads the Rust `cdylib`.
- Bevy 0.19 creates an Android window and renders a deterministic, asset-free
  first frame.
- The process survives one background/foreground cycle.

It does **not** validate Android WebSocket transport, authentication, character
selection, authoritative world state, gameplay, physical-device behavior,
signing, or store delivery. The rendered teal/gold marker is a launch probe,
not the Mir2 player UI.

## Source and build

- Branch: `codex/android-player-journey`
- Native M0 implementation commit: `21f58e4e5`
- Package: `com.mir2.web3`
- Version: `0.1.0-m0` (`versionCode=1`)
- Rust: `1.95.0`
- Target/ABI: `aarch64-linux-android` / `arm64-v8a`
- Android SDK: compile 35, target 35, minimum 31
- NDK: `26.1.10909125`
- Host: Apple Silicon macOS, Java 21, `cargo-ndk 4.1.2`

Commands used from `apps/game-client/platform-android`:

```bash
cargo +1.95.0 test --manifest-path Cargo.toml --locked
ANDROID_SDK_ROOT="$SDK" ANDROID_NDK_HOME="$NDK" ./build-android.sh
JAVA_HOME="$JAVA_21" ANDROID_SDK_ROOT="$SDK" \
  ANDROID_NDK_HOME="$NDK" MIR2_ANDROID_MODE=package ./build-android.sh
```

Results:

- Host unit tests: **56 passed, 0 failed**
- Android target check: **passed**, `--locked --offline`
- Gradle task: **assembleDebug passed**, 34 tasks up-to-date on the final run
- Exported native entry point: `android_main`
- Packaged native libraries: only
  `lib/arm64-v8a/libmir2_platform_android.so`

APK (generated, intentionally not committed):

```text
apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk
size: 111 MiB
sha256: 7b3d9a199530f5482784ebe39ba4e32b1289732634bda2c793f517982caf629c
```

## Emulator run

- AVD: `Pixel_5_API_31`
- Device model: `sdk_gphone64_arm64`
- Android: 12 / API 31
- ABI: `arm64-v8a`
- Fingerprint:
  `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`
- Emulator: `31.3.10.0` build `8807927`
- Renderer: Vulkan through SwiftShader software rendering

Install and cold launch both succeeded. Logcat recorded:

```text
GameActivity: Found library libmir2_platform_android.so. Loading...
mir2_platform_android: MIR2_ANDROID_M0_FRAME_READY
bevy_winit::system: Creating new window mir2-web3 (android)
```

The cold launch kept process PID `5373`. After Home and bringing the task back
to the foreground, Android reported a HOT launch; the PID remained `5373`, no
new fatal exception appeared, and the post-resume screenshot was byte-identical
to the first-frame screenshot.

The AVD initially had an 800 MiB data partition and could not install the APK.
Its old emulator package contained unsigned `e2fsck`/`resize2fs` helpers that
macOS killed before expansion. The helpers were backed up, temporarily ad-hoc
signed, used to expand the existing AVD to 20 GiB without wiping it, then
restored to their original SHA-256 values. `/data` subsequently reported 19 GiB
available.

## Visual evidence

- `mir2-native-m0-first-frame.png`
  - 2340 × 1080
  - SHA-256:
    `1e50476c1e24041762b23bb275fd939b8a1ba3260998f8ea518b4762e56a004e`
- `mir2-native-m0-after-resume.png`
  - 2340 × 1080
  - SHA-256:
    `1e50476c1e24041762b23bb275fd939b8a1ba3260998f8ea518b4762e56a004e`

## Remaining work

1. Connect the native host to the approved Gateway/WebSocket environment
   without duplicating server-authoritative rules.
2. Replace the M0 marker with the real shared login/player scene and verify
   versioned assets.
3. Run the complete login-to-reconnect journey on an Android physical device.
4. Record device performance, lifecycle, network-transition, signing, and
   distribution evidence separately.
