# Android Help rows — 2026-09-09

Source: `08f658a53` (full SHA in source-head.txt), isolated Android branch,
LOCAL ONLY. Uses the shared Help renderer, not a second Android help model.

## Debug report (investigate)

- Symptom: Help shortcut page 1's final text covered the page controls in
  the preceding 33-scene capture.
- Root cause: 18 rows starting at y=142 with pitch 20 put the final row at
  y=482, beyond the page label's y=480; 23px text boxes also overlapped rows.
- Fix: 18px pitch and text boxes keep every row inside the body (last bottom
  y=466, body limit 475). Titles, content, navigation, page count and sprites
  are unchanged. Only shared `crystal_ui/overlays.rs` changed.
- Failure-first regression: `help_shortcut_rows_stay_inside_body_without_losing_content`
  checks every information row on all three text pages exists exactly once
  and stays above the footer. Before fix, page 0 Mount row ended at y=485;
  after fix the full shared suite passes 577/577.
- Android host tests: normal 83/83, ui-preview 85/85, Rust 1.95.0 offline
  through the platform-android manifest. Build/test logs retained with
  trailing whitespace normalized only.
- API 31 arm64 Android 12 emulator-5554, 2340x1080: preview installation and
  cold launch pass; page1.png shows all 18 rows above the footer. Touch at
  (1241,873) advances to page2.png; all 18 rows fit there too.
- Status: DONE_WITH_CONCERNS for this bounded overflow bug. Phone-specific
  help content, small touch targets/column spacing, full-screen HUD layout,
  other dialogs, live gameplay and physical-device acceptance remain open.

Preview APK (relative to mir2-web3):
`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
SHA-256: `c75040f0ea47b083cdbf6fe5c2ed1e73776d59b0c49b7ff45be38121271622e4`.
Built using the full build-android.sh uiPreview package path with external
shared assets and OpenJDK 21. APK/resources/keys are not committed.

Normal debug APK also rebuilt successfully through the full script (not
direct Gradle over preview JNI output):
`apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
SHA-256: `ece51f9502e093ef6cbf757785e1aeae50a843e05bfc42261ca0bc024daf668a`.
Normal APK was built, not used for the offline screenshots.

This is network-disabled UI preview; no real account, server operation or
save was touched. Windows audio-enabled tests and original-client visual
parity were not verified. PR #251 read-only API still reports Draft head
`99a1cd1219fe1223d42049de94fc82b61f53e318`; no failing push was retried.
