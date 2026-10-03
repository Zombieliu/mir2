# Native Android Options focus — 2026-09-13

## Scope

This proof extends the interaction-safe phone treatment to the exact shared
259x354 Crystal Options panel. It adds no Android copy or Android-only options
state: the existing shared root receives a bounded `UiTransform`, and the
existing shared buttons continue to update `mir2-ui-core`.

## Behavior

- The shared Options panel grows relative to the fitted 1024x768 stage, up to
  the existing 3.2x focus ceiling.
- Scale and translation keep the complete panel within Android safe edges and
  a 16 logical-pixel gutter.
- Bevy rendering and picking consume the same transformed entity tree, so the
  visible ON/OFF controls and their touch regions stay aligned.
- Other platforms retain the identity transform and existing behavior.

## Verification

- Focused-panel geometry test, including Options at 891x411, 731x411 and
  610x274 logical phone viewports: 1/1 passed.
- Android Rust UI Preview suite: 170/170 passed.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Gradle `assembleUiPreview`, `testDebugUnitTest` and
  `testUiPreviewUnitTest`: passed under Android Studio JBR 17.
- Full licensed-asset APK streamed install: passed.
- Each resolution launched `ANDROID_UI_PREVIEW_READY scene=options`; the
  checked logs contained no panic, fatal exception, Bevy query conflict,
  missing-path error or Android cursor-position error.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
Every pair begins with the shared `SKILL BAR` option ON. An ADB tap on the
visibly enlarged OFF button changes the shared model and moves the Crystal
selection frame to OFF. Distinct hashes within every pair prove the tap was
accepted.

- [2340x1080 ON](options-2340x1080-skillbar-on.png) ->
  [OFF](options-2340x1080-skillbar-off.png)
  (`77703808b1ec638d249d58b2b5340e7cc888be8ffbc1dc311b9d7d70c02f8574` ->
  `341ecc50ddd13e7611ad22b76fd6c1e1a4c9f7e89b3b00b04c781a548cfdd330`)
- [1920x1080 ON](options-1920x1080-skillbar-on.png) ->
  [OFF](options-1920x1080-skillbar-off.png)
  (`738077abcf1146648d2306f6209ef03ebd93e84878d095a8f30218ff70c19e19` ->
  `d20d30f60114546f58f1306007ce74e7c0be6a86cd388caf5f792280d95051f6`)
- [1600x720 ON](options-1600x720-skillbar-on.png) ->
  [OFF](options-1600x720-skillbar-off.png)
  (`0622032485909f7cf232d3c2407857bc48995c289ae08371a67c9ee093ad9b74` ->
  `b2ab07781c914c3de41462c66498b9ee3c138dfeb2cf44c56f9cc6975bbfb1f7`)

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,875,947
- SHA-256: `f7a8f3411c0986bf21408b3ab3aabe2dc00a1978be19a1cd2b1056dfd349f074`
- UI source: byte-identical temporary copy of the previously validated local
  complete 12,781-PNG shared UI staging pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves the
shared Options panel's visible phone scaling and actual Bevy touch picking. It
does not prove approved-WSS authentication, live StartGame/map transitions,
production asset alignment, a physical Android device, signing/store readiness
or human acceptance. Compact NPC/Shop focus remains open.
