# Native Android small-landscape controls and resume evidence (2026-09-12)

Scope: the separate network-disabled `com.mir2.web3.uipreview` package on the
API 31 `Mir2_API_31_ARM64` emulator. This is synthetic UI and lifecycle
evidence, not approved-WSS login, live gameplay, physical-device or human
acceptance.

## Result

Blocking shared panels now own the entire touch surface. The Android joystick
and action pad are hidden while Inventory, Game Shop, Mail Compose or another
shared panel/dialog blocks world input, and the invisible joystick recognizer
cannot claim panel touches. The collapsed duplicate `Panels` rail is also
hidden; the directly reachable `Menu` action remains the single way to expand
that rail.

The before/after 1600x720 captures show the action pad no longer covering Game
Shop or Mail Compose:

- `before-gameshop-20x9.png` / `after-gameshop-20x9.png`
- `before-mail-compose-20x9.png` / `after-mail-compose-20x9.png`

World controls remain visible over the render-ready Bichon fixture in
`after-world-render-20x9.png`. `after-gameshop-16x9.png` and
`after-world-render-16x9.png` independently exercise 1920x1080 landscape.

Launcher re-entry previously created a second `GameActivity` in the same
process, destroyed the Activity backing the running Bevy loop and left a black
window with repeated `GameActivity was destroyed` errors. `MainActivity` is now
single-task. A Home then launcher re-entry on the emulator's unoverridden
2340x1080 cutout profile retained PID `5676`, restored the visible frame and immersive
landscape, and recorded zero destroyed-activity, panic or fatal-exception
errors. Compare `before-home-cutout.png` and `after-resume-cutout.png`; launch
records are retained in `cold-launch.txt` and `resume-launch.txt`.

## Device and dimensions

- Fingerprint: `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`
- Android API: 31, arm64
- Emulator-reported native display: 1080x2340 portrait / 2340x1080 forced game
  landscape, including the emulator's 136-pixel cutout inset
- Overrides: 720x1600 and 1080x1920 portrait, rendered as 1600x720 and
  1920x1080 landscape

## Verification

- Android Rust UI Preview library: 164/164 passed.
- Focused touch-control tests: 15/15 passed, including blocked-panel joystick
  ownership and scaled root visibility.
- Java host/policy unit tests: 22/22 passed in both Debug and UI Preview.
- Rust 1.95.0 + `cargo-ndk` 4.1.2 release build: arm64-v8a, API 31 passed.
- Gradle `assembleDebug` and `assembleUiPreview`: passed.
- Streamed UI Preview install and cold launches at all three dimensions:
  passed, with no panic, fatal exception or missing packaged path.
- Home/launcher re-entry: same process, visible render restored, passed.
- UI Preview APK (not committed):
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- APK size: 377,189,700 bytes
- APK SHA-256:
  `c4965f07b0452f462ebef2b0f5b9ae169cfa006be3297343dd2bab580c66f757`

## Remaining acceptance boundary

The package is intentionally offline. An approved WSS endpoint/account is
still required for real login, StartGame/map-transition and reconnect
acceptance. A physical Android device is still required for cutout variants,
touch feel, real IME behavior, network handoff and GPU/thermal acceptance.
