# Native Android character-operation host evidence — 2026-09-13

## Scope

This change connects the existing shared Crystal character-create and
delete-confirmation UI to Android's dedicated authenticated roster state
machine. It does not route either operation through the in-game producer and
does not update the shared roster optimistically.

- `CreateCharacter` emits the existing Gateway `newCharacter` BrowserCommand
  only while the authenticated host is in `CHARACTERS`.
- `ConfirmDeleteCharacter` emits `deleteCharacter` only for an index present
  in the authenticated roster.
- `NewCharacterSuccess.character` and
  `DeleteCharacterSuccess.characterIndex` are validated before a one-shot
  event changes the shared model.
- Failure results leave the authoritative roster unchanged and make the
  operation retryable. A pending create/delete also blocks StartGame so the
  roster cannot race the selected character.
- Local selection, modal, class/gender and cancel intents remain local; they no
  longer receive the old generic “not wired” error from the Android adapter.

## Deterministic verification

- Rust 1.95.0 formatting: passed.
- Android Rust `ui-preview` suite: **182 passed, 0 failed**.
- JVM MockWebServer suite: `GatewaySessionTest` **14 passed, 0 failed** in both
  Debug and UI Preview; both Gradle unit-test tasks completed successfully.
- The TLS fixture checks exact create/delete JSON, operation serialization,
  authoritative success/failure roster handling, retry, and StartGame gating.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31 ./build-android.sh`:
  arm64-v8a target check passed with Rust 1.95.0 and NDK 26.1.10909125.

These are local TLS fixtures with generated test certificates. No real account
or server-side character was created or deleted.

## API 31 emulator evidence

The current UI Preview APK was streamed to the dedicated
`Mir2_API_31_ARM64` AVD and both shared character-operation surfaces reached
their labelled ready state without an Android crash.

- ABI / API: `arm64-v8a` / 31
- display: 2340x1080 landscape, density 440
- fingerprint:
  `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`
- `00-character-create.png`: shared New Character dialog, SHA-256
  `43a47336954a4d5ba6ed290b390ec358e7979a072d670f83c5cc9349f5a27a1b`
- `01-character-delete.png`: shared Delete Character confirmation, SHA-256
  `81339ae683c0884b9fed5d904a969f9fadd3e391aa3bfd21382aad87cf04940e`

The screenshots intentionally say `OFFLINE UI PREVIEW — NOT LIVE GAMEPLAY`.
They also retain the shared Crystal 4:3 shell's pillarboxing on this 19.5:9
display; they are not evidence that wide-phone background composition is
finished.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444365736`
- SHA-256:
  `1afade922df0bbd4a802f2be7289832d31c95c27154b4a267319f71ca1c1241b`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK, generated packs and caches remain ignored and are not committed.

## Acceptance boundary

This closes Android's source-level and emulator-verifiable role-management
host wiring. No approved WSS endpoint, test account, physical Android device,
or current public asset-release alignment was available. Therefore this is not
real login, live character mutation, online StartGame/render-ready transition,
authoritative online gameplay, wide-phone visual acceptance, or physical-device
acceptance.
