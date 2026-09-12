# Native Android entity pack lock evidence — 2026-09-12

## Result

This Android-only slice started from
`codex/android-player-journey@cdf67bc21f6bafffcf0af083f55db3fdd160e83d`.
Android packaging now refuses an override entity pack without a bounded stable
pack ID. It validates the complete manifest/page closure, exact PNG byte counts
and dimensions, and every declared SHA-256 before copying generated APK assets.
It then embeds `bevy-entity-atlases/pack-lock.json` with the source mode, pack
ID, manifest digest and aggregate counts. The native loader independently
checks the canonical SHA-256 of every selected page before decode and carries
the manifest SHA-256 into the packaged world-frame diagnostic.

The tracked shared pack produced derived ID `tracked-2ae6fb0dfe626bc9` and
manifest SHA-256
`2ae6fb0dfe626bc90b41caac027a1fa219aaa11471ac27e42cc9980c286f48bd`.
The ignored local 35-atlas proof pack was explicitly bound as
`local-player-shards-20260912`, with manifest SHA-256
`0a7f57e99e4f1fa508e807be20ec3c957d9ecd1b598b468fc6905c8a394181e7`,
102 pages, 23,224 rects and 40,312,262 declared PNG bytes.

## Verification

- Rust 1.95.0 formatting passes and Android `ui-preview` is 153/153. The new
  negative test proves a selected PNG with the wrong declared SHA-256 is
  rejected (`rust-fmt.txt`, `rust-tests.txt`).
- The forced 35-atlas real-pack test passes 1/1 and therefore exercises hash
  validation on the selected player/equipment/mount pages
  (`sharded-atlas-test.txt`).
- The normal tracked pack passes the Gradle verifier and has an independently
  captured lock (`gradle-tracked-pack.txt`, `tracked-pack-lock.json`).
- The override without `MIR2_ANDROID_ENTITY_ASSET_PACK_ID` fails as required;
  the same override with `local-player-shards-20260912` passes
  (`gradle-override-missing-id.txt`, `gradle-override-valid.txt`,
  `override-pack-lock.json`).
- The API31 ARM64 `uiPreview` package gate passed. The resulting APK is
  375,663,860 bytes with SHA-256
  `f420968ea3ec633649bc0dd940594eb7fedeb12f5463af7c7bd2f1f8da20e10b`.
  Inspection finds exactly one manifest, one pack lock and all 102 entity PNGs;
  the embedded lock exactly identifies the override above
  (`apk-package.txt`, `apk-assets.txt`, `apk-embedded-pack-lock.json`,
  `build-android-output.txt`).
- Streamed install passed on the API31 `sdk_gphone64_arm64` emulator. The old
  Emulator 31.3.10 SwiftShader driver reached the Bevy window but raised
  `DeviceLost` before this retry produced a render-ready marker
  (`emulator-observation.txt`).

## Acceptance boundary

The pack lock binds this APK to one local proof pack; it does not approve that
pack as the Web/Android release. The proof pack and APK remain ignored and are
not committed. The tracked Web atlas was not changed. The emulator retry adds
package/install evidence only; the earlier visible render-ready evidence is not
promoted to stability or soak acceptance. No approved WSS endpoint, test
account or physical Android device was provided, so real login, live
render-ready map switching, touch/IME/background/reconnect, thermals and human
acceptance remain open.
