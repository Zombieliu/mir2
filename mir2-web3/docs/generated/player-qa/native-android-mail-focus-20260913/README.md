# Native Android shared Mail phone focus — 2026-09-13

## Scope

This proof keeps the shared Crystal Mail implementation and makes its real
`OverlayMail` root an Android phone-focus panel. The Android host now enlarges
and clamps the authored 312x444 Mail frame inside the safe landscape viewport;
it does not introduce a second Mail UI or duplicate mail rules.

While the Android IME is open, the focus calculation uses the visible compose
fields and reflowed shared footer rather than the hidden 444-pixel attachment
area. Recipient, message, Send and Cancel therefore remain visible above the
keyboard. Hiding the IME restores and enlarges the same shared attachment/gold
controls and footer. The UI Preview fixture includes offline gold so the real
Claim control can be inspected without fabricating a server result.

## Verification

- Android Rust UI Preview suite: 173/173 passed.
- Focused shared Mail button test passed: selection remains local while Read,
  Claim and Delete emit their exact shared intents and retain authoritative
  state until a server refresh.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed under Android
  Studio JBR 17 and the configured Android SDK.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Full licensed-asset APK streamed install: passed.
- `mail` and `mail-compose` reached `ANDROID_UI_PREVIEW_READY` at 2340x1080,
  1920x1080 and 1600x720. App logs contained no panic, fatal exception, Bevy
  query conflict or asset-load failure.
- The emulator size override was reset after capture; `wm size` again reports
  the physical 1080x2340 profile.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.

- Shared inbox with unread/attachment marker and Read/Delete/Claim controls:
  [2340x1080](mail-2340x1080.png),
  [1920x1080](mail-1920x1080.png), and
  [1600x720](mail-1600x720.png).
- Shared compose with the Android IME and the visible recipient/message/footer:
  [2340x1080](mail-compose-2340x1080.png),
  [1920x1080](mail-compose-1920x1080.png), and
  [1600x720](mail-compose-1600x720.png).
- Real ADB input entered `Receiver`, focused the message field, entered two
  lines, and kept Send/Cancel above the keyboard:
  [multiline IME](mail-compose-multiline-ime.png).
- Android Back hid the IME and restored the attachment/gold area without
  losing the draft: [IME hidden](mail-compose-ime-hidden.png).
- The shared close button restored world controls, then the compact Android
  panel rail reopened Mail and retained the same draft:
  [closed](mail-closed.png), [panel rail](mail-reopen-menu.png), and
  [reopened](mail-reopened.png).

Read, Claim and Delete were also tapped in the offline preview without a crash,
but the preview intentionally discards network effects. Their authoritative
success is not inferred from unchanged screenshots; exact intent generation is
covered by the focused shared test above.

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,890,187
- SHA-256: `6c63105e27e4e0f5f9c5b8c84c73f41c3e97d2ab53d56f63673667ba78a0f5a8`
- UI source: the same local complete shared Crystal UI staging pack used by
  the preceding Android UI proofs
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is labelled offline UI Preview evidence on an emulator. It proves shared
Mail phone geometry, real Android picking for compose/close/reopen, IME entry
and restoration, and shared Read/Claim/Delete intent semantics. It does not
prove approved-WSS authentication, server-confirmed mail operations, a live
player loop, a physical Android device, signing/store readiness or human
acceptance.
