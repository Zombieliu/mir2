# Android shared chat settings skin checkpoint

Source: local `5b909ce45`, branch `codex/android-player-journey`.
Status: bounded offline UI fix verified; not full phone UI or online acceptance.

## Root cause and fix

The investigate workflow used source/asset comparison and failure-first tests.
`ChatOptionDialog.cs:245-292` changes the label-bearing background to Title/467
on the Chat tab; shared `chat.rs` always loaded Title/466. The shared renderer
now selects the background from the active tab.

Visual inspection of the actual staged PNGs additionally found that Title
462/463 say CHAT BOX and 464/465 say FILTER, opposite the legacy C# button
index assumptions. The shared buttons now pair those labels with their actual
actions. No image assets or reducers were changed. Transparency ONE/TWO labels
are still the staged source images; phone-sized targets and clearer presentation
remain open, so this is not a claim of complete settings visual acceptance.

## Tests and build

`settings_background_tracks_active_tab` first failed for Chat (466 vs 467).
Its label assertions separately failed for Filters (463 vs 465), then passed.
It covers Filters → Chat → Filters, modal image and normal/hover/pressed label
families. Existing same-frame input/model-invalidation regression still passes.

- Shared Rust: 584 passed, 0 failed.
- Android normal: 86 passed, 0 failed.
- Android ui-preview: 89 passed, 0 failed.
- Both variants packaged using the full `build-android.sh`, sequentially.
- Java unchanged; Java unit tests not rerun in this slice.
- Build warnings remain; no unrelated warning cleanup.

Local full logs: `/tmp/android-chat-skin-{red,label-red,shared,normal,preview}.log`
and `/tmp/android-chat-skin-{preview,debug}-build.log`.
Compact extracts are in `results.txt`.

APKs relative to `mir2-web3/apps/game-client/platform-android/` (not committed):

- `android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA-256 `76faaa67f74495c0a5501652914b7d83b1be43e728aa5017076c2fdec1ec43b2`
- `android/app/build/outputs/apk/debug/app-debug.apk`
  SHA-256 `1caaa85747157526fec86677126d4a4d9afd7372a57771372cc8b86edd2d62ac`

## Emulator evidence

Existing Pixel_5_API_31, `emulator-5554`, Android 12/API31, arm64,
2340×1080 landscape; no AVD wipe. Installed final preview APK with `adb install -r`,
cold-launched `com.mir2.web3.uipreview/com.mir2.web3.MainActivity` with
`--es ui_scene chat-settings`.

1. `filter.png`: FILTER selected, filter content and checkboxes.
2. Tap physical `(1171,443)`; `chat.png`: CHAT BOX selected, transparency background.
3. Tap `(1218,558)`, then `(1248,645)` in separate steps;
   `applied.png`: modal closes and transparent chat frame is visible.

Before the final label change, the same package flow also verified a return
to Filter; the final regression test checks both directions. Final package
Cancel/Close was not independently rerun in this slice.

## Boundaries and next

Native compiled offline fixture, no remote Web page. Resources are the local
staged original-ui pack, not a newly released asset version. No Gateway login,
server state, world renderer, physical device, soak or production acceptance.
Bottom HUD/chat/dialogs still predominantly use a centered desktop fit.
Next: phone-wide bottom layout and matching input masks, mail keyboard
occlusion, phone targets/multitouch and real shared-state integration.

Commit is LOCAL ONLY. PR251 was previously verified Draft at remote
`99a1cd1219fe1223d42049de94fc82b61f53e318`; not rechecked this slice.
No repeat push after known transport failures, no deployment or merge.
