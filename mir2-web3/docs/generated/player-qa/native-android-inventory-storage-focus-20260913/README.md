# Native Android Inventory and Storage focus — 2026-09-13

## Scope

This proof extends the interaction-safe phone focus treatment to the exact
shared Crystal Inventory and Storage entities. It does not introduce an
Android copy of either panel. The Android stage fitter applies one bounded
`UiTransform` to each shared root, so Bevy drawing and picking consume the same
geometry.

The leaf also makes Inventory dragging touch-native. A drag owns only the
finger that began on exposed Inventory chrome, leaves another held joystick
finger alone, inverse-maps the focused hit point, and then follows raw
stage-space movement without jumping when the safe-edge translation is
materialized into the shared window position.

## Behavior

- Inventory and Storage can grow to 3.2x relative to the fitted 1024x768 stage.
- The focused panel stays within Android safe edges and a 16 logical-pixel
  gutter; the IME bottom inset participates in the same bound.
- Inventory category taps, Storage page taps, item cells and shared actions use
  the transformed Bevy picking geometry.
- Inventory drag tracks one touch identifier until release or cancellation and
  does not steal an already-held joystick finger.
- Android touch-to-pointer bridging emits Bevy `CursorMoved` messages rather
  than asking Android's window backend to move a nonexistent OS cursor.
- Locked Storage keeps the exact shared password model and panel, raises the
  soft keyboard above the safe bottom and retains Android `FLAG_SECURE` while
  the password editor is active.
- Other platforms retain identity transforms and the existing mouse behavior.

## Automated verification

- Shared focused-pointer round-trip and drag-materialization test: 1/1 passed.
- Shared multi-touch drag ownership test: 1/1 passed.
- Android Rust UI Preview suite: 170/170 passed.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Gradle `testDebugUnitTest`, `testUiPreviewUnitTest` and
  `assembleUiPreview`: passed.
- Full licensed-asset APK streamed install and launch on the API31 ARM64
  emulator: passed.
- The final Inventory launch and ADB swipe emitted
  `ANDROID_UI_PREVIEW_READY scene=inventory` with no panic, fatal exception,
  missing-path error, Bevy query conflict or Android cursor-position error.

## Visible and interaction evidence

The emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.
The physical profile is 1080x2340; captures are landscape. Every Items/Quest
and Page 1/Page 2 pair was produced by a real ADB tap and has distinct PNG
bytes, proving that the shared model changed rather than only the drawing.

- [2340x1080 Inventory Items](inventory-2340x1080-items.png) ->
  [Quest](inventory-2340x1080-quest.png)
- [1920x1080 Inventory Items](inventory-1920x1080-items.png) ->
  [Quest](inventory-1920x1080-quest.png)
- [1600x720 Inventory Items](inventory-1600x720-items.png) ->
  [Quest](inventory-1600x720-quest.png)
- [2340x1080 Storage Page 1](storage-2340x1080-page1.png) ->
  [Page 2](storage-2340x1080-page2.png)
- [1920x1080 Storage Page 1](storage-1920x1080-page1.png) ->
  [Page 2](storage-1920x1080-page2.png)
- [1600x720 Storage Page 1](storage-1600x720-page1.png) ->
  [Page 2](storage-1600x720-page2.png)
- [Inventory before native swipe](inventory-2340x1080-before-drag.png) ->
  [after native swipe](inventory-2340x1080-after-drag.png)
- [locked Storage above the IME](storage-locked-2340x1080-ime-console.png)

Representative SHA-256 pairs:

- 2340 Inventory Items / Quest:
  `0b83a2dfdce32f068317706e20bcc9aca9fb27cf5ab0aa8e01364c837d6b48d0` /
  `87f34e08cb3ad1b777abf00695bbd97908784adfba6ff7f628d5f38c9eb6a077`
- 2340 Storage Page 1 / Page 2:
  `e5087ebfe3f4d85d1baa7fc28917ae71cf5da84456af46e234c616bea24e263c` /
  `f7e8f0eefe01d0c84581ed49ca7635530e68d45ef74146423a53dd14863b197b`
- Inventory before / after drag:
  `0b83a2dfdce32f068317706e20bcc9aca9fb27cf5ab0aa8e01364c837d6b48d0` /
  `d37163a8f3bd533055ff48a73db7693933dca23fc4f70c564250c76077fc497f`
- Locked Storage with IME:
  `1daa6015418ff5565a97b3c5b80cc88f43816a369710d238a3ea8b7a43a07168`

While the password field was active, normal `adb screencap` returned zero
bytes because the Activity correctly set `FLAG_SECURE`. `dumpsys window`
confirmed `SECURE`, and `dumpsys input_method` reported both input flags true.
The checked-in image was captured through the emulator console solely to
inspect this test fixture's layout; it shows the complete locked shared panel
above Gboard. Back dismissed the IME first, then the panel, after which the
secure flag cleared and normal screenshots resumed.

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,868,403
- SHA-256: `21e75e3763d481ff81f1cb28319cb3d1e6d51f753104eb89a49411d0ab61f8b1`
- UI source: local complete 12,781-PNG shared UI pack
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves visible
shared Inventory/Storage scaling, real Bevy taps, native touch drag ownership
and secure IME layout. It does not prove approved-WSS authentication, live
StartGame or map transitions, production asset alignment, a physical Android
device, signing/store readiness or human acceptance. Real-account and
physical-device acceptance remain external gates.
