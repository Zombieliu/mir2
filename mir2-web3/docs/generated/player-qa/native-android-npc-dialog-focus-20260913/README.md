# Native Android NPC dialog focus — 2026-09-13

## Scope

This proof extends the interaction-safe Android phone treatment to the exact
shared 440x224 Crystal `NpcDialogPanel`. There is no Android-only NPC dialog or
duplicated dialog state. Android applies a bounded transform to the shared
panel while the shared `NpcDialogModel` is open.

The first 3.2x emulator build was deliberately rejected because the panel was
too large for its content and lower controls. The retained implementation caps
this panel at 2.2x while Chat Settings, Inventory, Storage and Options retain
their existing 3.2x ceiling.

The Crystal frame already paints a red close glyph. The prior shared renderer
also appended a text `Close` row after `Return`, which could land behind the
frame footer. The shared renderer now binds a 40x40 logical hit target to the
visible red glyph and removes that clipped duplicate row. The action still
uses the existing `QuestUiButton::CloseNpcDialog` path, queues `@Exit`, closes
the shared model and clears dialog navigation.

## Verification

- Focus geometry covers the 440x224 panel at 891x411, 731x411 and 610x274
  logical phone viewports: passed.
- The 2.2x ceiling keeps the complete frame inside safe edges; the geometry
  model keeps a 28px action row above 55 physical pixels at the smallest
  tested API31 profile.
- Android Rust UI Preview suite: 170/170 passed.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Gradle `testDebugUnitTest`, `testUiPreviewUnitTest` and
  `assembleUiPreview`: passed under Android Studio JBR 17.
- Full licensed-asset APK streamed install: passed.
- Each capture reached `ANDROID_UI_PREVIEW_READY scene=npc`; checked logs had
  no panic, fatal exception, Bevy query conflict, missing-path error or Android
  cursor-position error.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
Each open image shows the shared Crystal NPC frame, offline dialog specimen,
disabled option, `Return` control and visible red-X close control. An ADB touch
on that visible glyph takes the real shared close path; distinct hashes prove
the dialog was removed at every tested resolution.

- [2340x1080 open](npc-2340x1080-open.png) ->
  [closed](npc-2340x1080-closed.png)
  (`0edef3635f77ed2cda13571c36281963a3b9671c7199745a710a6955c01048e8` ->
  `36f895666cd92599da255cf4627b3b2681af683ca36ab14fde76e1bb7c36fc56`)
- [1920x1080 open](npc-1920x1080-open.png) ->
  [closed](npc-1920x1080-closed.png)
  (`1d43760538e907525d4e52d4284b2a58c301b4f75122d60e27b0448a02a64615` ->
  `2a25224258ade664923069495b9f940630a10b1b2c7e82b259a0652e5fd763b2`)
- [1600x720 open](npc-1600x720-open.png) ->
  [closed](npc-1600x720-closed.png)
  (`ab77d889f439404ff5d301762b588a7a4353eff019c66e89957bfc143d521e24` ->
  `1eb28e478c4c574ae62a9bddab300194899511bf48b7f943b02ba0f4eea0e11b`)

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,883,059
- SHA-256: `a60e26c27c030e333d77a1d927a3631044809fb9d1f8982d5b1b650e0251ee47`
- UI source: byte-identical temporary copy of the previously validated local
  complete 12,781-PNG shared UI staging pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves the
shared NPC dialog's bounded phone focus and actual Bevy close picking. It does
not prove a live NPC conversation, approved-WSS authentication, StartGame/map
transitions, production asset alignment, a physical Android device,
signing/store readiness or human acceptance. NPC Shop focus remains open.
