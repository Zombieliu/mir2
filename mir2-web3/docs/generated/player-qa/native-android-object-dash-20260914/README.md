# Native Android remote ObjectDash evidence (2026-09-14)

This evidence is for the bounded remote `ObjectDash` / `ObjectDashFail` slice
and exact unmounted-player art on `codex/android-player-journey`. It is not a live
login, online StartGame, public asset-release or physical-device result.

## Result

- `ObjectDash` accepts only an existing non-self actor and applies the server
  position/direction. An unmounted player with the validated Running catalog
  alternates `DashL` frames 0-2 and `DashR` frames 3-5 and emits three
  presentation phases.
- `ObjectDashFail` cancels an active dash at the authoritative transform,
  returns the actor to its standing facing and emits a one-phase motion
  correction. Unknown, removed, drop and malformed identities do not appear.
- Rust default tests: 184 passed, 0 failed.
- Rust `ui-preview` tests: 192 passed, 0 failed.
- Gradle Debug: 25 passed, 0 failures/errors.
- Gradle UI Preview: 25 passed, 0 failures/errors.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31`: passed for arm64-v8a.
- API 31 emulator streamed install and launch: passed.
- Visible world: 2340x1080 landscape, 849 map draws, seven entities and twelve
  entity layers. The log records `ANDROID_OBJECT_DASH_POSE_ACTIVE` at
  `offset_x=-32.0` and `ANDROID_OBJECT_DASH_POSE_SETTLED`; the captured frame
  is the settled authoritative endpoint. Crash/error scan found no panic,
  fatal exception, DeviceLost, surface error, OOM, ANR or fatal signal.

## APK

- Path (ignored build output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444493960`
- SHA-256:
  `566b27a7a78b8994523a5f30f816a04944bb995346f21a30009e4c3a9579e5ff`
- Package: `com.mir2.web3.uipreview`
- minSdk: 31; targetSdk: 35
- Local entity pack ID staged only into the APK:
  `android-archer-mount-bow-proof-20260912`

## Device and files

- AVD: `Mir2_API_31_ARM64`
- Emulator: 37.1.11.0
- Guest: API 31, arm64-v8a, 440 dpi
- `device.txt`: capture/device facts.
- `window-displays.txt`: focused Activity and 2340x1080 display evidence.
- `world-render-logcat.txt`: ready, active and settled markers plus full log.
- `world-render-object-dash.png`: settled visible world frame.

The APK and licensed/local asset inputs are intentionally not committed.
Separate local-player `UserDash` / `UserDashFail`, mounted exact dash artwork,
approved WSS credentials/environment, online render-ready transitions, public
asset alignment and physical-device/human acceptance remain unverified.
