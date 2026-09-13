# Native Android HUD and chat phone-scale focus — 2026-09-13

## Scope

This proof keeps the shared Crystal bottom HUD and chat implementation intact
and fixes the Android-private touch layer that sat above it. The prior overlay
cancelled the shared stage scale with fixed logical dimensions, leaving the
joystick and two-by-two action pad disproportionately large on a short
landscape viewport.

The joystick now follows the viewport short edge, capped at 16 percent with a
48 logical-pixel lower bound. Normal phone viewports keep a compact two-column
action pad. Viewports below 320 logical pixels use one four-button row so the
controls do not consume most of the playfield. Every action button keeps at
least 48 logical pixels of height. Opening the Android panel rail hides the
world controls; the narrow rail uses four columns and remains entirely inside
the viewport.

Chat and IME already caused `NativePlayerUiState::blocks_world_click()` to hide
the joystick and combat pad. That existing path was retained and verified
rather than duplicated.

## Verification

- Android Rust UI Preview suite: 172/172 passed.
- Focused Android mobile-UI suite: 16/16 passed.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed under Android
  Studio JBR 17 and the configured Android SDK.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Full licensed-asset APK streamed install: passed.
- HUD and chat reached `ANDROID_UI_PREVIEW_READY` at 2340x1080, 1920x1080 and
  1600x720. Checked logs contained no panic, fatal exception, missing-path
  error or Bevy query conflict.
- The emulator's temporary size overrides were removed after capture;
  `wm size` again reports the physical 1080x2340 profile.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.

- HUD: [2340x1080](hud-2340x1080.png),
  [1920x1080](hud-1920x1080.png), and
  [1600x720 compact row](hud-1600x720.png).
- A real ADB tap toggled Run off:
  [run-off](hud-1600x720-run-off.png).
- A second ADB tap opened the four-column rail. The action pad and joystick
  were hidden and all rail buttons remained visible:
  [panels-open](hud-1600x720-panels-open.png).
- Chat with the Android keyboard: [2340x1080](chat-2340x1080.png),
  [1920x1080](chat-1920x1080.png), and
  [1600x720](chat-1600x720.png). ADB IME input reached the shared chat draft
  while world controls stayed hidden:
  [typed mobile-ui](chat-1600x720-ime-text.png).

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,871,923
- SHA-256: `c19dd3f9838aa3b8a9999858ca476ccd5a2e628aab92ccb46576b97685dca0be`
- UI source: byte-identical temporary copy of the previously validated local
  complete 12,781-PNG shared UI staging pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves the
Android touch-layer geometry, shared HUD visibility, local Run/panel picking,
shared chat IME entry and world-control suppression while editing. It does not
prove approved-WSS authentication, a live server player loop, authoritative
movement/combat/chat delivery, a physical Android device, signing/store
readiness or human acceptance.
