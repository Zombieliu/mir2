# Native Android NPC sell and repair focus — 2026-09-13

## Scope

This proof repairs the shared Crystal NPC sell, normal-repair and
special-repair service surface. The old generic renderer placed its header,
ten backpack buttons, equipment grid and action row in one shrinkable Flex
column. On the API31 phone viewport the button backgrounds survived while
their text, header and actions collapsed into a nearly black panel.

The retained shared renderer uses one bounded 360x360 absolute layout with a
visible title, gold/rate labels, ten backpack targets, fourteen equipment-slot
targets and a fixed bottom action row. Sell uses a two-column backpack grid;
repair uses a backpack column beside the equipment grid. It still dispatches
the existing `SelectBagForSell`, `SelectBagForRepair`,
`SelectEquipForRepair`, `ShopSell`, `ShopRepair` and `ShopSRepair` actions.
No Android-only shop, copied inventory, client-side transaction rule or custom
touch-coordinate path was introduced.

Three compile-time isolated UI Preview scenes now expose Sell, Repair and
SpecialRepair independently. They never authenticate or send a transaction.

## Verification

- Android Rust UI Preview suite: 171/171 passed.
- Shared NPC shop root and bounded-control tests: 3/3 passed.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed under Android
  Studio JBR 17 and the configured Android SDK.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Full licensed-asset APK streamed install: passed.
- `ANDROID_UI_PREVIEW_READY` was observed for the service scenes; checked logs
  had no panic, fatal exception, Bevy query conflict, missing-path error or
  Android cursor-position error.
- The emulator's temporary 1920x1080 and 1600x720 overrides were removed after
  capture; `wm size` again reports only the physical 1080x2340 profile.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
Real ADB taps select backpack items and equipped items through Bevy picking,
increment the sell quantity and close the shared panel. Distinct hashes record
the resulting state; no Sell or Repair transaction was submitted because the
preview has no server authority.

- 2340x1080 Sell: [open](sell-2340x1080-open.png) ->
  [first item selected](sell-2340x1080-selected.png) ->
  [quantity 2](sell-2340x1080-quantity2.png) ->
  [closed](sell-2340x1080-closed.png)
  (`1ef4c8256d3e3fe3904cf443464b92cb5b434a138f4910463f5ed54d56796fc4` ->
  `390fc0b0280474ffc1e43eac8744ca922eff5c125d3622baed62543404dc52e1` ->
  `b8c1a83487924b3d5961b0f542ae149ab4cb3a3fbb4cba41ad7592712a249fe8` ->
  `92461b55fafffdb2a045fc4660c2cadc61c7d8020722ed1bbbcdd592437bd427`)
- 2340x1080 Repair: [open](repair-2340x1080-open.png) ->
  [backpack selected](repair-2340x1080-bag-selected.png) ->
  [equipped weapon selected](repair-2340x1080-equipped-selected.png) ->
  [closed](repair-2340x1080-closed.png)
  (`bb34e59ae24f5aaa55a5958f342be8b2c7a72aff18771522b5086bc3c8cfac` ->
  `1587c5aeee8e407a89c75729dbd76d488c6f9261b48aac8ffb9647363ec30820` ->
  `2a9cd7b05aca70be72d4f3e5b663d67a18895bb94bb85fca2d9c5dc412aa2627` ->
  `f23d65dc7db2853de5578a7b58f30366b4718046e727397a6ee53ac525a3196c`)
- 2340x1080 Special Repair: [open](srepair-2340x1080-open.png) ->
  [equipped weapon selected](srepair-2340x1080-equipped-selected.png)
  (`34a76656bc31f84ebacbdfee239dbc44610fa591fb68771522b5086bc3c8cfac` ->
  `4b50dd40d0b733f4454c7c97ffe868a3b2511ab3b3f4e7255d749007bb436d78`)
- 1920x1080 Sell: [open](sell-1920x1080-open.png) ->
  [selected](sell-1920x1080-selected.png) ->
  [quantity 2](sell-1920x1080-quantity2.png) ->
  [closed](sell-1920x1080-closed.png). Repair:
  [open](repair-1920x1080-open.png) ->
  [equipped weapon selected](repair-1920x1080-equipped-selected.png).
- 1600x720 Sell: [open](sell-1600x720-open.png) ->
  [selected](sell-1600x720-selected.png) ->
  [quantity 2](sell-1600x720-quantity2.png) ->
  [closed](sell-1600x720-closed.png). Repair:
  [open](repair-1600x720-open.png) ->
  [equipped weapon selected](repair-1600x720-equipped-selected.png).

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,864,011
- SHA-256: `7298d156a13a0d97c63525d3a6932699e28224c6c00ccce852f8a923583b976a`
- UI source: byte-identical temporary copy of the previously validated local
  complete 12,781-PNG shared UI staging pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves shared
service layout, visibility and local selection/quantity/close picking. It does
not prove an authoritative sale or repair, server pricing/durability mutation,
approved-WSS authentication, live StartGame/map transitions, production asset
alignment, a physical Android device, signing/store readiness or human
acceptance.
