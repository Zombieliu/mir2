# Native Android entity catalog evidence — 2026-09-12

## Result

This Android-private slice started from
`codex/android-player-journey@747d67679d6d63e96b15d7ce25b1b6d55a7b90b4`.
It adds action-specific player-library selection, bounded Archer and mounted
frame catalogs, mounted body/mount alignment, and Crystal-compatible Assassin
dual-weapon depth ordering. Atlas rect paths now canonicalize `%20` to a space
before strict library/frame matching, which makes the tracked `AWeapon` names
resolvable without accepting arbitrary percent escapes or packet-selected paths.

The tracked atlas contains the Assassin `AArmour`, `AHair` and `AWeapon`
libraries. The API 31 UI Preview therefore renders a layered Assassin in the
Bichon scene and injects an offline `ObjectAttack` through the production
Android packet-action path after the render-ready receipt. The scene also keeps
the previous remote backstep and effect specimens. `atlas-audit.txt` records the
one atlas, seven pages, 10,482 rects, and all packaged library roots.

## Verification

- Android Rust suite: 146 passed, 0 failed (`rust-tests.txt`).
- Opt-in tests against `apps/web/public/bevy-entity-atlases`: 2 passed, 0
  failed, including the real Assassin body/hair/dual-weapon lookup
  (`real-atlas-tests.txt`).
- Rust 1.95.0 API 31 ARM64 target check passed (`api31-check.txt`).
- `uiPreview` package gate passed (`package.txt`).
- Streamed install succeeded (`install.txt`).
- Cold launch succeeded for
  `com.mir2.web3.uipreview/com.mir2.web3.MainActivity`
  (`world-render-launch.txt`).
- Emulator: API 31 ARM64, physical size 1080 x 2340, rotated to landscape
  (`device-api.txt`, `device-size.txt`).
- Runtime reached `ANDROID_WORLD_RENDER_READY` at Bichon `(302,634)` with 849
  map draws, five entities and eight entity layers. It also emitted
  `ANDROID_EFFECT_RENDER_STATE_READY`,
  `ANDROID_ASSASSIN_ATTACK_PRESENTATION_STARTED`, and the active/settled remote
  motion markers (`ready-markers.txt`, `world-render-logcat.txt`).

Screenshots:

- `assassin-standing.png`: full-screen offline Bichon specimen with the layered
  Assassin and shared mobile HUD.
- `assassin-attack.png`: later frame after the packet-action specimen starts.

APK (generated locally and intentionally not committed):

```text
apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk
SHA-256 5a8ddade998f92488f9b15d7070228aaf098c8586dc3622fd0758bd2cd25a2d3
```

## Acceptance boundary

This is an explicitly labelled offline UI Preview. No approved WSS endpoint or
account was supplied, so it is not real login, character selection, StartGame,
or live packet-flow evidence. It was not run on a physical Android device and
is not physical-device or human acceptance.

The checked-in atlas does not currently contain `ARArmour`/`ARWeapon` or
`Mount` roots. Archer alternate and mounted catalog logic is covered by exact
fixture tests, but those variants were not visibly rendered in this emulator
run. Generating and packaging bounded pages from an approved licensed asset
source remains an asset-closure task.
