# Native Android wide-screen shell composition — 2026-09-13

## Scope

This change removes the visible black gutters around the shared Crystal login,
character roster, create/delete and starting surfaces on wide Android phones.
It does not stretch or replace the shared 1024x768 interactive stage.

- Android draws mirrored, non-interactive strips from the existing
  `original-ui/ChrSel/0.png` edge texture only outside the fitted stage.
- The strips use `FocusPolicy::Pass`, remain below the shared shell's z-order,
  and are removed from layout when no gutter exists.
- Entering `InGame` hides the shell decoration; the native map/object/entity
  renderer continues to draw the full device viewport.
- The Activity requests short-edge display-cutout layout. The resulting native
  surface is 2340x1080, while joystick, action controls and overlays continue
  to consume the real display-cutout safe insets.

## Deterministic verification

- Rust 1.95.0 formatting: passed.
- Android Rust `ui-preview` suite: **184 passed, 0 failed**.
- New geometry regressions cover 2340x1080 side gutters, a tall viewport,
  zero-gutter hiding and the `InGame` handoff.
- Gradle Debug and UI Preview unit suites: **25 passed, 0 failed** in each
  variant (14 Gateway session, 4 host policy, 4 foreground policy and 3 network
  policy tests); both Gradle tasks completed successfully.
- Full local licensed-asset UI Preview package and ADB streamed install passed.

## API 31 emulator evidence

All captures are 2340x1080 landscape frames from the dedicated
`Mir2_API_31_ARM64` AVD (`sdk_gphone64_arm64`, density 440). `dumpsys window`
reports a 2340x1080 requested/frame surface, zero content insets,
`layoutInDisplayCutoutMode=shortEdges`, and hidden status/navigation bars.

- [login.png](login.png): shared Login surface reaches both physical edges.
- [roster.png](roster.png): shared Character Select and preview retain their
  original geometry with background-only side extension.
- [create.png](create.png) and
  [delete-character.png](delete-character.png): real ADB taps on the shared
  roster's New Char and Erase Char buttons reached both authenticated character
  operation surfaces without the background extension claiming the input.
- [starting.png](starting.png): the render-ready loading barrier uses the same
  full-width composition.
- [world-render.png](world-render.png): entering the labelled offline world
  removes the shell decoration and leaves the actual map renderer full-screen.

Every scene emitted its exact `ANDROID_UI_PREVIEW_READY` marker. The checked
process logs contained no panic, fatal exception or missing-asset error.

Screenshot SHA-256 values:

```text
create.png            008e27169b3b3182a4a71e6100d2a7bf66ce3c4e9e1573ec09195f0254f5723d
delete-character.png  d8b094f5794d0dc52c6d209f2721d4559e23757974cc9f010d102a0219c89b39
login.png             a94f660a96849d7b320368c29e471d78f542a48cb06cb9369e702e704c743506
roster.png            56c2f4a42369c49e1a5df56723a832af7315b84a4bf11f8123aa80f75302c62a
starting.png          6034383bc4034ec7265e11040c699f2f8c704dfafb8414d0f7ca7c6ce389609b
world-render.png      49b01abbea732f23124546fb8d007ef977770de8aabae00240384426de7ca2fc
```

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444422784`
- SHA-256:
  `d0dd80dcbaa2be964ddc42575ec21e657d353aff6bd899ef5849fb71a4c09602`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK, decoded resources, caches and temporary copied source pack remain
outside Git.

## Acceptance boundary

This is deterministic layout and offline-emulator rendering evidence. No
approved WSS endpoint, test account or physical Android device was available,
so it is not real login, online StartGame/render-ready acceptance, public asset
release alignment or physical-device acceptance. The cutout policy is verified
on the API 31 emulator; OEM cutout/GPU/gesture behavior still requires a phone.
