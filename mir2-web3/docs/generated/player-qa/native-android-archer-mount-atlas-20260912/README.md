# Native Android Archer and mount atlas evidence — 2026-09-12

## Result

This Android-private slice started from
`codex/android-player-journey@8c1826bcd69ff79cbd3c6d1c529cdc590c0050bb`.
It adds an explicit local entity-atlas build input without changing the tracked
Web release, extends the offline `world-render` specimen with Archer and mounted
actors, and drives `ObjectRangeAttack`/`ObjectAttack` through the same bounded
packet projection and shared Bevy renderer used by authenticated packets.

The licensed proof pack was generated locally from tracked source assets and
was staged only into generated APK assets. It contains 7,120 exact frames across
five 2048 x 2048 pages (8,497,933 compressed bytes and 83,886,080 decoded RGBA
bytes), content hash
`9d4939a75e4c476e36c347d2e41830fe9b95a434143d5b30e1712b857cdaefb6`.
Its bounded roots are `CArmour/00`, `AArmour/00`, `AHair/00`,
`AWeapon/00 L`, `AWeapon/00 R`, `Monster/003`, `ARArmour/00`,
`ARWeapon/00 S`, and `Mount/00` (`atlas-audit.json`). The pack itself is
ignored and is not committed.

## Verification

- Android Rust suite with `ui-preview`: 147 passed, 0 failed
  (`rust-tests.txt`).
- The opt-in real-asset test resolved Archer range frames
  `ARArmour/00/112.png` + `ARWeapon/00 S/112.png` and mounted attack frames
  `Mount/00/192.png` + `CArmour/00/608.png`: 1 passed, 0 failed
  (`extended-atlas-test.txt`).
- Rust 1.95.0/NDK 26.1 built the ARM64 API 31 `uiPreview` APK; Gradle used
  JDK 17 and the package gate passed (`package.txt`).
- APK inspection found the exact manifest and all five named proof pages
  (`apk-assets.txt`). Streamed emulator install and cold launch succeeded
  (`install.txt`, `world-render-launch.txt`).
- Emulator: API 31, `arm64-v8a`, physical size 1080 x 2340, rendered landscape
  at 2340 x 1080 (`device-api.txt`, `device-abi.txt`, `device-size.txt`).
- Runtime reached Bichon render-ready at `(302,634)` with 849 map draws, seven
  objects, eleven entity layers, five selected entity pages, and zero unresolved
  map/entity draws. It then emitted Assassin attack, Archer range, mounted
  attack, and remote-backstep active/settled markers (`ready-markers.txt`,
  `world-render-logcat.txt`).

Screenshots:

- `world-render-action.png`: full-screen offline Bichon frame captured after
  the three action packets were accepted.
- `world-render-settled.png`: later full-screen frame after remote motion
  settled; the Archer and mounted actor remain visible.

APK (generated locally and intentionally not committed):

```text
apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk
SHA-256 da12e537f6fe272ced70f4147f9fc4ec1146231c471b445292357e9bce40b96c
```

The shared UI input was exported read-only from this branch's Git object into
the Android worktree `target/` because the separate main checkout is user-owned
and currently lacks one tracked UI file. The main checkout was not repaired,
reset, stashed, or otherwise modified.

## Acceptance boundary

This is an explicitly labelled offline UI Preview. No approved WSS endpoint or
account was supplied, so it is not real login, character selection, StartGame,
online map-transition, or live packet-flow evidence. It was not run on a
physical Android device and is not physical-device or human acceptance.

The tracked shared Web atlas is unchanged and still lacks the Archer/mount
roots. This local proof pack covers Archer alternate range action and one mount
variant; it is not a production asset release. Regular `ARWeapon/00` is not
included because source PNGs `808` through `831` have no frame metadata
(`arweapon-00-pngs-without-metadata.txt`). Therefore the standing Archer in the
screenshot intentionally has no regular bow layer. Correcting that source
metadata, generating the broader bounded release pack, and aligning the Web and
Android asset version remain open.
