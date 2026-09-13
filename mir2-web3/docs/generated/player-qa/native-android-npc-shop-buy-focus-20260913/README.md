# Native Android NPC Shop buy focus — 2026-09-13

## Scope

This proof extends the interaction-safe Android phone treatment to the exact
shared Crystal NPC Shop buy page. There is no Android-only shop, copied shop
state or rewritten touch coordinate path. Android applies one bounded Bevy
`UiTransform` to the shared `OverlayShop` tree while the shared player model
reports that the NPC shop is open.

The shop root previously advertised a stale 620x344 node while the buy page
actually renders inside a 242x330 Crystal frame. The root now uses the rendered
mode's exact bounds: 242x330 for the buy page and 360x360 for the generic
sell/repair service page. Phone focus is capped at 1.8x, keeping the full frame
inside safe edges while preserving a touch-safe physical size for the 32px
item rows on the smallest tested viewport.

This leaf accepts only the buy page. The generic sell/repair service page was
inspected separately and did not meet the visual gate, so it is deliberately
not represented by the screenshots below and remains the next UI leaf.

## Verification

- Focus geometry covers both the 242x330 buy root and 360x360 service root at
  891x411, 731x411 and 610x274 logical phone viewports: passed.
- At the 1.8x ceiling, a 32px shop cell remains at least 48 physical pixels on
  the smallest API31 profile while the complete buy frame stays in safe bounds.
- Android Rust UI Preview suite: 170/170 passed.
- Shared Bevy root-size test with `native-ui`: 1/1 passed.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed under Android
  Studio JBR 17 and the configured Android SDK.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Full licensed-asset APK streamed install: passed.
- Each capture reached `ANDROID_UI_PREVIEW_READY scene=npcshop`; checked logs
  had no panic, fatal exception, Bevy query conflict, missing-path error or
  Android cursor-position error.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
Every tested resolution keeps the shared frame, grid, scrollbar, quantity
controls, Buy control and close glyph visible. Real ADB taps select the first
shop cell and expose the shared green selection outline, then close the shop.
The native-size capture also records an actual plus tap changing quantity to 2.

- [2340x1080 open](npcshop-2340x1080-open.png) ->
  [selected](npcshop-2340x1080-selected.png) ->
  [quantity 2](npcshop-2340x1080-quantity2.png) ->
  [closed](npcshop-2340x1080-closed.png)
  (`3cb5ab3a1bc6d365cd37628592335b99a057428619cfce572d2817c5e28df6fa` ->
  `901d7d8cd4800ee326500866d45fe197205981710126ecc9fae217081324dd2b` ->
  `845e59e33902b6f657073b0923b3a6f0d05aad81d772a85cbe71b0b5f19abaa4` ->
  `984f9dc40b76c9742e9c3a24293d5034daf2a99115556df91981d7a95f90650f`)
- [1920x1080 open](npcshop-1920x1080-open.png) ->
  [selected](npcshop-1920x1080-selected.png) ->
  [closed](npcshop-1920x1080-closed.png)
  (`41598413a55c784e5e4b97d3e6fbc8307df8ed85aba8bfda71115dd986f3daf4` ->
  `be9536c47a9d08250d7965b68feae6fd456191aa605f737c6891cb5871afc0e9` ->
  `1684489ee7417e0ba134ba5731f059816dca1f45a8fe854be801f7fdfa433625`)
- [1600x720 open](npcshop-1600x720-open.png) ->
  [selected](npcshop-1600x720-selected.png) ->
  [closed](npcshop-1600x720-closed.png)
  (`b0d9d1c4ad135569ebf90595004c902db35346eec942f0b09927aebc5945bf0c` ->
  `9991980ebf7e54638bb3504459c55084722077fccddedfbe15fa141a17196c41` ->
  `3096943c1b7797558c53d5ca760b6c9a2b9e7b36e45d56931dfb31f3b539f102`)

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,889,691
- SHA-256: `c232da9277ab73f4db2a422f5cde58c3ea26c1a1fba7d672c780fa525a09b781`
- UI source: byte-identical temporary copy of the previously validated local
  complete 12,781-PNG shared UI staging pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves the
shared NPC Shop buy page's bounded phone focus, item selection, quantity
increment and close picking. It does not prove an authoritative purchase,
generic sell/repair service visuals, approved-WSS authentication, live
StartGame/map transitions, production asset alignment, a physical Android
device, signing/store readiness or human acceptance.
