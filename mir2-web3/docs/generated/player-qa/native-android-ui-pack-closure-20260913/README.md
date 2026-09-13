# Native Android shared-UI pack closure — 2026-09-13

## Scope

This proof covers the Android-only Gradle boundary that stages the licensed
shared Crystal UI into Debug and UI Preview APKs. It was triggered by a
34-scene API31 capture: the APK could satisfy four old sentinel checks while
shipping only 3,160 of the expected UI PNGs, leaving Chat Settings and other
dialogs visibly incomplete.

## Fix

- `MIR2_ANDROID_UI_ASSET_ROOT` must now identify an immutable source pack. The
  Gradle generated destination is explicitly rejected, so a stale output
  directory cannot validate or synchronize itself.
- The source must contain `original-ui/manifest.generated.json`.
- The manifest and every referenced frame are validated before `Sync`:
  - ChrSel: at least 1,146 frames
  - Help: at least 42 frames
  - MMap: at least 450 frames
  - Prguse: at least 2,447 frames
  - Prguse2: at least 1,602 frames
  - StateItem: at least 5,192 frames
  - Title: at least 899 frames
- Unsafe/missing manifest paths fail the package, and Items independently
  requires at least 1,003 PNGs.

The accepted pack staged 12,781 PNGs. Chat Settings now includes the original
FILTER/CHAT BOX frame and all channel controls instead of only a detached
checkmark, close button, and footer.

## Evidence

- Negative package gate using `android/app/build/generated/shared-ui-assets` as
  its own source: failed before Sync with
  `MIR2_ANDROID_UI_ASSET_ROOT must be a source pack`.
- Positive full-pack Debug/UI Preview assembly: passed.
- Java Debug unit tests: 23/23 passed.
- Java UI Preview unit tests: 23/23 passed.
- API31 ARM64 streamed install: passed.
- Full-pack UI Preview:
  - [world-render.png](world-render.png): Bichon frame with 849 map draws,
    seven entities and twelve entity layers.
  - [chat-settings.png](chat-settings.png): complete shared Crystal settings
    frame and controls.
  - [storage.png](storage.png): staged shared Storage/Inventory surface.
- The checked launch logs contained no panic, fatal exception or missing-path
  error.

## APK artifacts

Artifacts remain ignored build outputs and are not committed.

- Debug:
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  - bytes: 393,653,059
  - SHA-256: `d7e6df999df0fd007c4d8099523cadcd6a4d98532000ed1a3e0fc7b5a7cb7ab7`
- UI Preview:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  - bytes: 393,653,075
  - SHA-256: `71f0a881b65d9d2f286a5bd5d527aa6611300661e4308ca95fc9b58dac96f65b`

The package uses the local full UI source pack, the local keyed Bichon world
pack and the ignored 35-atlas player/entity proof pack. It is not a Web release
alignment claim.

## Acceptance boundary

This corrects asset completeness, not phone ergonomics. The desktop 1024x768
panel scale remains too small for several phone-size dialogs and needs a
separate interaction-safe mobile focus/zoom design. The captures are offline
fixtures; approved-WSS login, live StartGame/map transitions, physical-device
touch/IME/network/GPU behavior, signing/store and human acceptance remain open.
