# Native Android hero object evidence (2026-09-14)

This pack records the bounded hero/entity-object and emulator-rendering slice
on `codex/android-player-journey`. It uses the network-disabled UI Preview APK
and local proof assets. It is not approved-WSS login, online StartGame/map
transition, public asset-release alignment or physical-device acceptance.

## Result

- The authenticated Java host forwards `ObjectHero` and `ObjectMana` only
  after `IN_GAME`; its lifecycle fixture locks the additional mana packet.
- `ObjectHero` creates a distinct retained `hero` identity with bounded
  `ownerName`, optional normalized `classKey`, level and authoritative pose.
  Missing owner identity is rejected instead of producing a generic player.
- `ObjectMana` updates only an existing actor, survives temporary hide/show,
  and cannot create an unknown or removed object.
- The shared `mir2-client-bevy` entity model now accepts `kind: "hero"` and
  treats it as player-shaped in its fallback renderer and minimap. This closes
  the shared-model decode error that the first emulator run exposed.
- Android actor overlays retain owner/class/level/mana metadata. Only the
  local player's own hero gets a persistent HP bar and Crystal-gated MP bar;
  the offline Taoist level-12 specimen visibly has both.
- The world frame reports 849 map tiles, 8 entities, 13 entity layers and 2
  active effects. The screenshot also shows player, hero, mounted player,
  monster, NPC, ground item/gold, map objects, effects, HUD, minimap, touch
  joystick and action buttons.
- Android uses GLES, disables the pipelined renderer, serializes Render,
  RenderGraph, Core2d and Core3d schedules, and limits render-asset ingestion
  to 8 MiB per frame. The remaining reproducible panic was identified as
  `bevy_render::renderer::render_system` racing the default asynchronous
  pipeline compiler for the single EGL context. Android now enables Bevy's
  synchronous pipeline compilation. The final APK passed three cold launches
  and four same-process Home/resume cycles; this is an emulator mitigation,
  not a claim about every physical GPU/driver.
- `world-render-final-failure-logcat.txt` retains the pre-fix failure. The
  final `world-render-logcat.txt` has no shared entity decode error, wgpu/GLES
  panic, fatal exception, fatal signal, `DeviceLost`, out-of-memory error or
  app ANR match.

## Gates

- Shared Bevy client: 155 passed, 0 failed.
- Android Rust default: 189 passed, 0 failed.
- Android Rust `ui-preview`: 197 passed, 0 failed.
- Gradle Debug: 25 passed, 0 failures/errors.
- Gradle UI Preview: 25 passed, 0 failures/errors.
- `MIR2_ANDROID_MODE=check MIR2_ANDROID_API_LEVEL=31`: passed for arm64-v8a.
- Release-mode UI Preview package with the local world/entity inputs: passed.
- Streamed APK install: passed.
- Cold launches 1-3: all reached render-ready, displayed the owned hero MP
  bar and remained alive beyond the old failure window.
- Home/resume: four cycles retained the same process. The last three cycles
  used PID `7988`; post-cycle failure matches were zero.

See `test-results.txt` for the exact commands and `world-render-logcat.txt`
for the retained third cold launch plus three resume cycles.

## APK

- Ignored build output:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `447588528`
- SHA-256:
  `3ac955bd5bb8e3ab7d884a29d8d57aeb86ee8519e58475847fdcbfcb3cbf60aa`
- Package: `com.mir2.web3.uipreview`
- Version: `0.1.0-shared-ui`
- Local entity proof-pack ID staged only into the APK:
  `android-archer-mount-bow-proof-20260912`

The APK and licensed/local asset roots are intentionally not committed.

## Device and evidence files

- AVD: `Mir2_API_31_ARM64`
- Emulator: 37.1.11.0
- Guest: API31, arm64-v8a, 1080x2340 physical display at 440 dpi; the game
  Activity renders landscape at 2340x1080.
- `device.txt`: AVD, guest, package and final process facts.
- `window-resume.txt`: cold-launch and same-process Home/resume facts.
- `world-render-logcat.txt`: retained final cold-launch/resume application log.
- `world-render-final-failure-logcat.txt`: retained asynchronous-pipeline
  compilation failure from the immediately preceding APK.
- `world-render-object-hero.png`: final-source 2340x1080 visible world frame,
  SHA-256
  `f2258835b5e8d92d8a0df4228bc73621307fe35a444a8e0cb5db9c2a4bf40163`.

No physical Android device was connected. No approved Gateway endpoint or
test account was supplied, so real authentication, online character roster,
online StartGame/render-ready map change, disconnect/reconnect and saved
position recovery remain unaccepted. The local pack was not compared with an
approved public Web release. Human touch, keyboard, thermal and device-GPU
acceptance also remain open.
