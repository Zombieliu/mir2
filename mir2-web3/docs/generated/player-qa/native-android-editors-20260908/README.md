# Android long-form and gold editor regression

Status: emulator/local evidence, **not whole Android completion**.

Source `360fb79e6` extends the shared amount-dialog IME geometry to trade/guild
amounts and marks their text controls for same-field IME reopening. Preview
player gold now matches its explicitly offline inventory fixture; the actual
shared zero-gold guard remains unchanged.

Source `fafbc716f07d4febbdf391b592e31ee2496dde52` enables multiline OS editing
only for mail-message and guild-notice. A newline remains text, not shared
Submit. Credentials, recipient, character name, chat and amounts remain single
line. Shared mail reducer preserves newlines and enforces its 256-character cap.

API 31 arm64 Pixel_5 AVD, landscape 2340x1080, `emulator-5554`:

- Trade: launch offline `trade`, tap `(970,778)` to open amount. IME stays below
  the dialog; Back then tap `(1200,535)` reopens it. Two Back presses cancel
  only the amount dialog, leaving trade windows. No offer was confirmed.
  Captures use preview `e90bb28fb60ef8e4186270bc1830073d398df6d4b77651ebba62c0d44aad3c31`
  with the runtime changes of `360fb79e6`.
- Mail: launch offline `mail-compose`, Back hides recipient IME; tap
  `(1460,110)` focuses message. Type `A`, Enter, `B`; screenshot shows two lines
  and a newline IME key. Compose remains open; no mail was submitted.
  Capture uses preview `9025fbe604e76553f7b60b99734ecfd4ee439161231f7a6cee74e4f96910f64e`
  with the runtime changes of `fafbc716f` (test-only additions followed the build).
- Guild amount and multiline notice use these same host paths but were not
  separately accepted on a populated guild fixture or physical device.

Latest normal APK at `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`:
`f0e6dbfa041335ba4a313451b1f0cd5193fb6f2772f4064c421935c980992484`,
built at `fafbc716f`. Latest preview path is the sibling
`uiPreview/app-uiPreview.apk` with the `9025...` hash above. No APK is in Git.

Tests: Android normal 74/74; preview 76/76; shared UI 575/575; Java TLS fixtures
8/8. Shared tests were run after the gold changes; subsequent multiline changes
are Android-only. Fresh logs are included; generated trailing whitespace was
normalized for Git. No production endpoint, real login or real save was used.

Whole task remains open: other nested editors, touch gameplay, actual host
packet/read-model and intent wiring, Android audio, reconnect, physical tests
and exact human Crystal parity. Only an emulator is currently attached.

Publication check: GitHub PR #251 was still Draft with head `99a1cd121` after
HTTPS pushes failed with HTTP 408 and an SSH attempt was closed by the remote.
The evidence and later editor commits were LOCAL ONLY at this check. Do not
infer successful publication from Git's misleading `Everything up-to-date`
line after RPC failure; recheck the PR head before updating this status.
