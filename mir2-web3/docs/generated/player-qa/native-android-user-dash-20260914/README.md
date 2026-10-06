# Native Android local UserDash evidence (2026-09-14)

This evidence is for the bounded self-entity/action `UserDash` /
`UserDashFail` slice on `codex/android-player-journey`. It is an offline API31
emulator result, not live login, online StartGame, public asset-release or
physical-device acceptance.

## Result

- The authenticated Android host forwards both packet types only after
  `IN_GAME`.
- `UserDash` resolves only the retained `selfPlayer`. An exact
  location/direction echo remains a no-op; accepted updates apply the server
  transform and alternate validated `DashL` frames 0-2 / `DashR` frames 3-5.
- `UserDashFail` applies the authoritative transform, cancels the active self
  action and immediately restores the standing facing. It does not emit a
  remote-object presentation event.
- Rust default tests: 185 passed, 0 failed.
- Rust `ui-preview` tests: 193 passed, 0 failed.
- Gradle Debug: 25 passed, 0 failures/errors.
- Gradle UI Preview: 25 passed, 0 failures/errors.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31`: passed for arm64-v8a.
- API31 streamed install and launch: passed.
- The visible 2340x1080 world contains 849 map draws, seven entities and twelve
  entity layers. Logs record `ANDROID_USER_DASH_ACTION_ACTIVE` followed by
  `ANDROID_USER_DASH_ACTION_SETTLED`; the screenshot captures the settled
  world. Crash/error scan found no panic, fatal exception, DeviceLost, surface
  error, OOM, ANR or fatal signal.

## APK

- Path (ignored build output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444502784`
- SHA-256:
  `68ae05c0c26f1ca1505d4feba7d1afaa77c7d68c067d4398c63bec65381f7bfe`
- Package: `com.mir2.web3.uipreview`
- minSdk: 31; targetSdk: 35
- Local entity pack ID staged only into the APK:
  `android-archer-mount-bow-proof-20260912`

## Device and files

- AVD: `Mir2_API_31_ARM64`
- Emulator: 37.1.11.0
- Guest: API31, arm64-v8a, 440 dpi
- `device.txt`: capture/device facts.
- `window-displays.txt`: focused Activity and 2340x1080 display evidence.
- `world-render-logcat.txt`: world ready and self-action active/settled logs.
- `world-render-user-dash.png`: settled visible world frame.

APK and licensed/local asset inputs are intentionally not committed. This
slice does not claim Crystal-equivalent `MapControl.NextAction` timing,
sub-tile self/camera motion, mounted dash art, approved WSS/account validation,
online render-ready transitions, public asset alignment or physical-device /
human acceptance.
