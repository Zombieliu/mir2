# Android shared belt safe-edge slice — 2026-09-09

Source `f2246b476018c5ca079a36e1296a4acf5838d249`, isolated Android
worktree/branch, LOCAL ONLY. Shared write scope: `client-bevy/src/crystal_ui/hud.rs`;
Android write scope: `shared_shell.rs` and `MainActivity.java`.

The shared belt frame, six targets and Rotate/Close controls now have an
identity presentation parent. Windows geometry remains unchanged. Android
positions the group at the bottom-left safe edge only when the left gutter
can contain its current orientation; otherwise it retains source positioning.
Java supplies visible system-bar/cutout left/bottom insets. IME visibility
hides the group without changing the shared visible/orientation preference.
No item-use reducer, protocol, authentication or server rule is replaced.

## Verification

- Shared 581/581; Android normal 85/85; preview 87/87; Java 8/8.
  Tests include identity hierarchy/ten shared children and both orientations,
  bottom inset containment, narrow-screen and insufficient-gutter fallback.
- Both full-script APK builds pass (Rust 1.95.0, OpenJDK21, external staged assets).
- API31 Android12 arm64 emulator-5554, Pixel5 AVD, 2340x1080 landscape.
  Cold-launched `com.mir2.web3.uipreview`, `ui_scene=hud`.
- `horizontal.png`: empty six-slot belt at left bottom, no longer over chat.
- Touch (344,1018) rotates the shared controls and slots together:
  `vertical.png`.
- Mail (2160,219), compose (1360,606) opens IME: `ime.png`, belt hidden.
  Back key4 restores the vertical belt; inspected `/tmp/android-belt-restored.png`.
- Touch (35,1043) closes the restored vertical belt; inspected
  `/tmp/android-belt-closed.png`. Subsequent layout frames do not resurrect it.
  These two supplemental captures remain local, not in the compact tracked pack.

## APK provenance and limits

Paths relative to `mir2-web3`:

- `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA256 `7ccb4587544447c53fd1c5bba6ae3e6d81b24b9eb7512aeb2cb30e4c0f5baca5`.
- `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  SHA256 `d874bb4e89c31a1f05c94d9edc1a2110e6222bcd59cef9ea7cea8291604eb498`.

Builds were made from the exact three-file patch committed above, before
creating its commit. No remote Web page is loaded by this native preview;
the Bichon minimap and other values are offline specimens, not server state.
Full logs remain `/tmp/android-belt-*.log`; compact extracts are tracked.
No APK, resource pack or signing material is committed.

This is layout/empty-belt control evidence, NOT live item use or whole mobile
adaptation. Targets retain source scale and need phone-sized hit areas.
Bottom HUD/chat remain centered; chat has a conspicuous white empty region.
Mail composer bottom controls remain obscured by IME. Complete gameplay
input masks, joystick/combat layout, full world rendering, live player loop,
physical-device/cutout/soak/human acceptance remain open.

PR251 rechecked Draft with remote head `99a1cd1219fe1223d42049de94fc82b61f53e318`.
No repeated failed push, production deployment or real-save mutation.
