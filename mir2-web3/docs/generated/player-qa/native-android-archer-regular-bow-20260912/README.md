# Native Android Archer regular-bow evidence — 2026-09-12

## Result

This Android-private slice started from
`codex/android-player-journey@9185571cbd9541fa4515d7bd1c0def7d21f3750d`.
The atlas loader now distinguishes fully unindexed source PNGs from usable
Crystal frames. A rect with no `offsetX`, `offsetY`, or `frameIndex` remains
bounded and counted but cannot enter sprite lookup; partial metadata, negative
frame indices, duplicate paths, unsafe geometry, or unsafe pages still reject
the manifest. No placement metadata is fabricated.

The local licensed proof pack adds regular `ARWeapon/00` to the previous
Archer/mount roots. It contains 7,848 source PNGs across five 2048 x 2048 pages
(8,779,443 compressed bytes and 83,886,080 decoded RGBA bytes), content hash
`8502dd9c7ae3c340e46da732304fbb24a3df0234b77265e9cfaf418083ce6daa`.
Exactly 24 PNGs, `ARWeapon/00/808.png` through `831.png`, have no adjacent
Crystal metadata and are reported as unindexed. The regular standing bow uses
indexed frame `ARWeapon/00/8.png`; Archer range actions continue to use
`ARWeapon/00 S`.

The pack was staged only into generated APK assets through
`MIR2_ANDROID_ENTITY_ASSET_ROOT`. The pack and APK are ignored and were not
committed. The tracked shared Web atlas was not changed.

## Verification

- Android Rust suite with `ui-preview`: 149 passed, 0 failed
  (`rust-tests.txt`).
- The forced real-pack test resolved the indexed standing body + regular bow,
  Archer range body + bow, and mounted attack body + mount while asserting all
  24 unindexed rects: 1 passed, 0 failed (`extended-atlas-test.txt`).
- Rust 1.95.0/NDK 26.1 built the ARM64 API 31 `uiPreview` APK; Gradle used
  JDK 17 and the package gate passed (`package.txt`).
- APK inspection found the exact manifest and all five named proof pages
  (`apk-assets.txt`). Streamed emulator install and cold launch succeeded
  (`install-launch.txt`).
- Emulator: API 31, `arm64-v8a`, physical size 1080 x 2340, rendered landscape
  at 2340 x 1080 (`device.txt`).
- Runtime reached Bichon render-ready at `(302,634)` with 849 map draws, seven
  objects, twelve entity layers, five selected entity pages, zero unresolved
  map/entity draws, and the explicit `entity_unindexed_rects=24` diagnostic.
  Assassin attack, Archer range, mounted attack, and remote backstep active /
  settled markers also fired (`ready-markers.txt`).

Screenshots:

- `world-render-action.png`: full-screen offline Bichon frame captured when the
  packet action specimen started.
- `world-render-settled.png`: full-screen settled frame; the Archer now keeps
  the indexed regular bow layer in its standing pose.
- `archer-standing-bow-crop.png`: unscaled 500 x 500 crop of that settled
  framebuffer around the standing Archer for close inspection.

APK (generated locally and intentionally not committed):

```text
apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk
SHA-256 31fa876356d0e152efdeba937c0dcf4cf81e7453e22e100d0182a4ba3485c9a3
```

## Acceptance boundary

This is an explicitly labelled offline UI Preview on an API 31 emulator. No
approved WSS endpoint or account was supplied, so it is not real login,
character selection, StartGame, online map-transition, or live packet-flow
evidence. It was not run on a physical Android device and is not physical-device
or human acceptance.

The local proof pack is not an approved shared asset release. The 24 unindexed
PNGs remain unavailable to the renderer until their authoritative metadata is
recovered; this change only prevents them from invalidating the other 7,824
indexed frames. Broader class/equipment/mount coverage, tracked Web/Android
asset-version alignment, approved live login, and physical-device validation
remain open.
