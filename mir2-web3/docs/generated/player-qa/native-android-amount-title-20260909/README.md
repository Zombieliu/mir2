# Inventory amount title — 2026-09-09

Source `ce08b88c9` (full SHA in source-head.txt), isolated Android branch,
LOCAL ONLY. Shared renderer change only; no quantity/confirmation rules changed.

## Debug report (investigate)

- Symptom: the captured amount prompt cut off the `UI specimen 11` item name.
- Root cause: one NoWrap text node, height 14 and width 158, could not hold
  the complete sentence. This was independent of Android IME panning.
- Fix: use WordBoundary wrapping in a 28px title region at y=4. The item
  starts at y=34, input at y=43; the close button stays outside title width.
  No text, item identity or confirmation requirement was removed.
- Regression: `delete_amount_title_reserves_two_lines_before_item_and_input`
  initially fails on NoWrap versus WordBoundary; passes with this change.
  It checks full text preservation and node layout, not arbitrary rendered
  string lengths. Fresh shared578/normal83/preview85 tests pass (Rust 1.95.0,
  offline Cargo through the platform-android manifest).
- Device: emulator-5554, Android 12/API31 arm64, 2340x1080; no physical device.
  Installed preview APK, cold-launched `ui_scene=inventory-amount`.
  `title-ime.png` shows the full specimen name in two lines with the numeric
  keyboard and controls visible. Touch Cancel at (1235,587) closes dialog/IME;
  `cancel.png` retains the fixture stack quantity 12. No delete was executed.
- Status: DONE_WITH_CONCERNS. This closes the observed specimen truncation;
  names requiring more than two lines, no-space names, localization/font
  variants and physical-device usability remain unaccepted. Full-screen
  HUD layout and real gameplay integration are still open.

Both full-script package builds pass with external shared assets/OpenJDK21.
APK paths relative to `mir2-web3`:

- Preview: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA-256 `bc63364467af19c1b5e72ce61ebc5fbe3cbf15899c89ee6884db22bc8c28b299`.
- Normal: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  SHA-256 `c9f8906e92a473b0006c5d2683decd54e8c5417a05d50ab400682e879af2ff77`.

Compact test/build extracts are tracked; full logs remain at
`/tmp/android-amount-{before,shared,normal,preview,build,debug-build}.log`
on this Mac and are not durable remote artifacts. No APK/resources/keys in Git.
This is offline preview, not real login/StartGame or transaction acceptance.
PR #251 read-only check remains Draft at remote `99a1cd121`; no push retried.
