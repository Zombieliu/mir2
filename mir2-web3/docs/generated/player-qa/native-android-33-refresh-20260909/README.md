# Android 33-scene offline refresh — 2026-09-09

Source: `a8bb8195476f4ab110582f05aabff9db908ac180`, clean tracked worktree
at capture start. This checkpoint adds evidence only, not runtime changes.

Preview APK: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
(relative to `mir2-web3`). SHA-256:
`d94333da7a547fb741f4e119f35ca4d6633d953fdff9b419f477495dbd93a8f1`.
Built with `MIR2_ANDROID_VARIANT=uiPreview MIR2_ANDROID_MODE=package`,
external shared UI assets and OpenJDK 21; build log retained. Installed with
`adb install -r` on emulator-5554, API 31, Android 12 arm64, 2340x1080.
Exact device build fingerprint is in `device.txt`.

## Results

- `capture-ui-preview.sh` reached all 33 specimen-ready markers and screenshots.
  No panic, FATAL EXCEPTION or PathNotFound was found in the captured process logs.
- Fresh offline Cargo tests using the Android manifest and Rust 1.95.0:
  shared `-p mir2-client-bevy` 576/576; normal Android 83/83;
  `--features ui-preview` 85/85. Java and Windows gates were not rerun here.
- The checksum manifest lists all 33 local PNGs. Only death, Help and mail
  compose screenshots are tracked in this compact refresh; the complete PNG,
  launch and process-log set remains in this local ignored directory.
  A fresh checkout therefore cannot verify every manifest entry without that pack.

## Visual findings — not whole-UI acceptance

- Death: Android Revive is visible alongside the shared desktop V-key hint.
  This capture does not exercise the action or a real server response.
- Help: the final shortcut text overlaps the footer/page controls. Keep open
  for bounded list clipping/pagination and phone-specific input guidance.
- Mail compose: recipient/message fields and attachment rows are visible with
  IME open, but the bottom action area is obscured. Keyboard dismissal and
  targeted input tests from earlier packs do not establish full-panel visibility.
- Guild: populated offline notice and Edit affordance render; this capture
  does not exercise notice editing/publication.
- Create/amount were also sampled in the capture round: preview no longer
  covers Create; numeric amount editor is above IME, but its long heading clips.
- HUD remains a centered 1024x768 fit on the wide screen, with a white chat
  source frame. Full-screen adaptive HUD/world layout is NOT implemented.

All scenes are network-disabled fixtures; the empty background is not a
rendered online map. Protected password/SafeKey captures are not visual
acceptance. No live login, StartGame/player-loop, physical-device, touch/IME
matrix, real transaction, production deployment or save mutation is claimed.

PR #251 read-only API check on 2026-09-09 still returned Draft and remote
head `99a1cd1219fe1223d42049de94fc82b61f53e318`. Later source/evidence is
LOCAL ONLY. No unchanged failing push was retried.
