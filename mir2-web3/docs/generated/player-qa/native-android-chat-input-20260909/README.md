# Chat input repair — 2026-09-09

Source `a6af7cb6d6ae2fbf948d580f26cbaedd0e9d3185`, LOCAL ONLY,
isolated Android worktree. Shared write scope: crystal_ui/chat.rs. Android:
mobile_ui.rs and compile-time ui_preview.rs. No backend/auth/protocol changes.

## Debug report (investigate)

- Symptom: chat-settings tab did not react to API31 taps.
- Runtime trace proved physical cursor (1171,443) hit the intended
  SettingsTab(Chat) node with Interaction::Pressed. Capture was not lost
  in Android or coordinate conversion. Shared UI state was changed in the
  same frame; render_crystal_chat rebuilt/deleted the tree before the last
  system, consume_chat_button_interactions, read it.
- Fix: capture existing-tree button actions first, then reduce and render.
  Failure-first plugin test injects Pressed plus same-frame model change:
  old result Filters, expected Chat; now passes.
- Previous OverlayRoot/QuestUiRoot explicit Pass patches were removed.
  Bevy 0.19 FocusPolicy defaults to Pass; those additions were redundant.
  Their two tests were removed, explaining the lower shared count versus
  the diagnostic patch. No world/modal capture semantics were changed.
- Retained the independently tested Android no-op clamp guard: unchanged
  positions must not dirty all shared UI every frame. Old test changed
  ticks from 3 to 9, now stable. This alone had not fixed chat clicks.
- Default white area is authored Prguse/2221, not missing assets. Crystal
  MainDialogs.cs UpdateBackground uses index minus one in transparent mode:
  2220/2223/2226. Shared rendering now selects these rather than alpha-tinting
  the white frame and all text to 0.8. Six frame/mode cases tested.
- The offline chat-settings fixture previously set only panel, omitting its
  draft. It now opens from None through shared OpenChatSettings. A first
  fixture attempt toggled an already-open panel closed; an explicit
  initialization/Apply test was added and passed. Production guards unchanged.
- Temporary UI_TOUCH_TRACE instrumentation was removed after capture.

## Fresh verification

- Shared583/583, Android normal86/86, preview89/89 pass, Rust1.95.0 offline
  via platform-android manifest. Java unchanged/not rerun in this slice.
- Final preview installed on emulator-5554, Android12 API31 arm64 Pixel5
  AVD, 2340x1080 landscape. Existing AVD restarted without wipe.
- Cold launch chat-settings; (1171,443) switches tab (tab.png);
  (1218,558) selects transparent, (1248,645) Apply closes settings and
  switches chat background (applied.png). Earlier same sequence failed.
- Close/reopen via shared bottom Settings also worked in the diagnostic
  build. Final fixture initialization/Apply path independently verified.
- Both full-script packages passed with external staged assets, OpenJDK21.
  APKs were built from the exact patch above before commit creation.

Paths relative to mir2-web3:

- apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk
  SHA256 `e4e0c8617be47e13db3e26e77ab27d5b0f1a24674431a8987dd0e8d11a498de5`.
- apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk
  SHA256 `88d5ff454fc61c6591d08ca44fe7f2679033d74132f3483838c92c708de3bb48`.

Compact extracts and two screenshots tracked; full /tmp/android-chat-* logs
and prior failed local investigation pack retained. APK/assets/keys excluded.
No remote Web page loaded, real login, save mutation or live player loop.

Status DONE_WITH_CONCERNS for tab/input and transparent-frame repair, NOT
whole Android UI completion. Settings skin still shows inherited filter
background under ONE/TWO controls, so visual/source skin parity remains open.
Bottom HUD/chat/dialog mobile layout, phone-sized targets, gameplay masks,
multi-touch combat, mail IME, actual world/network projection and physical
device/soak/human acceptance remain open. Continue those tasks; no longer
waiting for approval to diagnose this click issue. PR251 remains Draft at
previously rechecked remote99a1cd121; no failed push retry in this slice.
