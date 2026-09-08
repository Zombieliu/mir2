# Native Android IME, Back and mail paging evidence

Scope: API 31 emulator, offline shared UI only. **Not whole Android completion,
real Gateway gameplay, physical-device or human Crystal parity acceptance.**

## Source and artifacts

- `79d6d7ce2`: shared modal Escape route, GameActivity Back interception,
  IME dismissal/reopening and bounded inventory amount editing.
- `99a1cd1219fe1223d42049de94fc82b61f53e318`: bounded shared mail compose layout,
  six-row attachment pages, three regression tests, 33-scene capture inventory.
- Latest normal APK: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  SHA-256 `80e6f7da6e8ea977e226b7a048da8b0004fc67f114ebfab8282da29b7f36ed1c`.
- Latest offline APK: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA-256 `1670d39fb2b8003ad043be3a67aa0d6decc95220866b274ac952028cf6af3c33`.
- Both latest APKs contain the source of `99a1cd121`; APKs and external resources
  are ignored, not committed. Native UI loads bundled Crystal images, not a
  remote Web page. No approved online Gateway version was exercised.
- API 31 Android 12 arm64 Pixel_5 AVD, `emulator-5554`, landscape 2340x1080.
  Exact fingerprint and capture provenance are in `scenes/`.

## Root causes and verified behavior

1. GameActivity 4.4.0 calls its native key handler before Activity fallback.
   The native path maps Android Back to BrowserBack, bypassing our
   `onBackPressed`. A temporary preview-only trace reproduced this (no callback
   after two Back presses); the trace was removed. The host now intercepts a
   tracked, uncancelled Back key-up, preserving IME pre-dispatch, then emits one
   shared Escape press/release pair. It does not reimplement modal cancellation.
2. OS IME dismissal left Java's editor state stale. Insets visibility clears it;
   shared editor presses are captured before overlay children rebuild so tapping
   the same field reopens IME without resetting it every held frame.
3. Quantity editing uses the existing shared numeric filter and upper bound.
   The offline fixture previously serialized `unique_id` instead of `uniqueId`,
   leaving items unaddressable. This fixture-only identity correction enables
   genuine shared local dialog testing; it does not fabricate server acceptance.
4. Mail compose previously emitted every bag item into a fixed 312x444 frame.
   Six-row paging now keeps all items reachable and Send/Cancel inside the frame.
   Shared attachment-count reducers and server authority are unchanged.

## Emulator interaction evidence

The `interaction/` screenshots were visually inspected:

- Amount: initial numeric IME → Back hides only IME → second Back cancels only
  modal, leaving bag. Retap `(1200,535)` reopens; two deletes plus `3` changes
  local draft from 12 to 3. No OK/delete operation was submitted.
- Mail: type `OfflineDraft` into recipient; first Back preserves draft and hides
  IME, second Back cancels compose and keeps the mail list.
- Help: Back closes the shared help panel.
- Paged mail: tap `(1620,518)` reaches page 2/2; tap `(1450,238)` selects slot 6
  as a local attachment (1/5, label becomes Remove); Back cancels compose.
  No mail was sent and no real inventory was modified.

Amount/initial mail/Help captures used preview SHA
`73b79fdebdc56d86080e08d58f71c6583a8bcf81d48cec103b4f836c722c0168`
(source `79d6d7ce2`). Paged mail captures used the latest preview APK above.
The current `scenes/` run uses the latest preview APK; protected password/SafeKey
frames are expected to be black and are not visual acceptance.

## Tests

- Android host Rust: 71/71 normal, 73/73 preview.
- Shared Bevy UI through Android manifest/feature graph: 575/575.
- Java TLS transport fixtures: 8/8 (mock server, not real account acceptance).
- Both APK package gates passed. Build/test logs are stored beside this file.
- Latest capture runner completed 33/33 scenes with no detected panic, FATAL
  exception or missing asset. This is a capture/runtime gate, not 33 complete
  interaction acceptances.
- Shared tests must use the Android lock graph here:
  `cargo +1.95.0 test --offline --manifest-path apps/game-client/platform-android/Cargo.toml -p mir2-client-bevy`.
  Standalone client-bevy offline resolution lacks its locked pkg-config 0.3.34;
  no lockfile was rewritten to mask that separate cache limitation.

## Remaining

Other nested fields' IME geometry, multiline input, touch-only gameplay,
complete real packet/read-model and intent host wiring, lifecycle/reconnect,
Android audio and physical acceptance remain open. The full task is not done.
See `docs/ANDROID-UI-COVERAGE.md` for the continuing acceptance boundary.
