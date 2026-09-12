# Native Android shared UI baseline — 2026-09-08

**Partial implementation baseline, not whole-UI acceptance.**

Source: `dd6f9a474c481e29721cde3fc16b730f8a284c16` on
`codex/android-player-journey`. Subsequent evidence-only commits do not change
these APKs. Original checkout and Windows backend remain untouched.

## Packages

Paths below are relative to `mir2-web3/apps/game-client/platform-android/`.

| Variant | APK | SHA-256 | Bytes |
| --- | --- | --- | --- |
| Normal debug | `android/app/build/outputs/apk/debug/app-debug.apk` | `16c44fa01d43ed0663d0a18101a7f1557a9356a4a4145d541907b36f53e1e533` | 325682470 |
| Isolated offline preview | `android/app/build/outputs/apk/uiPreview/app-uiPreview.apk` | `145c8cef470cd9b8a333c92ff9e1face9650cf589c36e43dedee7fcc8b4ffc26` | 369988966 |

Packages: `com.mir2.web3` and `com.mir2.web3.uipreview`. Both final APKs
built and installed successfully. Normal debug launches the shared Login
and explicitly reports that the test server is not configured. The preview
cannot connect/login/StartGame and is compile-time separated from normal builds.
No remote Web page is loaded by either native package; no Web release version
or Gateway gameplay acceptance is asserted.

Assets are external Crystal PNGs staged from the local shared-ui-assets export
(ChrSel, Prguse, Prguse2, Title, Items, Help, MMap, StateItem).
APKs, exported resources and signing keys are not committed.

## Verification

- Normal Android Rust host: **64/64**.
- Preview-feature Android Rust host: **65/65**; overlaps normal tests, do not sum.
- Shared client visual-feature tests: **572/572**.
- Java session/TLS unit tests: **8/8**.
- Normal and preview APK builds: passed, locked/offline Rust dependencies.
- API 31 Android 12 arm64 emulator, Pixel_5 AVD, landscape 2340x1080.
  Exact fingerprint is in `device.txt`.
- All **31 offline specimens** reached ready markers and were captured without
  panic/FATAL/missing-asset errors detected by the capture script. This is NOT
  31 fully tested user flows. Password/SafeKey protection remains enabled;
  black/protected images are not visual acceptance. Cash-shop/map are empty-state
  evidence only. No real map rendering is shown by the offline black background.
- Manual final-package touch smoke: Panels expanded; Close hid the bag and
  collapsed the launcher. **Failure:** the item-action overlay remained visible.
- **Failure/unverified:** a bag-title drag from (640,24) to (1050,320) invoked
  Android system chrome and did not establish working window drag. Top-edge
  gesture avoidance and nested overlays need further work.

The full 24 MB capture pack is retained locally in this folder. Git contains
selected screenshots, logs, source/device metadata and a PNG hash inventory;
not every captured PNG is published. `mir2-final-normal.png` is normal debug;
all other screenshots are explicitly offline preview.

## Remaining delivery gates

See [coverage and remaining work](../../../ANDROID-UI-COVERAGE.md): gameplay
read-model/intent wiring, actual account operations, nested dialogs, touch-only
joystick/combat/revival, the two touch failures above, Android audio and exact
Crystal A/B parity are not finished. No approved test Gateway or physical device
was available. No production deployment, real login or save mutation occurred.
Full Windows audio-enabled regression is not claimed (missing local Bevy audio
0.19 cache); the 572-test denominator is the shared visual feature set.
