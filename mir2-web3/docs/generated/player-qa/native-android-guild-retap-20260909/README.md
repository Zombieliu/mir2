# Guild notice keyboard reopening

Source `bed749479`, Android isolated worktree, 2026-09-09 Asia/Shanghai.

## Debug report (investigate)

- Symptom: Edit → Back → tap notice body did not reopen IME.
- Root cause: shared notice body was passive text, without the input marker
  consumed by Android's existing pre-update touch capture.
- Fix: a transparent Button + NativeTextInputTarget over the exact body bounds,
  only with notice permission, editing active and no pending receipt. No
  OverlayButton action is attached; retap cannot publish or reset the draft.
- Regression: rendered-entity test failed before (0 targets vs 1), then passed.
  Four states cover editable, read-only, pending and permission-revoked cases;
  assertions also lock geometry and absence of a publishing action.
- Fresh verification: API 31 Android 12 arm64 Pixel_5 emulator, 2340x1080.
  Edit `(802,821)`, newline + KEEP-DRAFT, Back, body `(950,370)` reopens IME;
  append -B, Back/body again retains KEEP-DRAFT-B. Back → Cancel `(870,821)`
  restores original notice; tapping the read-only body leaves keyboard hidden.
- Status: DONE for this bounded retap defect; whole Android acceptance remains open.

Tests: shared visual feature suite 576/576, Android normal78/preview80
(Android variants overlap). Both native APK package gates pass.
Java unchanged; previous 8/8 is not a fresh Java run. Windows audio-enabled
suite is not established by the shared visual suite.

APKs under `apps/game-client/platform-android/android/app/build/outputs/apk/`:

- debug/app-debug.apk SHA-256:
  `a91256190d5335bd337116ba41bddd8bb74fa858dd70090adb07ca7b6cf9c213`
- uiPreview/app-uiPreview.apk SHA-256:
  `65591cbcc893d30a53b15bd920cffa1ab62d7c534cae846964e587736d0bbbe5`

Before used `77edc7afb` preview `459eaa76...`; after used source above.
Builds preceded the source commit; only test whitespace was formatted afterward.
No APK, external resources or secrets committed. Logs have whitespace normalized.

Only shared `client-bevy/src/crystal_ui/overlays.rs` changed. It adds an inert
touch surface, not alternate client/server rules. No Publish action was used;
offline preview host still rejects network commands. No live Gateway/account,
save, production operation or original-worktree edit.

Remote PR #251 read-only check remains Draft `99a1cd121`; this is LOCAL ONLY.
No retry of the unchanged push failure. Full-screen layout, other nested
editors, touch gameplay, real network projection and physical device gates remain.
