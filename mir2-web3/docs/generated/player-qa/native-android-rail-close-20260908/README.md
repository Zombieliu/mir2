# Touch rail Close includes shared Help

Source `ac1a37d55`; API 31 arm64 Pixel_5 emulator, landscape 2340x1080.
Android-only change in `platform-android/src/mobile_ui.rs`.

Debug report (investigate):

- Symptom: offline Help → Panels `(2230,198)` → Close `(2230,770)` leaves Help visible.
- Cause: Android Close called `close_windows`, which deliberately omits Help;
  the shared `close_all_windows` adds `help.hide()`.
- Fix: call that shared method. No parallel cancellation rules, network command
  or server mutation was added. Existing amount-modal/inactive-shell guards stay.
- Regression: `touch_close_also_closes_shared_help` failed before the change;
  full host suite passes after it. An additional test checks both blocking guards.
- Fresh emulator replay: the before screenshot contains Help after Close;
  after screenshot shows Help dismissed and rail collapsed.
- Status: DONE for this bounded defect, not whole Android acceptance.

Tests: normal host 76/76, preview host 78/78 (overlapping), both package gates pass.
Shared UI source and Java unchanged; prior shared 575/575 and Java 8/8 are
earlier-checkpoint evidence, not rerun in this Android-only round.

APKs relative to `apps/game-client/platform-android/`:

- `android/app/build/outputs/apk/debug/app-debug.apk`:
  `aeb2d63eb1680decd2b3470ba89eef6019943a4d47bdad1718b24603b57b2871`.
- `android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`:
  `b4469eaaffa8a6a34ca13ac18694d4e6d143b301fba8095737a9388ef3d627c8`.

Before replay used the prior preview `9025fbe6...`; after replay uses the latest
preview above. APKs/assets are ignored; logs have generated whitespace normalized.
No live Gateway, physical device, credentials, production deployment or save was used.

Read-only GitHub check still reports PR #251 Draft at `99a1cd121`. This commit
and preceding local-only commits are not published. No repeated push attempt or
network/proxy change was made. Original checkout remains untouched.

The full Android UI/gameplay queue remains open; see `docs/ANDROID-UI-COVERAGE.md`.
