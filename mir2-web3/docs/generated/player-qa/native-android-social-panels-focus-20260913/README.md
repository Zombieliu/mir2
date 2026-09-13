# Native Android social panels phone focus — 2026-09-13

## Scope

This proof adapts the retained shared Crystal Group, Guild and Trade panels to
Android phone-sized landscape viewports. It does not add parallel Android
social or trade UI.

Group and Guild now expose exact bounded focus roots inside the shared overlay,
so Android can enlarge only the authored panel instead of scaling the entire
1024x768 desktop stage. Trade computes a focus rectangle from both draggable
shared trade windows and inverse-maps pointer input through the Android focus
transform. The accompanying inventory remains at its authored size, avoiding
the previous oversized overlap while retaining the shared item-offer surface.

While one of these blocking panels is open, the phone-only joystick, action
pad, bottom HUD and chat chrome are hidden. Closing the panel restores them.
No client-side group, guild or trade authority and no alternate transaction
path were introduced.

Three compile-time isolated UI Preview scenes provide bounded synthetic Group,
Guild and Trade models. They do not authenticate, connect to WSS or submit a
social/trade command.

## Verification

- Android Rust UI Preview suite: 177/177 passed.
- Focused shared-renderer tests for social geometry and dynamic trade focus:
  2/2 passed.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed with JDK 17
  and the configured Android SDK.
- `aarch64-linux-android` compile check at Android API 31: passed.
- Full shared-UI `uiPreview` APK package gate and streamed install: passed.
- The final APK reproduced the same 1600x720 Group screenshot hash as the
  inspected capture set after a cold launch.
- Checked launch and interaction logs contained no panic, fatal exception,
  ANR or SIGSEGV. `ANDROID_SOCIAL_FOCUS_APPLIED` and
  `ANDROID_SOCIAL_LAYOUT_COMPUTED` reported the bounded focused node.
- Temporary display overrides were removed after capture. `wm size` again
  reports only the physical 1080x2340 profile.

## Visible and touch evidence

The emulator is `Mir2_API_31_ARM64`, API 31, `arm64-v8a`, density 440, with
fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.

- Group open at [2340x1080](group-2340x1080-open.png),
  [1920x1080](group-1920x1080-open.png), and
  [1600x720](group-1600x720-open.png). A real ADB tap on the shared close
  target restored the HUD, chat, joystick and action pad:
  [closed](group-1600x720-closed.png).
- Guild open at [2340x1080](guild-2340x1080-open.png),
  [1920x1080](guild-1920x1080-open.png), and
  [1600x720](guild-1600x720-open.png). A real ADB tap selected the shared
  Members tab and displayed rank, username and status rows:
  [members](guild-1600x720-members.png).
- Trade open at [2340x1080](trade-2340x1080-open.png),
  [1920x1080](trade-1920x1080-open.png), and
  [1600x720](trade-1600x720-open.png). A real ADB tap on the shared own-gold
  target opened the shared amount modal and Android IME:
  [gold modal](trade-1600x720-gold-modal.png). The modal was not confirmed and
  no transaction was sent.

The primary 1600x720 open-state hashes are:

- Group: `b7e5759d089fba3eb313fdea71a3161a8f065f99ddd9edddbcb6440eb1a5ba7e`
- Guild: `37a26f4078277aae9bea4af70420de2d87ee9883f29d0ec67da4a34035cea675`
- Trade: `138adc3bfdd7e1b41a24d5d5b4cfa1464bcf9210d54c64d3a82bfe03b58061a7`

## APK artifact

The APK is an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 443,597,408
- SHA-256: `a7c3ccd1640962e4419905d63e4e9701e1ca2a5ea1cc0b9bcd55b96cafc9180e`
- UI source: the existing ignored complete shared-UI staging pack under
  `platform-android/target/shared-ui-assets`, containing 12,781 PNG files
- UI manifest SHA-256:
  `890ae5d8f0fca13246654c26c5826575887bf22faa7d44504c88d0dbd0307cca`

The APK, local UI staging pack, caches, credentials and signing material remain
outside Git.

## Acceptance boundary

This is offline UI Preview evidence on an API 31 ARM64 emulator. It proves the
shared Group, Guild and Trade visual geometry, bounded phone focus, the tested
local close/tab/amount-modal touch paths, and final APK installability. It does
not prove approved-WSS authentication, real account login, live group/guild
state, an authoritative trade transaction, the complete online player loop,
physical-device behavior, signing/store readiness, soak testing or human
acceptance. Overall Android completion must not be inferred from this proof.
