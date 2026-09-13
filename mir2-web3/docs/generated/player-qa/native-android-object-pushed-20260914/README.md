# Native Android authoritative ObjectPushed presentation — 2026-09-14

## Scope

This bounded Android slice carries typed remote `ObjectPushed` packets through
the authenticated post-`IN_GAME` Java allowlist and the retained native object
cache. The exact existing remote actor moves to the server-supplied endpoint;
the endpoint model remains authoritative while the shared Bevy presentation
clock supplies a temporary three-phase screen offset.

The implementation follows pinned Crystal commit
`0e315fe327192afe52c3d7357ddd1f5b7e26c5b8`: its
[`ObjectPushed` handler](https://github.com/Suprcode/Crystal/blob/0e315fe327192afe52c3d7357ddd1f5b7e26c5b8/Client/MirScenes/GameScene.cs#L4934-L4945)
ignores the local player and queues `MirAction.Pushed` only for an existing
object. Player and monster action setup reuse the Walking frame descriptor,
start from its final frame, then decrement the visible frame by two per motion
tick. Android derives only that bounded sequence from already-validated
packaged walking layers. For the ordinary six-frame catalog this is 5, 3, 1;
no new or guessed asset name enters the APK.

The offline `world-render` route applies one labelled push to the visible
unmounted monster after its render receipt. It exercises the production
reducer, retained atlas pose and shared remote-motion buffer, but networking is
disabled for this APK and the fixture is not login or gameplay evidence.

## Deterministic verification

- Android Rust default suite: **183 passed, 0 failed**.
- Android Rust `ui-preview` suite: **191 passed, 0 failed**.
- The focused reducer test covers the authoritative endpoint and direction,
  pushed frames 5/3/1, standing settle, a three-phase presentation event,
  local-self exclusion, unknown/removed/drop isolation and malformed payloads.
- The atlas fixture proves the pushed layer is derived as walking frames
  61/59/57 with the existing 100 ms interval.
- Gradle Debug and UI Preview unit suites: **25 passed, 0 failed** in each
  variant. `GatewaySessionTest` covers post-world packet forwarding.
- `aarch64-linux-android` API 31 target check passed with Rust 1.95.0 and NDK
  26.1.10909125.
- Release Rust plus `uiPreview` packaging and streamed emulator install passed
  with the existing local UI/world/entity proof packs.
- Targeted Rust formatting and `git diff --check` passed. The wider workspace
  still contains pre-existing rustfmt differences outside this Android slice.

## Emulator-visible evidence

The package was cold-launched on `Mir2_API_31_ARM64` using Emulator 37.1.11,
API 31, `arm64-v8a` and `sdk_gphone64_arm64`. The physical display is
1080x2340; the focused landscape app uses the full 2340x1080 display.

`world-render-logcat.txt` records:

- `ANDROID_WORLD_RENDER_READY` with 849 map draws, seven entities and twelve
  entity layers;
- `ANDROID_OBJECT_PUSHED_PRESENTATION_APPLIED`;
- `ANDROID_OBJECT_PUSHED_POSE_ACTIVE` at a 32 px offset;
- `ANDROID_OBJECT_PUSHED_POSE_SETTLED` after the offset returns to zero;
- no matched fatal exception, panic, `DeviceLost`, surface error, OOM, signal
  or app ANR line.

`world-render-object-pushed.png` is a 2340x1080 RGBA capture, 1,420,059 bytes,
SHA-256
`761f128cd5471f84852720551a1b45bb008b880cc15677b48ed427713feffeeb`.
It visibly retains the pushed monster at the authoritative endpoint in the
full-screen Bichon fixture. Exact motion timing and frame order come from the
deterministic tests and active/settled runtime markers rather than one still
image.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444442200`
- SHA-256:
  `7a45b1dd99a6eefd15d39ae6498bb14a616c6fa3f284c0f73eaff183f80ee99e`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK and source asset packs remain outside Git.

## Acceptance boundary

This slice implements the remote-object packet, endpoint, unmounted sprite
pose and presentation path only. Local-player `Pushed` still needs its distinct
input/camera behavior, and exact mounted push artwork remains unresolved.
`ObjectDash`/`UserDash`, `ObjectDeco`, resource-backed level effects and broader
object parity remain separate work.

No approved WSS endpoint, test account or physical Android device was
available. This evidence does not claim real login, online StartGame, a live
render-ready map transition, public Web/entity-release alignment, physical
device behavior or final human acceptance.
