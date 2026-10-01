# Android skill-icon package closure — 2026-10-01

Bounded Android source/resource checkpoint; goal remains Active. Previous exact
v8 source `8eb1a7d344e2240e1a499f42e37b454264ce3bc0` and its missing-icon
runtime failure are preserved in the [NI-07 evidence](../native-android-skill-ingress-20261001/README.md).
Parent of this repair is `6d9f024e1c7a71eeec04500ec6ef63439c5cde85`.
No current v9 APK/runtime/human acceptance is claimed before a clean build.

## Root cause and bounded repair

The real v8 typed FireBall model requested `original-ui/MagIcon/54.png`, but
the approved diagnostic UI pack and Android staging omitted both magic-icon
libraries. A successful build therefore did not establish complete skill images.

- Android `build.gradle` now validates/stages MagIcon and MagIcon2,224 frames
  each, alongside the existing seven mandatory UI libraries. Both require
  unique integer indices covering0..223 and an exact index-to-PNG path. The
  incomplete v8 pack is rejected with **MagIcon export is incomplete**, before
  packaging.
- Existing repository `export-crystal-ui.mjs` reads the raw libraries from the
  Data directory identified by the previously approved manifest. Full exports
  go to a **new**, ignored diagnostic pack; v8 source/images/manifests stay intact.
  No online resource release, production environment, Windows branch or artwork
  was changed. Licensed PNGs, APKs and caches remain outside Git.
- 448 exported PNGs match their source RGBA hashes and dimensions;12,789 prior
  files in the cloned UI pack remain byte-identical. Only the aggregate manifest
  is expected to change while adding the two new libraries.
- Preview `skills` now invokes the existing shared `toggle_skill` entry so it
  opens SPELLS directly rather than default CHAR. The offline fixture explicitly
  supplies level1. Production shared UI/skill rules remain unchanged.
- Diagnostic version9 is `0.1.6-skill-icons`; bind its exact source/APK/runtime
  identity only after the clean commit and subsequent build.

## Resource identity

| Input | SHA-256 | Bytes/coverage |
| --- | --- | --- |
| Raw MagIcon.Lib | `5f49bf35158f680852a66833ec3c706ad045448ef1526d1f7fdd116807de3030` | 199927bytes /224frames |
| Raw MagIcon2.Lib | `0875fc38fa58a101f09346374479de712661f672ef39e6c0ea9679d59aad2a4e` | 345567bytes /224frames |
| Old v8 aggregate manifest | `890ae5d8f0fca13246654c26c5826575887bf22faa7d44504c88d0dbd0307cca` | Unchanged |
| New diagnostic aggregate manifest | `e14940711b944e54baa41254fca3f4b8e0ca0ad3e5d92e76e12a366823ca7735` | Prior libraries plus both skill libraries |

New local pack: `apps/game-client/platform-android/target/skill-icons-v9-20261001/shared-ui-assets/`.
World/entity diagnostic manifests remain the G1 inputs; no full-map/public-release
alignment, all-spell/art coverage or licensing acceptance follows from this leaf.

## Source checks and remaining gates

- Android preview237/237; adds a direct shared SPELLS entry regression.
- New pack validation plus Java Debug32/32 and uiPreview32/32 pass with zero
  failures/errors/skips. Old pack rejection is an expected negative control.
- Read-only review found a P2: the initial count-only guard accepted224 repeated
  frame0 entries despite missing required icons. `frame-coverage-before.log`
  retains the failing test and actual Gradle success. The corrected guard passes
  seven real Gradle controls: complete input accepted; duplicate indices,
  missing54 and wrong frame54 path rejected for each library. Source manifest
  and448 PNG byte digest are verified unchanged before/after the fixture suite.
  `frame-coverage-after.log` records7/7. A subsequent script review also found
  missing `target` creation and an unsubstantiated whole-network flag. The
  fresh-directory ENOENT is preserved in `fresh-target-before.log`; explicit
  output-parent creation passes in `fresh-target-after.log`. The final seven
  cases rerun with Gradle `--offline` in `frame-coverage-final.log`. No APK or
  gameplay network was used; wrapper network traffic was **not observed**, not
  asserted absent. Gradle dependencies must be cached for this offline gate.
- Native skill cursor, Windows wrapper and normal Rust host are unchanged from
  the NI-07 source checkpoint; its227/1196+8ignored/25+7 results are historical,
  not freshly rerun for this Android-only packaging/preview change.
- Source formatting/diff checks pass. Raw test/build whitespace is preserved
  separately, not falsely described as a universally clean unfiltered diff.
- Final independent read-only review closes all three bounded P2 findings;
  no remaining P0/P1/P2 was reported for this repair. The reviewer read source
  and retained logs, but did not rebuild or operate a device.

Still OPEN: exact v9 Debug/preview packages, actual installed image repair and
touch/assignment UI; all-phone-window layout, actual JNI/real WSS/casts/key ACK,
inventory/services, complete maps/actors/audio, performance, device and human
acceptance. A typed offline fixture is not an account or server-authoritative play.

Reproduce the bounded asset gate with the approved local input:

```sh
ANDROID_HOME=/path/to/sdk JAVA_HOME=/path/to/jdk17 \
  node apps/game-client/platform-android/test-shared-skill-assets.mjs \
  /path/to/approved/shared-ui-assets
```

The script creates retained fixtures only under ignored Android `target/`; it
does not modify/publish the approved source pack or build/install an APK. It
creates its ignored output parent on a fresh checkout. Gradle offline dependency
resolution is enforced, but is not a network trace of the distribution wrapper.
