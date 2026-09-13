# Android edge layout, first slice — 2026-09-09

Source `74883f8b3` (full SHA in source-head.txt), isolated Android worktree.
LOCAL ONLY: PR251 read-only API remains Draft at `99a1cd121`; no push retry.

## Implemented boundary

Shared `hud.rs` groups minimap frames, title/coordinate containers, light/mail
indicators and Mail/BigMap/MinimapToggle buttons under `CrystalHudMiniMapLayer`.
The group's default transform is identity: desktop Crystal positions remain
unchanged. Bottom HUD/belt controls remain outside this group.

Android places this group and the separate minimap image root at the same
right/top screen origin, keeping existing uniform sprite scale. The host
reports visible system-bar/display-cutout top/right insets; layout reserves
these plus 8 OS-logical pixels. Nested group offsets compensate the centered
HUD parent, including its IME pan. Layout writes run before Bevy UI layout;
button hit testing uses the transformed nodes, without rewriting the window
cursor used by centered dialogs. Panels rail uses the same safe edge and
sits below the expanded/collapsed minimap, retaining its independent size.

Only the network-disabled HUD preview injects BichonProvince/320,43 to inspect
the minimap crop. This is NOT a real map bootstrap, account or world render.

## Verification

- Shared579/579: new hierarchy test checks minimap membership/default origin
  and separates non-map HUD controls. Windows audio-enabled suite not run.
- Android normal84/84 and preview86/86: edge geometry tested at 4:3, 16:9 and
  phone-wide dimensions, with DPI/safe inset and IME-parent compensation.
- Java GatewaySession tests8/8 pass. Both full-script APK package gates pass.
- API31 Android12 arm64 emulator-5554, 2340x1080, preview installed/cold-started.
  `expanded.png`: crop/frame aligned at right top; Panels below, not overlapping.
  Touch (2302,38) collapses; touch (2160,67) opens shared Mail (`mail-hit.png`).
  Touch compose (1360,606) opens IME (`ime-safe-edge.png`): map/rail shift left
  together to clear visible navigation bars, while the dialog stays separate.
  Current process log has READY and no panic/FATAL EXCEPTION/PathNotFound.
- Compact extracts tracked. Full build/test logs remain under
  `/tmp/android-edge-*.log` and process log `/tmp/android-edge-logcat.txt` locally.

APK paths relative to `mir2-web3`:

- Preview: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA256 `6022aa3963bce8b9ebd4802b1d0b681117117de34494237a10d26b54cf6194e3`.
- Normal: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  SHA256 `c20c909277eb37736ef52cecba9aafdbe6e60e59abb10bee792a4f1ed1d44935`.

## Still open

This is only the minimap/rail edge slice, NOT whole-phone fullscreen completion.
Bottom HUD/chat/belt remain centered, and world rendering/gameplay integration
is incomplete. Shared hover hints still clamp into old stage coordinates
(visible in mail-hit.png); hint positioning and broader gameplay input masks
must be adapted before whole-screen interaction acceptance. Coordinate text
spacing, small minimap targets, all device cutout/rotation/IME variants and
mail footer hidden by IME also remain open. No physical device, real login,
server-authoritative gameplay or save/production mutation was verified.
