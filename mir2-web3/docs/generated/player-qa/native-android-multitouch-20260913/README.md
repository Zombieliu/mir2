# Native Android multitouch ownership evidence — 2026-09-13

## Scope

This evidence closes the emulator-verifiable part of Android gameplay touch
ownership. It does **not** claim a live player loop or physical-device
acceptance.

The defect was specific to Bevy 0.19 UI focus: it resolves touch hover from
`first_pressed_position()`. Once the first finger owned the joystick, the
second finger could not activate a Bevy action button even though the Android
pointer bridge retained it correctly.

The Android shell now:

- reserves one touch ID for the joystick;
- evaluates later just-pressed touch IDs against the already-laid-out Android
  action-pad and panel-rail geometry after Bevy UI focus;
- emits the existing `Interaction::Pressed` edge for the shared action handler
  and clears that synthetic edge on the following frame;
- never applies this multi-pointer path to shared Crystal window content, so
  inventory/window drag ownership remains serialized through the existing
  single-pointer bridge;
- cancels joystick and pointer ownership when screen, panel, security panel,
  chat focus, amount modal, IME, help, trade dialog, Android lifecycle, or
  window-focus context changes.

## Deterministic verification

- `cargo +1.95.0 test --features ui-preview`: **182 passed, 0 failed**.
- The suite includes exact target routing while joystick touch ID `17` is
  owned: `AttackTarget { object_id: 731 }`,
  `PickUpObject { object_id: 44 }`, Run true-to-false, and local Skills open.
- It also covers a real secondary `TouchInput` hit against computed button
  geometry, same-frame joystick capture without a ghost UI press, secondary
  drag non-inheritance, panel transition cancellation, background/resume, and
  IME context cancellation.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31 ./build-android.sh`:
  arm64-v8a target check passed with Rust 1.95.0 and NDK 26.1.10909125.
- Gradle `testDebugUnitTest testUiPreviewUnitTest`: build successful, 42 tasks.

## API 31 emulator evidence

Device:

- AVD: `Mir2_API_31_ARM64`
- ABI / API: `arm64-v8a` / 31
- display: 2340x1080 landscape, density 440
- fingerprint:
  `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`

The UI Preview APK was installed with streamed ADB install. The userdebug
emulator's `/dev/input/event1` Protocol-B device was then used to hold slot 0
on the joystick while slot 1 independently tapped Android controls. This is
kernel-level concurrent emulator input, rather than two sequential
`adb input tap` commands.

Observed offline-preview events while slot 0 remained held:

```text
ANDROID_UI_PREVIEW_INTENT move x=0.6276678 y=-0.7784813 run=true
ANDROID_UI_PREVIEW_INTENT attack object_id=731 (queued only, no server)
ANDROID_UI_PREVIEW_INTENT pickUp object_id=44 (queued only, no server)
ANDROID_UI_PREVIEW_INTENT runMode run=false (local control only)
ANDROID_UI_PREVIEW_INTENT move x=0.6276678 y=-0.7784813 run=false
ANDROID_UI_PREVIEW_ACTION skills (local panel only)
ANDROID_UI_PREVIEW_TOUCH_CANCEL joystick owner=0 blocked=true
ANDROID_UI_PREVIEW_TOUCH_CANCEL pointer owner=Some(1) context_changed=true
```

Pressing Android Home while the joystick slot remained held produced:

```text
ANDROID_UI_PREVIEW_TOUCH_CANCEL joystick owner=0 focused=false
```

The slot was released in the background. Resuming produced no stale movement;
a new gesture is required.

Screenshots:

- `00-baseline.png`: explicitly marked offline multitouch fixture.
- `01-two-finger-actions.png`: action pad after the held-joystick secondary
  Attack, Pick, and Run sequence; Run is visibly toggled off.
- `02-joystick-menu-open.png`: panel rail opened by the second finger while the
  joystick remained active.
- `03-secondary-skills.png`: shared Skills panel opened by the second finger;
  world controls are removed and both touch owners are cancelled.
- `04-resume.png`: clean foreground state after Home/background/release/resume.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `443647280`
- SHA-256:
  `d733c3f3fb1b423c3c4be81b38af6d31a60880f1e75e5239a4ce6f281f25013e`
- The APK and packaged asset/cache outputs remain ignored and are not committed.

## Acceptance boundary

All targets and actions above are explicit offline UI Preview fixtures. No
approved WSS endpoint or real account was available, so this run did not prove
authenticated login, online StartGame, authoritative movement/combat/pickup,
or reconnect state. An emulator userdebug input device is also not a physical
screen/digitizer: real two-thumb ergonomics, vendor IME, interruptions,
thermal behavior, and true-device background/network recovery remain required
physical-device acceptance work.
