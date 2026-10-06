# Android pointer and drag regression — 2026-09-08

Source: `62344b0e573ec022378866f4223e7bb1fddd0bf9`. APKs were built from the
identical staged source immediately before this commit (no build-time source
SHA injection). Only `platform-android/src/mobile_ui.rs` changed. No shared
Windows source/backend, original checkout, credentials or saves were edited.

## Investigate report

- Symptom: fast taps could affect the previous pointer target, leaving an item
  overlay after the bag disappeared. A top-edge drag showed Android chrome.
- Root cause: the Android pointer adapter and Bevy UI Focus both ran after
  InputSystems, with no relative ordering. UI prefers Window cursor position
  over touch fallback. A same-frame start/end also synthesized a held mouse
  without the release edge. The old test now fails explicitly on missing release.
- Fix: adapter runs before UI Focus; same-frame tap preserves both edges but
  does not hold; release updates pointer position; focus loss clears it. Mobile
  panel actions run after shared mutations and before render. Only Android local
  movable bag/help positions receive a 24 OS-logical-pixel top gutter, keeping
  the common stage and pointer transform unchanged.
- Source Crystal tab/Add/Close controls are not drag handles. Initial probes
  at x640 and x800 were controls, not evidence of a failed drag; the verified
  exposed source drag strip was x766 at the initial bag location.
- Verification: normal host **68/68**, preview **69/69** (overlapping), shared
  visual client **572/572**. Both APK builds passed. No Java changes this slice;
  no new Java-suite result claimed.
- API 31 emulator: preview installed and ready; drag (766,82) → (1066,400)
  moved bag while preserving source layout and without system chrome. Fresh
  inventory specimen: short taps (487,140), (2230,198), (2230,770) produced
  item inspect → expanded Panels → closed bag AND inspect. Before/after PNGs
  are included. Screens are offline fixtures, not real inventory operations.
- Status: DONE_WITH_CONCERNS for this bounded regression. Whole Android UI,
  all nested controls, multi-touch gameplay, live Gateway and physical device
  remain open in `ANDROID-UI-COVERAGE.md`.

## Artifacts

Paths relative to `mir2-web3/apps/game-client/platform-android/`:

| Package | APK | SHA-256 |
| --- | --- | --- |
| com.mir2.web3 | android/app/build/outputs/apk/debug/app-debug.apk | 5f5f57bbc4e7d469f449aec9b82d5bb2b550588d0751ece284c84df209d4e707 |
| com.mir2.web3.uipreview | android/app/build/outputs/apk/uiPreview/app-uiPreview.apk | bf4a94288aa6c1d7cbd1eef67bba43aebb47f971d277eb22f9ae15d64c8dfafc |

Native packages, no remote webpage. External Crystal PNG assets unchanged from
the preceding player-UI baseline; no production resource version is asserted.
APKs/resources/keys remain ignored. Device fingerprint in `device.txt`.
The preview APK was installed/tested this slice; normal debug was built only.

## Next

Exercise nested panel/IME/Back and item-action intent routing, then bounded
touch-only gameplay controls. Continue offline work until the approved Gateway
and physical-device gates genuinely require user input. Do not reopen the two
now-tested failures merely because the automation prompt names the older head.
