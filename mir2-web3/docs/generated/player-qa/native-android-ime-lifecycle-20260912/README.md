# Native Android mail IME and lifecycle evidence — 2026-09-12

## Acceptance boundary

This is a synthetic, offline `uiPreview` run on an API 31 arm64 emulator. It
proves the shared Bevy mail composer can render, accept Android IME input, keep
its draft across an immediate Home/foreground cycle, and remain alive without
the previously observed native-heap runaway. It does **not** prove an approved
WSS endpoint, a real account/login, server persistence, or physical-device
acceptance.

The separate preview package displays `OFFLINE UI PREVIEW — mail-compose — NOT
LIVE GAMEPLAY` in every captured frame and does not open a network connection.

## Environment

- AVD: `Mir2_API_31_ARM64` (the existing `Pixel_5_API_31` AVD was retained)
- Android: API 31 / Android 12
- Image fingerprint: `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`
- ABI/model: `arm64-v8a` / `sdk_gphone64_arm64`
- Guest RAM: `4004500 kB`
- Data partition: `20G` (`19G` available during this run)
- Package: `com.mir2.web3.uipreview`, version code 3, min SDK 31, target SDK 35
- APK: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- APK bytes: `376822980`
- APK SHA-256: `f489d492cff2fcacc15a2e82dada3a85fc66f272da7aca9cfcbe3e7b4529fb27`

The APK, Gradle caches, generated assets, signing material, and credentials are
not part of this evidence directory and must not be committed.

## Root cause and repair evidence

Before the repair, a visible `mail-compose` fixture rebuilt the complete mail
entity tree on every Bevy `Update`. On the 4 GB AVD, `dumpsys meminfo` showed
total PSS climbing from roughly `340314 kB` at second 0 to `2766023 kB` at
second 12; a full sample near second 5 attributed `1244495 kB` PSS to native
heap, and Android later killed the process at about 3 GB RSS. Other preview
scenes stayed comparatively stable, isolating the issue to the mail overlay
rebuild path rather than APK installation or AVD capacity.

The repair retains the mail entity tree while its visible render inputs are
unchanged and invalidates it when the inbox, compose draft, attachment page, or
eligible attachment rows change. A regression test runs 120 unchanged updates,
asserts exact child entity identities remain stable, then changes the draft and
asserts a rerender occurs.

Post-repair `mail-compose`, before typing, stayed on PID `10359` for 44 seconds:

```text
seconds              0       10       20       30       40       44
native PSS kB   176297   176325   176329   176341   176321   176385
total PSS kB    264710   264707   264822   264693   264693   264842
```

After entering recipient `qarecipient` and multiline message `first\nsecond`,
the same PID stayed stable for another 30 seconds:

```text
seconds              0       10       20       30
native PSS kB   249422   249442   249422   249426
total PSS kB    338450   338478   338470   338478
```

Home/foreground recovery retained PID `10359` and the shared draft. Immediate
post-resume memory was `250042/339787 kB` native/total PSS. Tapping the shared
message field reopened Gboard (`mInputShown=true`) on the same PID, at
`249930/339615 kB`. Logcat contained no `Connection Lost`, panic, fatal
exception, or OOM signature for this cycle.

## Captures

- `emulator-mail-multiline-ime.png`: recipient and multiline draft visible
  while Gboard is open.
- `emulator-mail-after-resume.png`: same draft visible after Home/foreground;
  no synthetic reconnect or connection-lost panel.
- `emulator-mail-ime-reopened-after-resume.png`: Gboard reopened on the shared
  message field after resume, with the same process and draft.

## Automated gates

- `cargo +1.95.0 fmt --manifest-path apps/game-client/platform-android/Cargo.toml -- --check`
- `cargo +1.95.0 test --manifest-path apps/game-client/platform-android/Cargo.toml --features ui-preview --lib`: 162 passed, 0 failed
- targeted shared-UI retained-entity regression test: 1 passed, 0 failed
- `./gradlew :app:testDebugUnitTest :app:testUiPreviewUnitTest --no-daemon`: passed
- arm64 API 31 `cargo-ndk` release build with `ui-preview`: passed
- `assembleUiPreview` using the existing generated licensed asset inputs: passed
- streamed reinstall onto the dedicated AVD: passed

Warnings already present in the shared Bevy crate were not broadened into this
Android lifecycle fix.

## Remaining gates

- Approved test WSS endpoint plus disposable account for real login, roster,
  StartGame, authoritative world receipt, reconnect, and saved-position checks.
- Physical Android device for touch ergonomics, keyboard/vendor IME behavior,
  background/network transitions, temperature, memory, and rendering.
- Broader retained-tree work for other large desktop-derived overlays if future
  emulator profiling finds the same per-frame rebuild pattern outside mail.
