# Android native touch controls evidence (2026-09-12)

## Scope

This pack verifies the native Bevy Android UI-preview baseline after wiring the
mobile action pad and virtual joystick into the existing Android input and
gateway bridge. It does not claim live gameplay, production authentication, or
physical-device acceptance.

The joystick emits the browser-compatible `walk` / `run` direction envelope
through the Android host bridge. Only the newest unsent motion intent is kept,
and lifecycle or connectivity loss clears pending motion. Attack and pickup use
the shared authoritative UI intent paths; the preview does not mutate player
position, combat, or inventory locally.

## Source and build

- Baseline source commit: `3819b9ebf7c9bea788eafb9ccf99125f1e5895ef`
- Branch: `codex/android-player-journey`
- Variant: `uiPreview`, API 31, `arm64-v8a`
- Asset pack id: `release-20260912`
- Asset manifest SHA-256: `0a7f57e99e4f1fa508e807be20ec3c957d9ecd1b598b468fc6905c8a394181e7`
- Packed render data: 35 atlases, 102 pages, 23,224 rectangles, 40,312,262 page bytes
- APK (ignored, not committed): `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- APK size: 376,823,068 bytes
- APK SHA-256: `4406842747cf351ed27b7254769c2cc3d2fe199ced5c386d9fdd94f80e9e88b4`

## Automated checks

- `cargo +1.95.0 fmt --manifest-path apps/game-client/platform-android/Cargo.toml -- --check`: PASS
- `cargo +1.95.0 test --manifest-path apps/game-client/platform-android/Cargo.toml --features ui-preview --lib`: PASS, 162/162
- Coverage added for exact walk/run wire JSON, latest-motion coalescing,
  lifecycle clearing, foreground/in-game gating, joystick touch ownership,
  secondary action touch, and action-pad routing.

## Emulator evidence

- Emulator: Android Emulator 37.1.11.0, `sdk_gphone64_arm64`
- Android API: 31
- ABI: `arm64-v8a`
- Rendering: OpenGL ES 3.0 via Android Emulator OpenGL ES Translator
  (Apple M1 Max), Bevy backend `Gl`
- Install and cold launch: PASS (`com.mir2.web3.uipreview`)
- Readiness: `ANDROID_UI_PREVIEW_READY scene=world-render`
- World render: center `(302, 634)`, 849 map tiles, 7 entities, 12 entity layers
- Fatal exception, ANR, render-device-loss, and incomplete-framebuffer scan: none found
- Visible frame: [emulator-touch-controls.png](./emulator-touch-controls.png)
- Held joystick frame: [emulator-joystick-held.png](./emulator-joystick-held.png)
- Run-lock toggled off: [emulator-run-toggle-off.png](./emulator-run-toggle-off.png)

The visible frame proves the native full-screen world, layered entities,
joystick, action pad, minimap, HUD, and panel rail render together. It is
intentionally labelled `OFFLINE UI PREVIEW - NOT LIVE GAMEPLAY`.

The original long-running `adb shell input swipe` capture was a false-negative
test procedure: the separate capture transport did not preserve an observable
mid-swipe hold. Repeating the check with explicit touchscreen `DOWN`, `MOVE`,
capture, and `UP` events visibly moved the knob up-right while the full world
remained rendered. A separate `DOWN` / `UP` on the Run button changed its
highlighted state off, proving the action pad received the emulator touch too.
These frames count as synthetic emulator interaction evidence, not physical
finger or multi-touch acceptance.

## Remaining acceptance gates

- Approved HTTPS/WSS endpoint and test account are required for real login,
  character list, StartGame, authoritative movement, reconnect, and persistence.
- A physical Android device is required for touch, soft keyboard, background
  recovery, network switching, thermal behavior, and real rendering acceptance.
- This evidence must not be described as a native Android player loop or
  physical-device completion.
