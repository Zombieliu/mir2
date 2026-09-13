# Native Android Chat Settings focus scale — 2026-09-13

## Scope

This proof covers the first interaction-safe phone focus treatment for the
shared Crystal UI. The 224x180 Chat Settings window previously inherited only
the whole 1024x768 desktop-stage fit, making its controls visibly complete but
too small for reliable phone use.

The change does not create an Android copy of the window. The shared
`CrystalChatSettingsModal` receives an identity `UiTransform`, and the Android
stage fitter scales that exact entity inside the current safe viewport. Bevy
uses the resulting `UiGlobalTransform` for both drawing and picking, so the
visible tabs and their touch regions cannot diverge.

## Behavior

- Compact dialogs can grow to 3.2x relative to the fitted desktop stage.
- Scale is bounded by the safe viewport with a 16 logical-pixel edge gutter.
- The authored 224x180 layout and all shared chat settings actions remain
  unchanged.
- Other platforms retain the identity transform.
- This leaf intentionally covers Chat Settings only. Inventory, Storage,
  Options and compact NPC/Shop surfaces still need their own shared markers
  and interaction-safe focus treatment.

## Automated verification

- Android Rust UI Preview suite: 168/168 passed.
- New pure geometry coverage passes for 891x411, 731x411 and 610x274 logical
  phone viewports, including safe-edge bounds and the no-shrink invariant.
- `cargo +1.95.0 fmt`: passed.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Gradle UI Preview unit tests and full licensed-asset APK assembly: passed.
- API31 ARM64 streamed install and cold launch: passed.
- Launch log reached `ANDROID_UI_PREVIEW_READY scene=chat-settings` with no
  panic, fatal exception or missing-path error.

## Visible and touch evidence

The API31 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
Each pair starts on FILTER; an ADB touch on the visibly enlarged CHAT BOX tab
then changes the shared model and produces the CHAT BOX frame. Different PNG
hashes within every pair prove the tap was accepted rather than merely drawn.

- [2340x1080 FILTER](2340x1080-filter.png) ->
  [2340x1080 CHAT BOX](2340x1080-chat.png)
- [1920x1080 FILTER](1920x1080-filter.png) ->
  [1920x1080 CHAT BOX](1920x1080-chat.png)
- [1600x720 FILTER](1600x720-filter.png) ->
  [1600x720 CHAT BOX](1600x720-chat.png)

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 393,653,075
- SHA-256: `fff3ffcd64de5caa0c02026ff4b0433cda5670aceb00e8a0eb2644530741d4a8`
- UI source: local complete 12,781-PNG shared UI pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

These are labelled offline UI fixtures on an emulator. They prove visible
scaling and real Bevy button picking for this shared dialog; they do not prove
approved-WSS authentication, StartGame/map transitions, production asset
alignment, inventory/storage drag behavior under focus scaling, a physical
device, signing/store readiness or human acceptance.
