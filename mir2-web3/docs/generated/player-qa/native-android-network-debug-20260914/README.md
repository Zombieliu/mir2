# Native Android network Debug baseline (2026-09-14)

This evidence pack records the installable network build of the shared native
Android client. It is deliberately separate from the offline `uiPreview`
package and does not claim a real account login.

## Source and package

- Branch: `codex/android-player-journey`
- Source commit: `9e28126aa69bddef41a35c72e0e74dd461f2d5a5`
- Variant/package: `debug` / `com.mir2.web3`
- Version: `0.1.0-shared-ui` (`versionCode=3`)
- Minimum/target SDK: 31 / 35
- APK: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- APK bytes: `393644616`
- APK SHA-256: `25c932f55e759f5dad48554d43b22598553ecee6b949dd6e478a9b5bbde5b9ed`
- Entity asset pack: `android-archer-mount-bow-proof-20260912`
- Gateway build setting: absent; no production or test endpoint was injected

The APK is generated output and is not committed.

## Build and host verification

The aarch64 package gate passed with Rust 1.95.0, cargo-ndk 4.1.2, NDK
26.1.10909125, API 31 and OpenJDK 17. The first host attempt correctly exposed
that the interactive shell defaulted to Java 8; the successful rerun supplied
the installed OpenJDK 17 and Android SDK to that process only. No global Java
configuration or repository-local `local.properties` was changed.

Gradle `testDebugUnitTest testUiPreviewUnitTest` passed 50/50 tests with zero
failures, errors or skips. These include the in-memory TLS/protocol fixtures;
they are not real Gateway authentication evidence.

## API 31 emulator result

- AVD: `Mir2_API_31_ARM64` / `emulator-5554`
- Device: `sdk_gphone64_arm64`, Android 12 / API 31, `arm64-v8a`
- Physical display: 1080x2340 at 440 dpi
- App window: 2340x1080 landscape, fullscreen, short-edge cutout mode
- Install: `adb install -r` passed and preserved existing package data
- Process: `com.mir2.web3` remained alive after the visible frame was captured
- Lifecycle: three Home/foreground cycles retained the same PID (`8345`)
- Log scan: zero `panic`, `FATAL EXCEPTION`, `ANR in`, EGL deadlock or OOM matches

[`login-unconfigured.png`](login-unconfigured.png) shows the real shared
Crystal login surface from the network Debug APK. The visible red
`Test server not configured.` notice is the expected fail-closed result: no
fake connection, roster, character or world was created. `launch.txt` records
that `am start -W` timed out while the large native package was still starting;
the later screenshot, foreground-window record and live process establish the
rendered result. `logcat.txt` is the corresponding app-process log.
`resume.txt` and `resume-logcat.txt` record the bounded foreground recovery
check; this is login-surface lifecycle evidence, not authenticated session
resume.

## Unclosed external gates

No approved WSS endpoint/test account was available in the process environment,
and `adb devices -l` listed only the emulator. Therefore this pack does not
claim real authentication, online roster/StartGame, live render-ready map
transition, saved-position recovery or physical-device acceptance. Closing
those gates requires an explicitly approved non-production WSS environment and
one connected physical API 31+ arm64 Android device. Credentials must be
entered through the normal secure device flow and must not be committed or
placed in this evidence pack.
