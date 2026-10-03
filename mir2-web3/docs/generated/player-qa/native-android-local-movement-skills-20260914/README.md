# Native Android local movement-skill evidence (2026-09-14)

This evidence covers the bounded `Pushed`, `UserBackStep`, `UserDashAttack`
and `UserAttackMove` self-entity/action slice on
`codex/android-player-journey`. It is an offline API31 emulator result, not
approved-WSS login, online StartGame, public asset-release or physical-device
acceptance.

## Result

- The Android host forwards all four packet types only after `IN_GAME`.
- The Gateway now projects typed `UserBackStep` packets to the same bounded
  JSON event shape the Android host consumes; the exact payload has a focused
  regression. The other three packet projections already existed.
- Each packet resolves only the retained `selfPlayer`; none can target or
  synthesize a remote actor.
- `Pushed` always queues the authoritative push pose, matching Crystal.
  `UserBackStep`, `UserDashAttack` and `UserAttackMove` preserve Crystal's
  exact-location/exact-direction acknowledgement no-op.
- Accepted back-step and dash-attack packets select the retained `jump` and
  `dashAttack` action catalogs. Accepted attack-move applies the server
  transform, cancels the active action and restores standing.
- The behavior was checked against Crystal commit
  `0e315fe327192afe52c3d7357ddd1f5b7e26c5b8` before implementation.
- Rust default tests: 186 passed, 0 failed.
- Rust `ui-preview` tests: 194 passed, 0 failed.
- Gradle Debug: 25 passed, 0 failures/errors.
- Gradle UI Preview: 25 passed, 0 failures/errors.
- Focused Gateway `UserBackStep` projection regression: 1 passed, 0 failed.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31`: passed for arm64-v8a.
- API31 streamed install and host-GPU launch: passed.
- The visible 2340x1080 world contains 849 map draws, seven entities and
  twelve entity layers. Logs contain all four apply markers plus
  `ANDROID_LOCAL_MOVEMENT_SKILL_ACTION_ACTIVE` followed by
  `ANDROID_LOCAL_MOVEMENT_SKILL_ACTION_SETTLED`.
- The retained final log has no application panic, fatal exception,
  `DeviceLost`, surface error, `OutOfMemoryError`, app ANR or fatal signal.

The first launch used the emulator's SwiftShader software path and hit
wgpu-hal's GLES `Could not lock adapter context` panic after the world became
ready but before the local action specimen ran. That launch is not counted as
passing. The AVD was restarted without wiping it using the host GPU backend;
the complete retained run above then passed. This remains emulator-backend
stability evidence, not a physical-device result.

## APK

- Path (ignored build output):
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444535592`
- SHA-256:
  `2f6ed1d128c04c274133c8bb367709e00def24af4a56846da8ba26e5592bbb99`
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
- `world-render-logcat.txt`: final host-GPU world/action log.
- `world-render-local-movement-skills.png`: settled visible world frame.

APK and licensed/local asset inputs are intentionally not committed. This
slice does not claim the local input-delay / `MapControl.NextAction` gate,
sub-tile self/world-camera interpolation, mounted exact movement-skill art,
approved WSS/account validation, online render-ready transitions, public
asset alignment or physical-device/human acceptance.
